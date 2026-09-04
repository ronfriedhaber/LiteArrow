use std::{
    hint::black_box,
    io::Cursor,
    sync::Arc,
    time::{Duration, Instant},
};

use arrow_array::{RecordBatch, record_batch};
use bytes::Bytes;
use litearrow::{FileReader, FileWriter};
use parquet::{
    arrow::{ArrowWriter, arrow_reader::ParquetRecordBatchReaderBuilder},
    basic::{Compression, ZstdLevel},
    file::properties::WriterProperties,
};

type AnyResult<T> = Result<T, Box<dyn std::error::Error>>;

fn main() -> AnyResult<()> {
    let rows = argument(1, 1_000_000);
    let repetitions = argument(2, 5).max(1);
    let batch = synthetic_events(rows)?;
    let logical_bytes = rows * batch.num_columns() * 8;

    let (lite, lite_write) = repeat(repetitions, || write_litearrow(&batch))?;
    let (parquet, parquet_write) = repeat(repetitions, || write_parquet(&batch))?;
    let lite = Arc::<[u8]>::from(lite);
    let parquet = Bytes::from(parquet);
    let (_, lite_read) = repeat(repetitions, || read_litearrow(lite.clone()))?;
    let (_, parquet_read) = repeat(repetitions, || read_parquet(parquet.clone()))?;

    println!(
        "{rows} rows × {} Int64 columns; {repetitions} repetitions",
        batch.num_columns()
    );
    println!("                 bytes    write MiB/s     read MiB/s");
    report(
        "LiteArrow",
        lite.len(),
        logical_bytes,
        repetitions,
        lite_write,
        lite_read,
    );
    report(
        "Parquet+Zstd",
        parquet.len(),
        logical_bytes,
        repetitions,
        parquet_write,
        parquet_read,
    );
    Ok(())
}

fn write_litearrow(batch: &RecordBatch) -> AnyResult<Vec<u8>> {
    let mut writer = FileWriter::try_new(Vec::new(), batch.schema())?;
    writer.write_batch(batch)?;
    Ok(writer.finish()?)
}

fn write_parquet(batch: &RecordBatch) -> AnyResult<Vec<u8>> {
    let properties = WriterProperties::builder()
        .set_compression(Compression::ZSTD(ZstdLevel::default()))
        .build();
    let mut writer = ArrowWriter::try_new(Vec::new(), batch.schema(), Some(properties))?;
    writer.write(batch)?;
    Ok(writer.into_inner()?)
}

fn read_litearrow(bytes: Arc<[u8]>) -> AnyResult<usize> {
    let mut reader = FileReader::try_new(Cursor::new(bytes))?;
    Ok(black_box(reader.read_batch(0)?).num_rows())
}

fn read_parquet(bytes: Bytes) -> AnyResult<usize> {
    let reader = ParquetRecordBatchReaderBuilder::try_new(bytes)?.build()?;
    Ok(reader
        .map(|batch| batch.map(|batch| black_box(batch).num_rows()))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .sum())
}

fn repeat<T>(
    count: usize,
    mut operation: impl FnMut() -> AnyResult<T>,
) -> AnyResult<(T, Duration)> {
    let start = Instant::now();
    let mut result = operation()?;
    for _ in 1..count {
        result = operation()?;
    }
    Ok((result, start.elapsed()))
}

fn report(name: &str, size: usize, logical: usize, count: usize, write: Duration, read: Duration) {
    let mib = (logical * count) as f64 / 1_048_576.0;
    println!(
        "{name:13} {size:9} {write:14.1} {read:14.1}",
        write = mib / write.as_secs_f64(),
        read = mib / read.as_secs_f64()
    );
}

fn argument(index: usize, default: usize) -> usize {
    std::env::args()
        .nth(index)
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

fn random(state: &mut u64) -> u64 {
    *state = state
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1);
    *state
}

fn synthetic_events(rows: usize) -> AnyResult<RecordBatch> {
    let mut rng = 42;
    let (mut time, mut customer, mut product, mut country, mut quantity, mut price) =
        (vec![], vec![], vec![], vec![], vec![], vec![]);
    for row in 0..rows {
        let customer_id = (random(&mut rng) % 20_000) as i64;
        let product_id = (random(&mut rng) % 800) as i64;
        time.push(1_735_689_600_000 + row as i64 * 1_000);
        customer.push(customer_id);
        product.push(product_id);
        country.push(customer_id % 32);
        quantity.push(1 + (random(&mut rng) % 5) as i64);
        price.push(199 + product_id * 17 + (random(&mut rng) % 20) as i64);
    }
    Ok(record_batch!(
        ("event_time", Int64, time),
        ("customer_id", Int64, customer),
        ("product_id", Int64, product),
        ("country", Int64, country),
        ("quantity", Int64, quantity),
        ("price_cents", Int64, price)
    )?)
}
