//! LiteArrow is an experimental, Arrow-oriented columnar file format.
//!
//! One file describes one table. Rows are divided into blocks; every block
//! stores one independently encoded chunk per column. The complete schema and
//! block index live near the end of the file, followed by a fixed-size trailer.
//!
//! ```text
//! +--------+----------+-----+----------+---------------+---------+
//! | header | block 0  | ... | block N  | file metadata | trailer |
//! +--------+----------+-----+----------+---------------+---------+
//! ```
//!
//! All Arrow IPC types round-trip through [`FileWriter`] and [`FileReader`];
//! `Int64` additionally receives adaptive native compression.

mod checksum;
mod codec;
mod compression;
mod error;
mod format;
mod reader;
mod writer;

pub use codec::ColumnCodec;
pub use error::{Error, Result};
pub use reader::FileReader;
pub use writer::FileWriter;

pub(crate) use checksum::crc32c;
pub(crate) use compression::Encoding;
