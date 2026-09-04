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
    let mut time = Vec::with_capacity(ROWS);
    let mut customer = Vec::with_capacity(ROWS);
    let mut product = Vec::with_capacity(ROWS);
    let mut country = Vec::with_capacity(ROWS);
    let mut quantity = Vec::with_capacity(ROWS);
    let mut price_cents = Vec::with_capacity(ROWS);
    for row in 0..ROWS {
        let customer_id = (random(&mut rng) % 20_000) as i64;
        let product_id = (random(&mut rng) % 800) as i64;
        time.push(1_735_689_600_000_i64 + row as i64 * 1_000);
        customer.push(customer_id);
        product.push(product_id);
        country.push(customer_id % 32);
        quantity.push(1 + (random(&mut rng) % 5) as i64);
        price_cents.push(199 + product_id * 17 + (random(&mut rng) % 20) as i64);
    }
    let batch = record_batch!(
        ("event_time", Int64, time),
        ("customer_id", Int64, customer),
        ("product_id", Int64, product),
        ("country", Int64, country),
        ("quantity", Int64, quantity),
        ("price_cents", Int64, price_cents)
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
