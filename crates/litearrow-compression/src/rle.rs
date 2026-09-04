use crate::{Error, Result};

pub fn encode(bits: &[u8], count: usize) -> Vec<u8> {
    if count == 0 {
        return vec![];
    }
    let bit = |i| bits[i / 8] & (1_u8 << (i % 8)) != 0;
    let first = bit(0);
    let (_, run, mut out) = (1..count).fold(
        (first, 1_u32, vec![first as u8]),
        |(last, run, mut out), i| {
            if bit(i) == last {
                (last, run + 1, out)
            } else {
                out.extend(run.to_le_bytes());
                (!last, 1, out)
            }
        },
    );
    out.extend(run.to_le_bytes());
    out
}

pub fn decode(bytes: &[u8], count: usize) -> Result<Vec<u8>> {
    let (&first, runs) = bytes.split_first().ok_or(Error("missing RLE value"))?;
    if first > 1 || runs.len() % 4 != 0 {
        return Err(Error("invalid boolean RLE"));
    }
    let mut at = 0_usize;
    let mut value = first != 0;
    let mut bits = vec![0; count.div_ceil(8)];
    runs.chunks_exact(4).try_for_each(|run| {
        let end = at
            .checked_add(u32::from_le_bytes(run.try_into().unwrap()) as usize)
            .filter(|&end| end <= count)
            .ok_or(Error("invalid boolean run"))?;
        if value {
            (at..end).for_each(|i| bits[i / 8] |= 1 << (i % 8))
        }
        (at, value) = (end, !value);
        Ok(())
    })?;
    if at != count {
        return Err(Error("boolean runs do not fill column"));
    }
    Ok(bits)
}
