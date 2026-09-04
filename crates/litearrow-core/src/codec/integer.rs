use std::sync::Arc;

use arrow_array::types::{
    Int8Type, Int16Type, Int32Type, Int64Type, UInt8Type, UInt16Type, UInt32Type, UInt64Type,
};
use arrow_array::{Array, ArrayRef, PrimitiveArray};
use arrow_schema::{DataType, Field};

use crate::codec::{ColumnCodec, validity};
use litearrow_compression::{self as compression, Encoding};

use crate::{Error, Result};

pub(super) struct Integer;

impl ColumnCodec for Integer {
    fn id(&self) -> u8 {
        1
    }

    fn encode(&self, _: &Field, array: &dyn Array) -> Result<Option<Vec<u8>>> {
        macro_rules! values {
            ($ty:ty) => {
                array
                    .as_any()
                    .downcast_ref::<PrimitiveArray<$ty>>()
                    .map(|array| {
                        let mut previous = 0;
                        (0..array.len())
                            .map(|i| {
                                if array.is_valid(i) {
                                    previous = array.value(i) as i64
                                }
                                previous
                            })
                            .collect::<Vec<i64>>()
                    })
            };
        }
        let Some(values) = (match array.data_type() {
            DataType::Int8 => values!(Int8Type),
            DataType::Int16 => values!(Int16Type),
            DataType::Int32 => values!(Int32Type),
            DataType::Int64 => values!(Int64Type),
            DataType::UInt8 => values!(UInt8Type),
            DataType::UInt16 => values!(UInt16Type),
            DataType::UInt32 => values!(UInt32Type),
            DataType::UInt64 => values!(UInt64Type),
            _ => None,
        }) else {
            return Ok(None);
        };
        let (encoding, encoded) = compression::encode(&values)?;
        let mut out = Vec::new();
        put_encoding(&mut out, encoding);
        validity::encode(array, &mut out);
        out.extend(encoded);
        Ok(Some(out))
    }

    fn decode(&self, field: &Field, length: usize, bytes: &[u8]) -> Result<ArrayRef> {
        if !field.data_type().is_integer() {
            return Err(Error::InvalidMetadata(
                "integer codec used for another type",
            ));
        }
        let (encoding, mut at) = read_encoding(bytes)?;
        let nulls = validity::decode(bytes, &mut at, length)?;
        let values = compression::decode(encoding, &bytes[at..], length)?;
        macro_rules! array {
            ($ty:ty, $native:ty) => {{
                let values: Vec<$native> =
                    values.into_iter().map(|value| value as $native).collect();
                Arc::new(PrimitiveArray::<$ty>::new(values.into(), nulls)) as ArrayRef
            }};
        }
        Ok(match field.data_type() {
            DataType::Int8 => array!(Int8Type, i8),
            DataType::Int16 => array!(Int16Type, i16),
            DataType::Int32 => array!(Int32Type, i32),
            DataType::Int64 => array!(Int64Type, i64),
            DataType::UInt8 => array!(UInt8Type, u8),
            DataType::UInt16 => array!(UInt16Type, u16),
            DataType::UInt32 => array!(UInt32Type, u32),
            DataType::UInt64 => array!(UInt64Type, u64),
            _ => unreachable!(),
        })
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
