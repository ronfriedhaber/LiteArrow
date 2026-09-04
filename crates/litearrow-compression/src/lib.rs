//! Integer compression used by LiteArrow codecs.

mod delta_bitpacked;
mod delta_of_delta;
mod frame_of_reference;
mod raw;

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoding {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Error(pub &'static str);

pub type Result<T> = std::result::Result<T, Error>;

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.0)
    }
}

impl std::error::Error for Error {}

struct Candidate {
    encoding: Encoding,
    bytes: Vec<u8>,
}

/// Tries every version-zero algorithm and keeps the smallest result. Candidate
/// order breaks equal-size ties in favor of the simpler decoder.
pub fn encode(values: &[i64]) -> Result<(Encoding, Vec<u8>)> {
    let best = [
        Some(raw::encode(values)),
        Some(frame_of_reference::encode(values)),
        delta_bitpacked::encode(values),
        delta_of_delta::encode(values),
    ]
    .into_iter()
    .flatten()
    .min_by_key(|candidate| candidate.bytes.len())
    .expect("raw encoding always supplies one candidate");
    Ok((best.encoding, best.bytes))
}

pub fn decode(encoding: Encoding, bytes: &[u8], value_count: usize) -> Result<Vec<i64>> {
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
    values.iter().for_each(|&value| {
        pending |= u128::from(value) << pending_bits;
        pending_bits += bit_width;
        (0..pending_bits / 8).for_each(|_| {
            output.push(pending as u8);
            pending >>= 8;
            pending_bits -= 8;
        });
    });
    if pending_bits != 0 {
        output.push(pending as u8);
    }
    output
}

fn unpack(bytes: &[u8], value_count: usize, bit_width: u8) -> impl ExactSizeIterator<Item = u64> {
    let mut input = bytes.iter().copied();
    let mut pending = 0_u128;
    let mut pending_bits = 0_u8;
    let mask = match bit_width {
        0 => 0,
        64 => u64::MAX,
        width => (1_u64 << width) - 1,
    };
    (0..value_count).map(move |_| {
        let needed_bytes = bit_width.saturating_sub(pending_bits).div_ceil(8);
        (0..needed_bytes).for_each(|_| {
            pending |= u128::from(input.next().unwrap_or(0)) << pending_bits;
            pending_bits += 8;
        });
        let value = (pending as u64) & mask;
        pending >>= bit_width;
        pending_bits -= bit_width;
        value
    })
}

fn packed_length(value_count: usize, bit_width: u8) -> usize {
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
        (0..=64).for_each(|width| {
            let maximum = match width {
                0 => 0,
                64 => u64::MAX,
                width => (1_u64 << width) - 1,
            };
            let values = [0, maximum / 3, maximum];
            assert_eq!(
                unpack(&pack(&values, width), values.len(), width).collect::<Vec<_>>(),
                values
            );
        });
    }

    #[test]
    fn selector_round_trips_distinct_integer_patterns() {
        let cases = [
            vec![7; 100],
            (0..100).map(|value| value * 1_000).collect(),
            vec![i64::MIN, 0, i64::MAX],
            vec![10, -10, 10, -10, 10],
        ];
        cases.into_iter().for_each(|values| {
            let (encoding, bytes) = encode(&values).unwrap();
            assert_eq!(decode(encoding, &bytes, values.len()).unwrap(), values);
        });
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
