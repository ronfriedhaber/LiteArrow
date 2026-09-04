use std::sync::Arc;

use arrow_array::{Array, ArrayRef, BooleanArray};
use arrow_buffer::{BooleanBuffer, Buffer};
use arrow_schema::{DataType, Field};
use litearrow_compression::{bca, rle};

use crate::codec::{ColumnCodec, validity};
use crate::{Error, Result};

pub(super) struct Boolean;

impl ColumnCodec for Boolean {
    fn id(&self) -> u8 {
        3
    }

    fn encode(&self, _: &Field, array: &dyn Array) -> Result<Option<Vec<u8>>> {
        let Some(array) = array.as_any().downcast_ref::<BooleanArray>() else {
            return Ok(None);
        };
        let mut packed = vec![0; array.len().div_ceil(8)];
        (0..array.len())
            .filter(|&i| array.is_valid(i) && array.value(i))
            .for_each(|i| packed[i / 8] |= 1 << (i % 8));
        let candidates = [
            (1, rle::encode(&packed, array.len())),
            (2, bca::encode(&packed, array.len())),
        ];
        let (mode, values) = candidates.into_iter().fold((0, packed), |best, candidate| {
            if candidate.1.len() < best.1.len() {
                candidate
            } else {
                best
            }
        });
        let mut out = vec![mode];
        validity::encode(array, &mut out);
        out.extend(values);
        Ok(Some(out))
    }

    fn decode(&self, field: &Field, length: usize, bytes: &[u8]) -> Result<ArrayRef> {
        if field.data_type() != &DataType::Boolean {
            return Err(Error::InvalidMetadata(
                "boolean codec used for another type",
            ));
        }
        let mode = *bytes.first().ok_or(Error::UnexpectedEndOfInput)?;
        let mut at = 1;
        let nulls = validity::decode(bytes, &mut at, length)?;
        let values = match mode {
            0 if bytes.len() - at == length.div_ceil(8) => bytes[at..].to_vec(),
            0 => return Err(Error::InvalidMetadata("invalid boolean bitmap length")),
            1 => rle::decode(&bytes[at..], length)?,
            2 => bca::decode(&bytes[at..], length)?,
            _ => return Err(Error::InvalidMetadata("invalid boolean encoding")),
        };
        Ok(Arc::new(BooleanArray::new(
            BooleanBuffer::new(Buffer::from(values), 0, length),
            nulls,
        )))
    }
}
