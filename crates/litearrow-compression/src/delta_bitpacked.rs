use crate::{Encoding, Error, Result};

use super::{Candidate, pack, required_bits, unpack, unzigzag, zigzag};

pub(super) fn encode(values: &[i64]) -> Option<Candidate> {
    let first_value = values.first().copied().unwrap_or(0);
    let mut previous = first_value;
    let deltas = values
        .iter()
        .skip(1)
        .map(|&value| {
            let delta: i64 = (i128::from(value) - i128::from(previous)).try_into().ok()?;
            previous = value;
            Some(zigzag(delta))
        })
        .collect::<Option<Vec<_>>>()?;
    let bit_width = required_bits(deltas.iter().copied().max().unwrap_or(0));
    Some(Candidate {
        encoding: Encoding::DeltaBitPacked {
            first_value,
            bit_width,
        },
        bytes: pack(&deltas, bit_width),
    })
}

pub(super) fn decode(
    bytes: &[u8],
    value_count: usize,
    first_value: i64,
    bit_width: u8,
) -> Result<Vec<i64>> {
    if value_count == 0 {
        return Ok(Vec::new());
    }
    let mut values = Vec::with_capacity(value_count);
    values.push(first_value);
    unpack(bytes, value_count - 1, bit_width).try_for_each(|encoded| {
        let value = values
            .last()
            .unwrap()
            .checked_add(unzigzag(encoded))
            .ok_or(Error("delta value overflow"))?;
        values.push(value);
        Ok(())
    })?;
    Ok(values)
}
