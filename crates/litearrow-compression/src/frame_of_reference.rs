use crate::{Encoding, Error, Result};

use super::{Candidate, pack, required_bits, unpack};

pub(super) fn encode(values: &[i64]) -> Candidate {
    let minimum = values.iter().copied().min().unwrap_or(0);
    let offsets: Vec<u64> = values
        .iter()
        .map(|&value| (i128::from(value) - i128::from(minimum)) as u64)
        .collect();
    let bit_width = required_bits(offsets.iter().copied().max().unwrap_or(0));
    Candidate {
        encoding: Encoding::FrameOfReferenceBitPacked { minimum, bit_width },
        bytes: pack(&offsets, bit_width),
    }
}

pub(super) fn decode(
    bytes: &[u8],
    value_count: usize,
    minimum: i64,
    bit_width: u8,
) -> Result<Vec<i64>> {
    unpack(bytes, value_count, bit_width)
        .into_iter()
        .map(|offset| {
            let value = i128::from(minimum) + i128::from(offset);
            value
                .try_into()
                .map_err(|_| Error("frame-of-reference value overflow"))
        })
        .collect()
}
