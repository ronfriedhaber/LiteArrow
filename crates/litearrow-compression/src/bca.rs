use crate::{Error, Result};

/// Encodes either seven literal bits or 1–16 repetitions of a three-bit pattern
/// per byte. The high bit selects repetition; low bits hold payload.
pub fn encode(bits: &[u8], count: usize) -> Vec<u8> {
    let bit = |i: usize| bits[i / 8] & (1_u8 << (i % 8)) != 0;
    let mut at = 0;
    std::iter::from_fn(|| {
        (at < count).then(|| {
            let repeats = (1..=16)
                .take_while(|&n| {
                    at + n * 3 <= count && (0..3).all(|i| bit(at + (n - 1) * 3 + i) == bit(at + i))
                })
                .count();
            if repeats >= 3 {
                let pattern = (0..3).fold(0, |byte, i| byte | (bit(at + i) as u8) << (4 + i));
                at += repeats * 3;
                0x80 | pattern | (repeats as u8 - 1)
            } else {
                let length = (count - at).min(7);
                let literal = (0..length).fold(0, |byte, i| byte | (bit(at + i) as u8) << i);
                at += length;
                literal
            }
        })
    })
    .collect()
}

pub fn decode(bytes: &[u8], count: usize) -> Result<Vec<u8>> {
    let mut at = 0;
    let mut bits = vec![0; count.div_ceil(8)];
    bytes.iter().try_for_each(|&byte| {
        if at >= count {
            return Err(Error("extra BCA block"));
        }
        let length = if byte & 0x80 == 0 {
            (count - at).min(7)
        } else {
            3 * (usize::from(byte & 15) + 1)
        };
        if length > count - at {
            return Err(Error("BCA block exceeds column"));
        }
        (0..length)
            .filter(|&i| byte & (1 << if byte & 0x80 == 0 { i } else { 4 + i % 3 }) != 0)
            .for_each(|i| bits[(at + i) / 8] |= 1 << ((at + i) % 8));
        at += length;
        Ok(())
    })?;
    (at == count)
        .then_some(bits)
        .ok_or(Error("BCA blocks do not fill column"))
}
