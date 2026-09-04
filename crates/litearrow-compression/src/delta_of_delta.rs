use crate::{Encoding, Error, Result};

use super::{Candidate, pack, required_bits, unpack, unzigzag, zigzag};

pub(super) fn encode(values: &[i64]) -> Option<Candidate> {
    let first_value = values.first().copied().unwrap_or(0);
    let first_delta = difference(*values.get(1).unwrap_or(&first_value), first_value)?;
    let mut previous_value = *values.get(1).unwrap_or(&first_value);
    let mut previous_delta = first_delta;
    let deltas = values
        .iter()
        .skip(2)
        .map(|&value| {
            let delta = difference(value, previous_value)?;
            let encoded = zigzag(difference(delta, previous_delta)?);
            (previous_value, previous_delta) = (value, delta);
            Some(encoded)
        })
        .collect::<Option<Vec<_>>>()?;
    let bit_width = required_bits(deltas.iter().copied().max().unwrap_or(0));
    Some(Candidate {
        encoding: Encoding::DeltaOfDelta {
            first_value,
            first_delta,
            bit_width,
        },
        bytes: pack(&deltas, bit_width),
    })
}

pub(super) fn decode(
    bytes: &[u8],
    count: usize,
    first_value: i64,
    first_delta: i64,
    bit_width: u8,
) -> Result<Vec<i64>> {
    let mut values = Vec::with_capacity(count);
    if count == 0 {
        return Ok(values);
    }
    values.push(first_value);
    if count == 1 {
        return Ok(values);
    }
    let value = first_value
        .checked_add(first_delta)
        .ok_or(Error("delta-of-delta value overflow"))?;
    values.push(value);
    let mut delta = first_delta;
    let mut value = value;
    unpack(bytes, count - 2, bit_width).try_for_each(|encoded| {
        delta = delta
            .checked_add(unzigzag(encoded))
            .ok_or(Error("delta-of-delta overflow"))?;
        value = value
            .checked_add(delta)
            .ok_or(Error("delta-of-delta value overflow"))?;
        values.push(value);
        Ok(())
    })?;
    Ok(values)
}

fn difference(left: i64, right: i64) -> Option<i64> {
    (i128::from(left) - i128::from(right)).try_into().ok()
}
