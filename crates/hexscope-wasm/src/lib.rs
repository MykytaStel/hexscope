//! Browser bridge for `hexscope-core`.
//!
//! The parse tree crosses into JavaScript as parallel arrays — one entry per
//! node, node 0 is the root — rather than one JS object per node. A 10 MB PNG
//! has thousands of nodes, and building that many objects through the binding
//! layer costs far more than copying a handful of typed arrays.

#![forbid(unsafe_code)]

use hexscope_core::exif::PhotoFacts;
use hexscope_core::inflate::{
    BlockKind, Checkpoint, CheckpointSink, Decoder, InflateError, InflateEvent, Step, inflate,
};
use hexscope_core::model::{NodeKind, ParseTree, Value};
use hexscope_core::png::{MAX_PIXEL_BYTES, PngDocument};
use hexscope_core::zip::{ExtractError, ZipEntry, extract};
use hexscope_core::{Document, parse as parse_any};
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
    /// `[latitude, longitude, altitude or NaN, node]`.
    location: Option<[f64; 4]>,
    /// Bytes of wrapper around the DEFLATE data in `stream`: a zlib header
    /// and Adler-32 for PNG, nothing for a ZIP entry.
    header_len: usize,
    trailer_len: usize,
    zip: Option<ZipState>,
    /// Why the last `extract_entry` returned nothing.
    extract_error: String,
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
    match parse_any(bytes) {
        Document::Png(doc) => with_png(bytes, doc),
        Document::Jpeg(doc) => {
            let mut parsed = flatten(&doc.tree);
            parsed.format = "jpeg";
            parsed.dimensions = doc.width.zip(doc.height).map(|(w, h)| [w as u32, h as u32]);
            add_facts(&mut parsed, &doc.facts);
            parsed
        }
        Document::Zip(doc) => {
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
        Document::Unknown(tree) => flatten(&tree),
    }
}

fn with_png(bytes: &[u8], doc: PngDocument) -> Parsed {
    let mut parsed = flatten(&doc.tree);
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
        ("taken", &facts.taken),
        ("software", &facts.software),
        ("thumbnail", &facts.thumbnail),
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
        location: None,
        header_len: 0,
        trailer_len: 0,
        zip: None,
        extract_error: String::new(),
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
        assert_eq!(parsed.kinds[1], 3, "the signature failure is an error");
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
        let parsed = parse(b"%PDF-1.7 not an image");
        assert_eq!(parsed.format(), "unknown");
        assert!(parsed.labels.contains("PDF"));
        assert!(parsed.facts().is_empty() && parsed.location().is_empty());
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
}
