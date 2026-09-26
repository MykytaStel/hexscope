//! What one decoding step read, bit by bit, and the code tables it read with.
//! Built only when asked for, so plain decoding pays nothing for it.

use crate::InflateError;
use crate::engine::{BlockKind, Step};

/// What a run of bits meant. The numbering is part of the WASM bridge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum PartKind {
    Final = 0,
    BlockType,
    /// Bits skipped to reach a byte boundary before a stored block's length.
    Padding,
    StoredLen,
    StoredNLen,
    StoredByte,
    HLit,
    HDist,
    HClen,
    /// The lengths of the code that encodes the code lengths.
    CodeLengthCode,
    /// The run-length-encoded code lengths of both tables.
    CodeLengths,
    LitLen,
    LengthExtra,
    Distance,
    DistanceExtra,
    /// Bits consumed by a read that failed.
    Unreadable,
}

/// One run of bits within a step: `[bit_start, bit_end)` of the stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Part {
    pub kind: PartKind,
    pub bit_start: u64,
    pub bit_end: u64,
    /// The decoded meaning: symbol, extra-bit value, count or byte.
    pub value: u32,
    /// For Huffman codes: the code as the decoder assembled it, most
    /// significant bit first, and its length. Zero otherwise.
    pub code: u16,
    pub code_len: u8,
}

impl Part {
    pub(crate) fn plain(kind: PartKind, bit_start: u64, bit_end: u64, value: u32) -> Self {
        Self {
            kind,
            bit_start,
            bit_end,
            value,
            code: 0,
            code_len: 0,
        }
    }

    pub(crate) fn code(
        kind: PartKind,
        bit_start: u64,
        bit_end: u64,
        symbol: u16,
        code: u16,
        code_len: u8,
    ) -> Self {
        Self {
            kind,
            bit_start,
            bit_end,
            value: symbol as u32,
            code,
            code_len,
        }
    }
}

/// One step, explained.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Explained {
    /// `None` when the step failed.
    pub step: Option<Step>,
    pub error: Option<InflateError>,
    /// In reading order, contiguous. Empty for a step that reads nothing,
    /// such as the end of a stored block.
    pub parts: Vec<Part>,
    /// Bit position of the header of the block the step belongs to.
    pub block_start: u64,
}

/// The codes of one length: consecutive values from `first_code`, one per
/// symbol, in symbol order (RFC 1951 §3.2.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeGroup {
    pub len: u8,
    pub first_code: u16,
    pub symbols: Vec<u16>,
}

/// The counts a dynamic block's header declares.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DynamicHeader {
    pub hlit: u16,
    pub hdist: u8,
    pub hclen: u8,
}

/// The code tables a block decodes with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockTables {
    pub kind: BlockKind,
    /// Present for a dynamic block only.
    pub header: Option<DynamicHeader>,
    pub lit_len: Vec<CodeGroup>,
    pub distance: Vec<CodeGroup>,
}
