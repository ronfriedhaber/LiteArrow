use std::sync::Arc;

use arrow_array::{Array, ArrayRef, Int64Array};
use arrow_buffer::{BooleanBuffer, Buffer, NullBuffer};
use arrow_schema::{DataType, Field};

use crate::codec::ColumnCodec;
use litearrow_compression::{self as compression, Encoding};

use crate::{Error, Result};

pub(super) struct Int64;

impl ColumnCodec for Int64 {
    fn id(&self) -> u8 {
        1
    }

    fn encode(&self, _: &Field, array: &dyn Array) -> Result<Option<Vec<u8>>> {
        let Some(array) = array.as_any().downcast_ref::<Int64Array>() else {
            return Ok(None);
        };
        let mut previous = 0;
        let values: Vec<_> = (0..array.len())
            .map(|i| {
                if array.is_valid(i) {
                    previous = array.value(i)
                }
                previous
            })
            .collect();
        let (encoding, encoded) = compression::encode(&values)?;
        let mut out = Vec::new();
        put_encoding(&mut out, encoding);
        match array.null_count() {
            0 => out.push(0),
            n if n == array.len() => out.push(1),
            _ => {
                out.push(2);
                let mut bitmap = vec![0; array.len().div_ceil(8)];
                (0..array.len())
                    .filter(|&i| array.is_valid(i))
                    .for_each(|i| {
                        bitmap[i / 8] |= 1 << (i % 8);
                    });
                out.extend(bitmap);
            }
        }
        out.extend(encoded);
        Ok(Some(out))
    }

    fn decode(&self, field: &Field, length: usize, bytes: &[u8]) -> Result<ArrayRef> {
        if field.data_type() != &DataType::Int64 {
            return Err(Error::InvalidMetadata("Int64 codec used for another type"));
        }
        let (encoding, mut at) = read_encoding(bytes)?;
        let validity = *bytes.get(at).ok_or(Error::UnexpectedEndOfInput)?;
        at += 1;
        let bitmap = if validity == 2 {
            let end = at + length.div_ceil(8);
            let bitmap = bytes.get(at..end).ok_or(Error::UnexpectedEndOfInput)?;
            at = end;
            Some(bitmap)
        } else {
            None
        };
        let values = compression::decode(encoding, &bytes[at..], length)?;
        let nulls = match validity {
            0 => None,
            1 => Some(NullBuffer::new_null(length)),
            2 => Some(NullBuffer::new(BooleanBuffer::new(
                Buffer::from(bitmap.unwrap().to_vec()),
                0,
                length,
            ))),
            _ => return Err(Error::InvalidMetadata("invalid validity encoding")),
        };
        Ok(Arc::new(Int64Array::new(values.into(), nulls)))
    }
}

fn put_encoding(out: &mut Vec<u8>, encoding: Encoding) {
    match encoding {
        Encoding::Raw => out.push(0),
        Encoding::FrameOfReferenceBitPacked { minimum, bit_width } => {
            out.push(1);
            out.extend(minimum.to_le_bytes());
            out.push(bit_width);
        }
        Encoding::DeltaBitPacked {
            first_value,
            bit_width,
        } => {
            out.push(2);
            out.extend(first_value.to_le_bytes());
            out.push(bit_width);
        }
        Encoding::DeltaOfDelta {
            first_value,
            first_delta,
            bit_width,
        } => {
            out.push(3);
            out.extend(first_value.to_le_bytes());
            out.extend(first_delta.to_le_bytes());
            out.push(bit_width);
        }
    }
}

fn read_encoding(bytes: &[u8]) -> Result<(Encoding, usize)> {
    let i64_at = |at| {
        bytes
            .get(at..at + 8)
            .ok_or(Error::UnexpectedEndOfInput)
            .map(|bytes| i64::from_le_bytes(bytes.try_into().unwrap()))
    };
    let width = |at| bytes.get(at).copied().ok_or(Error::UnexpectedEndOfInput);
    Ok(
        match bytes.first().copied().ok_or(Error::UnexpectedEndOfInput)? {
            0 => (Encoding::Raw, 1),
            1 => (
                Encoding::FrameOfReferenceBitPacked {
                    minimum: i64_at(1)?,
                    bit_width: width(9)?,
                },
                10,
            ),
            2 => (
                Encoding::DeltaBitPacked {
                    first_value: i64_at(1)?,
                    bit_width: width(9)?,
                },
                10,
            ),
            3 => (
                Encoding::DeltaOfDelta {
                    first_value: i64_at(1)?,
                    first_delta: i64_at(9)?,
                    bit_width: width(17)?,
                },
                18,
            ),
            _ => return Err(Error::InvalidMetadata("invalid Int64 encoding")),
        },
    )
}
