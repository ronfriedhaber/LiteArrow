use std::collections::HashMap;

use arrow_array::{Array, ArrayRef, LargeStringArray, StringArray, make_array};
use arrow_buffer::Buffer;
use arrow_data::ArrayData;
use arrow_schema::{DataType, Field};

use crate::codec::{ColumnCodec, validity};
use crate::{Error, Result};

pub(super) struct String;

impl ColumnCodec for String {
    fn id(&self) -> u8 {
        4
    }

    fn encode(&self, _: &Field, array: &dyn Array) -> Result<Option<Vec<u8>>> {
        macro_rules! strings {
            ($ty:ty) => {
                array.as_any().downcast_ref::<$ty>().map(|array| {
                    (0..array.len())
                        .map(|i| {
                            if array.is_valid(i) {
                                array.value(i)
                            } else {
                                ""
                            }
                        })
                        .collect::<Vec<_>>()
                })
            };
        }
        let (values, width) = match array.data_type() {
            DataType::Utf8 => (strings!(StringArray), 4),
            DataType::LargeUtf8 => (strings!(LargeStringArray), 8),
            _ => return Ok(None),
        };
        let Some(values) = values else {
            return Ok(None);
        };
        let raw = raw(&values, width);
        let dictionary = dictionary(&values);
        let (mode, encoded) = if dictionary.len() < raw.len() {
            (1, dictionary)
        } else {
            (0, raw)
        };
        let mut out = vec![mode];
        validity::encode(array, &mut out);
        out.extend(encoded);
        Ok(Some(out))
    }

    fn decode(&self, field: &Field, length: usize, bytes: &[u8]) -> Result<ArrayRef> {
        let width = match field.data_type() {
            DataType::Utf8 => 4,
            DataType::LargeUtf8 => 8,
            _ => return Err(Error::InvalidMetadata("string codec used for another type")),
        };
        let mode = *bytes.first().ok_or(Error::UnexpectedEndOfInput)?;
        let mut at = 1;
        let nulls = validity::decode(bytes, &mut at, length)?;
        let (offsets, values) = match mode {
            0 => split_raw(&bytes[at..], length, width)?,
            1 => expand_dictionary(&bytes[at..], length, width)?,
            _ => return Err(Error::InvalidMetadata("invalid string encoding")),
        };
        Ok(make_array(
            ArrayData::builder(field.data_type().clone())
                .len(length)
                .add_buffer(Buffer::from(offsets))
                .add_buffer(Buffer::from(values))
                .nulls(nulls)
                .build()?,
        ))
    }
}

fn raw(values: &[&str], width: usize) -> Vec<u8> {
    let mut size = 0_u64;
    let offsets = std::iter::once(0).chain(values.iter().map(|value| {
        size += value.len() as u64;
        size
    }));
    offsets
        .flat_map(|offset| offset.to_le_bytes().into_iter().take(width))
        .chain(values.iter().flat_map(|value| value.bytes()))
        .collect()
}

fn dictionary(values: &[&str]) -> Vec<u8> {
    let mut map = HashMap::new();
    let mut dictionary = Vec::new();
    let indices: Vec<u32> = values
        .iter()
        .map(|&value| {
            *map.entry(value).or_insert_with(|| {
                dictionary.push(value);
                (dictionary.len() - 1) as u32
            })
        })
        .collect();
    let width = match dictionary.len() {
        0..=256 => 1,
        257..=65_536 => 2,
        _ => 4,
    };
    let mut out = (dictionary.len() as u32).to_le_bytes().to_vec();
    dictionary.iter().for_each(|value| {
        out.extend((value.len() as u64).to_le_bytes());
        out.extend(value.as_bytes());
    });
    out.push(width as u8);
    out.extend(
        indices
            .into_iter()
            .flat_map(|index| index.to_le_bytes().into_iter().take(width)),
    );
    out
}

fn split_raw(bytes: &[u8], count: usize, width: usize) -> Result<(Vec<u8>, Vec<u8>)> {
    let end = count
        .checked_add(1)
        .and_then(|count| count.checked_mul(width))
        .filter(|&end| end <= bytes.len())
        .ok_or(Error::InvalidMetadata("invalid string offsets"))?;
    let offsets = bytes[..end].to_vec();
    if word(&offsets[end - width..]) != (bytes.len() - end) as u64 {
        return Err(Error::InvalidMetadata("invalid string data length"));
    }
    Ok((offsets, bytes[end..].to_vec()))
}

fn expand_dictionary(bytes: &[u8], count: usize, width: usize) -> Result<(Vec<u8>, Vec<u8>)> {
    let mut at = 0;
    let entries = read_u32(bytes, &mut at)? as usize;
    let ranges = (0..entries)
        .map(|_| {
            let length: usize = read_u64(bytes, &mut at)?.try_into()?;
            let end = at
                .checked_add(length)
                .filter(|&end| end <= bytes.len())
                .ok_or(Error::UnexpectedEndOfInput)?;
            let range = at..end;
            at = end;
            Ok(range)
        })
        .collect::<Result<Vec<_>>>()?;
    let index_width = usize::from(*bytes.get(at).ok_or(Error::UnexpectedEndOfInput)?);
    at += 1;
    let index_bytes = count
        .checked_mul(index_width)
        .ok_or(Error::IntegerOverflow)?;
    if !matches!(index_width, 1 | 2 | 4) || bytes.len() - at != index_bytes {
        return Err(Error::InvalidMetadata("invalid dictionary indices"));
    }
    let mut values = Vec::new();
    let mut offsets = vec![0_u64];
    bytes[at..]
        .chunks_exact(index_width)
        .try_for_each(|index| {
            let range = ranges
                .get(word(index) as usize)
                .ok_or(Error::InvalidMetadata("invalid dictionary index"))?;
            values.extend(&bytes[range.clone()]);
            offsets.push(values.len() as u64);
            Ok::<_, Error>(())
        })?;
    if width == 4 && values.len() > i32::MAX as usize {
        return Err(Error::IntegerOverflow);
    }
    Ok((
        offsets
            .into_iter()
            .flat_map(|offset| offset.to_le_bytes().into_iter().take(width))
            .collect(),
        values,
    ))
}

fn read_u32(bytes: &[u8], at: &mut usize) -> Result<u32> {
    let end = *at + 4;
    let value = u32::from_le_bytes(
        bytes
            .get(*at..end)
            .ok_or(Error::UnexpectedEndOfInput)?
            .try_into()
            .unwrap(),
    );
    *at = end;
    Ok(value)
}

fn read_u64(bytes: &[u8], at: &mut usize) -> Result<u64> {
    let end = *at + 8;
    let value = u64::from_le_bytes(
        bytes
            .get(*at..end)
            .ok_or(Error::UnexpectedEndOfInput)?
            .try_into()
            .unwrap(),
    );
    *at = end;
    Ok(value)
}

fn word(bytes: &[u8]) -> u64 {
    bytes
        .iter()
        .enumerate()
        .fold(0, |word, (i, byte)| word | u64::from(*byte) << (i * 8))
}
