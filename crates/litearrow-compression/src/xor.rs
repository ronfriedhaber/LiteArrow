use crate::{Error, Result, pack, packed_length, required_bits, unpack};

#[derive(Clone, Copy)]
pub enum Encoding {
    Raw,
    Xor { first: u64, bit_width: u8 },
}

pub fn encode(values: &[u64], byte_width: usize) -> (Encoding, Vec<u8>) {
    let raw: Vec<_> = values
        .iter()
        .flat_map(|value| value.to_le_bytes().into_iter().take(byte_width))
        .collect();
    let first = values.first().copied().unwrap_or(0);
    let xors: Vec<_> = values.windows(2).map(|pair| pair[0] ^ pair[1]).collect();
    let bit_width = required_bits(xors.iter().copied().max().unwrap_or(0));
    let packed = pack(&xors, bit_width);
    if packed.len() + byte_width + 1 < raw.len() {
        (Encoding::Xor { first, bit_width }, packed)
    } else {
        (Encoding::Raw, raw)
    }
}

pub fn decode(
    encoding: Encoding,
    bytes: &[u8],
    count: usize,
    byte_width: usize,
) -> Result<Vec<u64>> {
    if let Encoding::Raw = encoding {
        if bytes.len() != count.saturating_mul(byte_width) {
            return Err(Error("invalid raw float length"));
        }
        return Ok(bytes
            .chunks_exact(byte_width)
            .map(|bytes| {
                bytes
                    .iter()
                    .enumerate()
                    .fold(0, |word, (i, byte)| word | u64::from(*byte) << (i * 8))
            })
            .collect());
    }
    let Encoding::Xor { first, bit_width } = encoding else {
        unreachable!()
    };
    if bytes.len() != packed_length(count.saturating_sub(1), bit_width) {
        return Err(Error("invalid XOR float length"));
    }
    let mut values = Vec::with_capacity(count);
    if count != 0 {
        values.push(first)
    }
    unpack(bytes, count.saturating_sub(1), bit_width)
        .for_each(|xor| values.push(values.last().copied().unwrap() ^ xor));
    Ok(values)
}
