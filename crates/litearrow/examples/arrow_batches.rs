use std::io::Cursor;

use arrow_array::record_batch;
use litearrow::{FileReader, FileWriter};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let batch = record_batch!(("answer", Int64, [Some(42), None, Some(7)]))?;

    let mut writer = FileWriter::try_new(Cursor::new(Vec::new()), batch.schema())?;
    writer.write_batch(&batch)?;
    let bytes = writer.finish()?.into_inner();

    let mut reader = FileReader::try_new(Cursor::new(bytes))?;
    let decoded = reader.read_batch(0)?;
    assert_eq!(batch, decoded);
    Ok(())
}
