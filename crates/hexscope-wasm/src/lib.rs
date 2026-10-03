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

/// What each node is, deduplicated: thousands of nodes share a few hundred
/// explanations, so each is sent once with a per-node index into it.
#[derive(Default)]
struct DocTables {
    ids: Vec<i32>,
    texts: Vec<&'static str>,
    cites: Vec<&'static str>,
    urls: Vec<&'static str>,
    concerns: Vec<u8>,
}

impl DocTables {
    fn build(tree: &ParseTree, format: Format) -> Self {
        let mut t = DocTables::default();
        // Docs already sent, by where their text and citation live: every
        // doc is a constant, so its strings' addresses name it. Sorted, for
        // binary search; a file has a few hundred kinds of part at most. A
        // hash map here cost 3 KB of the build.
        let mut seen: Vec<((usize, usize, u8), i32)> = Vec::new();
        for n in tree.nodes() {
            let id = match describe(tree, n.id, format) {
                None => -1,
                Some(doc) => {
                    let key = (
                        doc.text.as_ptr() as usize,
                        doc.spec.map_or(0, |s| s.cite.as_ptr() as usize),
                        doc.concern.map_or(0, |c| c as u8),
                    );
                    match seen.binary_search_by(|(k, _)| k.cmp(&key)) {
                        Ok(i) => seen[i].1,
                        Err(i) => {
                            t.texts.push(doc.text);
                            t.cites.push(doc.spec.map_or("", |s| s.cite));
                            t.urls.push(doc.spec.map_or("", |s| s.url));
                            t.concerns.push(doc.concern.map_or(0, |c| c as u8));
                            let id = t.texts.len() as i32 - 1;
                            seen.insert(i, (key, id));
                            id
                        }
                    }
                }
            };
            t.ids.push(id);
        }
        t
    }
}

/// What playing or opening a ZIP entry needs after parsing is done.
struct ZipState {
    source: Vec<u8>,
    entries: Vec<ZipEntry>,
}

/// Whether the player can decode this entry: deflated, readable, whole.
fn playable(e: &ZipEntry) -> bool {
    e.method == 8 && !e.is_encrypted() && e.compressed > 0 && e.data.len == e.compressed
}

/// Whether the entry can be opened as a file of its own: a file, not a
/// folder, with something in it, in a method this tool reads.
fn openable(e: &ZipEntry) -> bool {
    matches!(e.method, 0 | 8)
        && !e.is_encrypted()
        && !e.is_dir()
        && e.uncompressed > 0
        && e.data.len == e.compressed
}

fn extract_message(err: ExtractError) -> String {
    match err {
        ExtractError::Encrypted => "it is encrypted".into(),
        ExtractError::Unsupported(m) => {
            format!("its compression method ({m}) is not one this tool reads")
        }
        ExtractError::OutOfRange => "its data runs past the end of the file".into(),
        ExtractError::Inflate(e) => format!("its DEFLATE data is damaged ({e:?})"),
        ExtractError::Crc { stored, actual } => {
            format!("its CRC-32 does not match: stored {stored:08X}, computed {actual:08X}")
        }
        ExtractError::TooLarge => "it would decompress to more than 512 MB".into(),
    }
}

#[wasm_bindgen]
impl Parsed {
    /// Byte offset where each node begins.
    #[wasm_bindgen(getter)]
    pub fn starts(&self) -> Vec<f64> {
        self.starts.clone()
    }

    /// Length in bytes of each node. Zero means the node has no bytes to
    /// point at, e.g. "no IDAT chunk".
    #[wasm_bindgen(getter)]
    pub fn lens(&self) -> Vec<f64> {
        self.lens.clone()
    }

    /// Parent index of each node, `-1` for the root.
    #[wasm_bindgen(getter)]
    pub fn parents(&self) -> Vec<i32> {
        self.parents.clone()
    }

    /// 0 container, 1 field, 2 warning, 3 error.
    #[wasm_bindgen(getter)]
    pub fn kinds(&self) -> Vec<u8> {
        self.kinds.clone()
    }

    /// Node labels joined by U+001F.
    #[wasm_bindgen(getter)]
    pub fn labels(&self) -> String {
        self.labels.clone()
    }

    /// Display values joined by U+001F; empty where a node has no value.
    #[wasm_bindgen(getter)]
    pub fn values(&self) -> String {
        self.values.clone()
    }

    /// `[width, height, bitDepth, colorType, interlace]`, or empty when the
    /// file has no readable IHDR.
    #[wasm_bindgen(getter)]
    pub fn ihdr(&self) -> Vec<u32> {
        self.ihdr.map(|h| h.to_vec()).unwrap_or_default()
    }

    /// `[totalEvents, literals, matches, outputBytes]` for the DEFLATE stream,
    /// or empty when nothing was decompressed.
    #[wasm_bindgen(getter)]
    pub fn trace(&self) -> Vec<f64> {
        self.trace.map(|t| t.to_vec()).unwrap_or_default()
    }

    /// Compressed size: the IDAT payload bytes, summed across chunks.
    #[wasm_bindgen(getter, js_name = idatBytes)]
    pub fn idat_bytes(&self) -> f64 {
        self.idat_bytes
    }

    /// File position of each IDAT payload as `[start, len, start, len, ...]`.
    /// Concatenated in order they are the zlib stream, whose first two bytes
    /// are the zlib header — so compressed bit `b` of the DEFLATE data lives in
    /// stream byte `2 + b / 8`.
    #[wasm_bindgen(getter)]
    pub fn segments(&self) -> Vec<f64> {
        self.segments.clone()
    }

    /// `png`, `jpeg` or `unknown`.
    #[wasm_bindgen(getter)]
    pub fn format(&self) -> String {
        self.format.to_string()
    }

    /// A photo's EXIF orientation, 1 to 8: how to turn the stored picture
    /// to show it the right way up. 1 when it says nothing.
    #[wasm_bindgen(getter)]
    pub fn orientation(&self) -> u16 {
        self.orientation
    }

    /// `[width, height]`, or empty when the file does not say.
    #[wasm_bindgen(getter)]
    pub fn dimensions(&self) -> Vec<u32> {
        self.dimensions.map(|d| d.to_vec()).unwrap_or_default()
    }

    /// What a file's metadata reveals, as `kind, text, node` triples joined
    /// by U+001F. For a photo: camera, lens, serial, owner, software, taken,
    /// thumbnail. For an Office document: title, author, editor, created,
    /// modified, revisions, editing, company, application, template.
    #[wasm_bindgen(getter)]
    pub fn facts(&self) -> String {
        let sep = SEPARATOR.to_string();
        self.facts
            .iter()
            .flat_map(|(kind, text, node)| [kind.to_string(), text.clone(), node.to_string()])
            .collect::<Vec<_>>()
            .join(&sep)
    }

    /// Compares actual copy bytes with this already-parsed source. The extra
    /// summary parse is bounded for browser memory.
    #[wasm_bindgen(js_name = verifyCopy)]
    pub fn verify_copy(&self, bytes: &[u8]) -> String {
        const MAX_VERIFICATION_BYTES: usize = 10 * 1024 * 1024;
        let Some(source) = self.verification.as_ref() else {
            return hexscope_core::verification::VerificationReport::skipped().to_json();
        };
        if bytes.len() > MAX_VERIFICATION_BYTES {
            return hexscope_core::verification::VerificationReport::skipped().to_json();
        }
        let output =
            hexscope_core::verification::snapshot(&hexscope_core::summary::summarize(bytes));
        hexscope_core::verification::compare(source, &output).to_json()
    }

    /// `[latitude, longitude, altitude, node]` in decimal degrees and metres,
    /// altitude NaN when unknown; empty when the photo carries no location.
    #[wasm_bindgen(getter)]
    pub fn location(&self) -> Vec<f64> {
        self.location.map(|l| l.to_vec()).unwrap_or_default()
    }

    /// The decompressed stream (filtered scanlines). If decoding failed, the
    /// output produced before the failure.
    #[wasm_bindgen(getter)]
    pub fn inflated(&self) -> Vec<u8> {
        self.output.clone()
    }

    /// Where a JPEG's blocks are, scan by scan: `[progressive, components,
    /// scans]`, then for each scan `[components (a bit each), ss, se, ah,
    /// al, unitWidth, unitHeight, columns, rows, blocksPerUnit, end, n]` and
    /// its `n` numbers: each unit's first file bit, then the bit after the
    /// last. Empty when they cannot be found; [`Parsed::blocks_note`] says
    /// why. Found on first asking, since a large photo takes a moment.
    #[wasm_bindgen(js_name = blockMap)]
    pub fn block_map(&mut self) -> Vec<f64> {
        if self.blocks.is_none() {
            let found = hexscope_core::jpeg::blocks::block_map(&self.source);
            let (map, note) = match found {
                Ok(m) => {
                    let mut out = vec![
                        m.progressive as u8 as f64,
                        m.components as f64,
                        m.scans.len() as f64,
                    ];
                    for s in &m.scans {
                        out.extend([
                            s.components as f64,
                            s.ss as f64,
                            s.se as f64,
                            s.ah as f64,
                            s.al as f64,
                            s.unit_width as f64,
                            s.unit_height as f64,
                            s.columns as f64,
                            s.rows as f64,
                            s.blocks_per_unit as f64,
                            s.end as f64,
                            s.starts.len() as f64,
                        ]);
                        out.extend(s.starts.iter().map(|&b| b as f64));
                    }
                    (out, m.stopped.unwrap_or("").to_string())
                }
                Err(e) => (Vec::new(), e.reason().to_string()),
            };
            self.blocks = Some(map);
            self.blocks_note = note;
        }
        self.blocks.clone().unwrap_or_default()
    }

    /// Why the last [`Parsed::block_map`] is empty or stops short; empty when it is whole.
    #[wasm_bindgen(getter, js_name = blocksNote)]
    pub fn blocks_note(&self) -> String {
        self.blocks_note.clone()
    }

    /// A PDF's attached files, by name, joined by U+001F; each opens with
    /// `extractEntry` at its index.
    #[wasm_bindgen(getter)]
    pub fn attachments(&self) -> String {
        self.attachment_names.join(&SEPARATOR.to_string())
    }

    /// A PDF's pages with text under black boxes, to draw: per page
    /// `[page, left, bottom, right, top, boxes, texts, context]`, then four
    /// numbers per box, per covered text and per text left showing on the
    /// same lines, each `[left, bottom, right, top]` in the page's points.
    /// The texts are in `blackoutTexts`, covered first, in the same order.
    #[wasm_bindgen(getter)]
    pub fn blackouts(&self) -> Vec<f64> {
        self.blackouts.clone()
    }

    /// The texts of [`Parsed::blackouts`] joined by U+001F; an empty one
    /// is in a font whose codes do not read as letters.
    #[wasm_bindgen(getter, js_name = blackoutTexts)]
    pub fn blackout_texts(&self) -> String {
        self.blackout_texts.join(&SEPARATOR.to_string())
    }

    /// The picture's preview size, `[width, height]`; empty when there is none.
    #[wasm_bindgen(getter, js_name = previewSize)]
    pub fn preview_size(&self) -> Vec<u32> {
        self.preview.as_ref().map_or(Vec::new(), |p| vec![p.0, p.1])
    }

    /// The preview's pixels, four bytes each.
    #[wasm_bindgen(getter, js_name = previewPixels)]
    pub fn preview_pixels(&self) -> Vec<u8> {
        self.preview.as_ref().map_or(Vec::new(), |p| p.2.clone())
    }

    /// Each scanline's filter type, 0 to 4, in the order they are stored:
    /// top to bottom, or for an interlaced PNG pass by pass. Rows past a
    /// decoding failure are absent.
    #[wasm_bindgen(getter, js_name = rowFilters)]
    pub fn row_filters(&self) -> Vec<u8> {
        let Some([w, h, depth, color, interlace @ (0 | 1)]) = self.ihdr else {
            return Vec::new();
        };
        let ihdr = hexscope_core::png::fields::Ihdr {
            width: w,
            height: h,
            bit_depth: depth as u8,
            color_type: color as u8,
            interlace: interlace as u8,
        };
        // Each pass's rows: where the first starts, its length, how many.
        let rows: Vec<(usize, usize, usize)> = if interlace == 1 {
            let Some(passes) = hexscope_core::png::adam7::passes(&ihdr) else {
                return Vec::new();
            };
            passes
                .iter()
                .filter(|p| p.width > 0 && p.height > 0)
                .map(|p| (p.start, p.stride + 1, p.height as usize))
                .collect()
        } else {
            let Some(row) = ihdr.stride().and_then(|s| s.checked_add(1)) else {
                return Vec::new();
            };
            vec![(0, row, h as usize)]
        };
        rows.iter()
            .flat_map(|&(start, row, n)| {
                (0..n).map_while(move |r| self.output.get(start + r * row).copied())
            })
            .collect()
    }

    /// The step that wrote output byte `pos` (`by` 0) or read DEFLATE bit
    /// `pos` (`by` 1), as `[index, kind, a, b, bitStart, bitEnd, outStart]`
    /// in the layout of [`Parsed::steps`]; empty when no step does. Resumes
    /// from the nearest checkpoint, so it costs one interval at most.
    pub fn locate(&self, by: u8, pos: f64) -> Vec<f64> {
        let Some(body) = self.body() else {
            return Vec::new();
        };
        if !(pos.is_finite() && pos >= 0.0) {
            return Vec::new();
        }
        let pos = pos as u64;
        let before = |c: &Checkpoint| {
            if by == 0 {
                c.out_pos <= pos
            } else {
                c.bit_pos <= pos
            }
        };
        let nearest = self.checkpoints.partition_point(before);
        let mut d = nearest
            .checked_sub(1)
            .and_then(|i| self.checkpoints.get(i))
            .and_then(|cp| Decoder::resume(body, MAX_PIXEL_BYTES, &self.output, cp).ok())
            .unwrap_or_else(|| Decoder::new(body, MAX_PIXEL_BYTES));
        while let Some(Ok(step)) = d.step() {
            let produced = match step.event {
                InflateEvent::Literal { .. } => 1,
                InflateEvent::Match { length, .. } => length as u64,
                _ => 0,
            };
            let (start, end, done) = if by == 0 {
                (
                    step.out_start,
                    step.out_start + produced,
                    step.out_start > pos,
                )
            } else {
                (step.bit_start, step.bit_end, step.bit_start > pos)
            };
            if done {
                break;
            }
            if start <= pos && pos < end {
                let mut out = vec![step.index as f64];
                encode(&step, &mut out);
                return out;
            }
        }
        Vec::new()
    }

    /// Up to `count` DEFLATE steps starting at step `from`, flattened with
    /// [`STEP_STRIDE`] numbers per step:
    /// `[kind, a, b, bitStart, bitEnd, outStart]`.
    ///
    /// | kind | meaning        | a                              | b      |
    /// |------|----------------|--------------------------------|--------|
    /// | 0    | block start    | 0 stored, 1 fixed, 2 dynamic   | —      |
    /// | 1    | literal        | the byte                       | —      |
    /// | 2    | back-reference | distance                       | length |
    /// | 3    | block end      | —                              | —      |
    /// | 4    | decoding failed here; always the last step          |
    ///
    /// Bits count from the start of the DEFLATE data, after the zlib header.
    pub fn steps(&self, from: f64, count: u32) -> Vec<f64> {
        let Some(body) = self.body() else {
            return Vec::new();
        };
        let from = if from.is_finite() && from > 0.0 {
            from as u64
        } else {
            0
        };
        let wanted = count as usize * STEP_STRIDE;
        let mut decoder = self.decoder_at(body, from);

        // `count` comes from the caller: reserve for a normal batch, not for
        // whatever was asked, or a huge count aborts the worker before a
        // single step is decoded. The vector still grows to fit.
        let mut out = Vec::with_capacity(wanted.min(4096 * STEP_STRIDE));
        let mut last_end = 0u64;
        while out.len() < wanted {
            match decoder.step() {
                None => break,
                Some(Ok(step)) => {
                    last_end = step.bit_end;
                    if step.index >= from {
                        encode(&step, &mut out);
                    }
                }
                Some(Err(_)) => {
                    let at = decoder.output().len() as f64;
                    out.extend_from_slice(&[
                        4.0,
                        0.0,
                        0.0,
                        last_end as f64,
                        decoder.bit_pos() as f64,
                        at,
                    ]);
                    break;
                }
            }
        }
        out
    }

    /// What the file is made of: [`SLICE_STRIDE`] numbers per slice,
    /// `[start, len, role, node]`, covering every byte in order. Roles: 0
    /// content, 1 metadata, 2 thumbnail, 3 structure, 4 hidden, 5 damaged;
    /// node -1 where no node covers the bytes.
    #[wasm_bindgen(getter)]
    pub fn composition(&self) -> Vec<f64> {
        self.composition.clone()
    }

    /// Per node, an index into the explanation tables below, or -1.
    #[wasm_bindgen(getter, js_name = docIds)]
    pub fn doc_ids(&self) -> Vec<i32> {
        self.docs.ids.clone()
    }

    /// Explanations, one sentence each, joined by U+001F.
    #[wasm_bindgen(getter, js_name = docTexts)]
    pub fn doc_texts(&self) -> String {
        self.docs.texts.join(&SEPARATOR.to_string())
    }

    /// Where each explanation's subject is defined, e.g. "PNG §11.2.1";
    /// empty where none is cited. Joined by U+001F.
    #[wasm_bindgen(getter, js_name = docCites)]
    pub fn doc_cites(&self) -> String {
        self.docs.cites.join(&SEPARATOR.to_string())
    }

    /// The link for each citation, empty where none. Joined by U+001F.
    #[wasm_bindgen(getter, js_name = docUrls)]
    pub fn doc_urls(&self) -> String {
        self.docs.urls.join(&SEPARATOR.to_string())
    }

    /// Per explanation: 0 not a problem, 1 damage, 2 something hidden,
    /// 3 an oddity.
    #[wasm_bindgen(getter, js_name = docConcerns)]
    pub fn doc_concerns(&self) -> Vec<u8> {
        self.docs.concerns.clone()
    }

    /// Per ZIP entry, [`ENTRY_STRIDE`] numbers: `[node, method, flags,
    /// compressed, uncompressed, playable, openable]`. Empty for other formats.
    #[wasm_bindgen(getter)]
    pub fn entries(&self) -> Vec<f64> {
        let Some(zip) = &self.zip else {
            return Vec::new();
        };
        zip.entries
            .iter()
            .flat_map(|e| {
                [
                    e.node as f64,
                    e.method as f64,
                    e.flags as f64,
                    e.compressed as f64,
                    e.uncompressed as f64,
                    if playable(e) { 1.0 } else { 0.0 },
                    if openable(e) { 1.0 } else { 0.0 },
                ]
            })
            .collect()
    }

    /// Bytes of wrapper before the DEFLATE data in the stream the segments
    /// describe: compressed bit `b` lives in stream byte `streamHeader + b / 8`.
    #[wasm_bindgen(getter, js_name = streamHeader)]
    pub fn stream_header(&self) -> f64 {
        self.header_len as f64
    }

    /// ZIP entry `index`, decompressed and checked, to be parsed as a file
    /// of its own. Empty on failure, with the reason in `extractError`.
    #[wasm_bindgen(js_name = extractEntry)]
    pub fn extract_entry(&mut self, index: u32) -> Vec<u8> {
        self.extract_error.clear();
        let result = match &self.zip {
            _ if !cfg!(feature = "documents") => Err("this module opens no documents".to_string()),
            None if !self.attachment_names.is_empty() && self.format == "eml" => {
                hexscope_core::eml::attachment_bytes(&self.pdf_source, index as usize)
                    .map_err(str::to_string)
            }
            None if !self.attachment_names.is_empty() && self.format == "msg" => {
                hexscope_core::cfb::attachment_bytes(&self.pdf_source, index as usize)
                    .map_err(str::to_string)
            }
            None if !self.attachment_names.is_empty() => {
                hexscope_core::pdf::attachment_bytes(&self.pdf_source, index as usize)
                    .map_err(str::to_string)
            }
            None => Err("this file is not an archive".to_string()),
            Some(zip) => match zip.entries.get(index as usize) {
                None => Err(format!("there is no entry {index}")),
                Some(e) if !openable(e) => Err("this entry cannot be opened".to_string()),
                Some(e) => extract(&zip.source, e, MAX_PIXEL_BYTES).map_err(extract_message),
            },
        };
        result.unwrap_or_else(|reason| {
            self.extract_error = reason;
            Vec::new()
        })
    }

    #[wasm_bindgen(getter, js_name = extractError)]
    pub fn extract_error(&self) -> String {
        self.extract_error.clone()
    }

    /// Makes ZIP entry `index` the stream the player steps through: its data
    /// is decoded once, with checkpoints, as a PNG's IDAT stream is at parse
    /// time. Returns false, changing nothing, if the entry cannot be played.
    #[wasm_bindgen(js_name = selectEntry)]
    pub fn select_entry(&mut self, index: u32) -> bool {
        if !cfg!(feature = "documents") {
            return false;
        }
        let Some(zip) = &self.zip else {
            return false;
        };
        let Some(e) = zip.entries.get(index as usize).filter(|e| playable(e)) else {
            return false;
        };
        let (start, end) = (e.data.start as usize, e.data.end() as usize);
        let Some(data) = zip.source.get(start..end) else {
            return false;
        };
        let data = data.to_vec();
        let segment = [e.data.start as f64, e.data.len as f64];

        let mut sink = CheckpointSink::new(CHECKPOINT_INTERVAL);
        let result = inflate(&data, MAX_PIXEL_BYTES, &mut sink);
        let summary = sink.finish();
        self.output = match result {
            Ok(out) => out,
            // Replay to the failure so the player has real bytes to draw.
            Err(_) => {
                let mut d = Decoder::new(&data, MAX_PIXEL_BYTES);
                while let Some(Ok(_)) = d.step() {}
                d.into_output()
            }
        };
        self.trace = Some([
            summary.total_events as f64,
            summary.literals as f64,
            summary.matches as f64,
            summary.output_bytes as f64,
        ]);
        self.checkpoints = summary.checkpoints;
        self.segments = segment.to_vec();
        self.idat_bytes = segment[1];
        self.stream = data;
        self.header_len = 0;
        self.trailer_len = 0;
        true
    }

    /// What step `index` read, part by part. Layout: `[block_start, error,
    /// n, then n × (kind, bit_start, bit_end, value, code, code_len)]`,
    /// `error` -1 when the step decoded. Empty when there is no such step.
    pub fn explain(&self, index: f64) -> Vec<f64> {
        let Some(e) = self
            .decoder_before(index)
            .and_then(|mut d| d.explain_next())
        else {
            return Vec::new();
        };
        let mut out = Vec::with_capacity(3 + e.parts.len() * PART_STRIDE);
        out.push(e.block_start as f64);
        out.push(e.error.map_or(-1.0, error_code));
        out.push(e.parts.len() as f64);
        for p in &e.parts {
            out.extend_from_slice(&[
                p.kind as u8 as f64,
                p.bit_start as f64,
                p.bit_end as f64,
                p.value as f64,
                p.code as f64,
                p.code_len as f64,
            ]);
        }
        out
    }

    /// The code tables of the block holding step `index`. Layout: `[kind,
    /// hlit, hdist, hclen, then for literal/length and for distance: groups,
    /// then per group len, first_code, count, symbols…]`. Empty for a stored
    /// block or when there is no such step.
    pub fn tables(&self, index: f64) -> Vec<f64> {
        let Some(mut d) = self.decoder_before(index) else {
            return Vec::new();
        };
        // A block's codes are in place before its steps; a header step builds
        // them, so for it they appear only after.
        let tables = d.tables().or_else(|| {
            d.step();
            d.tables()
        });
        let Some(t) = tables else {
            return Vec::new();
        };
        let h = t.header;
        let mut out = vec![
            block_kind_code(t.kind),
            h.map_or(0.0, |h| h.hlit as f64),
            h.map_or(0.0, |h| h.hdist as f64),
            h.map_or(0.0, |h| h.hclen as f64),
        ];
        for groups in [&t.lit_len, &t.distance] {
            out.push(groups.len() as f64);
            for g in groups {
                out.extend_from_slice(&[g.len as f64, g.first_code as f64, g.symbols.len() as f64]);
                out.extend(g.symbols.iter().map(|&s| s as f64));
            }
        }
        out
    }
}

impl Parsed {
    /// A decoder whose next step is `index`, or `None` when decoding stops
    /// before reaching it.
    fn decoder_before(&self, index: f64) -> Option<Decoder<'_>> {
        let body = self.body()?;
        if !(index.is_finite() && index >= 0.0) {
            return None;
        }
        let index = index as u64;
        let mut d = self.decoder_at(body, index);
        while d.next_index() < index {
            d.step()?.ok()?;
        }
        Some(d)
    }

    /// The DEFLATE data: the zlib stream minus its 2-byte header and 4-byte
    /// Adler-32 trailer — exactly what the core decompressed.
    fn body(&self) -> Option<&[u8]> {
        let end = self.stream.len().checked_sub(self.trailer_len)?;
        self.stream.get(self.header_len..end)
    }

    /// A decoder positioned at or shortly before step `from`, resumed from the
    /// nearest checkpoint so seeking anywhere in a large file replays at most
    /// one checkpoint interval. Checkpoints recorded before a failure are
    /// still valid, since the partial output backs them.
    fn decoder_at<'a>(&'a self, body: &'a [u8], from: u64) -> Decoder<'a> {
        let nearest = self.checkpoints.partition_point(|c| c.event_index <= from);
        if let Some(cp) = nearest.checked_sub(1).and_then(|i| self.checkpoints.get(i))
            && let Ok(d) = Decoder::resume(body, MAX_PIXEL_BYTES, &self.output, cp)
        {
            return d;
        }
        Decoder::new(body, MAX_PIXEL_BYTES)
    }
}

fn block_kind_code(kind: BlockKind) -> f64 {
    match kind {
        BlockKind::Stored => 0.0,
        BlockKind::Fixed => 1.0,
        BlockKind::Dynamic => 2.0,
    }
}

/// The error's number on the JS side, where `codes.ts` words it.
fn error_code(err: InflateError) -> f64 {
    match err {
        InflateError::Bits => 0.0,
        InflateError::BadHuffmanCode => 1.0,
        InflateError::BadCodeLengths => 2.0,
        InflateError::BadDistance => 3.0,
        InflateError::BadSymbol => 4.0,
        InflateError::BadZlibHeader => 5.0,
        InflateError::ChecksumMismatch => 6.0,
        InflateError::OutputTooLarge => 7.0,
        InflateError::BadCheckpoint => 8.0,
    }
}

fn encode(step: &Step, out: &mut Vec<f64>) {
    let (kind, a, b) = match step.event {
        InflateEvent::BlockStart { kind, .. } => (0.0, block_kind_code(kind), 0.0),
        InflateEvent::Literal { byte } => (1.0, byte as f64, 0.0),
        InflateEvent::Match { distance, length } => (2.0, distance as f64, length as f64),
        InflateEvent::BlockEnd => (3.0, 0.0, 0.0),
    };
    out.extend_from_slice(&[
        kind,
        a,
        b,
        step.bit_start as f64,
        step.bit_end as f64,
        step.out_start as f64,
    ]);
}

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

/// The clean copy of a movie given as [`parse_movie`] takes it: the bytes
/// given, changed; each left-out body goes back after its header.
#[wasm_bindgen(js_name = cleanMovie)]
pub fn clean_movie(bytes: &[u8], gaps_at: &[f64]) -> CleanCopy {
    into_copy(clean_video_gapped(bytes, &gaps(gaps_at)))
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
            parsed.orientation = doc
                .facts
                .orientation
                .filter(|o| (1..=8).contains(o))
                .unwrap_or(1);
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
        Document::Video(doc) => {
            let mut parsed = flatten(&doc.tree);
            parsed.format = "video";
            parsed.dimensions = doc.width.zip(doc.height).map(|(w, h)| [w, h]);
            add_facts(&mut parsed, &doc.facts);
            parsed
        }
        Document::Pdf(doc) if cfg!(feature = "documents") => {
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
        Document::Zip(doc) if cfg!(feature = "documents") => {
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
        Document::Eml(doc) if cfg!(feature = "documents") => {
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
        Document::Cfb(doc) if cfg!(feature = "documents") => {
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
        Document::Wasm(doc) if cfg!(feature = "documents") => {
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

/// A copy of a file without what it reveals, or why there is none.
#[wasm_bindgen]
pub struct CleanCopy {
    bytes: Vec<u8>,
    removed: Vec<(String, u64)>,
    error: String,
    orientation: u16,
}

#[wasm_bindgen]
impl CleanCopy {
    /// The clean copy; empty when none was made.
    #[wasm_bindgen(getter)]
    pub fn bytes(&self) -> Vec<u8> {
        self.bytes.clone()
    }

    /// What was removed, as `what, bytes` pairs joined by U+001F.
    #[wasm_bindgen(getter)]
    pub fn removed(&self) -> String {
        let sep = SEPARATOR.to_string();
        self.removed
            .iter()
            .flat_map(|(what, n)| [sanitise(what), n.to_string()])
            .collect::<Vec<_>>()
            .join(&sep)
    }

    /// Why no copy was made; empty when one was.
    #[wasm_bindgen(getter)]
    pub fn error(&self) -> String {
        self.error.clone()
    }

    /// The orientation kept in a minimal EXIF block, or 0 when none was needed.
    #[wasm_bindgen(getter, js_name = orientationKept)]
    pub fn orientation_kept(&self) -> u16 {
        self.orientation
    }
}

/// Makes a copy of `bytes` without what it reveals about the people behind
/// it: see `hexscope_core::clean`. `notes` also empties Excel's and
/// PowerPoint's comments and speaker notes.
#[wasm_bindgen(js_name = cleanCopy)]
pub fn clean_copy(bytes: &[u8], notes: bool) -> CleanCopy {
    into_copy(clean_with(
        bytes,
        CleanOptions {
            comments_and_notes: notes,
        },
    ))
}

/// A copy, or why there is none, as the page reads it.
fn into_copy(
    result: Result<hexscope_core::clean::Cleaned, hexscope_core::clean::CleanError>,
) -> CleanCopy {
    match result {
        Ok(c) => CleanCopy {
            bytes: c.bytes,
            removed: c.removed.into_iter().map(|r| (r.what, r.bytes)).collect(),
            error: String::new(),
            orientation: c.orientation_kept.unwrap_or(0),
        },
        Err(e) => CleanCopy {
            bytes: Vec::new(),
            removed: Vec::new(),
            error: e.reason().to_string(),
            orientation: 0,
        },
    }
}

#[cfg(feature = "documents")]
fn page_areas(areas: &[f64]) -> Vec<(u32, [f64; 4])> {
    areas
        .as_chunks::<5>()
        .0
        .iter()
        .map(|[page, l, b, r, t]| (*page as u32, [*l, *b, *r, *t]))
        .collect()
}

/// A PDF's clean copy with chosen areas blacked out: `areas` is five
/// numbers per area — the page, counted from 1, then left, bottom, right
/// and top in its points. The JPEG pictures under them come painted, as
/// `jpegsUnder` asked: object numbers, each one's length, and their bytes
/// one after another. See `hexscope_core::clean::redact`.
#[cfg(feature = "documents")]
#[wasm_bindgen(js_name = redactCopy)]
pub fn redact_copy(
    bytes: &[u8],
    areas: &[f64],
    nums: &[u32],
    lens: &[u32],
    painted: &[u8],
) -> CleanCopy {
    let mut given = Vec::new();
    let mut at = 0usize;
    for (&num, &len) in nums.iter().zip(lens) {
        let end = at.saturating_add(len as usize);
        if let Some(b) = painted.get(at..end) {
            given.push((num, b.to_vec()));
        }
        at = end;
    }
    into_copy(hexscope_core::clean::redact(
        bytes,
        &page_areas(areas),
        &given,
    ))
}

/// The JPEG pictures chosen areas cover part of, which the caller paints:
/// for each, its object number, width, height, where its bytes start, how
/// many there are, how many rectangles, then each rectangle's `x0 y0 x1
/// y1` in pixels from the top left. See `hexscope_core::pdf::jpegs_under`.
#[cfg(feature = "documents")]
#[wasm_bindgen(js_name = jpegsUnder)]
pub fn jpegs_under(bytes: &[u8], areas: &[f64]) -> Vec<f64> {
    let mut out = Vec::new();
    for j in hexscope_core::pdf::jpegs_under(bytes, &page_areas(areas)) {
        out.extend([
            f64::from(j.num),
            f64::from(j.width),
            f64::from(j.height),
            j.start as f64,
            j.len as f64,
            j.rects.len() as f64,
        ]);
        out.extend(j.rects.iter().flatten().map(|&v| f64::from(v)));
    }
    out
}

/// Every page's visible text, glyph by glyph, for choosing what to black out.
#[cfg(feature = "documents")]
#[wasm_bindgen]
pub struct PageTexts {
    pages: Vec<hexscope_core::pdf::PageText>,
}

#[cfg(feature = "documents")]
#[wasm_bindgen]
impl PageTexts {
    /// Pages read.
    #[wasm_bindgen(getter)]
    pub fn count(&self) -> usize {
        self.pages.len()
    }

    /// A page's `[left, bottom, right, top]`.
    pub fn media(&self, i: usize) -> Vec<f64> {
        self.pages.get(i).map_or(Vec::new(), |p| p.media.to_vec())
    }

    /// Whether every supported form invocation on page `i` was inspected.
    pub fn complete(&self, i: usize) -> bool {
        self.pages.get(i).is_some_and(|page| page.complete)
    }

    /// A page's glyphs' areas, four numbers each.
    pub fn areas(&self, i: usize) -> Vec<f64> {
        self.pages
            .get(i)
            .map_or(Vec::new(), |p| p.glyphs.iter().flat_map(|g| g.0).collect())
    }

    /// A page's dark boxes and marks for redaction, four numbers each.
    pub fn boxes(&self, i: usize) -> Vec<f64> {
        self.pages
            .get(i)
            .map_or(Vec::new(), |p| p.boxes.iter().flatten().copied().collect())
    }

    /// A page's glyphs' text, joined by U+001F.
    pub fn texts(&self, i: usize) -> String {
        self.pages.get(i).map_or(String::new(), |p| {
            p.glyphs
                .iter()
                .map(|g| g.1.replace(SEPARATOR, ""))
                .collect::<Vec<_>>()
                .join(&SEPARATOR.to_string())
        })
    }
}

/// Reads every page's visible text: see `hexscope_core::pdf::page_texts`.
#[cfg(feature = "documents")]
#[wasm_bindgen(js_name = pageTexts)]
pub fn page_texts(bytes: &[u8]) -> PageTexts {
    PageTexts {
        pages: hexscope_core::pdf::page_texts(bytes),
    }
}

/// The pictures a PDF page draws, to show a scanned page while choosing
/// what to black out on it.
#[cfg(feature = "documents")]
#[wasm_bindgen]
pub struct PagePictures {
    pictures: Vec<hexscope_core::pdf::PagePicture>,
    /// The page each is on, counted from 1.
    pages: Vec<u32>,
}

#[cfg(feature = "documents")]
#[wasm_bindgen]
impl PagePictures {
    #[wasm_bindgen(getter)]
    pub fn count(&self) -> usize {
        self.pictures.len()
    }

    /// The page picture `i` is on, counted from 1.
    pub fn page(&self, i: usize) -> u32 {
        self.pages.get(i).copied().unwrap_or(0)
    }

    /// `[a b c d e f]`, from the picture's unit square to the page.
    pub fn matrix(&self, i: usize) -> Vec<f64> {
        self.pictures
            .get(i)
            .map_or(Vec::new(), |p| p.matrix.to_vec())
    }

    /// Where a JPEG's bytes start and how many there are; empty when the
    /// picture comes as pixels.
    pub fn jpeg(&self, i: usize) -> Vec<f64> {
        self.pictures
            .get(i)
            .and_then(|p| p.jpeg)
            .map_or(Vec::new(), |(s, n)| vec![s as f64, n as f64])
    }

    /// Width and height of a picture that comes as pixels.
    pub fn size(&self, i: usize) -> Vec<u32> {
        self.pictures
            .get(i)
            .and_then(|p| p.rgba.as_ref())
            .map_or(Vec::new(), |(w, h, _)| vec![*w, *h])
    }

    /// Its RGBA pixels, rows from the top.
    pub fn rgba(&self, i: usize) -> Vec<u8> {
        self.pictures
            .get(i)
            .and_then(|p| p.rgba.as_ref())
            .map_or(Vec::new(), |(_, _, px)| px.clone())
    }
}

/// See `hexscope_core::pdf::page_pictures`.
#[cfg(feature = "documents")]
#[wasm_bindgen(js_name = pagePictures)]
pub fn page_pictures(bytes: &[u8], page: u32) -> PagePictures {
    let pictures = hexscope_core::pdf::page_pictures(bytes, page);
    PagePictures {
        pages: vec![page; pictures.len()],
        pictures,
    }
}

/// The dark boxes pages fill, where there are enough to be a QR code drawn
/// in boxes: see `hexscope_core::pdf::shapes`.
#[cfg(feature = "documents")]
#[wasm_bindgen]
pub struct PdfShapes {
    pages: Vec<(u32, [f64; 4], Vec<[f64; 4]>)>,
}

#[cfg(feature = "documents")]
#[wasm_bindgen]
impl PdfShapes {
    #[wasm_bindgen(getter)]
    pub fn count(&self) -> usize {
        self.pages.len()
    }

    /// The page, counted from 1.
    pub fn page(&self, i: usize) -> u32 {
        self.pages.get(i).map_or(0, |p| p.0)
    }

    /// Its media box: left, bottom, right, top.
    pub fn media(&self, i: usize) -> Vec<f64> {
        self.pages.get(i).map_or(Vec::new(), |p| p.1.to_vec())
    }

    /// Its dark boxes, four numbers each: left, bottom, right, top.
    pub fn boxes(&self, i: usize) -> Vec<f64> {
        self.pages
            .get(i)
            .map_or(Vec::new(), |p| p.2.iter().flatten().copied().collect())
    }
}

#[cfg(feature = "documents")]
#[wasm_bindgen(js_name = pdfShapes)]
pub fn pdf_shapes(bytes: &[u8], pages: u32) -> PdfShapes {
    PdfShapes {
        pages: hexscope_core::pdf::shapes(bytes, pages),
    }
}

/// Every picture the first `pages` pages draw, each once: see
/// `hexscope_core::pdf::pictures`.
#[cfg(feature = "documents")]
#[wasm_bindgen(js_name = pdfPictures)]
pub fn pdf_pictures(bytes: &[u8], pages: u32) -> PagePictures {
    let (pages, pictures) = hexscope_core::pdf::pictures(bytes, pages)
        .into_iter()
        .unzip();
    PagePictures { pictures, pages }
}

/// A damaged file's copy with what survived put back in order.
#[wasm_bindgen]
pub struct RepairCopy {
    bytes: Vec<u8>,
    fixed: Vec<String>,
    error: String,
}

#[wasm_bindgen]
impl RepairCopy {
    /// The repaired copy; empty when none was made.
    #[wasm_bindgen(getter)]
    pub fn bytes(&self) -> Vec<u8> {
        self.bytes.clone()
    }

    /// What was done, in words, joined by U+001F.
    #[wasm_bindgen(getter)]
    pub fn fixed(&self) -> String {
        self.fixed
            .iter()
            .map(|f| sanitise(f))
            .collect::<Vec<_>>()
            .join(&SEPARATOR.to_string())
    }

    /// Why no copy was made; empty when one was.
    #[wasm_bindgen(getter)]
    pub fn error(&self) -> String {
        self.error.clone()
    }
}

/// Repairs what can be repaired: see `hexscope_core::repair`.
#[wasm_bindgen(js_name = repairCopy)]
pub fn repair_copy(bytes: &[u8]) -> RepairCopy {
    match hexscope_core::repair::repair(bytes) {
        Ok(r) => RepairCopy {
            bytes: r.bytes,
            fixed: r.fixed,
            error: String::new(),
        },
        Err(e) => RepairCopy {
            bytes: Vec::new(),
            fixed: Vec::new(),
            error: e.reason().to_string(),
        },
    }
}

/// Shannon entropy, 0 to 8 bits per byte, of up to `bins` windows covering
/// `bytes`, each at least 256 bytes. Separate from `parse` so the time shown
/// for parsing is the parse alone.
#[wasm_bindgen]
pub fn entropy(bytes: &[u8], bins: u32) -> Vec<f32> {
    window_entropy(bytes, bins as usize)
}

fn with_png(bytes: &[u8], doc: PngDocument) -> Parsed {
    let mut parsed = flatten(&doc.tree);
    parsed.preview = hexscope_core::png::preview::preview(bytes, &doc, PREVIEW_MAX)
        .map(|p| (p.width, p.height, p.rgba));
    parsed.format = "png";
    parsed.header_len = 2;
    parsed.trailer_len = 4;
    parsed.ihdr = doc.ihdr.map(|h| {
        [
            h.width,
            h.height,
            h.bit_depth as u32,
            h.color_type as u32,
            h.interlace as u32,
        ]
    });
    parsed.dimensions = doc.ihdr.map(|h| [h.width, h.height]);
    add_facts(&mut parsed, &doc.facts);
    parsed.trace = doc.trace.as_ref().map(|t| {
        [
            t.total_events as f64,
            t.literals as f64,
            t.matches as f64,
            t.output_bytes as f64,
        ]
    });
    parsed.idat_bytes = idat_payload_bytes(&doc.tree);

    for r in &doc.idat {
        let (start, end) = (r.start as usize, r.end() as usize);
        if let Some(payload) = bytes.get(start..end) {
            parsed.stream.extend_from_slice(payload);
        }
        parsed
            .segments
            .extend_from_slice(&[r.start as f64, r.len as f64]);
    }
    parsed.checkpoints = doc.trace.map(|t| t.checkpoints).unwrap_or_default();
    parsed.output = match doc.inflated {
        Some(output) => output,
        // Replay to the failure so the player has real bytes to draw.
        None => match parsed.body() {
            Some(body) => {
                let mut d = Decoder::new(body, MAX_PIXEL_BYTES);
                while let Some(Ok(_)) = d.step() {}
                d.into_output()
            }
            None => Vec::new(),
        },
    };
    parsed
}

fn add_facts(parsed: &mut Parsed, facts: &PhotoFacts) {
    let listed = [
        ("camera", &facts.camera),
        ("lens", &facts.lens),
        ("serial", &facts.serial),
        ("owner", &facts.owner),
        ("place", &facts.place),
        ("taken", &facts.taken),
        ("caption", &facts.caption),
        ("software", &facts.software),
        ("original", &facts.original),
        ("history", &facts.history),
        ("thumbnail", &facts.thumbnail),
        ("shutter", &facts.shutter),
        ("uptime", &facts.uptime),
        ("linked", &facts.linked),
        ("screenshot", &facts.screenshot),
        ("ai", &facts.ai),
        ("credentials", &facts.credentials),
        ("prompt", &facts.prompt),
    ];
    for (kind, fact) in listed {
        if let Some(f) = fact {
            parsed.facts.push((kind, sanitise(&f.text), f.node));
        }
    }
    parsed.location = facts.location.map(|l| {
        [
            l.latitude,
            l.longitude,
            l.altitude.unwrap_or(f64::NAN),
            l.node as f64,
        ]
    });
}

/// Replaces control characters so a corrupt chunk type or text payload cannot
/// inject separators, newlines or terminal escapes into the UI.
fn sanitise(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_control() { '\u{FFFD}' } else { c })
        .collect()
}

fn display(value: &Option<Value>) -> String {
    match value {
        None => String::new(),
        Some(Value::U64(n)) => n.to_string(),
        Some(Value::Text(t)) => sanitise(t),
        Some(Value::Bytes(n)) if *n == 1 => "1 byte".to_string(),
        Some(Value::Bytes(n)) => format!("{n} bytes"),
        Some(Value::Enum { raw, name }) => format!("{raw} ({name})"),
    }
}

fn kind_code(kind: NodeKind) -> u8 {
    match kind {
        NodeKind::Container => 0,
        NodeKind::Field => 1,
        NodeKind::Warning => 2,
        NodeKind::Error => 3,
    }
}

fn idat_payload_bytes(tree: &ParseTree) -> f64 {
    tree.nodes()
        .iter()
        .filter(|n| n.kind == NodeKind::Container && n.label == "IDAT")
        .map(|n| match n.value {
            Some(Value::Bytes(b)) => b as f64,
            _ => 0.0,
        })
        .sum()
}

/// The tree as parallel arrays. Format-specific extras are filled in by the
/// caller; for an unrecognised file this is everything there is.
pub fn flatten(tree: &ParseTree) -> Parsed {
    let nodes = tree.nodes();
    let sep = SEPARATOR.to_string();

    Parsed {
        starts: nodes.iter().map(|n| n.range.start as f64).collect(),
        lens: nodes.iter().map(|n| n.range.len as f64).collect(),
        parents: nodes
            .iter()
            .map(|n| n.parent.map_or(-1, |p| p as i32))
            .collect(),
        kinds: nodes.iter().map(|n| kind_code(n.kind)).collect(),
        labels: nodes
            .iter()
            .map(|n| sanitise(&n.label))
            .collect::<Vec<_>>()
            .join(&sep),
        values: nodes
            .iter()
            .map(|n| display(&n.value))
            .collect::<Vec<_>>()
            .join(&sep),
        ihdr: None,
        trace: None,
        idat_bytes: 0.0,
        stream: Vec::new(),
        segments: Vec::new(),
        output: Vec::new(),
        checkpoints: Vec::new(),
        format: "unknown",
        dimensions: None,
        facts: Vec::new(),
        verification: None,
        location: None,
        header_len: 0,
        trailer_len: 0,
        zip: None,
        extract_error: String::new(),
        docs: DocTables::default(),
        composition: Vec::new(),
        preview: None,
        blackouts: Vec::new(),
        blackout_texts: Vec::new(),
        pdf_source: Vec::new(),
        attachment_names: Vec::new(),
        source: Vec::new(),
        blocks: None,
        blocks_note: String::new(),
        orientation: 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn fixture(name: &str) -> Vec<u8> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../hexscope-core/tests/fixtures/pngsuite")
            .join(name);
        std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
    }

    #[test]
    fn every_array_describes_every_node() {
        let parsed = parse(&fixture("basn2c08.png"));
        let n = parsed.starts.len();

        assert!(n > 5);
        assert_eq!(parsed.lens.len(), n);
        assert_eq!(parsed.parents.len(), n);
        assert_eq!(parsed.kinds.len(), n);
        assert_eq!(parsed.labels.split(SEPARATOR).count(), n);
        assert_eq!(parsed.values.split(SEPARATOR).count(), n);
        assert_eq!(parsed.parents[0], -1, "node 0 is the root");
    }

    #[test]
    fn carries_file_level_facts() {
        let parsed = parse(&fixture("basn2c08.png"));
        assert_eq!(parsed.ihdr(), vec![32, 32, 8, 2, 0]);

        let trace = parsed.trace();
        assert_eq!(trace.len(), 4);
        assert!(trace[0] > 0.0, "some DEFLATE events were recorded");
        assert!(parsed.idat_bytes > 0.0);
    }

    #[test]
    fn a_non_png_still_flattens_to_a_tree() {
        let parsed = parse(b"definitely not a png");
        assert!(parsed.starts.len() >= 2);
        // Text is what the file is, not something wrong with it; an empty
        // file is.
        assert_eq!(parsed.kinds[1], 1, "the text is a field");
        assert_eq!(parse(b"").kinds[1], 3, "an empty file is an error");
        assert!(parsed.ihdr().is_empty());
        assert!(parsed.trace().is_empty());
    }

    /// Every step of a file, fetched the slow way: one call from the start.
    fn every_step(parsed: &Parsed) -> Vec<f64> {
        parsed.steps(0.0, u32::MAX / 8)
    }

    #[test]
    fn steps_cover_the_whole_stream_and_rebuild_the_output() {
        let parsed = parse(&fixture("basn2c08.png"));
        let steps = every_step(&parsed);
        let n = steps.len() / STEP_STRIDE;
        assert_eq!(n as f64, parsed.trace()[0], "one entry per traced event");

        // Replaying literals and back-references must rebuild the inflated
        // stream byte for byte: that is what the player draws.
        assert_eq!(rebuild(&steps), parsed.inflated());
    }

    #[test]
    fn a_window_anywhere_matches_the_full_sequence() {
        // Build a stream long enough to have many checkpoints, then check that
        // windows fetched by resuming agree with one straight pass.
        let parsed = parse(&fixture("basn2c08.png"));
        let all = every_step(&parsed);
        let n = all.len() / STEP_STRIDE;
        for from in [0, 1, n / 3, n / 2, n - 1] {
            let window = parsed.steps(from as f64, 5);
            let expected = &all[from * STEP_STRIDE..((from + 5).min(n)) * STEP_STRIDE];
            assert_eq!(window, expected, "window at step {from}");
        }
    }

    /// A zlib stream long enough to span many checkpoints.
    fn big_zlib() -> Vec<u8> {
        use flate2::{Compression, write::ZlibEncoder};
        use std::io::Write;

        let (w, h) = (300u32, 200u32);
        let mut raw = Vec::new();
        for y in 0..h {
            raw.push(0);
            for x in 0..w {
                raw.extend_from_slice(&[(x ^ y) as u8, (x * 3) as u8, (y * 7) as u8]);
            }
        }
        let mut enc = ZlibEncoder::new(Vec::new(), Compression::best());
        enc.write_all(&raw).unwrap();
        enc.finish().unwrap()
    }

    /// A 300x200 RGB PNG around `zlib`, split across 4 KB IDAT chunks.
    fn png_around(zlib: &[u8]) -> Vec<u8> {
        use hexscope_core::crc32::crc32;

        let chunk = |kind: &[u8; 4], data: &[u8]| {
            let mut out = (data.len() as u32).to_be_bytes().to_vec();
            out.extend_from_slice(kind);
            out.extend_from_slice(data);
            let mut c = kind.to_vec();
            c.extend_from_slice(data);
            out.extend_from_slice(&crc32(&c).to_be_bytes());
            out
        };
        let mut ihdr = 300u32.to_be_bytes().to_vec();
        ihdr.extend_from_slice(&200u32.to_be_bytes());
        ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);

        let mut png = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        png.extend(chunk(b"IHDR", &ihdr));
        for part in zlib.chunks(4096) {
            png.extend(chunk(b"IDAT", part));
        }
        png.extend(chunk(b"IEND", &[]));
        png
    }

    fn big_png() -> Vec<u8> {
        png_around(&big_zlib())
    }

    /// Replays literal and back-reference steps into the bytes they produce.
    fn rebuild(steps: &[f64]) -> Vec<u8> {
        let mut out: Vec<u8> = Vec::new();
        for s in steps.chunks(STEP_STRIDE) {
            match s[0] as u8 {
                1 => out.push(s[1] as u8),
                2 => {
                    let start = out.len() - s[1] as usize;
                    for i in 0..s[2] as usize {
                        let b = out[start + i];
                        out.push(b);
                    }
                }
                _ => {}
            }
        }
        out
    }

    #[test]
    fn windows_resumed_from_checkpoints_match_one_straight_pass() {
        let parsed = parse(&big_png());
        assert!(
            parsed.checkpoints.len() > 10,
            "need several checkpoints to test resume"
        );
        assert!(parsed.segments().len() / 2 > 3, "need several IDAT chunks");

        let all = every_step(&parsed);
        let n = all.len() / STEP_STRIDE;
        for from in (0..n).step_by(n / 40).chain([511, 512, 513, n - 1]) {
            let window = parsed.steps(from as f64, 7);
            let expected = &all[from * STEP_STRIDE..((from + 7).min(n)) * STEP_STRIDE];
            assert_eq!(window, expected, "window at step {from}");
        }
    }

    #[test]
    fn segments_map_stream_bytes_back_into_the_file() {
        let bytes = fixture("basn2c08.png");
        let parsed = parse(&bytes);
        let seg = parsed.segments();
        assert_eq!(seg.len() % 2, 0);
        let (start, len) = (seg[0] as usize, seg[1] as usize);
        assert_eq!(&bytes[start..start + len], &parsed.stream[..len]);
        // zlib header bytes: CMF 0x78 for a 32 KB window.
        assert_eq!(parsed.stream[0], 0x78);
    }

    #[test]
    fn a_truncated_stream_plays_up_to_where_it_breaks() {
        // Cut the compressed data in half: decoding produces real output, then
        // runs out of bits partway through a block. A fake trailer keeps the
        // stream long enough to have a body at all.
        let z = big_zlib();
        let mut cut = z[..z.len() / 2].to_vec();
        cut.extend_from_slice(&[0, 0, 0, 0]);
        let parsed = parse(&png_around(&cut));

        let steps = every_step(&parsed);
        let last = &steps[steps.len() - STEP_STRIDE..];
        assert_eq!(last[0] as u8, 4, "the final step reports the failure");
        assert!(last[4] >= last[3]);

        let output = parsed.inflated();
        assert!(
            output.len() > 10_000,
            "real output before the break, got {}",
            output.len()
        );
        // The partial output is exactly what the steps before the break
        // produce: the player draws one from the other.
        assert_eq!(rebuild(&steps), output);

        // Seeking into the middle still resumes from checkpoints recorded
        // before the failure, and agrees with the straight pass.
        let n = steps.len() / STEP_STRIDE;
        let from = n / 2;
        assert_eq!(
            parsed.steps(from as f64, 5),
            &steps[from * STEP_STRIDE..(from + 5) * STEP_STRIDE]
        );
    }

    #[test]
    fn a_corrupt_stream_still_rebuilds_its_output() {
        // Corruption early in the stream: whatever happens, the steps and the
        // output the player is handed must agree.
        let mut bytes = fixture("basn2c08.png");
        let start = parse(&bytes).segments()[0] as usize;
        for b in &mut bytes[start + 10..start + 20] {
            *b ^= 0x5A;
        }
        let parsed = parse(&bytes);
        let steps = every_step(&parsed);
        assert!(!steps.is_empty());
        assert_eq!(rebuild(&steps), parsed.inflated());
    }

    #[test]
    fn a_huge_count_is_not_trusted_for_allocation() {
        // Found on CI: preallocating count * STRIDE for u32::MAX steps asked
        // for 25 GB and aborted on Linux, where memory is not lazily
        // committed. The request must cost only what it actually returns.
        let parsed = parse(&fixture("basn2c08.png"));
        let steps = parsed.steps(0.0, u32::MAX);
        assert_eq!(steps.len() / STEP_STRIDE, parsed.trace()[0] as usize);
        assert!(steps.capacity() < 1 << 20, "capacity {}", steps.capacity());
    }

    #[test]
    fn a_photo_reports_its_format_facts_and_location() {
        let path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../hexscope-core/tests/fixtures/photo.jpg");
        let parsed = parse(&std::fs::read(path).unwrap());
        assert_eq!(parsed.format(), "jpeg");
        assert_eq!(parsed.dimensions(), vec![640, 480]);
        assert!(
            parsed.trace().is_empty(),
            "JPEG has no DEFLATE stream to play"
        );

        let facts: Vec<String> = parsed
            .facts()
            .split(SEPARATOR)
            .map(str::to_string)
            .collect();
        assert_eq!(facts.len() % 3, 0);
        let find = |kind: &str| {
            facts
                .chunks(3)
                .find(|t| t[0] == kind)
                .map(|t| t[1].clone())
                .unwrap_or_else(|| panic!("no {kind}"))
        };
        assert_eq!(find("serial"), "HX-000042");
        assert_eq!(find("camera"), "hexscope Sample Camera X1");

        let loc = parsed.location();
        assert!((loc[0] - 48.8584).abs() < 1e-4 && (loc[1] - 2.2945).abs() < 1e-4);
        // The node index points at the GPS IFD in the flattened tree.
        let labels: Vec<&str> = parsed.labels.split(SEPARATOR).collect();
        assert_eq!(labels[loc[3] as usize], "GPS IFD");
    }

    #[test]
    fn an_unknown_file_is_named_not_ignored() {
        let parsed = parse(b"\x1F\x8B\x08 not decoded");
        assert_eq!(parsed.format(), "unknown");
        assert!(parsed.labels.contains("gzip"));
        assert!(parsed.facts().is_empty() && parsed.location().is_empty());
    }

    #[test]
    fn a_pixel_leads_to_the_step_that_wrote_it_and_back() {
        let parsed = parse(&fixture("basn2c08.png"));
        assert_eq!(parsed.preview_size(), vec![32, 32]);
        assert_eq!(parsed.preview_pixels().len(), 32 * 32 * 4);
        // Every output byte has a step, and that step's bits lead back to it.
        for pos in [0.0, 1.0, 97.0, 500.0, 3000.0, 3103.0] {
            let s = parsed.locate(0, pos);
            assert_eq!(s.len(), 7, "{pos}");
            let (kind, len, bit_start, out_start) = (s[1], s[3], s[4], s[6]);
            let produced = if kind == 1.0 { 1.0 } else { len };
            assert!(
                out_start <= pos && pos < out_start + produced,
                "{pos}: {s:?}"
            );
            assert_eq!(parsed.locate(1, bit_start), s, "{pos}");
        }
        assert!(parsed.locate(0, 1e9).is_empty());
        assert!(parsed.locate(1, f64::NAN).is_empty());
    }

    #[test]
    fn a_photo_says_where_its_blocks_are() {
        let photo = std::fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../apps/web/public/samples/photo.jpg"),
        )
        .unwrap();
        let mut parsed = parse(&photo);
        let map = parsed.block_map();
        // Sequential, three components, one scan of all three: 40 × 30 MCUs of 16 × 16.
        assert_eq!(&map[..3], &[0.0, 3.0, 1.0]);
        assert_eq!(
            &map[3..13],
            &[7.0, 0.0, 63.0, 0.0, 0.0, 16.0, 16.0, 40.0, 30.0, 6.0]
        );
        assert_eq!(map[14], (40 * 30 + 1) as f64);
        assert_eq!(map.len(), 15 + 40 * 30 + 1);
        assert_eq!(parsed.blocks_note(), "");
        assert_eq!(parsed.block_map(), map, "found once, then kept");
        assert!(parse(b"\x89PNG\r\n\x1a\n").block_map().is_empty());
    }

    #[test]
    fn an_interlaced_png_has_a_picture_and_every_pass_row_filter() {
        let png = std::fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../hexscope-core/tests/fixtures/pngsuite/basi0g08.png"),
        )
        .unwrap();
        let parsed = parse(&png);
        assert_eq!(parsed.preview_size(), vec![32, 32]);
        // 32 × 32 in seven passes: 4, 4, 4, 8, 8, 16 and 16 rows.
        let filters = parsed.row_filters();
        assert_eq!(filters.len(), 60);
        assert!(filters.iter().all(|&f| f <= 4));
    }

    #[test]
    fn a_webassembly_module_says_who_built_it() {
        let module = std::fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../apps/web/public/samples/hello.wasm"),
        )
        .unwrap();
        let parsed = parse(&module);
        assert_eq!(parsed.format(), "wasm");
        let facts = parsed.facts();
        for kind in ["names", "language", "toolchain", "paths"] {
            assert!(facts.contains(kind), "{kind} in {facts}");
        }
        assert!(facts.contains("/Users/sample/projects/hello"));
    }

    #[test]
    fn a_photo_says_which_way_up_it_is() {
        let mut photo = std::fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../apps/web/public/samples/photo.jpg"),
        )
        .unwrap();
        assert_eq!(parse(&photo).orientation(), 1);
        // Its Orientation entry, turned to "rotate 90° clockwise".
        let entry: &[u8] = &[0x01, 0x12, 0x00, 0x03, 0, 0, 0, 1, 0, 1];
        let at = photo.windows(entry.len()).position(|w| w == entry).unwrap();
        photo[at + 9] = 6;
        assert_eq!(parse(&photo).orientation(), 6);
        assert_eq!(parse(b"\x89PNG\r\n\x1a\n").orientation(), 1);
    }

    #[test]
    fn a_video_says_where_it_was_recorded() {
        let mov = std::fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../hexscope-core/tests/fixtures/iphone.mov"),
        )
        .unwrap();
        let parsed = parse(&mov);
        assert_eq!(parsed.format(), "video");
        assert_eq!(parsed.dimensions(), vec![64, 48]);
        assert_eq!(parsed.location().len(), 4);
        let c = clean_copy(&mov, false);
        assert_eq!(c.error(), "");
        assert!(parse(&c.bytes()).location().is_empty());
    }

    #[test]
    fn a_pdf_says_who_wrote_it() {
        let pdf = std::fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../hexscope-core/tests/fixtures/report.pdf"),
        )
        .unwrap();
        let parsed = parse(&pdf);
        assert_eq!(parsed.format(), "pdf");
        assert!(parsed.facts().contains("Olena Koval"));
    }

    #[test]
    fn a_png_still_carries_its_deflate_stream() {
        let parsed = parse(&fixture("basn2c08.png"));
        assert_eq!(parsed.format(), "png");
        assert_eq!(parsed.dimensions(), vec![32, 32]);
        assert!(!parsed.trace().is_empty() && !parsed.segments().is_empty());
    }

    #[test]
    fn control_characters_cannot_break_the_join() {
        assert_eq!(sanitise("IH\u{1F}DR\n"), "IH\u{FFFD}DR\u{FFFD}");
        assert!(!sanitise("a\u{1F}b").contains(SEPARATOR));
    }

    #[test]
    fn values_render_for_humans() {
        assert_eq!(display(&Some(Value::Bytes(1))), "1 byte");
        assert_eq!(display(&Some(Value::Bytes(13))), "13 bytes");
        assert_eq!(
            display(&Some(Value::Enum {
                raw: 6,
                name: "RGBA"
            })),
            "6 (RGBA)"
        );
        assert_eq!(display(&None), "");
    }

    #[test]
    fn explain_agrees_with_steps() {
        let parsed = parse(&fixture("basn2c08.png"));
        let steps = every_step(&parsed);
        for (i, s) in steps.chunks(STEP_STRIDE).enumerate() {
            let e = parsed.explain(i as f64);
            assert_eq!(e[1], -1.0, "step {i} decodes");
            let n = e[2] as usize;
            assert_eq!(e.len(), 3 + n * PART_STRIDE);
            if n > 0 {
                assert_eq!(e[3 + 1], s[3], "step {i}: first part starts with the step");
                assert_eq!(
                    e[3 + (n - 1) * PART_STRIDE + 2],
                    s[4],
                    "step {i}: last part ends with it"
                );
            }
        }
    }

    #[test]
    fn tables_follow_the_block() {
        let parsed = parse(&fixture("basn2c08.png"));
        let t = parsed.tables(0.0);
        assert!(t[0] == 1.0 || t[0] == 2.0, "a coded block: {}", t[0]);
        assert!(t[4] > 0.0, "the literal/length table has groups");
    }

    #[test]
    fn explaining_nothing_is_empty() {
        let parsed = parse(&fixture("basn2c08.png"));
        for index in [-1.0, f64::NAN, 1e12] {
            assert!(parsed.explain(index).is_empty(), "{index}");
            assert!(parsed.tables(index).is_empty(), "{index}");
        }
    }

    fn docx(path: &str) -> Vec<u8> {
        let p = Path::new(env!("CARGO_MANIFEST_DIR")).join(path);
        std::fs::read(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
    }

    #[test]
    fn every_deflated_entry_plays_back_to_its_bytes() {
        let bytes = docx("../hexscope-core/tests/fixtures/report.docx");
        let mut parsed = parse(&bytes);
        assert_eq!(parsed.format(), "zip");
        let doc = hexscope_core::zip::parse_zip(&bytes);
        let entries = parsed.entries();
        assert_eq!(entries.len(), doc.entries.len() * ENTRY_STRIDE);

        for (i, e) in doc.entries.iter().enumerate() {
            assert_eq!(entries[i * ENTRY_STRIDE + 5], 1.0, "{} is playable", e.name);
            assert!(parsed.select_entry(i as u32), "{}", e.name);
            assert_eq!(parsed.stream_header(), 0.0);
            assert_eq!(parsed.segments(), [e.data.start as f64, e.data.len as f64]);
            let want = hexscope_core::zip::extract(&bytes, e, u64::MAX).unwrap();
            assert_eq!(rebuild(&every_step(&parsed)), want, "{}", e.name);
            assert_eq!(parsed.inflated(), want);
        }
    }

    #[test]
    fn what_cannot_be_played_is_refused() {
        let bytes = docx("../../apps/web/public/samples/report.docx");
        let mut parsed = parse(&bytes);
        let entries = parsed.entries();
        let n = entries.len() / ENTRY_STRIDE;
        let stored = (0..n)
            .find(|&i| entries[i * ENTRY_STRIDE + 1] == 0.0)
            .expect("the sample stores its photo");
        assert_eq!(entries[stored * ENTRY_STRIDE + 5], 0.0);
        assert!(!parsed.select_entry(stored as u32));
        assert!(!parsed.select_entry(n as u32));
        assert!(parsed.trace().is_empty(), "a refused entry changes nothing");

        let png = parse(&fixture("basn2c08.png"));
        assert_eq!(png.stream_header(), 2.0);
        assert!(png.entries().is_empty());
    }

    #[test]
    fn an_entry_opens_as_a_file_of_its_own() {
        let bytes = docx("../../apps/web/public/samples/report.docx");
        let mut parsed = parse(&bytes);
        let entries = parsed.entries();
        let n = entries.len() / ENTRY_STRIDE;
        let photo = (0..n)
            .find(|&i| entries[i * ENTRY_STRIDE + 1] == 0.0)
            .expect("the stored photo");
        assert_eq!(
            entries[photo * ENTRY_STRIDE + 6],
            1.0,
            "stored entries open"
        );

        let jpeg = parsed.extract_entry(photo as u32);
        assert!(jpeg.starts_with(&[0xFF, 0xD8, 0xFF]));
        assert_eq!(parsed.extract_error(), "");
        let inner = parse(&jpeg);
        assert_eq!(inner.format(), "jpeg");
        assert!(
            !inner.location().is_empty(),
            "the photo inside knows where it was taken"
        );

        let facts = parsed.facts();
        assert!(facts.contains("author\u{1F}Olena Koval"), "{facts}");

        assert!(parsed.extract_entry(n as u32).is_empty());
        assert_eq!(parsed.extract_error(), format!("there is no entry {n}"));
    }

    #[test]
    fn every_node_carries_its_explanation() {
        let parsed = parse(&fixture("basn2c08.png"));
        let ids = parsed.doc_ids();
        assert_eq!(ids.len(), parsed.labels().split(SEPARATOR).count());
        let texts: Vec<String> = parsed
            .doc_texts()
            .split(SEPARATOR)
            .map(str::to_string)
            .collect();
        let labels: Vec<String> = parsed
            .labels()
            .split(SEPARATOR)
            .map(str::to_string)
            .collect();
        let ihdr = labels.iter().position(|l| l == "IHDR").unwrap();
        assert!(texts[ids[ihdr] as usize].starts_with("The image header"));
        assert!(ids.iter().all(|&i| i >= 0), "every PNG node is explained");
        // A palette of 256 colours is 256 nodes and one explanation.
        let palette = parse(&fixture("basn3p08.png"));
        let shared = palette.doc_texts().split(SEPARATOR).count();
        assert!(
            shared * 4 < palette.doc_ids().len(),
            "explanations are shared, not repeated"
        );

        // A damaged CRC: a problem that is damage.
        let mut bytes = fixture("basn2c08.png");
        bytes[29] ^= 0xFF;
        let parsed = parse(&bytes);
        let labels: Vec<String> = parsed
            .labels()
            .split(SEPARATOR)
            .map(str::to_string)
            .collect();
        let crc = labels
            .iter()
            .position(|l| l.starts_with("CRC mismatch"))
            .unwrap();
        assert_eq!(parsed.doc_concerns()[parsed.doc_ids()[crc] as usize], 1);
        let cites: Vec<String> = parsed
            .doc_cites()
            .split(SEPARATOR)
            .map(str::to_string)
            .collect();
        assert_eq!(cites.len(), parsed.doc_concerns().len());
    }

    #[test]
    fn composition_covers_the_file_and_entropy_is_bounded() {
        let bytes = fixture("basn2c08.png");
        let parsed = parse(&bytes);
        let c = parsed.composition();
        assert_eq!(c.len() % SLICE_STRIDE, 0);
        let covered: f64 = c.chunks(SLICE_STRIDE).map(|s| s[1]).sum();
        assert_eq!(covered, bytes.len() as f64);
        assert!(
            c.chunks(SLICE_STRIDE).any(|s| s[2] == 0.0),
            "a PNG has picture bytes"
        );

        let h = entropy(&bytes, 1024);
        assert_eq!(h.len(), (bytes.len() / 256).max(1));
        assert!(h.iter().all(|&x| (0.0..=8.0).contains(&x)));
    }

    #[test]
    fn a_clean_copy_of_the_sample_photo_reveals_nothing() {
        let photo = std::fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../apps/web/public/samples/photo.jpg"),
        )
        .unwrap();
        let c = clean_copy(&photo, false);
        assert_eq!(c.error(), "");
        assert!(c.removed().starts_with("EXIF"));
        let again = parse(&c.bytes());
        assert!(again.facts().is_empty());
        assert!(again.location().is_empty());

        let refused = clean_copy(b"\x1F\x8B\x08 not cleaned", false);
        assert!(refused.bytes().is_empty());
        assert!(refused.error().contains("WebAssembly modules only"));
        let bare = clean_copy(&fixture("basn2c08.png"), false);
        assert!(bare.error().contains("nothing in it"));
    }

    #[test]
    fn verifies_the_clean_copy_using_the_source_parse() {
        let photo = std::fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../apps/web/public/samples/photo.jpg"),
        )
        .unwrap();
        let source = parse(&photo);
        let copy = clean_copy(&photo, false);
        let bytes = copy.bytes();

        let report = source.verify_copy(&bytes);

        assert!(report.contains("\"schema_version\":1"), "{report}");
        assert!(report.contains("\"kind\":\"location\""), "{report}");
        assert!(report.contains("\"removed\":["), "{report}");
        assert!(report.contains("\"present\":[]"), "{report}");
        assert!(report.contains("\"kind\":\"lens\""), "{report}");
        assert!(
            report.contains("\"reason\":\"coverage_incomplete\""),
            "{report}"
        );
    }

    #[test]
    fn reports_an_unchanged_copy_as_still_present() {
        let photo = std::fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../apps/web/public/samples/photo.jpg"),
        )
        .unwrap();
        let source = parse(&photo);

        let report = source.verify_copy(&photo);

        assert!(report.contains("\"reason\":\"still_present\""), "{report}");
        assert!(!report.contains("\"removed\":[{"), "{report}");
    }

    #[test]
    fn skips_a_copy_over_ten_mib_without_reparsing_it() {
        let photo = std::fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../apps/web/public/samples/photo.jpg"),
        )
        .unwrap();
        let source = parse(&photo);
        let bytes = vec![0; 10 * 1024 * 1024 + 1];

        let report = source.verify_copy(&bytes);

        assert!(
            report.contains("\"reason\":\"verification_skipped\""),
            "{report}"
        );
        assert!(report.contains("\"unchecked\":[{"), "{report}");
    }

    #[test]
    fn a_blacked_out_pdf_says_where_its_boxes_and_their_text_are() {
        let p = parse(&docx("../hexscope-core/tests/fixtures/redacted.pdf"));
        let b = p.blackouts();
        // One page, A4, three boxes over three pieces of text, three labels.
        assert_eq!(b[..8], [1.0, 0.0, 0.0, 595.0, 842.0, 3.0, 3.0, 3.0]);
        // Then page 2: the mark for redaction, the name under it, and the
        // words either side of it on the line.
        assert_eq!(b[44..52], [2.0, 0.0, 0.0, 595.0, 842.0, 1.0, 1.0, 2.0]);
        assert_eq!(b.len(), 44 + 8 + 4 + 4 + 2 * 4);
        let texts: Vec<String> = p
            .blackout_texts()
            .split(SEPARATOR)
            .map(String::from)
            .collect();
        assert_eq!(
            texts,
            [
                "Olena Koval",
                "+380 67 123 4567",
                "EUR 48,000",
                "Claimant:",
                "Phone:",
                "Settlement:",
                "Petro Ivanenko",
                "Signed in Kyiv on 3 March 2026 by",
                "."
            ]
        );
        // Each text lies inside the box drawn over it.
        for i in 0..3 {
            let bx = &b[8 + i * 4..8 + i * 4 + 4];
            let tx = &b[20 + i * 4..20 + i * 4 + 4];
            assert!(bx[0] <= tx[0] && tx[2] <= bx[2] + 1.0, "{bx:?} {tx:?}");
        }
        assert!(
            parse(&docx("../hexscope-core/tests/fixtures/report.pdf"))
                .blackouts()
                .is_empty()
        );
    }
}
