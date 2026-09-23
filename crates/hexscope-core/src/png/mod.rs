pub mod chunks;
pub mod fields;
pub mod unfilter;

use crate::inflate::trace::{CheckpointSink, TraceSummary};
use crate::inflate::zlib::zlib_decompress;
use crate::inflate::{Decoder, InflateError};
use crate::model::{ByteRange, NodeId, NodeKind, ParseTree, Value};
use crate::png::chunks::{ChunkError, PNG_SIGNATURE, next_chunk};
use crate::png::fields::{
    Ihdr, decode_gama, decode_ihdr, decode_phys, decode_plte, decode_text, decode_trns,
};
use crate::png::unfilter::unfilter;
use crate::reader::Reader;

/// Caps decompressed IDAT output at 512 MB so a compression bomb cannot
/// exhaust memory.
pub const MAX_PIXEL_BYTES: u64 = 512 * 1024 * 1024;

/// One checkpoint per 512 events keeps the list small while staying fine
/// enough for a scrubber.
const CHECKPOINT_INTERVAL: u64 = 512;

#[derive(Debug)]
pub struct PngDocument {
    pub tree: ParseTree,
    pub ihdr: Option<Ihdr>,
    pub pixels: Option<Vec<u8>>,
    pub trace: Option<TraceSummary>,
    /// File ranges of every IDAT payload, in order. Concatenated, they form
    /// the zlib stream — this is how a position in that stream maps back to
    /// bytes in the file.
    pub idat: Vec<ByteRange>,
    /// The decompressed zlib stream: filtered scanlines, before unfiltering.
    pub inflated: Option<Vec<u8>>,
}

/// Parses a PNG. Never fails: damage is recorded as `Warning`/`Error` nodes.
pub fn parse_png(data: &[u8]) -> PngDocument {
    let mut tree = ParseTree::new();
    let root = tree.add(
        None,
        "PNG",
        ByteRange::new(0, data.len() as u64),
        NodeKind::Container,
        None,
    );

    let mut r = Reader::new(data);

    match r.bytes(8) {
        Ok(sig) if sig == PNG_SIGNATURE => {
            tree.add(
                Some(root),
                "signature",
                ByteRange::new(0, 8),
                NodeKind::Field,
                Some(Value::Bytes(8)),
            );
        }
        _ => {
            tree.add(
                Some(root),
                "not a PNG signature",
                ByteRange::new(0, data.len().min(8) as u64),
                NodeKind::Error,
                None,
            );
            return PngDocument {
                tree,
                ihdr: None,
                pixels: None,
                trace: None,
                idat: Vec::new(),
                inflated: None,
            };
        }
    }

    let mut ihdr: Option<Ihdr> = None;
    let mut idat: Vec<u8> = Vec::new();
    let mut idat_ranges: Vec<ByteRange> = Vec::new();
    let mut idat_nodes: Vec<NodeId> = Vec::new();
    let mut saw_iend = false;

    loop {
        // Captured before the call: `next_chunk` seeks to end-of-input on any
        // error so the walk self-terminates, which means the reader's position
        // afterwards is always EOF and says nothing about where the damage is.
        // The whole point of this tool is pointing at the broken bytes.
        let chunk_start = r.pos();
        let Some(result) = next_chunk(&mut r) else {
            break;
        };

        let chunk = match result {
            Ok(c) => c,
            Err(err) => {
                let label = match err {
                    ChunkError::Truncated => "truncated chunk",
                    ChunkError::LengthTooLarge => "chunk length out of range",
                };
                tree.add(
                    Some(root),
                    label,
                    ByteRange::new(chunk_start, data.len() as u64 - chunk_start),
                    NodeKind::Error,
                    None,
                );
                break;
            }
        };

        let node = tree.add(
            Some(root),
            chunk.kind_str(),
            chunk.range,
            NodeKind::Container,
            Some(Value::Bytes(chunk.data.len() as u64)),
        );

        if !chunk.crc_ok() {
            tree.add(
                Some(node),
                format!(
                    "CRC mismatch: stored {:08X}, computed {:08X}",
                    chunk.declared_crc, chunk.actual_crc
                ),
                ByteRange::new(chunk.range.end() - 4, 4),
                NodeKind::Warning,
                None,
            );
        }

        match &chunk.kind {
            b"IHDR" => ihdr = decode_ihdr(&chunk, &mut tree, node),
            b"PLTE" => decode_plte(&chunk, &mut tree, node),
            b"tEXt" => decode_text(&chunk, &mut tree, node),
            b"pHYs" => decode_phys(&chunk, &mut tree, node),
            b"gAMA" => decode_gama(&chunk, &mut tree, node),
            b"tRNS" => decode_trns(&chunk, &mut tree, node, ihdr.map(|h| h.color_type)),
            b"IDAT" => {
                idat.extend_from_slice(chunk.data);
                idat_ranges.push(chunk.data_range);
                idat_nodes.push(node);
            }
            b"IEND" => saw_iend = true,
            _ => {}
        }
    }

    // A PNG without image data, or without a terminator, parses cleanly chunk
    // by chunk and is still not a usable file. Report it rather than shrug.
    if idat.is_empty() {
        tree.add(
            Some(root),
            "no IDAT chunk: the file carries no image data",
            // Zero-length: something absent has no bytes to point at, and a
            // whole-file range would swallow every hover in the hex view.
            ByteRange::new(0, 0),
            NodeKind::Warning,
            None,
        );
    }
    if !saw_iend {
        tree.add(
            Some(root),
            "no IEND chunk: the file does not end properly",
            ByteRange::new(data.len() as u64, 0),
            NodeKind::Warning,
            None,
        );
    }

    let chunks = IdatChunks {
        nodes: &idat_nodes,
        ranges: &idat_ranges,
    };
    let decoded = decode_pixels(&idat, &chunks, ihdr, &mut tree, root);

    PngDocument {
        tree,
        ihdr,
        pixels: decoded.pixels,
        trace: decoded.trace,
        idat: idat_ranges,
        inflated: decoded.inflated,
    }
}

#[derive(Default)]
struct Decoded {
    pixels: Option<Vec<u8>>,
    trace: Option<TraceSummary>,
    inflated: Option<Vec<u8>>,
}

/// The IDAT chunks, in file order: their tree nodes and payload ranges.
struct IdatChunks<'a> {
    nodes: &'a [NodeId],
    ranges: &'a [ByteRange],
}

impl IdatChunks<'_> {
    /// The chunk holding byte `n` of the reassembled zlib stream, and that
    /// byte's file offset. Past the end, the last byte of the last chunk.
    fn locate(&self, n: u64) -> Option<(NodeId, u64, ByteRange)> {
        let mut before = 0u64;
        for (&node, &range) in self.nodes.iter().zip(self.ranges) {
            if n < before + range.len {
                return Some((node, range.start + (n - before), range));
            }
            before += range.len;
        }
        let (&node, &range) = self.nodes.last().zip(self.ranges.last())?;
        Some((node, range.end().saturating_sub(1).max(range.start), range))
    }
}

/// Where in the zlib stream decompression went wrong, as a stream byte.
fn failure_byte(stream: &[u8], err: InflateError) -> u64 {
    match err {
        InflateError::BadZlibHeader => 0,
        // The DEFLATE data decoded; only the Adler-32 trailer disagrees.
        InflateError::ChecksumMismatch => stream.len().saturating_sub(4) as u64,
        _ => {
            // Replay to find the first bit of the step that could not decode:
            // the damage lies there or just after.
            let Some(body) = stream.get(2..stream.len().saturating_sub(4)) else {
                return 0;
            };
            let mut decoder = Decoder::new(body, MAX_PIXEL_BYTES);
            let mut last_good = 0;
            while let Some(Ok(step)) = decoder.step() {
                last_good = step.bit_end;
            }
            2 + last_good / 8
        }
    }
}

fn decode_pixels(
    idat: &[u8],
    chunks: &IdatChunks,
    ihdr: Option<Ihdr>,
    tree: &mut ParseTree,
    root: crate::model::NodeId,
) -> Decoded {
    let Some(ihdr) = ihdr else {
        return Decoded::default();
    };
    if idat.is_empty() {
        return Decoded::default();
    }

    let mut sink = CheckpointSink::new(CHECKPOINT_INTERVAL);
    let raw = match zlib_decompress(idat, MAX_PIXEL_BYTES, &mut sink) {
        Ok(raw) => raw,
        Err(err) => {
            // Point at the byte where decoding broke, inside the chunk that
            // holds it: from there to the end of that chunk's data is what
            // could not be read. Nested under the chunk, it never overlaps a
            // sibling in the tree.
            let at = failure_byte(idat, err);
            let (parent, range) = match chunks.locate(at) {
                Some((node, offset, chunk)) => {
                    let len = if err == InflateError::ChecksumMismatch {
                        4.min(chunk.end() - offset)
                    } else {
                        chunk.end() - offset
                    };
                    (node, ByteRange::new(offset, len))
                }
                None => (root, ByteRange::new(0, 0)),
            };
            tree.add(
                Some(parent),
                format!("IDAT decompression failed: {err:?}"),
                range,
                NodeKind::Error,
                None,
            );
            return Decoded {
                trace: Some(sink.finish()),
                ..Decoded::default()
            };
        }
    };
    let trace = Some(sink.finish());

    // Interlaced images use a seven-pass layout; v1 decompresses them, so the
    // DEFLATE trace still works, but does not reassemble the pixels.
    if ihdr.interlace != 0 {
        tree.add(
            Some(root),
            "interlaced image: pixel preview unavailable",
            ByteRange::new(0, 0),
            NodeKind::Warning,
            None,
        );
        return Decoded {
            trace,
            inflated: Some(raw),
            ..Decoded::default()
        };
    }

    let Some(stride) = ihdr.stride() else {
        tree.add(
            Some(root),
            "image dimensions are too large to represent",
            ByteRange::new(0, 0),
            NodeKind::Error,
            None,
        );
        return Decoded {
            trace,
            inflated: Some(raw),
            ..Decoded::default()
        };
    };

    match unfilter(&raw, stride, ihdr.height, ihdr.filter_distance()) {
        Ok(pixels) => Decoded {
            pixels: Some(pixels),
            trace,
            inflated: Some(raw),
        },
        Err(err) => {
            // Unfiltering works on decompressed scanlines, which have no
            // position in the file, so there are no bytes to point at.
            tree.add(
                Some(root),
                format!("unfiltering failed: {err:?}"),
                ByteRange::new(0, 0),
                NodeKind::Error,
                None,
            );
            Decoded {
                trace,
                inflated: Some(raw),
                ..Decoded::default()
            }
        }
    }
}
