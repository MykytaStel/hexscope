//! Browser bridge for `hexscope-core`.
//!
//! The parse tree crosses into JavaScript as parallel arrays — one entry per
//! node, node 0 is the root — rather than one JS object per node. A 10 MB PNG
//! has thousands of nodes, and building that many objects through the binding
//! layer costs far more than copying a handful of typed arrays.

#![forbid(unsafe_code)]

use hexscope_core::model::{NodeKind, ParseTree, Value};
use hexscope_core::png::{PngDocument, parse_png};
use wasm_bindgen::prelude::*;

/// Separates entries in the joined label and value strings. It is a control
/// character, and every string is sanitised of control characters before
/// joining, so it can never appear inside an entry.
pub const SEPARATOR: char = '\u{1F}';

/// A parsed file, flattened. Index `i` in every array describes node `i`.
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
}

/// Parses a file. Never throws: damage is reported as warning and error nodes.
#[wasm_bindgen]
pub fn parse(bytes: &[u8]) -> Parsed {
    flatten(&parse_png(bytes))
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
