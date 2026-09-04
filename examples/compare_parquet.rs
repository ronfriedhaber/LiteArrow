use std::io::Cursor;

use arrow_array::record_batch;
use litearrow::FileWriter;
use parquet::{
    arrow::ArrowWriter,
    basic::{Compression, ZstdLevel},
    file::properties::WriterProperties,
};
const ROWS: usize = 100_000;
fn random(state: &mut u64) -> u64 {
    *state = state
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1);
    *state
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut rng = 42_u64;
    let rows: Vec<_> = (0..ROWS)
        .map(|row| {
            let customer_id = (random(&mut rng) % 20_000) as i64;
            let product_id = (random(&mut rng) % 800) as i64;
            [
                1_735_689_600_000_i64 + row as i64 * 1_000,
                customer_id,
                product_id,
                customer_id % 32,
                1 + (random(&mut rng) % 5) as i64,
                199 + product_id * 17 + (random(&mut rng) % 20) as i64,
            ]
        })
        .collect();
    let column = |index| rows.iter().map(|row| row[index]).collect::<Vec<_>>();
    let batch = record_batch!(
        ("event_time", Int64, column(0)),
        ("customer_id", Int64, column(1)),
        ("product_id", Int64, column(2)),
        ("country", Int64, column(3)),
        ("quantity", Int64, column(4)),
        ("price_cents", Int64, column(5))
    )?;

    let mut lite = FileWriter::try_new(Cursor::new(Vec::new()), batch.schema())?;
    lite.write_batch(&batch)?;
    let lite_bytes = lite.finish()?.into_inner();

    let properties = WriterProperties::builder()
        .set_compression(Compression::ZSTD(ZstdLevel::default()))
        .build();
    let mut parquet = ArrowWriter::try_new(Vec::new(), batch.schema(), Some(properties))?;
    parquet.write(&batch)?;
    let parquet_bytes = parquet.into_inner()?;

    println!("rows: {ROWS}, columns: {}", batch.num_columns());
    println!("LiteArrow: {} bytes", lite_bytes.len());
    println!("Parquet+Zstd: {} bytes", parquet_bytes.len());
    println!(
        "ratio: {:.2}x",
        lite_bytes.len() as f64 / parquet_bytes.len() as f64
    );
    Ok(())
}
