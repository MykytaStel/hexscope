//! DEFLATE (RFC 1951) and zlib (RFC 1950), decoded step by step so each
//! step can be explained, and the bit reader under them. A crate of its own
//! so the browser build can be optimised for size everywhere but here,
//! where hexscope spends its time on a large PNG.

#![forbid(unsafe_code)]

pub mod bits;
pub mod engine;
pub mod explain;
pub mod huffman;
pub mod tables;
pub mod trace;
pub mod zlib;

pub use engine::{BlockKind, Checkpoint, Decoder, EventSink, InflateEvent, NoTrace, Step, inflate};
pub use explain::{BlockTables, CodeGroup, DynamicHeader, Explained, Part, PartKind};
pub use huffman::Huffman;
pub use trace::{BlockSpan, CheckpointSink, TraceSummary};
pub use zlib::{adler32, zlib_decompress};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InflateError {
    /// Ran out of input bits.
    Bits,
    /// Bit pattern matches no code in the table.
    BadHuffmanCode,
    /// Code lengths do not form a valid canonical code.
    BadCodeLengths,
    /// Back-reference points before the start of the output.
    BadDistance,
    /// Symbol outside the defined alphabet.
    BadSymbol,
    /// zlib CMF/FLG bytes are not a DEFLATE stream.
    BadZlibHeader,
    /// Adler-32 over the output does not match the stored value.
    ChecksumMismatch,
    /// Output exceeded the caller's limit.
    OutputTooLarge,
    /// A checkpoint does not describe a valid position in this stream, or the
    /// caller did not supply the output it depends on.
    BadCheckpoint,
}
