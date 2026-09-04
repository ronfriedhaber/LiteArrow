use std::{io::Cursor, sync::Arc};

use arrow_array::{Int64Array, RecordBatch, record_batch};
use arrow_schema::{DataType, Field, Schema};
use litearrow::{FileReader, FileWriter};

#[test]
fn multiple_nullable_arrow_batches_round_trip() {
    let schema = Arc::new(Schema::new(vec![
        Field::new("id", DataType::Int64, false),
        Field::new("reading", DataType::Int64, true),
    ]));
    let batches = [
        RecordBatch::try_new(
            schema.clone(),
            vec![
                Arc::new(Int64Array::from(vec![1, 2, 3])),
                Arc::new(Int64Array::from(vec![Some(10), None, Some(30)])),
            ],
        )
        .unwrap(),
        RecordBatch::try_new(
            schema.clone(),
            vec![
                Arc::new(Int64Array::from(vec![4, 5])),
                Arc::new(Int64Array::from(vec![None, None])),
            ],
        )
        .unwrap(),
    ];

    let mut writer = FileWriter::try_new(Cursor::new(Vec::new()), schema).unwrap();
    batches
        .iter()
        .try_for_each(|batch| writer.write_batch(batch))
        .unwrap();
    let mut reader = FileReader::try_new(writer.finish().unwrap()).unwrap();

    assert_eq!(reader.num_batches(), batches.len());
    batches.iter().enumerate().for_each(|(index, expected)| {
        assert_eq!(&reader.read_batch(index).unwrap(), expected);
    });
}

#[test]
fn ipc_fallback_round_trips_other_arrow_types() {
    let batch = record_batch!(
        ("flag", Boolean, [Some(true), None, Some(false)]),
        ("count", Int32, [1, 2, 3]),
        ("ratio", Float64, [1.5, 2.5, 3.5]),
        ("label", Utf8, ["alpha", "beta", "gamma"])
    )
    .unwrap();
    let mut writer = FileWriter::try_new(Cursor::new(Vec::new()), batch.schema()).unwrap();
    writer.write_batch(&batch).unwrap();
    let mut reader = FileReader::try_new(writer.finish().unwrap()).unwrap();
    assert_eq!(reader.read_batch(0).unwrap(), batch);
}
