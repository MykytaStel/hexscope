use crate::bits::{BitError, BitReader};
use crate::inflate::InflateError;
use crate::inflate::explain::{BlockTables, DynamicHeader, Explained, Part, PartKind};
use crate::inflate::huffman::Huffman;
use crate::inflate::tables::{
    CODE_LENGTH_ORDER, DIST_BASE, DIST_EXTRA, LENGTH_BASE, LENGTH_EXTRA, fixed_distance_lengths,
    fixed_literal_lengths,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockKind {
    Stored,
    Fixed,
    Dynamic,
}

/// What one decoding step did, as the UI wants to animate it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InflateEvent {
    /// A block header was read. `bit_pos` is just past BFINAL and BTYPE.
    BlockStart {
        kind: BlockKind,
        bit_pos: u64,
    },
    Literal {
        byte: u8,
    },
    Match {
        distance: u16,
        length: u16,
    },
    BlockEnd,
}

/// One decoding step and exactly where in both streams it happened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Step {
    pub event: InflateEvent,
    /// Position of this step in the whole stream, from zero.
    pub index: u64,
    /// Bit position of the header of the block this step belongs to.
    pub block_start: u64,
    /// Compressed bits this step consumed: `[bit_start, bit_end)`. Steps are
    /// contiguous, so these partition the stream.
    pub bit_start: u64,
    pub bit_end: u64,
    /// Output offset where this step's bytes begin.
    pub out_start: u64,
}

impl Step {
    /// A resume point just before this step: [`Decoder::resume`] from here
    /// produces this very step next.
    pub fn checkpoint(&self) -> Checkpoint {
        Checkpoint {
            event_index: self.index,
            block_start: self.block_start,
            bit_pos: self.bit_start,
            out_pos: self.out_start,
        }
    }
}

/// Where decoding can restart without replaying from the beginning.
///
/// A checkpoint is always the state *before* some step, never after the last
/// one, so it can always be resumed. The enclosing block's header position is
/// kept because a block's Huffman tables live in its header: resuming re-reads
/// them rather than storing them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Checkpoint {
    pub event_index: u64,
    pub block_start: u64,
    pub bit_pos: u64,
    pub out_pos: u64,
}

/// Receives steps as decoding proceeds. A sink rather than a returned `Vec`
/// lets a caller decode a 200 MB stream without holding millions of steps.
pub trait EventSink {
    fn emit(&mut self, step: &Step);
}

/// Sink that discards everything — for plain decoding.
pub struct NoTrace;

impl EventSink for NoTrace {
    fn emit(&mut self, _step: &Step) {}
}

fn bit_err(_: BitError) -> InflateError {
    InflateError::Bits
}

/// Notes a part while a step is being explained; only a branch otherwise.
fn note(record: &mut Option<Vec<Part>>, part: impl FnOnce() -> Part) {
    if let Some(parts) = record {
        parts.push(part());
    }
}

enum Block {
    /// The next thing to read is a block header.
    Header,
    Stored {
        remaining: usize,
    },
    Coded {
        kind: BlockKind,
        lit: Huffman,
        dist: Huffman,
        header: Option<DynamicHeader>,
    },
    /// The final block has ended, or decoding failed.
    Done,
}

/// A DEFLATE decoder that advances one step at a time, so it can be paused,
/// inspected, and resumed from a [`Checkpoint`].
///
/// Every step either consumes at least one bit or moves from one block state
/// to another, so iterating it always terminates.
pub struct Decoder<'a> {
    br: BitReader<'a>,
    out: Vec<u8>,
    max_output: u64,
    block: Block,
    is_final: bool,
    block_start: u64,
    index: u64,
    /// Parts of the current step, collected only while explaining.
    record: Option<Vec<Part>>,
}

impl<'a> Decoder<'a> {
    /// `max_output` caps the output so a crafted stream cannot exhaust memory.
    pub fn new(data: &'a [u8], max_output: u64) -> Self {
        Self {
            br: BitReader::new(data),
            out: Vec::new(),
            max_output,
            block: Block::Header,
            is_final: false,
            block_start: 0,
            index: 0,
            record: None,
        }
    }

    /// Restarts decoding at `cp`. `produced` is output already decoded from
    /// this stream — at least `cp.out_pos` bytes of it — which back-references
    /// after the checkpoint may copy from.
    pub fn resume(
        data: &'a [u8],
        max_output: u64,
        produced: &[u8],
        cp: &Checkpoint,
    ) -> Result<Self, InflateError> {
        let out_pos = usize::try_from(cp.out_pos).map_err(|_| InflateError::BadCheckpoint)?;
        let prefix = produced.get(..out_pos).ok_or(InflateError::BadCheckpoint)?;
        if cp.bit_pos < cp.block_start {
            return Err(InflateError::BadCheckpoint);
        }

        let mut d = Self {
            br: BitReader::new(data),
            out: prefix.to_vec(),
            max_output,
            block: Block::Header,
            is_final: false,
            block_start: cp.block_start,
            index: cp.event_index,
            record: None,
        };
        d.br.seek_bits(cp.block_start);

        if cp.bit_pos > cp.block_start {
            // Mid-block: re-read the header to rebuild the block's state, then
            // jump forward to where the step begins.
            d.open_block()?;
            let data_start = d.br.bit_pos();
            if cp.bit_pos < data_start {
                return Err(InflateError::BadCheckpoint);
            }
            if let Block::Stored { remaining } = &mut d.block {
                let skipped = cp.bit_pos - data_start;
                if !skipped.is_multiple_of(8) {
                    return Err(InflateError::BadCheckpoint);
                }
                *remaining = remaining
                    .checked_sub((skipped / 8) as usize)
                    .ok_or(InflateError::BadCheckpoint)?;
            }
            d.br.seek_bits(cp.bit_pos);
        }
        Ok(d)
    }

    /// Current position in the compressed stream. After an error, this is
    /// where decoding stopped.
    pub fn bit_pos(&self) -> u64 {
        self.br.bit_pos()
    }

    /// Output decoded so far.
    pub fn output(&self) -> &[u8] {
        &self.out
    }

    pub fn into_output(self) -> Vec<u8> {
        self.out
    }

    /// The index the next step will have.
    pub fn next_index(&self) -> u64 {
        self.index
    }

    /// Decodes one step like [`step`](Self::step), also recording what each
    /// run of bits it read meant. `None` once decoding has finished.
    pub fn explain_next(&mut self) -> Option<Explained> {
        let start = self.br.bit_pos();
        self.record = Some(Vec::new());
        let result = self.step();
        let mut parts = self.record.take().unwrap_or_default();
        let (step, error) = match result? {
            Ok(step) => (Some(step), None),
            Err(err) => {
                let from = parts.last().map_or(start, |p| p.bit_end);
                let mut to = self.br.bit_pos();
                // A read that ran out of input consumed nothing: blame the
                // bits that were left, since they were too few.
                if to == from && err == InflateError::Bits {
                    to = from + self.br.remaining_bits();
                }
                if to > from {
                    parts.push(Part::plain(PartKind::Unreadable, from, to, 0));
                }
                (None, Some(err))
            }
        };
        Some(Explained {
            step,
            error,
            parts,
            block_start: self.block_start,
        })
    }

    /// The code tables of the block being decoded: `None` for a stored block
    /// and between blocks.
    pub fn tables(&self) -> Option<BlockTables> {
        let Block::Coded {
            kind,
            lit,
            dist,
            header,
        } = &self.block
        else {
            return None;
        };
        Some(BlockTables {
            kind: *kind,
            header: *header,
            lit_len: lit.groups(),
            distance: dist.groups(),
        })
    }

    /// Decodes one step. `None` once the final block has ended; after an
    /// error, the error is returned once and then `None`.
    pub fn step(&mut self) -> Option<Result<Step, InflateError>> {
        if matches!(self.block, Block::Done) {
            return None;
        }
        let bit_start = self.br.bit_pos();
        let out_start = self.out.len() as u64;
        if matches!(self.block, Block::Header) {
            self.block_start = bit_start;
        }
        let block_start = self.block_start;

        match self.advance() {
            Ok(event) => {
                let step = Step {
                    event,
                    index: self.index,
                    block_start,
                    bit_start,
                    bit_end: self.br.bit_pos(),
                    out_start,
                };
                self.index += 1;
                Some(Ok(step))
            }
            Err(err) => {
                self.block = Block::Done;
                Some(Err(err))
            }
        }
    }

    fn advance(&mut self) -> Result<InflateEvent, InflateError> {
        match &self.block {
            Block::Header => {
                let (kind, bit_pos) = self.open_block()?;
                Ok(InflateEvent::BlockStart { kind, bit_pos })
            }
            Block::Stored { remaining } => {
                let remaining = *remaining;
                if remaining == 0 {
                    return Ok(self.close_block());
                }
                if self.out.len() as u64 + 1 > self.max_output {
                    return Err(InflateError::OutputTooLarge);
                }
                let &[byte] = self.br.bytes(1).map_err(bit_err)? else {
                    return Err(InflateError::Bits);
                };
                let end = self.br.bit_pos();
                note(&mut self.record, || {
                    Part::plain(PartKind::StoredByte, end - 8, end, byte as u32)
                });
                self.out.push(byte);
                self.block = Block::Stored {
                    remaining: remaining - 1,
                };
                Ok(InflateEvent::Literal { byte })
            }
            Block::Coded { .. } => self.coded_symbol(),
            Block::Done => Err(InflateError::Bits),
        }
    }

    /// Reads a block header and prepares the block's state. Returns its kind
    /// and the bit position just past BFINAL and BTYPE.
    fn open_block(&mut self) -> Result<(BlockKind, u64), InflateError> {
        let start = self.br.bit_pos();
        self.is_final = self.br.bits(1).map_err(bit_err)? == 1;
        let is_final = self.is_final as u32;
        note(&mut self.record, || {
            Part::plain(PartKind::Final, start, start + 1, is_final)
        });
        let btype = self.br.bits(2).map_err(bit_err)?;
        note(&mut self.record, || {
            Part::plain(PartKind::BlockType, start + 1, start + 3, btype)
        });
        let kind = match btype {
            0 => BlockKind::Stored,
            1 => BlockKind::Fixed,
            2 => BlockKind::Dynamic,
            _ => return Err(InflateError::BadSymbol),
        };
        let after_type = self.br.bit_pos();

        self.block = match kind {
            BlockKind::Stored => {
                let &[l0, l1, n0, n1] = self.br.bytes(4).map_err(bit_err)? else {
                    return Err(InflateError::Bits);
                };
                let len = u16::from_le_bytes([l0, l1]);
                let nlen = u16::from_le_bytes([n0, n1]);
                let end = self.br.bit_pos();
                let fields = end - 32;
                if fields > after_type {
                    note(&mut self.record, || {
                        Part::plain(PartKind::Padding, after_type, fields, 0)
                    });
                }
                note(&mut self.record, || {
                    Part::plain(PartKind::StoredLen, fields, fields + 16, len as u32)
                });
                note(&mut self.record, || {
                    Part::plain(PartKind::StoredNLen, fields + 16, end, nlen as u32)
                });
                if nlen != !len {
                    return Err(InflateError::BadSymbol);
                }
                Block::Stored {
                    remaining: len as usize,
                }
            }
            BlockKind::Fixed => Block::Coded {
                kind,
                lit: Huffman::from_lengths(&fixed_literal_lengths())?,
                dist: Huffman::from_lengths(&fixed_distance_lengths())?,
                header: None,
            },
            BlockKind::Dynamic => {
                let (lit, dist, header) = read_dynamic_tables(&mut self.br, &mut self.record)?;
                Block::Coded {
                    kind,
                    lit,
                    dist,
                    header: Some(header),
                }
            }
        };
        Ok((kind, after_type))
    }

    fn close_block(&mut self) -> InflateEvent {
        self.block = if self.is_final {
            Block::Done
        } else {
            Block::Header
        };
        InflateEvent::BlockEnd
    }

    fn coded_symbol(&mut self) -> Result<InflateEvent, InflateError> {
        let Block::Coded { lit, dist, .. } = &self.block else {
            return Err(InflateError::Bits);
        };
        let s = self.br.bit_pos();
        let (symbol, code, code_len) = lit.decode_traced(&mut self.br)?;
        let e = self.br.bit_pos();
        note(&mut self.record, || {
            Part::code(PartKind::LitLen, s, e, symbol, code, code_len)
        });
        match symbol {
            0..=255 => {
                if self.out.len() as u64 + 1 > self.max_output {
                    return Err(InflateError::OutputTooLarge);
                }
                let byte = symbol as u8;
                self.out.push(byte);
                Ok(InflateEvent::Literal { byte })
            }
            256 => Ok(self.close_block()),
            257..=285 => {
                let idx = symbol as usize - 257;
                let s = self.br.bit_pos();
                let extra = self.br.bits(LENGTH_EXTRA[idx] as u32).map_err(bit_err)?;
                let e = self.br.bit_pos();
                if e > s {
                    note(&mut self.record, || {
                        Part::plain(PartKind::LengthExtra, s, e, extra)
                    });
                }
                let length = LENGTH_BASE[idx] as u32 + extra;

                let s = self.br.bit_pos();
                let (dist_symbol, code, code_len) = dist.decode_traced(&mut self.br)?;
                let e = self.br.bit_pos();
                note(&mut self.record, || {
                    Part::code(PartKind::Distance, s, e, dist_symbol, code, code_len)
                });
                let dist_symbol = dist_symbol as usize;
                if dist_symbol >= DIST_BASE.len() {
                    return Err(InflateError::BadSymbol);
                }
                let s = self.br.bit_pos();
                let extra = self
                    .br
                    .bits(DIST_EXTRA[dist_symbol] as u32)
                    .map_err(bit_err)?;
                let e = self.br.bit_pos();
                if e > s {
                    note(&mut self.record, || {
                        Part::plain(PartKind::DistanceExtra, s, e, extra)
                    });
                }
                let distance = DIST_BASE[dist_symbol] as u32 + extra;

                if distance as usize > self.out.len() {
                    return Err(InflateError::BadDistance);
                }
                if self.out.len() as u64 + length as u64 > self.max_output {
                    return Err(InflateError::OutputTooLarge);
                }

                let start = self.out.len() - distance as usize;
                for i in 0..length as usize {
                    // Byte-by-byte on purpose: overlapping copies are legal and
                    // are how run-length encoding falls out of LZ77.
                    let byte = self.out[start + i];
                    self.out.push(byte);
                }
                Ok(InflateEvent::Match {
                    distance: distance as u16,
                    length: length as u16,
                })
            }
            _ => Err(InflateError::BadSymbol),
        }
    }
}

impl Iterator for Decoder<'_> {
    type Item = Result<Step, InflateError>;

    fn next(&mut self) -> Option<Self::Item> {
        self.step()
    }
}

/// Decompresses a raw DEFLATE stream, reporting every step to `sink`.
/// `max_output` caps the result so a crafted file cannot exhaust memory.
pub fn inflate(
    data: &[u8],
    max_output: u64,
    sink: &mut dyn EventSink,
) -> Result<Vec<u8>, InflateError> {
    let mut decoder = Decoder::new(data, max_output);
    while let Some(step) = decoder.step() {
        sink.emit(&step?);
    }
    Ok(decoder.into_output())
}

fn read_dynamic_tables(
    br: &mut BitReader,
    record: &mut Option<Vec<Part>>,
) -> Result<(Huffman, Huffman, DynamicHeader), InflateError> {
    let start = br.bit_pos();
    let hlit = br.bits(5).map_err(bit_err)? as usize + 257;
    note(record, || {
        Part::plain(PartKind::HLit, start, start + 5, hlit as u32)
    });
    let hdist = br.bits(5).map_err(bit_err)? as usize + 1;
    note(record, || {
        Part::plain(PartKind::HDist, start + 5, start + 10, hdist as u32)
    });
    let hclen = br.bits(4).map_err(bit_err)? as usize + 4;
    note(record, || {
        Part::plain(PartKind::HClen, start + 10, start + 14, hclen as u32)
    });

    let mut code_lengths = [0u8; 19];
    for &slot in CODE_LENGTH_ORDER.iter().take(hclen) {
        code_lengths[slot] = br.bits(3).map_err(bit_err)? as u8;
    }
    let lengths_start = br.bit_pos();
    note(record, || {
        Part::plain(
            PartKind::CodeLengthCode,
            start + 14,
            lengths_start,
            hclen as u32,
        )
    });
    let code_huff = Huffman::from_lengths(&code_lengths)?;

    let total = hlit + hdist;
    let mut lengths = vec![0u8; total];
    let mut i = 0;
    while i < total {
        let symbol = code_huff.decode(br)?;
        match symbol {
            0..=15 => {
                lengths[i] = symbol as u8;
                i += 1;
            }
            16 => {
                if i == 0 {
                    return Err(InflateError::BadCodeLengths);
                }
                let prev = lengths[i - 1];
                let repeat = 3 + br.bits(2).map_err(bit_err)? as usize;
                if i + repeat > total {
                    return Err(InflateError::BadCodeLengths);
                }
                lengths[i..i + repeat].fill(prev);
                i += repeat;
            }
            17 => {
                let repeat = 3 + br.bits(3).map_err(bit_err)? as usize;
                if i + repeat > total {
                    return Err(InflateError::BadCodeLengths);
                }
                i += repeat;
            }
            18 => {
                let repeat = 11 + br.bits(7).map_err(bit_err)? as usize;
                if i + repeat > total {
                    return Err(InflateError::BadCodeLengths);
                }
                i += repeat;
            }
            _ => return Err(InflateError::BadSymbol),
        }
    }

    let end = br.bit_pos();
    note(record, || {
        Part::plain(PartKind::CodeLengths, lengths_start, end, total as u32)
    });

    let lit = Huffman::from_lengths(&lengths[..hlit])?;
    let dist = Huffman::from_lengths(&lengths[hlit..])?;
    let header = DynamicHeader {
        hlit: hlit as u16,
        hdist: hdist as u8,
        hclen: hclen as u8,
    };
    Ok((lit, dist, header))
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::Compression;
    use flate2::write::DeflateEncoder;
    use std::io::Write;

    fn deflate(input: &[u8], level: Compression) -> Vec<u8> {
        let mut enc = DeflateEncoder::new(Vec::new(), level);
        enc.write_all(input).unwrap();
        enc.finish().unwrap()
    }

    use crate::inflate::explain::{Explained, PartKind};

    /// Bit fields packed in DEFLATE order. Header fields are numbers, read
    /// least significant bit first; Huffman codes go most significant first.
    fn pack_fields(fields: &[(u32, u8, bool)]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut n = 0usize;
        for &(value, len, msb_first) in fields {
            for i in 0..len {
                let bit = if msb_first {
                    (value >> (len - 1 - i)) & 1
                } else {
                    (value >> i) & 1
                };
                if n.is_multiple_of(8) {
                    out.push(0);
                }
                *out.last_mut().unwrap() |= (bit as u8) << (n % 8);
                n += 1;
            }
        }
        out
    }

    type Pair = (Option<Result<Step, InflateError>>, Explained);

    /// Walks a plain and an explaining decoder side by side.
    fn explain_all(data: &[u8]) -> Vec<Pair> {
        let mut plain = Decoder::new(data, u64::MAX);
        let mut explaining = Decoder::new(data, u64::MAX);
        let mut out = Vec::new();
        while let Some(e) = explaining.explain_next() {
            out.push((plain.step(), e));
        }
        assert!(plain.step().is_none(), "both decoders end together");
        out
    }

    fn assert_partitions(e: &Explained, start: u64, end: u64) {
        let mut at = start;
        for p in &e.parts {
            assert_eq!(p.bit_start, at, "parts are contiguous: {e:?}");
            assert!(p.bit_end > p.bit_start, "no empty parts: {e:?}");
            at = p.bit_end;
        }
        let expected = if e.parts.is_empty() { start } else { end };
        assert_eq!(at, expected, "{e:?}");
    }

    fn sample_input() -> Vec<u8> {
        let mut input = b"hexscope hexscope hexscope! ".repeat(40);
        input.extend((0..3000u32).map(|i| (i * 7 % 251) as u8));
        input
    }

    fn roundtrip(input: &[u8], level: Compression) {
        let compressed = deflate(input, level);
        let out = inflate(&compressed, u64::MAX, &mut NoTrace).expect("inflate succeeds");
        assert_eq!(out, input, "roundtrip mismatch at level {level:?}");
    }

    #[test]
    fn inflates_stored_blocks() {
        roundtrip(b"hexscope stored block", Compression::none());
    }

    #[test]
    fn inflates_fixed_blocks() {
        roundtrip(b"aaaaaaaaaaaaaaaaaaaaaaaabbbbcccc", Compression::fast());
    }

    #[test]
    fn inflates_dynamic_blocks() {
        let mut input = Vec::new();
        for i in 0..8000u32 {
            input.extend_from_slice(format!("line {} of the corpus\n", i % 97).as_bytes());
        }
        roundtrip(&input, Compression::best());
    }

    #[test]
    fn inflates_empty_input() {
        roundtrip(b"", Compression::fast());
    }

    #[test]
    fn emits_events_that_reconstruct_the_output() {
        // Highly repetitive input guarantees at least one back-reference,
        // without depending on exactly how flate2 chooses to split matches.
        let compressed = deflate(b"abcabcabcabcabcabcabcabc", Compression::best());
        let mut trace = CollectEvents::default();
        let out = inflate(&compressed, u64::MAX, &mut trace).unwrap();

        assert_eq!(out, b"abcabcabcabcabcabcabcabc");
        assert!(matches!(
            trace.events.first(),
            Some(InflateEvent::BlockStart { .. })
        ));
        assert_eq!(trace.events.last(), Some(&InflateEvent::BlockEnd));

        let match_count = trace
            .events
            .iter()
            .filter(|e| matches!(e, InflateEvent::Match { .. }))
            .count();
        assert!(
            match_count >= 1,
            "repetitive input must produce a back-reference"
        );

        // Replaying the events by hand must rebuild the exact output — this is
        // the property the animation depends on.
        let mut replayed: Vec<u8> = Vec::new();
        for event in &trace.events {
            match *event {
                InflateEvent::Literal { byte } => replayed.push(byte),
                InflateEvent::Match { distance, length } => {
                    let start = replayed.len() - distance as usize;
                    for i in 0..length as usize {
                        let byte = replayed[start + i];
                        replayed.push(byte);
                    }
                }
                _ => {}
            }
        }
        assert_eq!(replayed, out);
    }

    #[test]
    fn returns_an_error_for_a_truncated_stream() {
        let compressed = deflate(
            b"some reasonably compressible content here",
            Compression::best(),
        );
        // Cut the stream in half: the decoder must give up cleanly.
        let result = inflate(&compressed[..compressed.len() / 2], u64::MAX, &mut NoTrace);
        assert!(result.is_err(), "expected an error, got {result:?}");
    }

    #[test]
    fn returns_an_error_for_a_reserved_block_type() {
        // Block type 3 is reserved and must be rejected, not guessed at.
        // Bits: BFINAL=1, BTYPE=11 -> 0b111 in the low three bits.
        let result = inflate(&[0b0000_0111], u64::MAX, &mut NoTrace);
        assert_eq!(result, Err(InflateError::BadSymbol));
    }

    #[test]
    fn stops_at_the_output_limit() {
        let input = vec![b'x'; 100_000];
        let compressed = deflate(&input, Compression::best());
        assert_eq!(
            inflate(&compressed, 1000, &mut NoTrace),
            Err(InflateError::OutputTooLarge)
        );
    }

    #[test]
    fn never_panics_on_random_bytes() {
        for seed in 0..500u32 {
            let bytes: Vec<u8> = (0..64u32)
                .map(|i| (seed.wrapping_mul(2654435761).wrapping_add(i * 40503) >> 13) as u8)
                .collect();
            let _ = inflate(&bytes, 1 << 20, &mut NoTrace);
        }
    }

    /// Every step of a full decode, plus the output.
    fn all_steps(compressed: &[u8]) -> (Vec<Step>, Vec<u8>) {
        let mut d = Decoder::new(compressed, u64::MAX);
        let mut steps = Vec::new();
        while let Some(step) = d.step() {
            steps.push(step.expect("valid stream"));
        }
        (steps, d.into_output())
    }

    /// Inputs that exercise stored, fixed and dynamic blocks, and streams of
    /// several blocks (flate2 starts a new block every few tens of KB).
    fn corpus() -> Vec<(Vec<u8>, Compression)> {
        let mut text = Vec::new();
        for i in 0..20_000u32 {
            text.extend_from_slice(format!("row {} value {}\n", i, i % 31).as_bytes());
        }
        let noise: Vec<u8> = (0..70_000u32)
            .map(|i| (i.wrapping_mul(2654435761) >> 13) as u8)
            .collect();
        vec![
            (text.clone(), Compression::best()),
            (text[..3000].to_vec(), Compression::fast()),
            (noise.clone(), Compression::none()),
            (noise, Compression::best()),
        ]
    }

    #[test]
    fn steps_partition_both_streams() {
        // The hex view's reading head depends on this: every compressed bit
        // belongs to exactly one step, and every output byte too.
        for (input, level) in corpus() {
            let compressed = deflate(&input, level);
            let (steps, out) = all_steps(&compressed);
            assert_eq!(out, input);
            assert_eq!(steps[0].bit_start, 0);
            for pair in steps.windows(2) {
                assert_eq!(
                    pair[0].bit_end, pair[1].bit_start,
                    "bit gap at step {}",
                    pair[1].index
                );
                let produced = match pair[0].event {
                    InflateEvent::Literal { .. } => 1,
                    InflateEvent::Match { length, .. } => length as u64,
                    _ => 0,
                };
                assert_eq!(pair[0].out_start + produced, pair[1].out_start);
                assert_eq!(pair[0].index + 1, pair[1].index);
            }
        }
    }

    #[test]
    fn resuming_from_any_step_reproduces_the_same_steps() {
        for (input, level) in corpus() {
            let compressed = deflate(&input, level);
            let (steps, out) = all_steps(&compressed);
            let stride = (steps.len() / 60).max(1);
            let mut checked = 0;

            for k in (0..steps.len()).step_by(stride).chain([steps.len() - 1]) {
                let cp = steps[k].checkpoint();
                let mut resumed = Decoder::resume(&compressed, u64::MAX, &out, &cp)
                    .unwrap_or_else(|e| panic!("resume at step {k} failed: {e:?}"));
                for expected in &steps[k..(k + 40).min(steps.len())] {
                    let got = resumed.step().expect("a step").expect("no error");
                    assert_eq!(
                        &got, expected,
                        "diverged resuming at step {k}, level {level:?}"
                    );
                }
                checked += 1;
            }
            assert!(checked > 30, "only {checked} resume points checked");
        }
    }

    #[test]
    fn a_resumed_decoder_finishes_with_the_same_output() {
        let (input, level) = corpus().swap_remove(0);
        let compressed = deflate(&input, level);
        let (steps, out) = all_steps(&compressed);
        let cp = steps[steps.len() / 2].checkpoint();

        let mut d = Decoder::resume(&compressed, u64::MAX, &out, &cp).unwrap();
        while let Some(step) = d.step() {
            step.unwrap();
        }
        assert_eq!(d.into_output(), out);
    }

    #[test]
    fn a_checkpoint_the_caller_cannot_back_is_rejected() {
        let compressed = deflate(b"abcabcabcabcabcabcabcabc", Compression::best());
        let (steps, out) = all_steps(&compressed);
        let cp = steps[steps.len() - 1].checkpoint();
        // Too little output supplied to back the checkpoint.
        let short = &out[..(cp.out_pos as usize).saturating_sub(1)];
        assert!(matches!(
            Decoder::resume(&compressed, u64::MAX, short, &cp),
            Err(InflateError::BadCheckpoint)
        ));
    }

    #[derive(Default)]
    struct CollectEvents {
        events: Vec<InflateEvent>,
    }

    impl EventSink for CollectEvents {
        fn emit(&mut self, step: &Step) {
            self.events.push(step.event);
        }
    }

    #[test]
    fn explained_parts_partition_every_step() {
        let input = sample_input();
        let mut seen = std::collections::HashSet::new();
        for level in [
            Compression::none(),
            Compression::fast(),
            Compression::best(),
        ] {
            for (plain, e) in explain_all(&deflate(&input, level)) {
                let step = plain.unwrap().unwrap();
                assert_eq!(e.step, Some(step), "explaining changes nothing");
                assert_eq!(e.error, None);
                assert_eq!(e.block_start, step.block_start);
                assert_partitions(&e, step.bit_start, step.bit_end);
                seen.extend(e.parts.iter().map(|p| p.kind));
            }
        }
        for kind in [
            PartKind::Final,
            PartKind::BlockType,
            PartKind::StoredLen,
            PartKind::StoredNLen,
            PartKind::StoredByte,
            PartKind::HLit,
            PartKind::HDist,
            PartKind::HClen,
            PartKind::CodeLengthCode,
            PartKind::CodeLengths,
            PartKind::LitLen,
            PartKind::LengthExtra,
            PartKind::Distance,
            PartKind::DistanceExtra,
        ] {
            assert!(seen.contains(&kind), "{kind:?} never appeared");
        }
    }

    #[test]
    fn explained_codes_sit_in_their_tables() {
        let data = deflate(&sample_input(), Compression::best());
        let mut d = Decoder::new(&data, u64::MAX);
        let mut checked = 0;
        loop {
            // Tables before the step: a block's end code is read with them,
            // and afterwards the block is gone.
            let before = d.tables();
            let Some(e) = d.explain_next() else { break };
            let Some(tables) = before.or_else(|| d.tables()) else {
                continue;
            };
            for p in &e.parts {
                let groups = match p.kind {
                    PartKind::LitLen => &tables.lit_len,
                    PartKind::Distance => &tables.distance,
                    _ => continue,
                };
                let g = groups
                    .iter()
                    .find(|g| g.len == p.code_len)
                    .expect("a group of that length");
                let k = (p.code - g.first_code) as usize;
                assert_eq!(g.symbols[k], p.value as u16, "{p:?}");
                checked += 1;
            }
        }
        assert!(checked > 100, "only {checked} codes checked");
    }

    #[test]
    fn tables_describe_the_block_being_decoded() {
        // A fixed block holding only its end code (256 = 0000000).
        let fixed = pack_fields(&[(1, 1, false), (1, 2, false), (0, 7, true)]);
        let mut d = Decoder::new(&fixed, u64::MAX);
        assert_eq!(d.tables(), None, "no block before its header");
        d.explain_next();
        let t = d.tables().expect("fixed tables");
        assert_eq!(t.kind, BlockKind::Fixed);
        assert_eq!(t.header, None);
        let lens: Vec<_> = t.lit_len.iter().map(|g| g.len).collect();
        assert_eq!(lens, [7, 8, 9]);
        assert_eq!(d.next_index(), 1);

        let dynamic = deflate(&sample_input(), Compression::best());
        let mut d = Decoder::new(&dynamic, u64::MAX);
        d.explain_next();
        let t = d.tables().expect("dynamic tables");
        assert_eq!(t.kind, BlockKind::Dynamic);
        let h = t.header.expect("dynamic header");
        let lit_symbols: usize = t.lit_len.iter().map(|g| g.symbols.len()).sum();
        assert!(lit_symbols <= h.hlit as usize && h.hlit >= 257);

        let stored = deflate(b"abc", Compression::none());
        let mut d = Decoder::new(&stored, u64::MAX);
        d.explain_next();
        assert_eq!(d.tables(), None, "stored blocks have no codes");
    }

    #[test]
    fn a_truncated_stream_explains_where_it_ran_out() {
        let data = deflate(&sample_input(), Compression::best());
        let cut = &data[..data.len() / 2];
        let steps = explain_all(cut);
        let (_, last) = steps.last().unwrap();
        assert_eq!(last.error, Some(InflateError::Bits));
        let p = last.parts.last().expect("an unreadable part");
        assert_eq!(p.kind, PartKind::Unreadable);
        assert_eq!(
            p.bit_end,
            cut.len() as u64 * 8,
            "blames the bits that were left"
        );
    }

    #[test]
    fn an_undefined_symbol_is_named_by_its_code() {
        // Fixed block, then code 286 (11000110): valid bits, undefined symbol.
        let data = pack_fields(&[(1, 1, false), (1, 2, false), (0b1100_0110, 8, true)]);
        let steps = explain_all(&data);
        let (_, e) = &steps[1];
        assert_eq!(e.error, Some(InflateError::BadSymbol));
        let kinds: Vec<_> = e.parts.iter().map(|p| (p.kind, p.value)).collect();
        assert_eq!(
            kinds,
            [(PartKind::LitLen, 286)],
            "every bit read was understood"
        );
    }
}
