//! Browser bridge for `hexscope-core`.
//!
//! The parse tree crosses into JavaScript as parallel arrays — one entry per
//! node, node 0 is the root — rather than one JS object per node. A 10 MB PNG
//! has thousands of nodes, and building that many objects through the binding
//! layer costs far more than copying a handful of typed arrays.

#![forbid(unsafe_code)]

use hexscope_core::clean::{CleanOptions, clean_video_gapped, clean_with};
use hexscope_core::docs::describe;
use hexscope_core::exif::PhotoFacts;
use hexscope_core::inflate::{
    BlockKind, Checkpoint, CheckpointSink, Decoder, InflateError, InflateEvent, Step, inflate,
};
use hexscope_core::map::{composition, entropy as window_entropy};
use hexscope_core::model::{NodeKind, ParseTree, Value};
use hexscope_core::png::{MAX_PIXEL_BYTES, PngDocument};
use hexscope_core::verification::VerificationSnapshot;
use hexscope_core::video::{Gap, parse_video_gapped};
use hexscope_core::zip::{ExtractError, ZipEntry, extract};
use hexscope_core::{Document, Format, parse as parse_any};
use wasm_bindgen::prelude::*;

/// Separates entries in the joined label and value strings. It is a control
/// character, and every string is sanitised of control characters before
/// joining, so it can never appear inside an entry.
pub const SEPARATOR: char = '\u{1F}';

/// Numbers per step in the array [`Parsed::steps`] returns.
pub const STEP_STRIDE: usize = 6;

/// Numbers per entry in [`Parsed::entries`]: node, method, flags,
/// compressed, uncompressed, playable, openable.
pub const ENTRY_STRIDE: usize = 7;

/// Numbers per slice in [`Parsed::composition`].
pub const SLICE_STRIDE: usize = 4;

/// Checkpoints per decoded ZIP entry, as for a PNG's IDAT stream.
const CHECKPOINT_INTERVAL: u64 = 512;

/// Numbers per part in [`Parsed::explain`]: kind, bit_start, bit_end, value,
/// code, code_len.
pub const PART_STRIDE: usize = 6;

/// A parsed file, flattened. Index `i` in every node array describes node `i`.
///
/// It also keeps what the DEFLATE player needs — the reassembled zlib stream,
/// its output, and resume checkpoints — so steps can be produced on demand
/// instead of millions of them being held in memory.
#[wasm_bindgen]
pub struct Parsed {
    starts: Vec<f64>,
    lens: Vec<f64>,
    parents: Vec<i32>,
    kinds: Vec<u8>,
    labels: String,
    values: String,
    ihdr: Option<[u32; 5]>,
    trace: Option<[f64; 4]>,
    idat_bytes: f64,
    /// IDAT payloads concatenated: the zlib stream, header and trailer included.
    stream: Vec<u8>,
    segments: Vec<f64>,
    /// Decompressed output. For a stream that failed, the part decoded before
    /// the failure — still exactly what the steps up to there produce.
    output: Vec<u8>,
    checkpoints: Vec<Checkpoint>,
    format: &'static str,
    dimensions: Option<[u32; 2]>,
    /// Photo facts as (kind, text, node).
    facts: Vec<(&'static str, String, u32)>,
    /// The source facts already extracted while parsing this document.
    verification: Option<VerificationSnapshot>,
    /// `[latitude, longitude, altitude or NaN, node]`.
    location: Option<[f64; 4]>,
    /// Bytes of wrapper around the DEFLATE data in `stream`: a zlib header
    /// and Adler-32 for PNG, nothing for a ZIP entry.
    header_len: usize,
    trailer_len: usize,
    zip: Option<ZipState>,
    /// Why the last `extract_entry` returned nothing.
    extract_error: String,
    docs: DocTables,
    /// `[start, len, role, node or -1] × n`: what the file is made of.
    composition: Vec<f64>,
    /// The picture, scaled to fit: `(width, height, rgba)`.
    preview: Option<(u32, u32, Vec<u8>)>,
    /// A PDF's pages with text under black boxes: the layout of
    /// [`Parsed::blackouts`], and the covered texts in order.
    blackouts: Vec<f64>,
    blackout_texts: Vec<String>,
    /// A PDF's or an email's bytes and its attachments' names, when it has
    /// any: they open as files of their own.
    pdf_source: Vec<u8>,
    attachment_names: Vec<String>,
    /// A JPEG's bytes, for finding its blocks when asked.
    source: Vec<u8>,
    /// Its blocks, once found: the layout of [`Parsed::block_map`].
    blocks: Option<Vec<f64>>,
    /// Why the blocks were not found, or stop short.
    blocks_note: String,
    /// A photo's EXIF orientation, 1 to 8; 1 when it has none.
    orientation: u16,
}

/// The preview's longer side, in pixels: sharp on a 2× screen at the width
/// the file panel gives it.
const PREVIEW_MAX: u32 = 768;

mod copies;
mod flat;
mod parsed;
#[cfg(feature = "pdf")]
mod pdf;
mod steps;
mod tables;
#[cfg(test)]
mod tests;

pub use copies::{CleanCopy, RepairCopy, clean_copy, clean_movie, repair_copy};
pub use flat::flatten;
use flat::{add_facts, sanitise, with_png};
#[cfg(feature = "pdf")]
pub use pdf::{
    PagePictures, PageTexts, PdfShapes, jpegs_under, page_pictures, page_texts, pdf_pictures,
    pdf_shapes, redact_copy,
};
use tables::{DocTables, ZipState};

/// Parses a file of any supported format. Never throws: damage is reported
/// as warning and error nodes, and an unsupported format still gets a tree.
#[wasm_bindgen]
pub fn parse(bytes: &[u8]) -> Parsed {
    parsed(parse_any(bytes), bytes, bytes.len() as u64)
}

/// Whether a file of `len` bytes that starts with `head` is an MP4 or
/// QuickTime movie, and so can be read by [`parse_movie`].
#[wasm_bindgen(js_name = isMovie)]
pub fn is_movie(head: &[u8], len: f64) -> bool {
    !hexscope_core::heif::is_heif(head) && hexscope_core::video::starts_a_video(head, len as u64)
}

/// `[at, len]` pairs: each box whose body was left out of the bytes.
fn gaps(pairs: &[f64]) -> Vec<Gap> {
    pairs
        .as_chunks::<2>()
        .0
        .iter()
        .map(|&[at, len]| Gap {
            at: at as u64,
            len: len as u64,
        })
        .collect()
}

/// A movie too large to hold, given without the bodies of the boxes in
/// `gaps`: see `hexscope_core::video::parse_video_gapped`.
#[wasm_bindgen(js_name = parseMovie)]
pub fn parse_movie(bytes: &[u8], gaps_at: &[f64]) -> Parsed {
    let gaps = gaps(gaps_at);
    let len = gaps
        .iter()
        .fold(bytes.len() as u64, |n, g| n.saturating_add(g.len));
    parsed(
        Document::Video(parse_video_gapped(bytes, &gaps)),
        bytes,
        len,
    )
}

fn parsed(doc: Document, bytes: &[u8], len: u64) -> Parsed {
    let verification =
        hexscope_core::verification::snapshot(&hexscope_core::summary::summary_of(&doc));
    let docs = DocTables::build(doc.tree(), doc.format());
    let slices: Vec<f64> = composition(doc.tree(), doc.format(), len)
        .iter()
        .flat_map(|s| {
            [
                s.start as f64,
                s.len as f64,
                s.role as u8 as f64,
                s.node.map_or(-1.0, |n| n as f64),
            ]
        })
        .collect();
    let mut parsed = match doc {
        Document::Png(doc) => with_png(bytes, doc),
        Document::Jpeg(doc) => {
            let mut parsed = flatten(&doc.tree);
            parsed.format = "jpeg";
            parsed.source = bytes.to_vec();
            parsed.dimensions = doc.width.zip(doc.height).map(|(w, h)| [w as u32, h as u32]);
            add_facts(&mut parsed, &doc.facts);
            parsed
        }
        Document::Heif(doc) => {
            let mut parsed = flatten(&doc.tree);
            parsed.format = "heif";
            parsed.dimensions = doc.width.zip(doc.height).map(|(w, h)| [w, h]);
            add_facts(&mut parsed, &doc.facts);
            parsed
        }
        Document::Webp(doc) => {
            let mut parsed = flatten(&doc.tree);
            parsed.format = "webp";
            parsed.dimensions = doc.width.zip(doc.height).map(|(w, h)| [w, h]);
            add_facts(&mut parsed, &doc.facts);
            parsed
        }
        Document::Gif(doc) => {
            let mut parsed = flatten(&doc.tree);
            parsed.format = "gif";
            parsed.dimensions = doc.width.zip(doc.height).map(|(w, h)| [w, h]);
            add_facts(&mut parsed, &doc.facts);
            parsed
        }
        Document::Tiff(doc) => {
            let mut parsed = flatten(&doc.tree);
            parsed.format = "tiff";
            parsed.dimensions = doc.width.zip(doc.height).map(|(w, h)| [w, h]);
            add_facts(&mut parsed, &doc.facts);
            parsed
        }
        Document::Video(doc) => {
            let mut parsed = flatten(&doc.tree);
            parsed.format = "video";
            parsed.dimensions = doc.width.zip(doc.height).map(|(w, h)| [w, h]);
            add_facts(&mut parsed, &doc.facts);
            parsed
        }
        Document::Pdf(doc) if cfg!(feature = "pdf") => {
            let mut parsed = flatten(&doc.tree);
            parsed.format = "pdf";
            for f in &doc.facts {
                parsed.facts.push((f.kind, sanitise(&f.text), f.node));
            }
            if !doc.attachments.is_empty() {
                parsed.pdf_source = bytes.to_vec();
                parsed.attachment_names =
                    doc.attachments.iter().map(|(n, _)| sanitise(n)).collect();
            }
            for b in &doc.blackouts {
                parsed.blackouts.extend([b.page as f64]);
                parsed.blackouts.extend(b.media);
                parsed.blackouts.extend([
                    b.boxes.len() as f64,
                    b.texts.len() as f64,
                    b.context.len() as f64,
                ]);
                for area in &b.boxes {
                    parsed.blackouts.extend(area);
                }
                for (area, text) in b.texts.iter().chain(&b.context) {
                    parsed.blackouts.extend(area);
                    parsed.blackout_texts.push(sanitise(text));
                }
            }
            parsed
        }
        Document::Zip(doc) if cfg!(feature = "office") => {
            let mut parsed = flatten(&doc.tree);
            parsed.format = "zip";
            for f in &doc.facts {
                parsed.facts.push((f.kind, sanitise(&f.text), f.node));
            }
            parsed.zip = Some(ZipState {
                source: bytes.to_vec(),
                entries: doc.entries,
            });
            parsed
        }
        Document::Eml(doc) if cfg!(feature = "mail") => {
            let mut parsed = flatten(&doc.tree);
            parsed.format = "eml";
            for f in &doc.facts {
                parsed.facts.push((f.kind, sanitise(&f.text), f.node));
            }
            if !doc.attachments.is_empty() {
                parsed.pdf_source = bytes.to_vec();
                parsed.attachment_names = doc.attachments.iter().map(|n| sanitise(n)).collect();
            }
            parsed
        }
        Document::Cfb(doc) if cfg!(feature = "office") => {
            let mut parsed = flatten(&doc.tree);
            parsed.format = doc.kind.format();
            for f in &doc.facts {
                parsed.facts.push((f.kind, sanitise(&f.text), f.node));
            }
            if !doc.attachments.is_empty() {
                parsed.pdf_source = bytes.to_vec();
                parsed.attachment_names = doc.attachments.iter().map(|n| sanitise(n)).collect();
            }
            parsed
        }
        Document::Rtf(doc) if cfg!(feature = "office") => {
            let mut parsed = flatten(&doc.tree);
            parsed.format = "rtf";
            for f in &doc.facts {
                parsed.facts.push((f.kind, sanitise(&f.text), f.node));
            }
            parsed
        }
        Document::Svg(doc) if cfg!(feature = "svg") => {
            let mut parsed = flatten(&doc.tree);
            parsed.format = "svg";
            for f in &doc.facts {
                parsed.facts.push((f.kind, sanitise(&f.text), f.node));
            }
            parsed
        }
        Document::Wasm(doc) if cfg!(feature = "module") => {
            let mut parsed = flatten(&doc.tree);
            parsed.format = "wasm";
            for f in &doc.facts {
                parsed.facts.push((f.kind, sanitise(&f.text), f.node));
            }
            parsed
        }
        Document::Unknown(tree) => flatten(&tree),
        // A document in the small module, which never makes one.
        other => flatten(other.tree()),
    };
    parsed.docs = docs;
    parsed.composition = slices;
    parsed.verification = Some(verification);
    parsed
}

/// Shannon entropy, 0 to 8 bits per byte, of up to `bins` windows covering
/// `bytes`, each at least 256 bytes. Separate from `parse` so the time shown
/// for parsing is the parse alone.
#[wasm_bindgen]
pub fn entropy(bytes: &[u8], bins: u32) -> Vec<f32> {
    window_entropy(bytes, bins as usize)
}
