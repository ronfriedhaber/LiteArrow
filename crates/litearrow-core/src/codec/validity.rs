use arrow_array::Array;
use arrow_buffer::{BooleanBuffer, Buffer, NullBuffer};

use crate::{Error, Result};

pub fn encode(array: &dyn Array, out: &mut Vec<u8>) {
    match array.null_count() {
        0 => out.push(0),
        n if n == array.len() => out.push(1),
        _ => {
            out.push(2);
            let mut bitmap = vec![0; array.len().div_ceil(8)];
            (0..array.len())
                .filter(|&i| array.is_valid(i))
                .for_each(|i| bitmap[i / 8] |= 1 << (i % 8));
            out.extend(bitmap);
        }
    }
}

pub fn decode(bytes: &[u8], at: &mut usize, length: usize) -> Result<Option<NullBuffer>> {
    let validity = *bytes.get(*at).ok_or(Error::UnexpectedEndOfInput)?;
    *at += 1;
    Ok(match validity {
        0 => None,
        1 => Some(NullBuffer::new_null(length)),
        2 => {
            let end = *at + length.div_ceil(8);
            let bitmap = bytes.get(*at..end).ok_or(Error::UnexpectedEndOfInput)?;
            *at = end;
            Some(NullBuffer::new(BooleanBuffer::new(
                Buffer::from(bitmap.to_vec()),
                0,
                length,
            )))
        }
        _ => return Err(Error::InvalidMetadata("invalid validity encoding")),
    })
}
