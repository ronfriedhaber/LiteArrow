mod formats;
mod timing;
mod workloads;

use std::{sync::Arc, time::Duration};

use bytes::Bytes;
use formats::*;
use timing::{Timing, measure};
use workloads::{Workload, all};

type AnyResult<T> = Result<T, Box<dyn std::error::Error>>;

fn main() -> AnyResult<()> {
    if cfg!(debug_assertions) {
        return Err("benchmark with: cargo run --release --bin litearrow-bench".into());
    }
    let rows = argument(1, 1_000_000).max(1);
    let repetitions = argument(2, 21).max(1);
    let block_rows = argument(3, 100_000).clamp(1, rows);
    println!(
        "in-memory encode/decode; {rows} rows; {block_rows} rows/block; median of {repetitions}"
    );
    println!(
        "Parquet uses Zstd; LiteArrow uses {} Rayon threads\n",
        rayon::current_num_threads()
    );
    all(rows, block_rows)?
        .iter()
        .try_for_each(|workload| benchmark(workload, repetitions))
}

fn benchmark(workload: &Workload, repetitions: usize) -> AnyResult<()> {
    let lite = Arc::<[u8]>::from(write_litearrow(&workload.batches)?);
    let parquet = Bytes::from(write_parquet(&workload.batches)?);
    let expected = concatenate(&workload.batches)?;
    assert_eq!(concatenate(&read_litearrow(lite.clone())?)?, expected);
    assert_eq!(
        concatenate(&read_parquet(parquet.clone(), expected.num_rows())?)?,
        expected
    );

    let lite_write = measure(repetitions, || write_litearrow(&workload.batches))?;
    let parquet_write = measure(repetitions, || write_parquet(&workload.batches))?;
    let lite_read = measure(repetitions, || read_litearrow(lite.clone()))?;
    let parquet_read = measure(repetitions, || {
        read_parquet(parquet.clone(), expected.num_rows())
    })?;

    println!(
        "{}: {} columns × {} blocks; input {:.1} MiB",
        workload.name,
        expected.num_columns(),
        workload.batches.len(),
        workload.input_bytes as f64 / 1_048_576.0
    );
    println!("                 bytes   ratio  encode MiB/s  decode MiB/s  enc p95 ms  dec p95 ms");
    report(
        "LiteArrow",
        lite.len(),
        workload.input_bytes,
        lite_write,
        lite_read,
    );
    report(
        "Parquet+Zstd",
        parquet.len(),
        workload.input_bytes,
        parquet_write,
        parquet_read,
    );
    println!();
    Ok(())
}

fn report(name: &str, size: usize, input: usize, write: Timing, read: Timing) {
    let rate = |duration: Duration| input as f64 / 1_048_576.0 / duration.as_secs_f64();
    println!(
        "{name:13} {size:9} {ratio:7.3} {write_rate:13.1} {read_rate:13.1} {write_p95:11.2} {read_p95:11.2}",
        ratio = size as f64 / input as f64,
        write_rate = rate(write.median),
        read_rate = rate(read.median),
        write_p95 = write.p95.as_secs_f64() * 1_000.0,
        read_p95 = read.p95.as_secs_f64() * 1_000.0
    );
}

fn argument(index: usize, default: usize) -> usize {
    std::env::args()
        .nth(index)
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}
