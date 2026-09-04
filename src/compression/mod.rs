//! Small, typed compression algorithms and their size-based selector.

mod delta_bitpacked;
mod delta_of_delta;
mod frame_of_reference;
mod raw;

use crate::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Encoding {
    Raw,
    FrameOfReferenceBitPacked {
        minimum: i64,
        bit_width: u8,
    },
    DeltaBitPacked {
        first_value: i64,
        bit_width: u8,
    },
    DeltaOfDelta {
        first_value: i64,
        first_delta: i64,
        bit_width: u8,
    },
}

struct Candidate {
    encoding: Encoding,
    bytes: Vec<u8>,
}

/// Tries every version-zero algorithm and keeps the smallest result. Candidate
/// order breaks equal-size ties in favor of the simpler decoder.
pub(crate) fn encode(values: &[i64]) -> Result<(Encoding, Vec<u8>)> {
    let mut candidates = vec![raw::encode(values)];
    candidates.push(frame_of_reference::encode(values));
    if let Some(delta) = delta_bitpacked::encode(values) {
        candidates.push(delta);
    }
    if let Some(delta_of_delta) = delta_of_delta::encode(values) {
        candidates.push(delta_of_delta);
    }
    let best = candidates
        .into_iter()
        .min_by_key(|candidate| candidate.bytes.len())
        .expect("raw encoding always supplies one candidate");
    Ok((best.encoding, best.bytes))
}

pub(crate) fn decode(encoding: Encoding, bytes: &[u8], value_count: usize) -> Result<Vec<i64>> {
    match encoding {
        Encoding::Raw => raw::decode(bytes, value_count),
        Encoding::FrameOfReferenceBitPacked { minimum, bit_width } => {
            frame_of_reference::decode(bytes, value_count, minimum, bit_width)
        }
        Encoding::DeltaBitPacked {
            first_value,
            bit_width,
        } => delta_bitpacked::decode(bytes, value_count, first_value, bit_width),
        Encoding::DeltaOfDelta {
            first_value,
            first_delta,
            bit_width,
        } => delta_of_delta::decode(bytes, value_count, first_value, first_delta, bit_width),
    }
}

/// Packs unsigned integers consecutively, least-significant bit first.
fn pack(values: &[u64], bit_width: u8) -> Vec<u8> {
    let mut output = Vec::with_capacity(packed_length(values.len(), bit_width));
    let mut pending = 0_u128;
    let mut pending_bits = 0_u8;
    for &value in values {
        pending |= u128::from(value) << pending_bits;
        pending_bits += bit_width;
        while pending_bits >= 8 {
            output.push(pending as u8);
            pending >>= 8;
            pending_bits -= 8;
        }
    }
    if pending_bits != 0 {
        output.push(pending as u8);
    }
    output
}

fn unpack(bytes: &[u8], value_count: usize, bit_width: u8) -> Vec<u64> {
    let mut values = Vec::with_capacity(value_count);
    let mut input = bytes.iter().copied();
    let mut pending = 0_u128;
    let mut pending_bits = 0_u8;
    let mask = match bit_width {
        0 => 0,
        64 => u64::MAX,
        width => (1_u64 << width) - 1,
    };
    for _ in 0..value_count {
        while pending_bits < bit_width {
            pending |= u128::from(input.next().unwrap_or(0)) << pending_bits;
            pending_bits += 8;
        }
        values.push((pending as u64) & mask);
        pending >>= bit_width;
        pending_bits -= bit_width;
    }
    values
}

pub(crate) fn packed_length(value_count: usize, bit_width: u8) -> usize {
    value_count
        .saturating_mul(usize::from(bit_width))
        .div_ceil(8)
}

fn required_bits(maximum: u64) -> u8 {
    (u64::BITS - maximum.leading_zeros()) as u8
}

fn zigzag(value: i64) -> u64 {
    ((value as u64) << 1) ^ ((value >> 63) as u64)
}

fn unzigzag(value: u64) -> i64 {
    ((value >> 1) as i64) ^ -((value & 1) as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bit_packing_round_trips_every_width() {
        for width in 0..=64 {
            let maximum = match width {
                0 => 0,
                64 => u64::MAX,
                width => (1_u64 << width) - 1,
            };
            let values = [0, maximum / 3, maximum];
            assert_eq!(unpack(&pack(&values, width), values.len(), width), values);
        }
    }

    #[test]
    fn selector_round_trips_distinct_integer_patterns() {
        let cases = [
            vec![7; 100],
            (0..100).map(|value| value * 1_000).collect(),
            vec![i64::MIN, 0, i64::MAX],
            vec![10, -10, 10, -10, 10],
        ];
        for values in cases {
            let (encoding, bytes) = encode(&values).unwrap();
            assert_eq!(decode(encoding, &bytes, values.len()).unwrap(), values);
        }
    }

    #[test]
    fn selector_prefers_delta_of_delta_for_regular_timestamps() {
        let values: Vec<i64> = (0..1_000)
            .map(|row| 1_735_689_600_000 + row * 1_000)
            .collect();
        let (encoding, _) = encode(&values).unwrap();
        assert!(matches!(encoding, Encoding::DeltaOfDelta { .. }));
    }
}
