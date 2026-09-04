use arrow_array::{Array, ArrayRef, make_array};
use arrow_buffer::Buffer;
use arrow_data::ArrayData;
use arrow_schema::{DataType, Field};
use litearrow_compression::xor::{self, Encoding};

use crate::codec::{ColumnCodec, validity};
use crate::{Error, Result};

pub(super) struct Float;

impl ColumnCodec for Float {
    fn id(&self) -> u8 {
        2
    }

    fn encode(&self, _: &Field, array: &dyn Array) -> Result<Option<Vec<u8>>> {
        let width = match array.data_type() {
            DataType::Float16 => 2,
            DataType::Float32 => 4,
            DataType::Float64 => 8,
            _ => return Ok(None),
        };
        let data = array.to_data();
        let start = data.offset() * width;
        let bytes = &data.buffers()[0][start..start + array.len() * width];
        let mut previous = 0;
        let values = bytes
            .chunks_exact(width)
            .enumerate()
            .map(|(i, bytes)| {
                if array.is_valid(i) {
                    previous = word(bytes)
                }
                previous
            })
            .collect::<Vec<_>>();
        let (encoding, encoded) = xor::encode(&values, width);
        let mut out = vec![matches!(encoding, Encoding::Xor { .. }) as u8];
        if let Encoding::Xor { first, bit_width } = encoding {
            out.extend(&first.to_le_bytes()[..width]);
            out.push(bit_width);
        }
        validity::encode(array, &mut out);
        out.extend(encoded);
        Ok(Some(out))
    }

    fn decode(&self, field: &Field, length: usize, bytes: &[u8]) -> Result<ArrayRef> {
        let width = match field.data_type() {
            DataType::Float16 => 2,
            DataType::Float32 => 4,
            DataType::Float64 => 8,
            _ => return Err(Error::InvalidMetadata("float codec used for another type")),
        };
        let (encoding, mut at) = match bytes.first() {
            Some(0) => (Encoding::Raw, 1),
            Some(1) => {
                let end = 1 + width;
                let first = word(bytes.get(1..end).ok_or(Error::UnexpectedEndOfInput)?);
                let bit_width = *bytes.get(end).ok_or(Error::UnexpectedEndOfInput)?;
                (Encoding::Xor { first, bit_width }, end + 1)
            }
            _ => return Err(Error::InvalidMetadata("invalid float encoding")),
        };
        let nulls = validity::decode(bytes, &mut at, length)?;
        let values = xor::decode(encoding, &bytes[at..], length, width)?;
        let values: Vec<_> = values
            .into_iter()
            .flat_map(|value| value.to_le_bytes().into_iter().take(width))
            .collect();
        Ok(make_array(
            ArrayData::builder(field.data_type().clone())
                .len(length)
                .add_buffer(Buffer::from(values))
                .nulls(nulls)
                .build()?,
        ))
    }
}

fn word(bytes: &[u8]) -> u64 {
    bytes
        .iter()
        .enumerate()
        .fold(0, |word, (i, byte)| word | u64::from(*byte) << (i * 8))
}
