use std::{io::Cursor, sync::Arc};

use arrow_array::RecordBatch;
use arrow_select::concat::concat_batches;
use bytes::Bytes;
use litearrow::{FileReader, FileWriter};
use parquet::{
    arrow::{ArrowWriter, arrow_reader::ParquetRecordBatchReaderBuilder},
    basic::{Compression, ZstdLevel},
    file::properties::WriterProperties,
};

use crate::AnyResult;

pub fn write_litearrow(batches: &[RecordBatch]) -> AnyResult<Vec<u8>> {
    let mut writer = FileWriter::try_new(Vec::new(), batches[0].schema())?;
    batches
        .iter()
        .try_for_each(|batch| writer.write_batch(batch))?;
    Ok(writer.finish()?)
}

pub fn write_parquet(batches: &[RecordBatch]) -> AnyResult<Vec<u8>> {
    let properties = WriterProperties::builder()
        .set_compression(Compression::ZSTD(ZstdLevel::default()))
        .set_max_row_group_row_count(Some(batches[0].num_rows()))
        .build();
    let mut writer = ArrowWriter::try_new(Vec::new(), batches[0].schema(), Some(properties))?;
    batches.iter().try_for_each(|batch| writer.write(batch))?;
    Ok(writer.into_inner()?)
}

pub fn read_litearrow(bytes: Arc<[u8]>) -> AnyResult<Vec<RecordBatch>> {
    let mut reader = FileReader::try_new(Cursor::new(bytes))?;
    Ok((0..reader.num_batches())
        .map(|index| reader.read_batch(index))
        .collect::<Result<_, _>>()?)
}

pub fn read_parquet(bytes: Bytes, batch_size: usize) -> AnyResult<Vec<RecordBatch>> {
    Ok(ParquetRecordBatchReaderBuilder::try_new(bytes)?
        .with_batch_size(batch_size)
        .build()?
        .collect::<Result<_, _>>()?)
}

pub fn concatenate(batches: &[RecordBatch]) -> AnyResult<RecordBatch> {
    Ok(concat_batches(&batches[0].schema(), batches.iter())?)
}
