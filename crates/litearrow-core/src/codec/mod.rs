mod boolean;
mod float;
mod integer;
mod ipc;
mod string;
mod validity;

use arrow_array::{Array, ArrayRef};
use arrow_schema::Field;

use crate::Result;

/// An independent physical representation for one Arrow array.
pub trait ColumnCodec: Send + Sync {
    fn id(&self) -> u8;
    fn encode(&self, field: &Field, array: &dyn Array) -> Result<Option<Vec<u8>>>;
    fn decode(&self, field: &Field, length: usize, bytes: &[u8]) -> Result<ArrayRef>;
}

static BOOLEAN: boolean::Boolean = boolean::Boolean;
static FLOAT: float::Float = float::Float;
static INTEGER: integer::Integer = integer::Integer;
static IPC: ipc::Ipc = ipc::Ipc;
static STRING: string::String = string::String;

pub fn specialized() -> [&'static dyn ColumnCodec; 4] {
    [&INTEGER, &FLOAT, &BOOLEAN, &STRING]
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
