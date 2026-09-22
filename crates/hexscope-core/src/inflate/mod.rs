pub mod engine;
pub mod huffman;
pub mod tables;
pub mod trace;
pub mod zlib;

pub use engine::{BlockKind, EventSink, InflateEvent, NoTrace, inflate};
pub use huffman::Huffman;
pub use trace::{Checkpoint, CheckpointSink, TraceSummary};
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
}
