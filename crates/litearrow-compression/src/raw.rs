use crate::{Encoding, Error, Result};

use super::Candidate;

pub(super) fn encode(values: &[i64]) -> Candidate {
    let bytes = values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect();
    Candidate {
        encoding: Encoding::Raw,
        bytes,
    }
}

pub(super) fn decode(bytes: &[u8], value_count: usize) -> Result<Vec<i64>> {
    if bytes.len() != value_count.saturating_mul(8) {
        return Err(Error("invalid raw value length"));
    }
    Ok(bytes
        .chunks_exact(8)
        .map(|value| i64::from_le_bytes(value.try_into().unwrap()))
        .collect())
}
