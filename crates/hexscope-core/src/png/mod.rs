pub mod chunks;
pub mod fields;
pub mod unfilter;

use crate::inflate::trace::{CheckpointSink, TraceSummary};
use crate::inflate::zlib::zlib_decompress;
use crate::model::{ByteRange, NodeKind, ParseTree, Value};
use crate::png::chunks::{ChunkError, PNG_SIGNATURE, next_chunk};
use crate::png::fields::{
    Ihdr, decode_gama, decode_ihdr, decode_phys, decode_plte, decode_text, decode_trns,
};
use crate::png::unfilter::unfilter;
use crate::reader::Reader;

/// Caps decompressed IDAT output at 512 MB so a compression bomb cannot
/// exhaust memory.
const MAX_PIXEL_BYTES: u64 = 512 * 1024 * 1024;

/// One checkpoint per 512 events keeps the list small while staying fine
/// enough for a scrubber.
const CHECKPOINT_INTERVAL: u64 = 512;

#[derive(Debug)]
pub struct PngDocument {
    pub tree: ParseTree,
    pub ihdr: Option<Ihdr>,
    pub pixels: Option<Vec<u8>>,
    pub trace: Option<TraceSummary>,
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
            };
        }
    }

    let mut ihdr: Option<Ihdr> = None;
    let mut idat: Vec<u8> = Vec::new();

    while let Some(result) = next_chunk(&mut r) {
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
                    ByteRange::new(r.pos(), r.remaining() as u64),
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
            b"IDAT" => idat.extend_from_slice(chunk.data),
            _ => {}
        }
    }

    let (pixels, trace) = decode_pixels(&idat, ihdr, &mut tree, root);

    PngDocument {
        tree,
        ihdr,
        pixels,
        trace,
    }
}

fn decode_pixels(
    idat: &[u8],
    ihdr: Option<Ihdr>,
    tree: &mut ParseTree,
    root: crate::model::NodeId,
) -> (Option<Vec<u8>>, Option<TraceSummary>) {
    let Some(ihdr) = ihdr else {
        return (None, None);
    };
    if idat.is_empty() {
        return (None, None);
    }
    // Interlaced images use a seven-pass layout; v1 shows the tree but not the
    // pixels for them.
    if ihdr.interlace != 0 {
        tree.add(
            Some(root),
            "interlaced image: pixel preview unavailable",
            ByteRange::new(0, 0),
            NodeKind::Warning,
            None,
        );
        return (None, None);
    }

    let mut sink = CheckpointSink::new(CHECKPOINT_INTERVAL);
    let raw = match zlib_decompress(idat, MAX_PIXEL_BYTES, &mut sink) {
        Ok(raw) => raw,
        Err(err) => {
            tree.add(
                Some(root),
                format!("IDAT decompression failed: {err:?}"),
                ByteRange::new(0, 0),
                NodeKind::Error,
                None,
            );
            return (None, Some(sink.finish()));
        }
    };
    let summary = sink.finish();

    match unfilter(&raw, ihdr.width, ihdr.height, ihdr.bytes_per_pixel()) {
        Ok(pixels) => (Some(pixels), Some(summary)),
        Err(err) => {
            tree.add(
                Some(root),
                format!("unfiltering failed: {err:?}"),
                ByteRange::new(0, 0),
                NodeKind::Error,
                None,
            );
            (None, Some(summary))
        }
    }
}
