use std::io::Cursor;
use std::sync::Arc;

use arrow_array::{Array, ArrayRef, RecordBatch, make_array};
use arrow_ipc::{reader::StreamReader, writer::StreamWriter};
use arrow_schema::{Field, Schema, SchemaRef};

use crate::codec::ColumnCodec;
use crate::{Error, Result};

pub(super) struct Ipc;

impl ColumnCodec for Ipc {
    fn id(&self) -> u8 {
        255
    }

    fn encode(&self, field: &Field, array: &dyn Array) -> Result<Option<Vec<u8>>> {
        let schema = Arc::new(Schema::new(vec![field.clone()]));
        let batch = RecordBatch::try_new(schema.clone(), vec![make_array(array.to_data())])?;
        let mut bytes = Vec::new();
        let mut writer = StreamWriter::try_new(&mut bytes, &schema)?;
        writer.write(&batch)?;
        writer.finish()?;
        drop(writer);
        Ok(Some(bytes))
    }

    fn decode(&self, _: &Field, length: usize, bytes: &[u8]) -> Result<ArrayRef> {
        let mut reader = StreamReader::try_new(Cursor::new(bytes), None)?;
        let batch = reader.next().ok_or(Error::UnexpectedEndOfInput)??;
        if batch.num_rows() != length {
            return Err(Error::InvalidMetadata("IPC column length changed"));
        }
        Ok(batch.column(0).clone())
    }
}

pub(crate) fn encode_schema(schema: &SchemaRef) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    let mut writer = StreamWriter::try_new(&mut bytes, schema)?;
    writer.finish()?;
    drop(writer);
    Ok(bytes)
}

pub(crate) fn decode_schema(bytes: &[u8]) -> Result<SchemaRef> {
    Ok(StreamReader::try_new(Cursor::new(bytes), None)?.schema())
}
