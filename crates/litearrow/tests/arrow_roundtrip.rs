use std::{io::Cursor, sync::Arc};

use arrow_array::{Date32Array, Int64Array, RecordBatch, record_batch};
use arrow_schema::{DataType, Field, Schema};
use half::f16;
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
    let schema = Arc::new(Schema::new(vec![Field::new(
        "date",
        DataType::Date32,
        false,
    )]));
    let batch =
        RecordBatch::try_new(schema, vec![Arc::new(Date32Array::from(vec![1, 2, 3]))]).unwrap();
    let mut writer = FileWriter::try_new(Cursor::new(Vec::new()), batch.schema()).unwrap();
    writer.write_batch(&batch).unwrap();
    let mut reader = FileReader::try_new(writer.finish().unwrap()).unwrap();
    assert_eq!(reader.read_batch(0).unwrap(), batch);
}

#[test]
fn string_offsets_and_dictionaries_round_trip_natively() {
    let batch = record_batch!(
        (
            "utf8",
            Utf8,
            [Some("alpha"), None, Some("alpha"), Some("beta")]
        ),
        (
            "large",
            LargeUtf8,
            ["a long value", "another", "a long value", "unique"]
        )
    )
    .unwrap();
    let mut writer = FileWriter::try_new(Cursor::new(Vec::new()), batch.schema()).unwrap();
    writer.write_batch(&batch).unwrap();
    let mut reader = FileReader::try_new(writer.finish().unwrap()).unwrap();
    assert_eq!(reader.read_batch(0).unwrap(), batch);
}

#[test]
fn boolean_bitpacking_and_runs_round_trip_natively() {
    let batch = record_batch!(
        ("constant", Boolean, [true, true, true, true, true]),
        ("alternating", Boolean, [true, false, true, false, true]),
        (
            "nullable",
            Boolean,
            [Some(true), None, Some(false), None, Some(true)]
        )
    )
    .unwrap();
    let mut writer = FileWriter::try_new(Cursor::new(Vec::new()), batch.schema()).unwrap();
    writer.write_batch(&batch).unwrap();
    let mut reader = FileReader::try_new(writer.finish().unwrap()).unwrap();
    assert_eq!(reader.read_batch(0).unwrap(), batch);
}

#[test]
fn every_integer_type_round_trips_natively() {
    let batch = record_batch!(
        ("i8", Int8, [i8::MIN, 0, i8::MAX]),
        ("i16", Int16, [i16::MIN, 0, i16::MAX]),
        ("i32", Int32, [i32::MIN, 0, i32::MAX]),
        ("i64", Int64, [i64::MIN, 0, i64::MAX]),
        ("u8", UInt8, [u8::MIN, 1, u8::MAX]),
        ("u16", UInt16, [u16::MIN, 1, u16::MAX]),
        ("u32", UInt32, [u32::MIN, 1, u32::MAX]),
        ("u64", UInt64, [u64::MIN, 1, u64::MAX])
    )
    .unwrap();
    let mut writer = FileWriter::try_new(Cursor::new(Vec::new()), batch.schema()).unwrap();
    writer.write_batch(&batch).unwrap();
    let mut reader = FileReader::try_new(writer.finish().unwrap()).unwrap();
    assert_eq!(reader.read_batch(0).unwrap(), batch);
}

#[test]
fn every_float_type_round_trips_natively() {
    let batch = record_batch!(
        ("f16", Float16, [f16::MIN, f16::ZERO, f16::MAX]),
        ("f32", Float32, [f32::MIN, -0.0, f32::MAX]),
        ("f64", Float64, [f64::MIN, -0.0, f64::MAX])
    )
    .unwrap();
    let mut writer = FileWriter::try_new(Cursor::new(Vec::new()), batch.schema()).unwrap();
    writer.write_batch(&batch).unwrap();
    let mut reader = FileReader::try_new(writer.finish().unwrap()).unwrap();
    assert_eq!(reader.read_batch(0).unwrap(), batch);
}
