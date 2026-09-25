#![forbid(unsafe_code)]

pub mod bits;
mod bmff;
pub mod clean;
pub mod crc32;
pub mod crypto;
pub mod docs;
pub mod document;
pub mod exif;
pub mod heif;
pub mod inflate;
pub mod jpeg;
pub mod map;
pub mod model;
pub mod pdf;
pub mod png;
pub mod reader;
pub mod video;
pub mod wasm;
pub mod zip;

pub use document::{Document, Format, parse};
pub use model::{ByteRange, Node, NodeId, NodeKind, ParseTree, Value};
pub use png::{PngDocument, parse_png};
pub use reader::{ReadError, Reader};
