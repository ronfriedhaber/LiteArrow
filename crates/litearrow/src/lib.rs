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

pub use litearrow_core::{ColumnCodec, Error, Result};
pub use litearrow_reader::FileReader;
pub use litearrow_writer::FileWriter;
