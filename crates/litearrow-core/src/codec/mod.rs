mod int64;
mod ipc;

use arrow_array::{Array, ArrayRef};
use arrow_schema::Field;

use crate::Result;

/// An independent physical representation for one Arrow array.
pub trait ColumnCodec: Send + Sync {
    fn id(&self) -> u8;
    fn encode(&self, field: &Field, array: &dyn Array) -> Result<Option<Vec<u8>>>;
    fn decode(&self, field: &Field, length: usize, bytes: &[u8]) -> Result<ArrayRef>;
}

static INT64: int64::Int64 = int64::Int64;
static IPC: ipc::Ipc = ipc::Ipc;

pub fn specialized() -> [&'static dyn ColumnCodec; 1] {
    [&INT64]
}

pub fn fallback() -> &'static dyn ColumnCodec {
    &IPC
}

pub fn get(id: u8) -> Option<&'static dyn ColumnCodec> {
    specialized()
        .into_iter()
        .chain([fallback()])
        .find(|codec| codec.id() == id)
}

pub use ipc::{decode_schema, encode_schema};
