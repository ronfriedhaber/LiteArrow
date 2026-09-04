use std::{
    hint::black_box,
    time::{Duration, Instant},
};

use crate::AnyResult;

#[derive(Clone, Copy)]
pub struct Timing {
    pub median: Duration,
    pub p95: Duration,
}

pub fn measure<T>(count: usize, mut operation: impl FnMut() -> AnyResult<T>) -> AnyResult<Timing> {
    black_box(operation()?);
    let mut samples = (0..count)
        .map(|_| {
            let start = Instant::now();
            black_box(operation()?);
            Ok(start.elapsed())
        })
        .collect::<AnyResult<Vec<_>>>()?;
    samples.sort_unstable();
    Ok(Timing {
        median: samples[count / 2],
        p95: samples[count.saturating_mul(95).div_ceil(100) - 1],
    })
}
