//! File = header, opaque column chunks, footer, footer length, footer CRC, magic.

use std::io::{Cursor, Read};

use crate::{Error, Result, crc32c};

pub(crate) const HEADER: [u8; 8] = *b"LTAR\x01\0\0\0";
pub(crate) const TRAILER_LENGTH: u64 = 16;
const MAGIC: [u8; 4] = *b"LTAR";

pub(crate) struct Metadata {
    pub schema: Vec<u8>,
    pub field_count: u32,
    pub blocks: Vec<Block>,
}

#[derive(Clone)]
pub(crate) struct Block {
    pub rows: u32,
    pub columns: Vec<Chunk>,
}

#[derive(Clone, Copy)]
pub(crate) struct Chunk {
    pub codec: u8,
    pub offset: u64,
    pub length: u64,
}

pub(crate) fn encode(metadata: &Metadata) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    u32(&mut out, length(metadata.schema.len())?);
    out.extend(&metadata.schema);
    u32(&mut out, metadata.field_count);
    u32(&mut out, length(metadata.blocks.len())?);
    metadata.blocks.iter().for_each(|block| {
        u32(&mut out, block.rows);
        block.columns.iter().for_each(|column| {
            out.push(column.codec);
            u64(&mut out, column.offset);
            u64(&mut out, column.length);
        });
    });
    Ok(out)
}

pub(crate) fn decode(bytes: &[u8]) -> Result<Metadata> {
    let mut input = Cursor::new(bytes);
    let schema_length = read_u32(&mut input)? as usize;
    let mut schema = vec![0; schema_length];
    input.read_exact(&mut schema)?;
    let field_count = read_u32(&mut input)?;
    let block_count = read_u32(&mut input)?;
    let blocks = (0..block_count)
        .map(|_| {
            let rows = read_u32(&mut input)?;
            let columns = (0..field_count)
                .map(|_| {
                    Ok(Chunk {
                        codec: read::<1>(&mut input)?[0],
                        offset: read_u64(&mut input)?,
                        length: read_u64(&mut input)?,
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            Ok(Block { rows, columns })
        })
        .collect::<Result<Vec<_>>>()?;
    if input.position() != bytes.len() as u64 {
        return Err(Error::InvalidMetadata("trailing footer bytes"));
    }
    Ok(Metadata {
        schema,
        field_count,
        blocks,
    })
}

pub(crate) fn trailer(footer: &[u8]) -> [u8; 16] {
    let mut out = [0; 16];
    out[..8].copy_from_slice(&(footer.len() as u64).to_le_bytes());
    out[8..12].copy_from_slice(&crc32c(footer).to_le_bytes());
    out[12..].copy_from_slice(&MAGIC);
    out
}

pub(crate) fn read_trailer(bytes: [u8; 16]) -> Result<(u64, u32)> {
    if bytes[12..] != MAGIC {
        return Err(Error::InvalidMagic {
            structure: "trailer",
        });
    }
    Ok((
        u64::from_le_bytes(bytes[..8].try_into().unwrap()),
        u32::from_le_bytes(bytes[8..12].try_into().unwrap()),
    ))
}

fn length(value: usize) -> Result<u32> {
    value.try_into().map_err(|_| Error::IntegerOverflow)
}
fn u32(out: &mut Vec<u8>, value: u32) {
    out.extend(value.to_le_bytes())
}
fn u64(out: &mut Vec<u8>, value: u64) {
    out.extend(value.to_le_bytes())
}
fn read_u32(input: &mut Cursor<&[u8]>) -> Result<u32> {
    Ok(u32::from_le_bytes(read(input)?))
}
fn read_u64(input: &mut Cursor<&[u8]>) -> Result<u64> {
    Ok(u64::from_le_bytes(read(input)?))
}
fn read<const N: usize>(input: &mut Cursor<&[u8]>) -> Result<[u8; N]> {
    let mut bytes = [0; N];
    input.read_exact(&mut bytes)?;
    Ok(bytes)
}
