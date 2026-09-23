//! The IDAT stream: decompressing it, pointing at where it breaks, and
//! splitting it into zlib header, DEFLATE blocks and trailer.

use crate::inflate::trace::{CheckpointSink, TraceSummary};
use crate::inflate::zlib::zlib_decompress;
use crate::inflate::{BlockKind, InflateError};
use crate::model::{ByteRange, NodeId, NodeKind, ParseTree, Value};
use crate::png::MAX_PIXEL_BYTES;
use crate::png::fields::Ihdr;
use crate::png::unfilter::unfilter;
use crate::reader::Reader;

/// One checkpoint per 512 events keeps the list small while staying fine
/// enough for a scrubber.
const CHECKPOINT_INTERVAL: u64 = 512;

/// What decompressing the IDAT stream produced; each part may be missing.
#[derive(Default)]
pub(super) struct Decoded {
    pub pixels: Option<Vec<u8>>,
    pub trace: Option<TraceSummary>,
    pub inflated: Option<Vec<u8>>,
}

/// The IDAT chunks, in file order: their tree nodes and payload ranges.
pub(super) struct IdatChunks<'a> {
    pub nodes: &'a [NodeId],
    pub ranges: &'a [ByteRange],
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

    /// Adds a node for stream bytes `[a, b)`, split at chunk boundaries so
    /// each part nests inside the IDAT chunk that holds it.
    fn split(&self, tree: &mut ParseTree, a: u64, b: u64, label: &str, value: Option<Value>) {
        let mut before = 0u64;
        let mut first = true;
        for (&node, &range) in self.nodes.iter().zip(self.ranges) {
            let (start, end) = (before, before + range.len);
            let (s, e) = (a.max(start), b.min(end));
            if s < e {
                let label = if first {
                    label.to_string()
                } else {
                    format!("{label} (continued)")
                };
                let len = e - s;
                tree.add(
                    Some(node),
                    label,
                    ByteRange::new(range.start + (s - start), len),
                    NodeKind::Field,
                    Some(value.clone().unwrap_or(Value::Bytes(len))),
                );
                first = false;
            }
            before = end;
            if before >= b {
                break;
            }
        }
    }
}

/// Where in the zlib stream decompression went wrong, as a stream byte.
/// `last_bit` is where the last successful step ended: the damage lies at the
/// first bit of the step after it, or just past it.
fn failure_byte(stream: &[u8], err: InflateError, last_bit: u64) -> u64 {
    match err {
        InflateError::BadZlibHeader => 0,
        // The DEFLATE data decoded; only the Adler-32 trailer disagrees.
        InflateError::ChecksumMismatch => stream.len().saturating_sub(4) as u64,
        _ => 2 + last_bit / 8,
    }
}

/// Gives the IDAT chunks children: the zlib header, each DEFLATE block and the
/// Adler-32 trailer. After a failure, only what lies before the damage, so no
/// child overlaps the error node that starts there.
fn annotate_idat(
    tree: &mut ParseTree,
    chunks: &IdatChunks,
    stream: &[u8],
    trace: &TraceSummary,
    failed_at: Option<u64>,
) {
    let len = stream.len() as u64;
    // Too short to be zlib, or a bad header: nothing in it can be trusted.
    if len < 6 || failed_at == Some(0) {
        return;
    }
    let body_end = len - 4;
    let limit = failed_at.unwrap_or(len);

    let mut r = Reader::new(stream);
    if let Ok(cmf) = r.u8() {
        let window = 1u64 << ((cmf >> 4) + 8);
        let text = format!("DEFLATE, {} KB window", (window / 1024).max(1));
        chunks.split(tree, 0, 2, "zlib header", Some(Value::Text(text)));
    }

    for (i, block) in trace.blocks.iter().enumerate() {
        // Block boundaries are in bits; nodes are in bytes. Each block runs to
        // the byte where the next begins, so parts never overlap.
        let start = 2 + block.start_bit / 8;
        let end = match (trace.blocks.get(i + 1), block.end_bit) {
            (Some(next), _) => 2 + next.start_bit / 8,
            (None, Some(e)) => 2 + e.div_ceil(8),
            (None, None) => 2 + trace.last_bit / 8,
        }
        .min(body_end)
        .min(limit);
        let kind = match block.kind {
            BlockKind::Stored => "stored",
            BlockKind::Fixed => "fixed Huffman",
            BlockKind::Dynamic => "dynamic Huffman",
        };
        if start < end {
            chunks.split(tree, start, end, &format!("block {} · {kind}", i + 1), None);
        }
    }

    // The trailer, unless decoding broke before it (then it is inside the
    // unreadable region the error node covers).
    if failed_at.is_none_or(|f| f >= body_end) {
        r.seek(body_end);
        if let Ok(stored) = r.u32_be() {
            chunks.split(
                tree,
                body_end,
                len,
                "Adler-32",
                Some(Value::Text(format!("{stored:08X}"))),
            );
        }
    }
}

/// Decompresses the IDAT stream, annotates it and, where the image allows,
/// unfilters it into pixels.
pub(super) fn decode_pixels(
    idat: &[u8],
    chunks: &IdatChunks,
    ihdr: Option<Ihdr>,
    tree: &mut ParseTree,
    root: NodeId,
) -> Decoded {
    let Some(ihdr) = ihdr else {
        return Decoded::default();
    };
    if idat.is_empty() {
        return Decoded::default();
    }

    let mut sink = CheckpointSink::new(CHECKPOINT_INTERVAL);
    let result = zlib_decompress(idat, MAX_PIXEL_BYTES, &mut sink);
    let summary = sink.finish();
    let raw = match result {
        Ok(raw) => {
            annotate_idat(tree, chunks, idat, &summary, None);
            raw
        }
        Err(err) => {
            // Point at the byte where decoding broke, inside the chunk that
            // holds it: from there to the end of that chunk's data is what
            // could not be read. Nested under the chunk, it never overlaps a
            // sibling in the tree.
            let at = failure_byte(idat, err, summary.last_bit);
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
            tree.error(parent, format!("IDAT decompression failed: {err:?}"), range);
            annotate_idat(tree, chunks, idat, &summary, Some(at));
            return Decoded {
                trace: Some(summary),
                ..Decoded::default()
            };
        }
    };
    let pixels = unfiltered(&raw, ihdr, tree, root);
    Decoded {
        pixels,
        trace: Some(summary),
        inflated: Some(raw),
    }
}

/// The image's pixels, from the decompressed scanlines.
fn unfiltered(raw: &[u8], ihdr: Ihdr, tree: &mut ParseTree, root: NodeId) -> Option<Vec<u8>> {
    // Interlaced images use a seven-pass layout that v1 does not reassemble.
    // That is a limit of this tool, not a problem with the file, so it adds
    // no node: the file summary already says the image is interlaced.
    if ihdr.interlace != 0 {
        return None;
    }
    let Some(stride) = ihdr.stride() else {
        tree.error(
            root,
            "image dimensions are too large to represent",
            ByteRange::new(0, 0),
        );
        return None;
    };
    match unfilter(raw, stride, ihdr.height, ihdr.filter_distance()) {
        Ok(pixels) => Some(pixels),
        Err(err) => {
            // Unfiltering works on decompressed scanlines, which have no
            // position in the file, so there are no bytes to point at.
            tree.error(
                root,
                format!("unfiltering failed: {err:?}"),
                ByteRange::new(0, 0),
            );
            None
        }
    }
}
