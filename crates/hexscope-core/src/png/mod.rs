pub mod adam7;
pub mod chunks;
pub(crate) mod docs;
pub mod fields;
mod idat;
pub mod preview;
pub mod unfilter;

use crate::exif::{Fact, PhotoFacts, parse_tiff};
use crate::inflate::trace::TraceSummary;
use crate::model::{ByteRange, NodeId, NodeKind, ParseTree, Value};
use crate::png::chunks::{ChunkError, PNG_SIGNATURE, next_chunk, resync};
use crate::png::fields::{
    Ihdr, TextNote, decode_gama, decode_ihdr, decode_itxt, decode_phys, decode_plte, decode_text,
    decode_time, decode_trns, decode_ztxt,
};
use crate::png::idat::{IdatChunks, decode_pixels};
use crate::reader::Reader;

/// Caps decompressed IDAT output at 512 MB so a compression bomb cannot
/// exhaust memory.
pub const MAX_PIXEL_BYTES: u64 = 512 * 1024 * 1024;

#[derive(Debug, Default)]
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
    /// What its eXIf and text chunks say about who made it.
    pub facts: PhotoFacts,
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
        // "PNG" intact but the rest damaged — the classic result of a
        // text-mode transfer. Worth reporting, and worth reading on.
        Ok(sig) if sig.get(1..4) == Some(b"PNG") => {
            // Read fine, just wrong — a warning under the NodeKind rule, and
            // parsing carries on.
            tree.warning(root, "damaged PNG signature", ByteRange::new(0, 8));
        }
        _ => {
            tree.error(
                root,
                "not a PNG signature",
                ByteRange::new(0, data.len().min(8) as u64),
            );
            return PngDocument {
                tree,
                ..PngDocument::default()
            };
        }
    }

    let mut ihdr: Option<Ihdr> = None;
    let mut idat: Vec<u8> = Vec::new();
    let mut idat_ranges: Vec<ByteRange> = Vec::new();
    let mut idat_nodes: Vec<NodeId> = Vec::new();
    let mut saw_iend = false;
    let mut notes: Vec<TextNote> = Vec::new();
    let mut exif = PhotoFacts::default();

    loop {
        // Captured before the call: `next_chunk` seeks to end-of-input on any
        // error so the walk self-terminates, which means the reader's position
        // afterwards is always EOF and says nothing about where the damage is.
        // The whole point of this tool is pointing at the broken bytes.
        let chunk_start = r.pos();
        // A PNG ends at IEND. Readers ignore what follows, so whatever is
        // there — another file, a payload — is kept out of sight, not broken.
        if saw_iend {
            if chunk_start < data.len() as u64 {
                tree.warning(
                    root,
                    "data after the end of the image",
                    ByteRange::new(chunk_start, data.len() as u64 - chunk_start),
                );
            }
            break;
        }
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
                // A corrupt length usually damages one chunk, not the file:
                // resume at the next intact chunk if there is one.
                if let Some(next) = resync(data, chunk_start + 1) {
                    tree.error(
                        root,
                        format!("{label}: skipped to the next intact chunk"),
                        ByteRange::new(chunk_start, next - chunk_start),
                    );
                    r.seek(next);
                    continue;
                }
                tree.error(
                    root,
                    label,
                    ByteRange::new(chunk_start, data.len() as u64 - chunk_start),
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

        if !chunk.kind_is_valid() {
            tree.warning(
                node,
                "chunk type is not four ASCII letters",
                ByteRange::new(chunk.range.start + 4, 4),
            );
        }

        if !chunk.crc_ok() {
            tree.warning(
                node,
                format!(
                    "CRC mismatch: stored {:08X}, computed {:08X}",
                    chunk.declared_crc, chunk.actual_crc
                ),
                ByteRange::new(chunk.range.end() - 4, 4),
            );
        }

        match &chunk.kind {
            b"IHDR" => ihdr = decode_ihdr(&chunk, &mut tree, node),
            b"PLTE" => decode_plte(&chunk, &mut tree, node),
            b"tEXt" => notes.extend(decode_text(&chunk, &mut tree, node)),
            b"zTXt" => notes.extend(decode_ztxt(&chunk, &mut tree, node)),
            b"iTXt" => notes.extend(decode_itxt(&chunk, &mut tree, node)),
            b"tIME" => {
                decode_time(&chunk, &mut tree, node);
            }
            b"eXIf" => exif.fill_from(parse_tiff(
                &mut tree,
                node,
                chunk.data,
                chunk.data_range.start,
            )),
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
        tree.warning(
            root,
            "no IDAT chunk: the file carries no image data",
            // Zero-length: something absent has no bytes to point at, and a
            // whole-file range would swallow every hover in the hex view.
            ByteRange::new(0, 0),
        );
    }
    if !saw_iend {
        tree.warning(
            root,
            "no IEND chunk: the file does not end properly",
            ByteRange::new(data.len() as u64, 0),
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
        facts: {
            // EXIF first, as in a photo; text notes fill what it lacks.
            exif.fill_from(text_facts(&notes));
            exif
        },
    }
}

/// Longest text fact kept, in characters.
const MAX_FACT: usize = 200;

/// The text notes that say who made the image, with what and when: the
/// keywords PNG defines (§11.3.3.1) and, in an XMP packet, their like.
fn text_facts(notes: &[TextNote]) -> PhotoFacts {
    let mut f = PhotoFacts::default();
    let set = |slot: &mut Option<Fact>, text: &str, node: NodeId| {
        let text = text.trim();
        if slot.is_none() && !text.is_empty() {
            *slot = Some(Fact {
                text: text.chars().take(MAX_FACT).collect(),
                node,
            });
        }
    };
    for (keyword, text, node) in notes {
        let node = *node;
        match keyword.as_str() {
            "Author" => set(&mut f.owner, text, node),
            "Software" => set(&mut f.software, text, node),
            "Creation Time" => set(&mut f.taken, text, node),
            "Source" => set(&mut f.camera, text, node),
            "XML:com.adobe.xmp" => {
                use crate::pdf::facts::{xmp_date, xmp_text};
                if let Some(t) = xmp_text(text, "dc:creator") {
                    set(&mut f.owner, &t, node);
                }
                if let Some(t) = xmp_text(text, "xmp:CreatorTool") {
                    set(&mut f.software, &t, node);
                }
                if let Some(t) = xmp_text(text, "tiff:Model") {
                    set(&mut f.camera, &t, node);
                }
                let taken = [
                    "exif:DateTimeOriginal",
                    "photoshop:DateCreated",
                    "xmp:CreateDate",
                ]
                .iter()
                .find_map(|k| xmp_text(text, k));
                if let Some(t) = taken {
                    set(&mut f.taken, &xmp_date(&t), node);
                }
            }
            _ => {}
        }
    }
    f
}
