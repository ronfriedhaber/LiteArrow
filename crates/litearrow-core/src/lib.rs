//! Shared format, metadata, and codecs used by LiteArrow readers and writers.

mod checksum;
pub mod codec;
mod compression;
mod error;
pub mod format;

pub use checksum::crc32c;
pub use codec::ColumnCodec;
pub use error::{Error, Result};

pub(crate) use compression::Encoding;
