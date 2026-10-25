//! SVG: a drawing written as XML. What it says about the people behind it
//! lies beside the drawing, not in it:
//!
//! - the program's own attributes — Inkscape's `inkscape:` and `sodipodi:`
//!   ones name the file the drawing was saved as, where it was exported to,
//!   with its path on the author's computer, and the program's version;
//! - a `<metadata>` block of RDF or XMP: creator, title, dates, keywords;
//! - a comment naming the program that wrote it, as Illustrator and Sketch
//!   leave;
//! - Illustrator's private copy of the drawing (`i:pgf`), which can hold
//!   layers and objects the drawing does not show;
//! - pictures linked by their path on the author's computer, and photos
//!   embedded as base64, which keep their own EXIF.
//!
//! One pass over the tags, keeping a stack of open elements; only what is
//! worth naming becomes a node. Scripts are found and named, and stay in a
//! clean copy: they are the drawing's, not about its author.

pub(crate) mod docs;

use crate::clean::{CleanError, Cleaned};
use crate::facts::DocumentFact;
use crate::model::{ByteRange, NodeId, NodeKind, ParseTree, Value};
use crate::rtf::Edit;

const INKSCAPE: &str = "http://www.inkscape.org/namespaces/inkscape";
const SODIPODI: &str = "http://sodipodi.sourceforge.net/DTD/sodipodi-0.dtd";
/// Longest text kept for one fact, in bytes.
const MAX_TEXT: usize = 512;
/// Embedded pictures read for what they say, at most, and how large.
const MAX_PICTURES: usize = 16;
const MAX_PICTURE: usize = 32 << 20;

#[derive(Debug)]
pub struct SvgDocument {
    pub tree: ParseTree,
    pub facts: Vec<DocumentFact>,
    /// What a clean copy cuts or replaces, in the order it lies in the file.
    pub(crate) edits: Vec<Edit>,
}

/// Whether the file is an SVG: after an optional byte-order mark, XML
/// declaration, comments and DOCTYPE, its first element's local name is
/// `svg`.
pub fn is_svg(data: &[u8]) -> bool {
    let mut i = if data.starts_with(b"\xEF\xBB\xBF") {
        3
    } else {
        0
    };
    loop {
        while data.get(i).is_some_and(u8::is_ascii_whitespace) {
            i += 1;
        }
        let rest = data.get(i..).unwrap_or(&[]);
        let close = if rest.starts_with(b"<?") {
            find(data, i, b"?>").map(|e| e + 2)
        } else if rest.starts_with(b"<!--") {
            find(data, i, b"-->").map(|e| e + 3)
        } else if rest.starts_with(b"<!") {
            doctype_end(data, i)
        } else {
            if !rest.starts_with(b"<") || rest.starts_with(b"</") {
                return false;
            }
            let Some(name_end) = rest[1..]
                .iter()
                .position(|&c| c.is_ascii_whitespace() || c == b'>' || c == b'/')
            else {
                return false;
            };
            let name = &rest[1..name_end + 1];
            let Some(local_start) = name.iter().rposition(|&c| c == b':') else {
                return name == b"svg";
            };
            return name.get(local_start + 1..) == Some(b"svg")
                && local_start > 0
                && !name[..local_start].contains(&b':');
        };
        match close {
            Some(next) => i = next,
            None => return false,
        }
    }
}

/// Where `needle` starts at or after `from`.
fn find(data: &[u8], from: usize, needle: &[u8]) -> Option<usize> {
    data.get(from..)?
        .windows(needle.len())
        .position(|w| w == needle)
        .map(|p| from + p)
}

/// The end of a `<!DOCTYPE …>` at `at`, past an internal subset in brackets.
fn doctype_end(data: &[u8], at: usize) -> Option<usize> {
    let gt = find(data, at, b">")?;
    match find(data, at, b"[") {
        Some(open) if open < gt => {
            let close = find(data, open, b"]")?;
            find(data, close, b">").map(|e| e + 1)
        }
        _ => Some(gt + 1),
    }
}

/// One attribute of a tag: its name, where the whole of it lies (with the
/// space before it, so cutting it leaves the tag whole), and its value.
struct Attr<'a> {
    name: &'a str,
    whole: (usize, usize),
    value: (usize, usize),
}

/// The attributes of a start tag, read from `i`, just past its name, to
/// `end`, its `>` (or the `/` before it).
fn attributes(data: &[u8], mut i: usize, end: usize) -> Vec<Attr<'_>> {
    let mut out = Vec::new();
    while i < end {
        let space = i;
        while i < end && data[i].is_ascii_whitespace() {
            i += 1;
        }
        let name_at = i;
        while i < end && !data[i].is_ascii_whitespace() && !matches!(data[i], b'=' | b'/' | b'>') {
            i += 1;
        }
        if i == name_at {
            i += 1;
            continue;
        }
        let name = std::str::from_utf8(&data[name_at..i]).unwrap_or("");
        while i < end && data[i].is_ascii_whitespace() {
            i += 1;
        }
        if data.get(i) != Some(&b'=') {
            continue;
        }
        i += 1;
        while i < end && data[i].is_ascii_whitespace() {
            i += 1;
        }
        let Some(&quote) = data.get(i).filter(|q| matches!(q, b'"' | b'\'')) else {
            continue;
        };
        let Some(close) = data[i + 1..end].iter().position(|&c| c == quote) else {
            break;
        };
        let value = (i + 1, i + 1 + close);
        i = value.1 + 1;
        out.push(Attr {
            name,
            whole: (space, i),
            value,
        });
    }
    out
}

/// Where a tag that opens at `at` closes: its `>`, outside quoted values.
fn tag_end(data: &[u8], at: usize) -> Option<usize> {
    let mut quote = 0u8;
    for (i, &c) in data.iter().enumerate().skip(at) {
        if quote != 0 {
            if c == quote {
                quote = 0;
            }
        } else if c == b'"' || c == b'\'' {
            quote = c;
        } else if c == b'>' {
            return Some(i);
        }
    }
    None
}

/// An element still open: its name, where its start tag begins, and the
/// node it became, if any.
struct Open<'a> {
    name: &'a str,
    start: usize,
    node: Option<NodeId>,
}

/// What a fact of each kind has said so far, and the node it points to.
#[derive(Default)]
struct Said {
    lines: Vec<(&'static str, String, NodeId)>,
}

impl Said {
    fn add(&mut self, kind: &'static str, raw: &str, node: NodeId) {
        // Its words, one space apart.
        let mut text = String::with_capacity(raw.len());
        let mut space = false;
        for c in raw.chars() {
            if c.is_whitespace() {
                space = !text.is_empty();
            } else {
                if space {
                    text.push(' ');
                    space = false;
                }
                text.push(c);
            }
        }
        if text.is_empty() {
            return;
        }
        match self.lines.iter_mut().find(|(k, ..)| *k == kind) {
            Some((_, said, _)) if said.split(", ").any(|s| s == text) => {}
            Some((_, said, _)) => {
                if said.len() < MAX_TEXT {
                    said.push_str(", ");
                    said.push_str(&text);
                }
            }
            None => self.lines.push((kind, text, node)),
        }
    }
}

fn local(name: &str) -> &str {
    name.bytes()
        .rposition(|b| b == b':')
        .map_or(name, |i| &name[i + 1..])
}

/// A path on someone's computer, rather than an address or a name beside the drawing.
fn local_path(value: &str) -> bool {
    let b = value.as_bytes();
    value.starts_with("file:")
        || value.starts_with('/')
        || value.starts_with("\\\\")
        || (b.len() > 2
            && b[0].is_ascii_alphabetic()
            && b[1] == b':'
            && matches!(b[2], b'\\' | b'/'))
}

/// The program a comment names, as `Generator: Adobe Illustrator 27.5.0,
/// SVG Export Plug-In` or `Generator: Sketch 52.5 (67469) - http://…` do.
fn generator(comment: &str) -> Option<&str> {
    let rest = comment.split_once("Generator:")?.1;
    let end = [", SVG Export", " - http", "-->"]
        .iter()
        .filter_map(|s| rest.find(s))
        .min()
        .unwrap_or(rest.len());
    Some(rest[..end].trim()).filter(|s| !s.is_empty())
}

pub fn parse_svg(data: &[u8]) -> SvgDocument {
    let mut tree = ParseTree::new();
    let root = tree.add(
        None,
        "SVG",
        ByteRange::new(0, data.len() as u64),
        NodeKind::Container,
        None,
    );
    let text = |a: usize, b: usize| {
        crate::zip::office::decode(&String::from_utf8_lossy(data.get(a..b).unwrap_or(&[])))
    };
    let mut said = Said::default();
    let mut edits: Vec<Edit> = Vec::new();
    let mut open: Vec<Open> = Vec::new();
    // The prefixes bound to Inkscape's and Sodipodi's namespaces.
    let mut prefixes: Vec<String> = vec!["inkscape".into(), "sodipodi".into()];
    let (mut svg_node, mut scripts, mut handlers, mut pictures) = (None, 0usize, 0usize, 0usize);
    let mut damaged = false;
    let mut i = if data.starts_with(b"\xEF\xBB\xBF") {
        3
    } else {
        0
    };

    while let Some(lt) = find(data, i, b"<") {
        // The nearest open element that is a node, or the file outside the drawing.
        let parent = open.iter().rev().find_map(|o| o.node).unwrap_or(root);
        let rest = &data[lt..];
        // Comments, declarations and CDATA: where each ends.
        if rest.starts_with(b"<!--") {
            let Some(end) = find(data, lt + 4, b"-->").map(|e| e + 3) else {
                damaged = true;
                break;
            };
            let body = String::from_utf8_lossy(&data[lt..end]);
            if open.len() <= 1 || generator(&body).is_some() {
                let node = tree.add(
                    Some(parent),
                    "comment",
                    ByteRange::new(lt as u64, (end - lt) as u64),
                    NodeKind::Field,
                    Some(Value::Text(body[4..body.len() - 3].trim().to_string())),
                );
                if let Some(program) = generator(&body) {
                    said.add("software", program, node);
                    let end = if data.get(end) == Some(&b'\n') {
                        end + 1
                    } else {
                        end
                    };
                    edits.push(Edit {
                        range: ByteRange::new(lt as u64, (end - lt) as u64),
                        with: Vec::new(),
                        what: "the comment naming the program",
                    });
                }
            }
            i = end;
            continue;
        }
        if rest.starts_with(b"<![CDATA[") {
            i = find(data, lt, b"]]>").map_or(data.len(), |e| e + 3);
            continue;
        }
        if rest.starts_with(b"<?") || rest.starts_with(b"<!") {
            let end = if rest.starts_with(b"<?") {
                find(data, lt, b"?>").map(|e| e + 2)
            } else {
                doctype_end(data, lt)
            };
            let Some(end) = end else {
                damaged = true;
                break;
            };
            if open.is_empty() {
                let label = if rest.starts_with(b"<?xml ") {
                    "XML declaration"
                } else if rest.starts_with(b"<?") {
                    "processing instruction"
                } else {
                    "DOCTYPE"
                };
                tree.add(
                    Some(root),
                    label,
                    ByteRange::new(lt as u64, (end - lt) as u64),
                    NodeKind::Field,
                    None,
                );
            }
            i = end;
            continue;
        }
        let Some(gt) = tag_end(data, lt) else {
            damaged = true;
            break;
        };
        i = gt + 1;
        // An end tag closes what is open, up to the element it names.
        if rest.starts_with(b"</") {
            let name = std::str::from_utf8(&data[lt + 2..gt]).unwrap_or("").trim();
            let Some(depth) = open.iter().rposition(|o| o.name == name) else {
                damaged = true;
                continue;
            };
            if depth + 1 != open.len() {
                damaged = true;
            }
            while open.len() > depth + 1 {
                open.pop();
            }
            if let Some(o) = open.pop() {
                close(data, &mut tree, &mut edits, &o, gt + 1, &prefixes);
            }
            continue;
        }
        let name_end = (lt + 1..gt)
            .find(|&p| data[p].is_ascii_whitespace() || data[p] == b'/')
            .unwrap_or(gt);
        let name = std::str::from_utf8(&data[lt + 1..name_end]).unwrap_or("");
        let empty = data[gt - 1] == b'/';
        let attrs = attributes(data, name_end, if empty { gt - 1 } else { gt });
        let in_metadata = open.iter().any(|o| local(o.name) == "metadata");
        let range = ByteRange::new(lt as u64, (gt + 1 - lt) as u64);

        // The drawing itself: its namespaces say which prefixes are Inkscape's.
        let mut node = None;
        if svg_node.is_none() && local(name) == "svg" {
            for a in &attrs {
                if let Some(p) = a.name.strip_prefix("xmlns:") {
                    let uri = text(a.value.0, a.value.1);
                    if (uri == INKSCAPE || uri == SODIPODI) && !prefixes.iter().any(|q| q == p) {
                        prefixes.push(p.to_string());
                    }
                }
            }
            node = Some(tree.add(Some(root), "svg element", range, NodeKind::Container, None));
            svg_node = node;
        } else if matches!(local(name), "metadata" | "script" | "pgf" | "image")
            || name.ends_with(":namedview")
        {
            let label = match local(name) {
                "pgf" => "Illustrator data",
                "namedview" => "sodipodi:namedview",
                l => l,
            };
            node = Some(tree.add(Some(parent), label, range, NodeKind::Container, None));
        }
        let at = node.unwrap_or(parent);

        for a in &attrs {
            let value = text(a.value.0, a.value.1);
            let (prefix, attr) = a.name.split_once(':').unwrap_or(("", a.name));
            let program = prefixes.iter().any(|p| p == prefix);
            let kind = if program {
                match attr {
                    "docname" => Some("original"),
                    "export-filename" | "absref" if local_path(&value) => Some("links"),
                    "version" => Some("software"),
                    _ => None,
                }
            } else if in_metadata {
                match attr {
                    "CreatorTool" => Some("software"),
                    "CreateDate" => Some("created"),
                    "ModifyDate" => Some("modified"),
                    _ => None,
                }
            } else {
                None
            };
            let href = attr == "href" && (prefix.is_empty() || prefix == "xlink");
            if let Some(kind) = kind {
                let label = if program {
                    let canonical = if attr == "docname" || attr == "absref" {
                        "sodipodi"
                    } else {
                        "inkscape"
                    };
                    format!("{canonical}:{attr}")
                } else {
                    attr.to_string()
                };
                let field = tree.add(
                    Some(at),
                    label,
                    ByteRange::new(a.whole.0 as u64, (a.whole.1 - a.whole.0) as u64),
                    NodeKind::Field,
                    Some(Value::Text(value.clone())),
                );
                let shown = if kind == "software" && program {
                    format!("Inkscape {value}")
                } else {
                    value.clone()
                };
                said.add(kind, &shown, field);
            }
            if program {
                edits.push(Edit {
                    range: ByteRange::new(a.whole.0 as u64, (a.whole.1 - a.whole.0) as u64),
                    with: Vec::new(),
                    what: "Inkscape's attributes: the file's name, where it was exported, the version",
                });
            } else if attr.starts_with("on") && attr.len() > 2 {
                handlers += 1;
                if handlers == 1 {
                    tree.add(
                        Some(at),
                        "event handler",
                        ByteRange::new(a.whole.0 as u64, (a.whole.1 - a.whole.0) as u64),
                        NodeKind::Field,
                        Some(Value::Text(a.name.to_string())),
                    );
                }
            } else if href && local(name) == "image" {
                picture(
                    &mut tree,
                    &mut said,
                    &mut edits,
                    at,
                    (&value, a.value),
                    &mut pictures,
                );
            }
        }
        if local(name) == "script" {
            scripts += 1;
        }
        if local(name) == "pgf" {
            said.add(
                "embedded",
                "Illustrator's own copy of the drawing, which can hold layers and objects the drawing does not show",
                at,
            );
        }
        let o = Open {
            name,
            start: lt,
            node,
        };
        if empty {
            close(data, &mut tree, &mut edits, &o, gt + 1, &prefixes);
        } else {
            open.push(o);
        }
    }

    // What a metadata block says, read from the text of its elements.
    metadata(data, &tree, &mut said);
    if scripts + handlers > 0 {
        let node = tree
            .nodes()
            .iter()
            .find(|n| matches!(n.label.as_str(), "script" | "event handler"))
            .map_or(root, |n| n.id);
        let mut parts = Vec::new();
        if scripts > 0 {
            parts.push(format!(
                "{scripts} {}",
                if scripts == 1 { "script" } else { "scripts" }
            ));
        }
        if handlers > 0 {
            parts.push(format!(
                "{handlers} event {}",
                if handlers == 1 { "handler" } else { "handlers" }
            ));
        }
        said.add(
            "scripts",
            &format!(
                "{}, which run when the drawing is opened on its own in a browser — not when a page shows it as a picture",
                parts.join(" and ")
            ),
            node,
        );
    }
    if damaged || !open.is_empty() || svg_node.is_none() {
        tree.error(
            root,
            "the tags do not close: the drawing is cut short or damaged",
            ByteRange::new(data.len() as u64, 0),
        );
    }
    let facts = said
        .lines
        .into_iter()
        .map(|(kind, text, node)| DocumentFact {
            kind,
            text: crate::zip::office::cap(text),
            node,
        })
        .collect();
    SvgDocument { tree, facts, edits }
}

/// An element that closed at `end`: its node gets its full range, and a
/// clean copy cuts it if it is about the author rather than the drawing.
fn close(
    data: &[u8],
    tree: &mut ParseTree,
    edits: &mut Vec<Edit>,
    o: &Open,
    end: usize,
    prefixes: &[String],
) {
    let range = ByteRange::new(o.start as u64, (end - o.start) as u64);
    if let Some(node) = o.node {
        tree.set_range(node, range);
    }
    let (prefix, _) = o.name.split_once(':').unwrap_or(("", o.name));
    let what = match local(o.name) {
        "metadata" => "metadata: who made it, its title, dates and keywords",
        "namedview" if prefixes.iter().any(|p| p == prefix) => {
            "Inkscape's settings for its window and pages"
        }
        "pgf" => "Illustrator's own copy of the drawing",
        _ => return,
    };
    // What was to be cut inside it goes with it.
    while edits
        .last()
        .is_some_and(|e| e.range.start >= o.start as u64)
    {
        edits.pop();
    }
    // An element on lines of its own goes with its indent and line end.
    let mut start = o.start;
    while start > 0 && matches!(data[start - 1], b' ' | b'\t') {
        start -= 1;
    }
    let end = if (start == 0 || data[start - 1] == b'\n') && data.get(end) == Some(&b'\n') {
        end + 1
    } else {
        start = o.start;
        end
    };
    edits.push(Edit {
        range: ByteRange::new(start as u64, (end - start) as u64),
        with: Vec::new(),
        what,
    });
}

/// A picture an `<image>` links to or carries: a path on the author's
/// computer is named and cut to its file name; a photo embedded as base64
/// is read for what it says, and its copy cleaned as a photo of its own.
fn picture(
    tree: &mut ParseTree,
    said: &mut Said,
    edits: &mut Vec<Edit>,
    at: NodeId,
    (value, (start, end)): (&str, (usize, usize)),
    pictures: &mut usize,
) {
    let range = ByteRange::new(start as u64, (end - start) as u64);
    if local_path(value) {
        let name = &value[value
            .bytes()
            .rposition(|b| b == b'/' || b == b'\\')
            .map_or(0, |i| i + 1)..];
        said.add("links", value.trim_start_matches("file://"), at);
        edits.push(Edit {
            range,
            with: name.as_bytes().to_vec(),
            what: "the folders of linked pictures (their names stay)",
        });
        return;
    }
    let Some((head, body)) = value.split_once(";base64,") else {
        return;
    };
    if !head.starts_with("data:image/")
        || *pictures >= MAX_PICTURES
        || body.len() > MAX_PICTURE / 3 * 4
    {
        return;
    }
    *pictures += 1;
    let bytes = crate::eml::base64(body.as_bytes());
    let Some(facts) = photo_facts(&bytes) else {
        return;
    };
    let Some(says) = crate::zip::office::photo_says(&facts) else {
        return;
    };
    let node = tree.add(
        Some(at),
        "embedded picture",
        range,
        NodeKind::Field,
        Some(Value::Bytes(bytes.len() as u64)),
    );
    let kind = if facts.location.is_some() {
        "photoplace"
    } else {
        "photo"
    };
    said.add(kind, &format!("picture {}: {says}", *pictures), node);
    let Ok(copy) = crate::clean::clean(&bytes) else {
        return;
    };
    // The cut is the data's own bytes, so it lies inside the attribute; the
    // value is a data URL with no entities in it.
    let base = start + head.len() + ";base64,".len();
    let mut with = Vec::with_capacity(copy.bytes.len() / 3 * 4 + 4);
    encode(&copy.bytes, &mut with);
    edits.push(Edit {
        range: ByteRange::new(base as u64, (end - base) as u64),
        with,
        what: "photos inside it, cleaned as photos of their own",
    });
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RasterKind {
    Jpeg,
    Png,
    Webp,
    Tiff,
    Heif,
}

/// Identify supported raster bytes before parsing them. Do not route an
/// embedded data URL through the general document dispatcher: it can recurse
/// into SVGs or invoke unrelated parsers for mislabeled payloads.
fn raster_kind(data: &[u8]) -> Option<RasterKind> {
    if data.starts_with(&crate::jpeg::MAGIC) {
        Some(RasterKind::Jpeg)
    } else if data.starts_with(b"\x89PNG") {
        Some(RasterKind::Png)
    } else if crate::webp::is_webp(data) {
        Some(RasterKind::Webp)
    } else if crate::tiff::is_tiff(data) {
        Some(RasterKind::Tiff)
    } else if crate::heif::is_heif(data) {
        Some(RasterKind::Heif)
    } else {
        None
    }
}

/// Metadata from a still image the core can both read and clean. Nested SVGs
/// are deliberately excluded: this path is for raster photos only.
fn photo_facts(data: &[u8]) -> Option<crate::exif::PhotoFacts> {
    match raster_kind(data)? {
        RasterKind::Jpeg => Some(crate::jpeg::parse_jpeg(data).facts),
        RasterKind::Png => Some(crate::png::parse_png(data).facts),
        RasterKind::Webp => Some(crate::webp::parse_webp(data).facts),
        RasterKind::Tiff => Some(crate::tiff::parse_tiff_file(data).facts),
        RasterKind::Heif => Some(crate::heif::parse_heif(data).facts),
    }
}

/// Base64, as a data URL carries it.
fn encode(bytes: &[u8], out: &mut Vec<u8>) {
    const ABC: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    for chunk in bytes.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0u32, |n, (i, &b)| n | (b as u32) << (16 - 8 * i));
        for i in 0..4 {
            out.push(if i <= chunk.len() {
                ABC[(n >> (18 - 6 * i) & 63) as usize]
            } else {
                b'='
            });
        }
    }
}

/// The facts a `<metadata>` block's text gives: who made the drawing — a
/// creator, contributor or publisher, as RDF and XMP name them — its title,
/// dates, description and keywords.
fn metadata(data: &[u8], tree: &ParseTree, said: &mut Said) {
    for block in tree.nodes().iter().filter(|n| n.label == "metadata") {
        let (start, end) = (block.range.start as usize, block.range.end() as usize);
        let mut path: Vec<&str> = Vec::new();
        let mut i = start;
        while let Some(lt) = find(data, i, b"<").filter(|&lt| lt < end) {
            // The text before this tag belongs to the element open around it.
            let words = crate::zip::office::decode(&String::from_utf8_lossy(&data[i..lt]));
            if !words.trim().is_empty() {
                let leaf = path.last().copied().unwrap_or("");
                let within = |n: &str| path.contains(&n);
                let kind = if within("creator") || within("contributor") {
                    Some("author")
                } else if within("publisher") {
                    Some("company")
                } else if within("subject") {
                    Some("keywords")
                } else {
                    match leaf {
                        "title" => Some("title"),
                        "date" | "CreateDate" => Some("created"),
                        "ModifyDate" => Some("modified"),
                        "description" => Some("description"),
                        "CreatorTool" => Some("software"),
                        _ => None,
                    }
                };
                if let Some(kind) = kind {
                    said.add(kind, &words, block.id);
                }
            }
            if data[lt..].starts_with(b"<!--")
                || data[lt..].starts_with(b"<?")
                || data[lt..].starts_with(b"<![")
            {
                i = find(data, lt, b">").map_or(end, |e| e + 1);
                continue;
            }
            let Some(gt) = tag_end(data, lt) else {
                break;
            };
            let tag = std::str::from_utf8(&data[lt + 1..gt]).unwrap_or("");
            if let Some(name) = tag.strip_prefix('/') {
                if let Some(p) = path.iter().rposition(|n| *n == local(name.trim())) {
                    path.truncate(p);
                }
            } else if !tag.ends_with('/') {
                let name = tag
                    .split(|c: char| c.is_ascii_whitespace())
                    .next()
                    .unwrap_or("");
                path.push(local(name));
            }
            i = gt + 1;
        }
    }
}

/// A copy without what the drawing says about its author: the edits the
/// parse found, made in one pass.
pub(crate) fn clean_svg(data: &[u8], doc: &SvgDocument) -> Result<Cleaned, CleanError> {
    if doc.tree.nodes().iter().any(|n| n.kind == NodeKind::Error) {
        return Err(CleanError::Damaged);
    }
    crate::clean::apply(data, &doc.edits)
}

#[cfg(test)]
mod tests;
