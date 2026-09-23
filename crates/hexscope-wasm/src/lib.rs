//! Browser bridge for `hexscope-core`.
//!
//! The parse tree crosses into JavaScript as parallel arrays — one entry per
//! node, node 0 is the root — rather than one JS object per node. A 10 MB PNG
//! has thousands of nodes, and building that many objects through the binding
//! layer costs far more than copying a handful of typed arrays.

#![forbid(unsafe_code)]

use hexscope_core::inflate::{BlockKind, Checkpoint, Decoder, InflateEvent, Step};
use hexscope_core::model::{NodeKind, ParseTree, Value};
use hexscope_core::png::{PngDocument, parse_png};
use wasm_bindgen::prelude::*;

/// Separates entries in the joined label and value strings. It is a control
/// character, and every string is sanitised of control characters before
/// joining, so it can never appear inside an entry.
pub const SEPARATOR: char = '\u{1F}';

/// Numbers per step in the array [`Parsed::steps`] returns.
pub const STEP_STRIDE: usize = 6;

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
    inflated: Option<Vec<u8>>,
    checkpoints: Vec<Checkpoint>,
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

    /// The decompressed stream (filtered scanlines), or empty if it failed.
    #[wasm_bindgen(getter)]
    pub fn inflated(&self) -> Vec<u8> {
        self.inflated.clone().unwrap_or_default()
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

        let mut out = Vec::with_capacity(wanted);
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
}

impl Parsed {
    /// The DEFLATE data: the zlib stream minus its 2-byte header and 4-byte
    /// Adler-32 trailer — exactly what the core decompressed.
    fn body(&self) -> Option<&[u8]> {
        let end = self.stream.len().checked_sub(4)?;
        self.stream.get(2..end)
    }

    /// A decoder positioned at or shortly before step `from`: resumed from the
    /// nearest checkpoint when the stream decoded cleanly, so seeking anywhere
    /// in a large file replays at most one checkpoint interval. A stream that
    /// failed has no complete output to resume against, so it replays from
    /// the start.
    fn decoder_at<'a>(&'a self, body: &'a [u8], from: u64) -> Decoder<'a> {
        if let Some(inflated) = &self.inflated {
            let nearest = self.checkpoints.partition_point(|c| c.event_index <= from);
            if let Some(cp) = nearest.checked_sub(1).and_then(|i| self.checkpoints.get(i))
                && let Ok(d) = Decoder::resume(body, u64::MAX, inflated, cp)
            {
                return d;
            }
        }
        Decoder::new(body, u64::MAX)
    }
}

fn encode(step: &Step, out: &mut Vec<f64>) {
    let (kind, a, b) = match step.event {
        InflateEvent::BlockStart { kind, .. } => {
            let k = match kind {
                BlockKind::Stored => 0.0,
                BlockKind::Fixed => 1.0,
                BlockKind::Dynamic => 2.0,
            };
            (0.0, k, 0.0)
        }
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

/// Parses a file. Never throws: damage is reported as warning and error nodes.
#[wasm_bindgen]
pub fn parse(bytes: &[u8]) -> Parsed {
    let doc = parse_png(bytes);
    let mut parsed = flatten(&doc);

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
    parsed.inflated = doc.inflated;
    parsed
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

pub fn flatten(doc: &PngDocument) -> Parsed {
    let nodes = doc.tree.nodes();
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
        ihdr: doc.ihdr.map(|h| {
            [
                h.width,
                h.height,
                h.bit_depth as u32,
                h.color_type as u32,
                h.interlace as u32,
            ]
        }),
        trace: doc.trace.as_ref().map(|t| {
            [
                t.total_events as f64,
                t.literals as f64,
                t.matches as f64,
                t.output_bytes as f64,
            ]
        }),
        idat_bytes: idat_payload_bytes(&doc.tree),
        stream: Vec::new(),
        segments: Vec::new(),
        inflated: None,
        checkpoints: Vec::new(),
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
        assert_eq!(out, parsed.inflated());
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

    /// A PNG big enough to span many checkpoints and several IDAT chunks.
    fn big_png() -> Vec<u8> {
        use flate2::{Compression, write::ZlibEncoder};
        use hexscope_core::crc32::crc32;
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
        let z = enc.finish().unwrap();

        let chunk = |kind: &[u8; 4], data: &[u8]| {
            let mut out = (data.len() as u32).to_be_bytes().to_vec();
            out.extend_from_slice(kind);
            out.extend_from_slice(data);
            let mut c = kind.to_vec();
            c.extend_from_slice(data);
            out.extend_from_slice(&crc32(&c).to_be_bytes());
            out
        };
        let mut ihdr = w.to_be_bytes().to_vec();
        ihdr.extend_from_slice(&h.to_be_bytes());
        ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);

        let mut png = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        png.extend(chunk(b"IHDR", &ihdr));
        for part in z.chunks(4096) {
            png.extend(chunk(b"IDAT", part));
        }
        png.extend(chunk(b"IEND", &[]));
        png
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
    fn a_corrupt_stream_plays_up_to_where_it_breaks() {
        let mut bytes = fixture("basn2c08.png");
        let seg = parse(&bytes).segments();
        let start = seg[0] as usize;
        for b in &mut bytes[start + 10..start + 20] {
            *b ^= 0x5A;
        }
        let parsed = parse(&bytes);
        let steps = every_step(&parsed);
        let last = &steps[steps.len() - STEP_STRIDE..];
        // Either the DEFLATE data itself fails (kind 4), or it decodes to
        // garbage the Adler-32 check rejects — in which case steps still play,
        // they just produce the wrong bytes.
        assert!(!steps.is_empty());
        assert!(
            parsed.inflated().is_empty(),
            "a failed stream has no trusted output"
        );
        if last[0] as u8 == 4 {
            assert!(
                last[4] >= last[3],
                "failure position is not before the last good bit"
            );
        }
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
}
