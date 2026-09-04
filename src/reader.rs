use std::io::{Read, Seek, SeekFrom};

use arrow_array::RecordBatch;
use arrow_schema::SchemaRef;

use crate::codec;
use crate::format::{self, HEADER, Metadata, TRAILER_LENGTH};
use crate::{Error, Result, crc32c};

pub struct FileReader<R> {
    input: R,
    metadata: Metadata,
    schema: SchemaRef,
}

impl<R: Read + Seek> FileReader<R> {
    pub fn try_new(mut input: R) -> Result<Self> {
        let file_length = input.seek(SeekFrom::End(0))?;
        if file_length < HEADER.len() as u64 + TRAILER_LENGTH {
            return Err(Error::UnexpectedEndOfInput);
        }
        input.seek(SeekFrom::Start(0))?;
        let mut header = [0; HEADER.len()];
        input.read_exact(&mut header)?;
        if header != HEADER {
            return Err(Error::InvalidMagic {
                structure: "header",
            });
        }
        input.seek(SeekFrom::End(-(TRAILER_LENGTH as i64)))?;
        let mut trailer = [0; TRAILER_LENGTH as usize];
        input.read_exact(&mut trailer)?;
        let (footer_length, checksum) = format::read_trailer(trailer)?;
        if footer_length > file_length - TRAILER_LENGTH - HEADER.len() as u64 {
            return Err(Error::InvalidMetadata("footer length exceeds file"));
        }
        let footer_offset = file_length - TRAILER_LENGTH - footer_length;
        input.seek(SeekFrom::Start(footer_offset))?;
        let mut footer = vec![
            0;
            footer_length
                .try_into()
                .map_err(|_| Error::IntegerOverflow)?
        ];
        input.read_exact(&mut footer)?;
        if crc32c(&footer) != checksum {
            return Err(Error::ChecksumMismatch {
                structure: "footer",
            });
        }
        let metadata = format::decode(&footer)?;
        let schema = codec::decode_schema(&metadata.schema)?;
        if schema.fields().len() != metadata.field_count as usize {
            return Err(Error::InvalidMetadata("schema field count changed"));
        }
        Ok(Self {
            input,
            metadata,
            schema,
        })
    }

    pub fn schema(&self) -> &SchemaRef {
        &self.schema
    }
    pub fn num_batches(&self) -> usize {
        self.metadata.blocks.len()
    }

    pub fn read_batch(&mut self, index: usize) -> Result<RecordBatch> {
        let block = self
            .metadata
            .blocks
            .get(index)
            .cloned()
            .ok_or(Error::InvalidMetadata("batch index is out of bounds"))?;
        let arrays = self
            .schema
            .fields()
            .iter()
            .zip(block.columns)
            .map(|(field, chunk)| {
                let codec = codec::get(chunk.codec)
                    .ok_or(Error::InvalidMetadata("unknown column codec"))?;
                self.input.seek(SeekFrom::Start(chunk.offset))?;
                let mut bytes = vec![
                    0;
                    chunk
                        .length
                        .try_into()
                        .map_err(|_| Error::IntegerOverflow)?
                ];
                self.input.read_exact(&mut bytes)?;
                codec.decode(field, block.rows as usize, &bytes)
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(RecordBatch::try_new(self.schema.clone(), arrays)?)
    }
}
