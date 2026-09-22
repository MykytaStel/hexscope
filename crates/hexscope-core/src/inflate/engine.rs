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

/// One decoding step, as the UI wants to animate it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InflateEvent {
    BlockStart { kind: BlockKind, bit_pos: u64 },
    Literal { byte: u8 },
    Match { distance: u16, length: u16 },
    BlockEnd,
}

/// Receives events as decoding proceeds. Keeping this a sink rather than a
/// returned `Vec` is what lets a caller decode a 200 MB stream without holding
/// millions of events in memory.
pub trait EventSink {
    fn emit(&mut self, event: InflateEvent);
}

/// Sink that discards everything — for plain decoding.
pub struct NoTrace;

impl EventSink for NoTrace {
    fn emit(&mut self, _event: InflateEvent) {}
}

fn bit_err(_: BitError) -> InflateError {
    InflateError::Bits
}

/// Decompresses a raw DEFLATE stream. `max_output` caps the result so a crafted
/// file cannot exhaust memory.
pub fn inflate(
    data: &[u8],
    max_output: u64,
    sink: &mut dyn EventSink,
) -> Result<Vec<u8>, InflateError> {
    let mut br = BitReader::new(data);
    let mut out: Vec<u8> = Vec::new();

    loop {
        let is_final = br.bits(1).map_err(bit_err)? == 1;
        let kind = match br.bits(2).map_err(bit_err)? {
            0 => BlockKind::Stored,
            1 => BlockKind::Fixed,
            2 => BlockKind::Dynamic,
            _ => return Err(InflateError::BadSymbol),
        };
        sink.emit(InflateEvent::BlockStart {
            kind,
            bit_pos: br.bit_pos(),
        });

        match kind {
            BlockKind::Stored => {
                let header = br.bytes(4).map_err(bit_err)?;
                let len = u16::from_le_bytes([header[0], header[1]]) as usize;
                let nlen = u16::from_le_bytes([header[2], header[3]]);
                if nlen != !(len as u16) {
                    return Err(InflateError::BadSymbol);
                }
                if out.len() as u64 + len as u64 > max_output {
                    return Err(InflateError::OutputTooLarge);
                }
                let payload = br.bytes(len).map_err(bit_err)?;
                for &byte in payload {
                    sink.emit(InflateEvent::Literal { byte });
                }
                out.extend_from_slice(payload);
            }
            BlockKind::Fixed => {
                let lit = Huffman::from_lengths(&fixed_literal_lengths())?;
                let dist = Huffman::from_lengths(&fixed_distance_lengths())?;
                decode_block(&mut br, &lit, &dist, &mut out, max_output, sink)?;
            }
            BlockKind::Dynamic => {
                let (lit, dist) = read_dynamic_tables(&mut br)?;
                decode_block(&mut br, &lit, &dist, &mut out, max_output, sink)?;
            }
        }

        sink.emit(InflateEvent::BlockEnd);
        if is_final {
            break;
        }
    }

    Ok(out)
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

fn decode_block(
    br: &mut BitReader,
    lit: &Huffman,
    dist: &Huffman,
    out: &mut Vec<u8>,
    max_output: u64,
    sink: &mut dyn EventSink,
) -> Result<(), InflateError> {
    loop {
        let symbol = lit.decode(br)?;
        match symbol {
            0..=255 => {
                if out.len() as u64 + 1 > max_output {
                    return Err(InflateError::OutputTooLarge);
                }
                let byte = symbol as u8;
                out.push(byte);
                sink.emit(InflateEvent::Literal { byte });
            }
            256 => return Ok(()),
            257..=285 => {
                let idx = symbol as usize - 257;
                let length =
                    LENGTH_BASE[idx] as u32 + br.bits(LENGTH_EXTRA[idx] as u32).map_err(bit_err)?;

                let dist_symbol = dist.decode(br)? as usize;
                if dist_symbol >= DIST_BASE.len() {
                    return Err(InflateError::BadSymbol);
                }
                let distance = DIST_BASE[dist_symbol] as u32
                    + br.bits(DIST_EXTRA[dist_symbol] as u32).map_err(bit_err)?;

                if distance as usize > out.len() {
                    return Err(InflateError::BadDistance);
                }
                if out.len() as u64 + length as u64 > max_output {
                    return Err(InflateError::OutputTooLarge);
                }

                let start = out.len() - distance as usize;
                for i in 0..length as usize {
                    // Byte-by-byte on purpose: overlapping copies are legal and
                    // are how run-length encoding falls out of LZ77.
                    let byte = out[start + i];
                    out.push(byte);
                }
                sink.emit(InflateEvent::Match {
                    distance: distance as u16,
                    length: length as u16,
                });
            }
            _ => return Err(InflateError::BadSymbol),
        }
    }
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

    #[derive(Default)]
    struct CollectEvents {
        events: Vec<InflateEvent>,
    }

    impl EventSink for CollectEvents {
        fn emit(&mut self, event: InflateEvent) {
            self.events.push(event);
        }
    }
}
