#![forbid(unsafe_code)]

pub mod bits;
pub mod crc32;
pub mod document;
pub mod exif;
pub mod inflate;
pub mod jpeg;
pub mod model;
pub mod png;
pub mod reader;

pub use document::{Document, Format, parse};
pub use model::{ByteRange, Node, NodeId, NodeKind, ParseTree, Value};
pub use png::{PngDocument, parse_png};
pub use reader::{ReadError, Reader};
