use arrow_array::{RecordBatch, record_batch};

use crate::AnyResult;

pub struct Workload {
    pub name: &'static str,
    pub batches: Vec<RecordBatch>,
    pub input_bytes: usize,
}

pub fn all(rows: usize, block_rows: usize) -> AnyResult<Vec<Workload>> {
    [
        ("telemetry", telemetry as fn(_, _) -> _),
        ("random-i64", random_i64 as fn(_, _) -> _),
        ("mixed-arrow", mixed_arrow as fn(_, _) -> _),
    ]
    .into_iter()
    .map(|(name, make)| workload(name, rows, block_rows, make))
    .collect()
}

fn workload(
    name: &'static str,
    rows: usize,
    block_rows: usize,
    make: fn(usize, usize) -> AnyResult<RecordBatch>,
) -> AnyResult<Workload> {
    let batches = (0..rows)
        .step_by(block_rows)
        .map(|start| make(start, block_rows.min(rows - start)))
        .collect::<AnyResult<Vec<_>>>()?;
    let input_bytes = batches
        .iter()
        .flat_map(RecordBatch::columns)
        .map(|array| array.get_array_memory_size())
        .sum();
    Ok(Workload {
        name,
        batches,
        input_bytes,
    })
}

fn telemetry(start: usize, len: usize) -> AnyResult<RecordBatch> {
    let rows: Vec<_> = (start..start + len)
        .map(|row| {
            let customer = (hash(row as u64) % 20_000) as i64;
            let product = (hash(row as u64 + 1) % 800) as i64;
            [
                1_735_689_600_000 + row as i64 * 1_000,
                customer,
                product,
                customer % 32,
                1 + (hash(row as u64 + 2) % 5) as i64,
                199 + product * 17,
            ]
        })
        .collect();
    let column = |i| rows.iter().map(|row| row[i]).collect::<Vec<_>>();
    Ok(record_batch!(
        ("time", Int64, column(0)),
        ("customer", Int64, column(1)),
        ("product", Int64, column(2)),
        ("country", Int64, column(3)),
        ("quantity", Int64, column(4)),
        ("price", Int64, column(5))
    )?)
}

fn random_i64(start: usize, len: usize) -> AnyResult<RecordBatch> {
    let column = |salt| {
        (start..start + len)
            .map(|row| hash(row as u64 ^ salt) as i64)
            .collect::<Vec<_>>()
    };
    Ok(record_batch!(
        ("a", Int64, column(1)),
        ("b", Int64, column(2)),
        ("c", Int64, column(3)),
        ("d", Int64, column(4)),
        ("e", Int64, column(5)),
        ("f", Int64, column(6))
    )?)
}

fn mixed_arrow(start: usize, len: usize) -> AnyResult<RecordBatch> {
    let rows = start..start + len;
    const COUNTRIES: [&str; 5] = ["IL", "US", "DE", "JP", "BR"];
    Ok(record_batch!(
        (
            "time",
            Int64,
            rows.clone()
                .map(|row| 1_735_689_600_000 + row as i64 * 1_000)
                .collect::<Vec<_>>()
        ),
        (
            "customer",
            Int64,
            rows.clone()
                .map(|row| (row % 20_000) as i64)
                .collect::<Vec<_>>()
        ),
        (
            "amount",
            Float64,
            rows.clone()
                .map(|row| (hash(row as u64) % 100_000) as f64 / 100.0)
                .collect::<Vec<_>>()
        ),
        (
            "country",
            Utf8,
            rows.clone()
                .map(|row| COUNTRIES[row % COUNTRIES.len()])
                .collect::<Vec<_>>()
        ),
        (
            "success",
            Boolean,
            rows.map(|row| row % 11 != 0).collect::<Vec<_>>()
        )
    )?)
}

fn hash(mut value: u64) -> u64 {
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}
