use crate::bits::{BitError, BitReader};
use crate::inflate::InflateError;
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

enum Block {
    /// The next thing to read is a block header.
    Header,
    Stored {
        remaining: usize,
    },
    Coded {
        lit: Huffman,
        dist: Huffman,
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

    /// Output decoded so far.
    pub fn output(&self) -> &[u8] {
        &self.out
    }

    pub fn into_output(self) -> Vec<u8> {
        self.out
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
        self.is_final = self.br.bits(1).map_err(bit_err)? == 1;
        let kind = match self.br.bits(2).map_err(bit_err)? {
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
                if u16::from_le_bytes([n0, n1]) != !len {
                    return Err(InflateError::BadSymbol);
                }
                Block::Stored {
                    remaining: len as usize,
                }
            }
            BlockKind::Fixed => Block::Coded {
                lit: Huffman::from_lengths(&fixed_literal_lengths())?,
                dist: Huffman::from_lengths(&fixed_distance_lengths())?,
            },
            BlockKind::Dynamic => {
                let (lit, dist) = read_dynamic_tables(&mut self.br)?;
                Block::Coded { lit, dist }
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
        let Block::Coded { lit, dist } = &self.block else {
            return Err(InflateError::Bits);
        };
        let symbol = lit.decode(&mut self.br)?;
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
                let length = LENGTH_BASE[idx] as u32
                    + self.br.bits(LENGTH_EXTRA[idx] as u32).map_err(bit_err)?;

                let dist_symbol = dist.decode(&mut self.br)? as usize;
                if dist_symbol >= DIST_BASE.len() {
                    return Err(InflateError::BadSymbol);
                }
                let distance = DIST_BASE[dist_symbol] as u32
                    + self
                        .br
                        .bits(DIST_EXTRA[dist_symbol] as u32)
                        .map_err(bit_err)?;

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

fn read_dynamic_tables(br: &mut BitReader) -> Result<(Huffman, Huffman), InflateError> {
    let hlit = br.bits(5).map_err(bit_err)? as usize + 257;
    let hdist = br.bits(5).map_err(bit_err)? as usize + 1;
    let hclen = br.bits(4).map_err(bit_err)? as usize + 4;

    let mut code_lengths = [0u8; 19];
    for &slot in CODE_LENGTH_ORDER.iter().take(hclen) {
        code_lengths[slot] = br.bits(3).map_err(bit_err)? as u8;
    }
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

    let lit = Huffman::from_lengths(&lengths[..hlit])?;
    let dist = Huffman::from_lengths(&lengths[hlit..])?;
    Ok((lit, dist))
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
}
