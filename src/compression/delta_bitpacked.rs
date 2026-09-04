use crate::{Encoding, Error, Result};

use super::{Candidate, pack, required_bits, unpack, unzigzag, zigzag};

pub(super) fn encode(values: &[i64]) -> Option<Candidate> {
    let first_value = values.first().copied().unwrap_or(0);
    let mut previous = first_value;
    let mut deltas = Vec::with_capacity(values.len().saturating_sub(1));
    for &value in values.iter().skip(1) {
        let delta: i64 = (i128::from(value) - i128::from(previous)).try_into().ok()?;
        deltas.push(zigzag(delta));
        previous = value;
    }
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
    let mut previous = first_value;
    for encoded_delta in unpack(bytes, value_count - 1, bit_width) {
        previous = previous
            .checked_add(unzigzag(encoded_delta))
            .ok_or(Error::InvalidMetadata("delta value overflow"))?;
        values.push(previous);
    }
    Ok(values)
}
