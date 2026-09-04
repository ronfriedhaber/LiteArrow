use std::io::Write;

use arrow_array::RecordBatch;
use arrow_schema::SchemaRef;

use crate::codec;
use crate::format::{self, Block, Chunk, HEADER, Metadata};
use crate::{Error, Result};

pub struct FileWriter<W> {
    output: W,
    schema: SchemaRef,
    blocks: Vec<Block>,
    offset: u64,
}

impl<W: Write> FileWriter<W> {
    pub fn try_new(mut output: W, schema: SchemaRef) -> Result<Self> {
        output.write_all(&HEADER)?;
        Ok(Self {
            output,
            schema,
            blocks: vec![],
            offset: HEADER.len() as u64,
        })
    }

    pub fn write_batch(&mut self, batch: &RecordBatch) -> Result<()> {
        if batch.schema() != self.schema {
            return Err(Error::InvalidMetadata("record batch schema changed"));
        }
        if batch.num_rows() == 0 {
            return Ok(());
        }
        let columns = self
            .schema
            .fields()
            .iter()
            .zip(batch.columns())
            .map(|(field, array)| {
                let candidates = codec::specialized()
                    .into_iter()
                    .filter_map(|codec| {
                        codec
                            .encode(field, array.as_ref())
                            .transpose()
                            .map(|result| result.map(|bytes| (codec, bytes)))
                    })
                    .collect::<Result<Vec<_>>>()?;
                let (codec, bytes) = candidates
                    .into_iter()
                    .min_by_key(|(_, bytes)| bytes.len())
                    .map(Ok)
                    .unwrap_or_else(|| {
                        let fallback = codec::fallback();
                        fallback
                            .encode(field, array.as_ref())?
                            .ok_or(Error::InvalidMetadata("fallback rejected column"))
                            .map(|bytes| (fallback, bytes))
                    })?;
                self.output.write_all(&bytes)?;
                let chunk = Chunk {
                    codec: codec.id(),
                    offset: self.offset,
                    length: bytes.len() as u64,
                };
                self.offset += bytes.len() as u64;
                Ok(chunk)
            })
            .collect::<Result<Vec<_>>>()?;
        self.blocks.push(Block {
            rows: batch
                .num_rows()
                .try_into()
                .map_err(|_| Error::IntegerOverflow)?,
            columns,
        });
        Ok(())
    }

    pub fn finish(mut self) -> Result<W> {
        let footer = format::encode(&Metadata {
            schema: codec::encode_schema(&self.schema)?,
            field_count: self
                .schema
                .fields()
                .len()
                .try_into()
                .map_err(|_| Error::IntegerOverflow)?,
            blocks: self.blocks,
        })?;
        self.output.write_all(&footer)?;
        self.output.write_all(&format::trailer(&footer))?;
        Ok(self.output)
    }
}
