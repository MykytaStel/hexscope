//! PDF documents (ISO 32000-1:2008), read in file order.
//!
//! A reader that follows the cross-reference table sees only the objects the
//! latest table points at. This one scans the bytes instead, so it shows what
//! the file holds rather than what a viewer draws: every object, including
//! the ones an incremental update replaced, which are still there.

pub(crate) mod docs;
mod facts;
mod lexer;

use crate::model::{ByteRange, NodeId, NodeKind, ParseTree, Value};
use crate::zip::DocumentFact;
use lexer::{Entry, Item, Lexer, Obj};

/// Cross-reference entries past this many get no node of their own.
const MAX_XREF_NODES: usize = 4096;
/// The header may start this far into the file (7.5.2, and what readers
/// accept in practice).
const HEADER_WINDOW: usize = 1024;

#[derive(Debug)]
pub struct PdfDocument {
    pub tree: ParseTree,
    /// From the header, such as `1.7`.
    pub version: Option<String>,
    /// Sections ending in `%%EOF`: the original, then one per update.
    pub revisions: usize,
    /// Its strings are encrypted, so none were read as facts.
    pub encrypted: bool,
    pub facts: Vec<DocumentFact>,
}

/// `%PDF-` within the first kilobyte.
pub fn is_pdf(data: &[u8]) -> bool {
    header_at(data).is_some()
}

fn header_at(data: &[u8]) -> Option<usize> {
    find(&data[..data.len().min(HEADER_WINDOW)], b"%PDF-")
}

pub(crate) fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

/// One object as read, for what is worked out once the scan is done.
#[derive(Debug)]
pub(crate) struct ObjRec {
    pub num: u32,
    pub start: u64,
    pub node: NodeId,
    pub value: Obj,
    /// The node of each top-level key, such as `Author`.
    pub keys: Vec<(String, NodeId)>,
    pub stream: Option<(ByteRange, NodeId)>,
}

#[derive(Debug, Default)]
pub(crate) struct Ctx {
    pub objects: Vec<ObjRec>,
    /// Trailer dictionaries and cross-reference stream dictionaries, in
    /// file order.
    pub trailers: Vec<Obj>,
    xref_at: Vec<u64>,
    startxrefs: Vec<(NodeId, u64, ByteRange)>,
    xref_nodes: usize,
    endobj: NextOf,
    endstream: NextOf,
    items: NextItem,
}

/// The next line that starts an item, remembering the last search the way
/// [`NextOf`] does.
#[derive(Debug, Default)]
struct NextItem {
    searched: Option<(usize, usize)>,
}

impl NextItem {
    fn at(&mut self, data: &[u8], from: usize) -> usize {
        if let Some((start, found)) = self.searched
            && start <= from
            && from <= found
        {
            return found;
        }
        let found = resync(data, from);
        self.searched = Some((from, found));
        found
    }
}

/// Where a keyword next occurs, remembering the last search: the scan only
/// moves forward, so a file of a million unclosed objects is still read in
/// one pass rather than a million.
#[derive(Debug, Default)]
struct NextOf {
    searched: Option<(usize, Option<usize>)>,
}

impl NextOf {
    fn at(&mut self, data: &[u8], needle: &[u8], from: usize) -> Option<usize> {
        if let Some((start, found)) = self.searched
            && start <= from
            && found.is_none_or(|f| f >= from)
        {
            return found;
        }
        let found = find(data.get(from..).unwrap_or_default(), needle).map(|i| from + i);
        self.searched = Some((from, found));
        found
    }
}

/// Parses a PDF. Never fails: damage is recorded as nodes.
pub fn parse_pdf(data: &[u8]) -> PdfDocument {
    let mut tree = ParseTree::new();
    let len = data.len();
    let root = tree.add(
        None,
        "PDF",
        ByteRange::new(0, len as u64),
        NodeKind::Container,
        None,
    );
    let ends = revision_ends(data);
    let mut revs = Revisions {
        root,
        ends: &ends,
        nodes: Vec::new(),
    };
    let mut ctx = Ctx::default();
    let mut version = None;
    let mut lx = Lexer::new(data, 0);

    if let Some(h) = header_at(data) {
        if h > 0 {
            tree.warning(
                root,
                "data before the PDF header",
                ByteRange::new(0, h as u64),
            );
        }
        let line = line_end(data, h);
        let v = String::from_utf8_lossy(&data[h + 5..line])
            .trim()
            .to_string();
        let parent = revs.parent(&mut tree, h as u64);
        let node = tree.add(
            Some(parent),
            "header",
            span(h, line),
            NodeKind::Field,
            Some(Value::Text(format!("PDF {v}"))),
        );
        version = Some(v);
        lx.pos = line;
        lx.skip_ws();
        extend(&mut tree, node, lx.pos);
        // A comment of four or more high bytes tells transfer programs the
        // file is binary (7.5.2).
        if lx.at(b"%") && !lx.at(b"%%EOF") {
            let start = lx.pos;
            let end = line_end(data, start);
            if data[start + 1..end].iter().filter(|&&b| b >= 128).count() >= 4 {
                let node = tree.add(
                    Some(parent),
                    "binary marker",
                    span(start, end),
                    NodeKind::Field,
                    None,
                );
                lx.pos = end;
                lx.skip_ws();
                extend(&mut tree, node, lx.pos);
            }
        }
    }

    loop {
        lx.skip_ws();
        if lx.pos >= len {
            break;
        }
        let start = lx.pos;
        let parent = revs.parent(&mut tree, start as u64);
        let node = if lx.at(b"%%EOF") {
            lx.pos += 5;
            Some(tree.add(
                Some(parent),
                "%%EOF",
                span(start, lx.pos),
                NodeKind::Field,
                None,
            ))
        } else if lx.keyword(b"xref") {
            Some(xref_table(&mut tree, parent, &mut lx, start, &mut ctx))
        } else if lx.keyword(b"trailer") {
            Some(trailer(&mut tree, parent, &mut lx, start, &mut ctx))
        } else if lx.keyword(b"startxref") {
            Some(startxref(&mut tree, parent, &mut lx, start, &mut ctx))
        } else if let Some((num, gen_)) = lx.object_header() {
            Some(object(
                &mut tree, parent, &mut lx, start, num, gen_, &mut ctx,
            ))
        } else {
            None
        };
        match node {
            Some(node) => {
                lx.skip_ws();
                extend(&mut tree, node, lx.pos);
            }
            None => {
                let next = ctx.items.at(data, start + 1);
                let range = span(start, next);
                if ends.last().is_some_and(|&e| start as u64 >= e) {
                    tree.warning(root, "data after the end of the document", range);
                } else {
                    tree.error(parent, "bytes that are not part of any object", range);
                }
                lx.pos = next;
            }
        }
    }

    // A file cut short stops before its last `%%EOF`.
    let tail = ends.last().map_or(0, |&e| e as usize);
    let unfinished = tree.get(root).children.iter().any(|&c| {
        let n = tree.get(c);
        n.range.start as usize >= tail && n.kind != NodeKind::Warning && n.label != "header"
    }) || ends.is_empty();
    if unfinished && len > 0 {
        let last_line = data[..len - 1]
            .iter()
            .rposition(|&b| b == b'\n' || b == b'\r')
            .map_or(0, |i| i + 1);
        tree.error(
            root,
            "the file ends without %%EOF: it was cut short",
            span(last_line, len),
        );
    }

    check_startxrefs(&mut tree, &ctx);
    revs.finish(&mut tree);

    let encrypted = ctx.trailers.iter().any(|t| t.get("Encrypt").is_some());
    let linearized = ctx
        .objects
        .first()
        .is_some_and(|o| o.value.get("Linearized").is_some());
    // A linearized file has two sections from the start: one for the first
    // page, one for the rest. Neither is an edit.
    let original = if linearized { 2 } else { 1 };
    let edits = ends.len().saturating_sub(original);
    let mut facts = facts::collect(data, &mut tree, &ctx, encrypted);
    if edits > 0
        && let Some(&node) = revs.nodes.get(original)
    {
        let text = if edits == 1 {
            "once after it was first saved; the earlier version is still inside".to_string()
        } else {
            format!("{edits} times after it was first saved; the earlier versions are still inside")
        };
        facts::insert_update(
            &mut facts,
            DocumentFact {
                kind: "updates",
                text,
                node,
            },
        );
    }

    let n = ctx.objects.len();
    let mut summary = match &version {
        Some(v) => format!("PDF {v}"),
        None => "PDF".to_string(),
    };
    summary.push_str(&format!(
        " · {n} {}",
        if n == 1 { "object" } else { "objects" }
    ));
    if ends.len() > 1 {
        summary.push_str(&format!(" · {} revisions", ends.len()));
    }
    tree.set_value(root, Some(Value::Text(summary)));

    PdfDocument {
        tree,
        version,
        revisions: ends.len(),
        encrypted,
        facts,
    }
}

/// Where each revision ends: just past every `startxref N %%EOF`. A stream
/// could hold those bytes by chance, and would then only split a revision
/// in two; nothing is read differently.
fn revision_ends(data: &[u8]) -> Vec<u64> {
    let mut ends = Vec::new();
    let mut from = 0;
    while let Some(i) = find(&data[from..], b"startxref") {
        let at = from + i;
        from = at + 9;
        let mut lx = Lexer::new(data, from);
        lx.skip_ws();
        if lx.uint().is_none() {
            continue;
        }
        lx.skip_ws();
        if lx.at(b"%%EOF") {
            lx.pos += 5;
            // The end-of-line marker after %%EOF belongs to it.
            if lx.at(b"\r\n") {
                lx.pos += 2;
            } else if lx.at(b"\n") || lx.at(b"\r") {
                lx.pos += 1;
            }
            ends.push(lx.pos as u64);
            from = lx.pos;
        }
    }
    ends
}

/// The top level, grouped by revision when there is more than one.
struct Revisions<'a> {
    root: NodeId,
    ends: &'a [u64],
    nodes: Vec<NodeId>,
}

impl Revisions<'_> {
    /// The parent for an item starting at `at`, creating revisions in order,
    /// before any of their children.
    fn parent(&mut self, tree: &mut ParseTree, at: u64) -> NodeId {
        if self.ends.len() < 2 {
            return self.root;
        }
        let k = self.ends.partition_point(|&e| e <= at);
        if k >= self.ends.len() {
            return self.root;
        }
        while self.nodes.len() <= k {
            let i = self.nodes.len();
            let node = tree.add(
                Some(self.root),
                format!("revision {}", i + 1),
                ByteRange::new(at, 0),
                NodeKind::Container,
                Some(Value::Text(if i == 0 {
                    "the original".to_string()
                } else {
                    "an update".to_string()
                })),
            );
            self.nodes.push(node);
        }
        self.nodes[k]
    }

    /// Each revision spans its children.
    fn finish(&self, tree: &mut ParseTree) {
        for &r in &self.nodes {
            let n = tree.get(r);
            let ranges = n.children.iter().map(|&c| tree.get(c).range);
            let start = ranges.clone().map(|r| r.start).min();
            let end = ranges.map(|r| r.end()).max();
            if let (Some(s), Some(e)) = (start, end) {
                let objects = n
                    .children
                    .iter()
                    .filter(|&&c| tree.get(c).label.starts_with("object "))
                    .count();
                let what = if r == self.nodes[0] {
                    "the original"
                } else {
                    "an update"
                };
                tree.set_range(r, ByteRange::new(s, e - s));
                tree.set_value(
                    r,
                    Some(Value::Text(format!(
                        "{what} · {objects} {}",
                        if objects == 1 { "object" } else { "objects" }
                    ))),
                );
            }
        }
    }
}

fn span(start: usize, end: usize) -> ByteRange {
    ByteRange::new(start as u64, end.saturating_sub(start) as u64)
}

fn line_end(data: &[u8], from: usize) -> usize {
    data[from..]
        .iter()
        .position(|&b| b == b'\n' || b == b'\r')
        .map_or(data.len(), |i| from + i)
}

/// Stretches a top-level item over the whitespace after it, so that bytes
/// between items belong to something.
fn extend(tree: &mut ParseTree, node: NodeId, end: usize) {
    let start = tree.get(node).range.start;
    tree.set_range(
        node,
        ByteRange::new(start, (end as u64).saturating_sub(start)),
    );
}

/// A value that may not run past `bound`: a string or dictionary left open
/// stops at the next object, not at the end of the file, so a file of many
/// unclosed values is still read in one pass.
fn bounded_value(lx: &mut Lexer, bound: usize) -> Option<Item> {
    let mut bounded = Lexer::new(&lx.data[..bound.min(lx.data.len())], lx.pos);
    let value = bounded.value();
    lx.pos = bounded.pos;
    value
}

/// The next line that starts something this reader knows.
fn resync(data: &[u8], from: usize) -> usize {
    let mut i = from;
    while i < data.len() {
        let line_start = i == 0 || matches!(data[i - 1], b'\n' | b'\r');
        if line_start {
            let mut lx = Lexer::new(data, i);
            let known = lx.at(b"%%EOF")
                || lx.keyword(b"xref")
                || lx.keyword(b"trailer")
                || lx.keyword(b"startxref")
                || lx.object_header().is_some();
            if known {
                return i;
            }
        }
        i += 1;
    }
    data.len()
}

fn object(
    tree: &mut ParseTree,
    parent: NodeId,
    lx: &mut Lexer,
    start: usize,
    num: u32,
    gen_: u16,
    ctx: &mut Ctx,
) -> NodeId {
    let header = span(start, lx.pos);
    let node = tree.add(
        Some(parent),
        format!("object {num} {gen_}"),
        header,
        NodeKind::Container,
        None,
    );
    lx.skip_ws();
    let value_start = lx.pos;
    let next = ctx.items.at(lx.data, value_start + 1);
    let endobj = ctx.endobj.at(lx.data, b"endobj", value_start);
    let value = bounded_value(lx, next.min(endobj.unwrap_or(next)));
    let Some(item) = value else {
        // Up to its endobj, when that comes before anything else.
        let stop = endobj.filter(|&e| e < next).map_or(next, |e| e + 6);
        tree.error(
            node,
            format!("object {num} {gen_} cannot be read"),
            span(value_start, stop),
        );
        lx.pos = stop;
        return node;
    };

    let keys = match &item.obj {
        Obj::Dict(entries) => emit_entries(tree, node, entries),
        other => {
            tree.add(
                Some(node),
                "value",
                item.range,
                NodeKind::Field,
                Some(render(other)),
            );
            Vec::new()
        }
    };
    lx.skip_ws();
    let stream = if matches!(item.obj, Obj::Dict(_)) && lx.keyword(b"stream") {
        let s = stream(tree, node, lx, &item.obj, &mut ctx.endstream);
        lx.skip_ws();
        Some(s)
    } else {
        None
    };
    if !lx.keyword(b"endobj") {
        tree.warning(node, format!("object {num} {gen_} has no endobj"), header);
    }
    tree.set_value(
        node,
        Some(Value::Text(kind_of(&item.obj, stream.is_some()))),
    );
    if item.obj.get("Type").and_then(Obj::name) == Some("XRef") {
        ctx.trailers.push(item.obj.clone());
    }
    ctx.objects.push(ObjRec {
        num,
        start: start as u64,
        node,
        value: item.obj,
        keys,
        stream,
    });
    node
}

/// What an object is, in a few words.
fn kind_of(obj: &Obj, stream: bool) -> String {
    let ty = obj.get("Type").and_then(Obj::name);
    let sub = obj.get("Subtype").and_then(Obj::name);
    match (ty, sub) {
        (Some("XRef"), _) => return "cross-reference stream".into(),
        (Some("ObjStm"), _) => return "object stream".into(),
        (Some("Metadata"), _) => return "XMP metadata".into(),
        (Some("EmbeddedFile"), _) => return "embedded file".into(),
        (_, Some("Image")) if stream => return "image".into(),
        _ => {}
    }
    if obj.get("Linearized").is_some() {
        return "linearization".into();
    }
    match (obj, ty, stream) {
        (_, Some(t), true) => format!("{t} stream"),
        (_, None, true) => "stream".into(),
        (_, Some(t), false) => t.to_string(),
        (Obj::Dict(_), None, false) => "dictionary".into(),
        (Obj::Array(_), ..) => "array".into(),
        (Obj::Str(_), ..) => "string".into(),
        (Obj::Name(_), ..) => "name".into(),
        (Obj::Int(_) | Obj::Real(_), ..) => "number".into(),
        (Obj::Ref(..), ..) => "reference".into(),
        (Obj::Bool(_), ..) => "boolean".into(),
        (Obj::Null, ..) => "null".into(),
    }
}

/// A stream's data: `/Length` bytes when that lands on `endstream`,
/// otherwise up to the next `endstream`.
fn stream(
    tree: &mut ParseTree,
    node: NodeId,
    lx: &mut Lexer,
    dict: &Obj,
    endstream: &mut NextOf,
) -> (ByteRange, NodeId) {
    let data = lx.data;
    // CRLF or LF; a lone CR breaks the rule but is common enough to accept.
    if lx.at(b"\r\n") {
        lx.pos += 2;
    } else if lx.at(b"\n") || lx.at(b"\r") {
        lx.pos += 1;
    }
    let start = lx.pos;
    let declared = dict
        .get("Length")
        .and_then(Obj::int)
        .and_then(|n| usize::try_from(n).ok());
    let lands = |end: usize| {
        let mut l = Lexer::new(data, end);
        if l.at(b"\r\n") {
            l.pos += 2;
        } else if l.at(b"\n") || l.at(b"\r") {
            l.pos += 1;
        }
        l.at(b"endstream").then_some(l.pos + 9)
    };
    let direct = declared
        .and_then(|n| start.checked_add(n))
        .filter(|&end| end <= data.len())
        .and_then(|end| lands(end).map(|after| (end, after)));
    let (end, after, found) = match direct {
        Some((end, after)) => (end, after, true),
        None => match endstream.at(data, b"endstream", start) {
            Some(k) => {
                let mut end = k;
                if end > start && data[end - 1] == b'\n' {
                    end -= 1;
                }
                if end > start && data[end - 1] == b'\r' {
                    end -= 1;
                }
                (end, k + 9, true)
            }
            None => (data.len(), data.len(), false),
        },
    };
    let range = span(start, end);
    let data_node = tree.add(
        Some(node),
        "stream data",
        range,
        NodeKind::Field,
        Some(Value::Bytes(range.len)),
    );
    if !found {
        tree.error(
            node,
            "the stream has no endstream: the file was cut short",
            range,
        );
    } else if let Some(n) = declared.filter(|&n| n != end - start) {
        tree.warning(
            node,
            format!(
                "the stream's /Length says {n} bytes, but it holds {}",
                end - start
            ),
            range,
        );
    }
    lx.pos = after;
    (range, data_node)
}

/// A dictionary's keys as nodes; returns each top-level key's node.
fn emit_entries(tree: &mut ParseTree, parent: NodeId, entries: &[Entry]) -> Vec<(String, NodeId)> {
    let mut keys = Vec::with_capacity(entries.len());
    for e in entries {
        let node = emit(tree, parent, &format!("/{}", e.key), e.range, &e.value);
        keys.push((e.key.clone(), node));
        // What runs or hides, in the entries that say so.
        let obj = &e.value.obj;
        if e.key == "JS" {
            tree.warning(node, "the document runs JavaScript", e.range);
        } else if e.key == "S" && obj.name() == Some("Launch") {
            tree.warning(node, "an action that starts a program", e.range);
        } else if e.key == "Type" && obj.name() == Some("EmbeddedFile") {
            tree.warning(node, "a file carried inside the document", e.range);
        }
    }
    keys
}

fn emit(
    tree: &mut ParseTree,
    parent: NodeId,
    label: &str,
    range: ByteRange,
    item: &Item,
) -> NodeId {
    match &item.obj {
        Obj::Dict(entries) => {
            let n = entries.len();
            let node = tree.add(
                Some(parent),
                label,
                range,
                NodeKind::Container,
                Some(Value::Text(format!(
                    "{n} {}",
                    if n == 1 { "entry" } else { "entries" }
                ))),
            );
            emit_entries(tree, node, entries);
            node
        }
        Obj::Array(items) if items.iter().any(|i| matches!(i.obj, Obj::Dict(_))) => {
            let node = tree.add(
                Some(parent),
                label,
                range,
                NodeKind::Container,
                Some(render(&item.obj)),
            );
            for (i, it) in items.iter().enumerate() {
                if matches!(it.obj, Obj::Dict(_)) {
                    emit(tree, node, &format!("item {}", i + 1), it.range, it);
                }
            }
            node
        }
        other => tree.add(
            Some(parent),
            label,
            range,
            NodeKind::Field,
            Some(render(other)),
        ),
    }
}

/// Longest rendering of an array.
const MAX_RENDERED: usize = 60;

fn render(obj: &Obj) -> Value {
    match obj {
        Obj::Int(i) if *i >= 0 => Value::U64(*i as u64),
        Obj::Ref(n, g) => Value::Text(format!("→ object {n} {g}")),
        Obj::Str(s) => Value::Text(format!("“{}”", facts::cap(facts::text(s)))),
        other => Value::Text(short(other)),
    }
}

fn short(obj: &Obj) -> String {
    match obj {
        Obj::Null => "null".into(),
        Obj::Bool(b) => b.to_string(),
        Obj::Int(i) => i.to_string(),
        Obj::Real(r) => r.clone(),
        Obj::Name(n) => format!("/{n}"),
        Obj::Ref(n, g) => format!("{n} {g} R"),
        Obj::Str(s) => {
            let t = facts::text(s);
            if t.chars().count() > 20 {
                "(…)".into()
            } else {
                format!("({t})")
            }
        }
        Obj::Dict(e) => format!("<< {} >>", e.len()),
        Obj::Array(items) => {
            let mut out = String::from("[");
            for (i, it) in items.iter().enumerate() {
                let s = short(&it.obj);
                if out.len() + s.len() > MAX_RENDERED {
                    out.push_str(&format!(" … {} more", items.len() - i));
                    break;
                }
                if i > 0 {
                    out.push(' ');
                }
                out.push_str(&s);
            }
            out.push(']');
            out
        }
    }
}

fn xref_table(
    tree: &mut ParseTree,
    parent: NodeId,
    lx: &mut Lexer,
    start: usize,
    ctx: &mut Ctx,
) -> NodeId {
    ctx.xref_at.push(start as u64);
    let node = tree.add(
        Some(parent),
        "cross-reference table",
        span(start, lx.pos),
        NodeKind::Container,
        None,
    );
    let mut total = 0u64;
    'sections: loop {
        lx.skip_ws();
        let sub_start = lx.pos;
        let Some(first) = lx.uint() else { break };
        lx.skip_ws();
        let Some(count) = lx.uint() else {
            lx.pos = sub_start;
            break;
        };
        lx.skip_ws();
        // `N G obj` is an object after a table with no trailer.
        if lx.at(b"obj") {
            lx.pos = sub_start;
            break;
        }
        let label = match count {
            0 => format!("objects from {first}, none"),
            1 => format!("object {first}"),
            _ => format!("objects {first}–{}", first.saturating_add(count - 1)),
        };
        let sub = tree.add(
            Some(node),
            label,
            span(sub_start, lx.pos),
            NodeKind::Container,
            Some(Value::Text(format!(
                "{count} {}",
                if count == 1 { "entry" } else { "entries" }
            ))),
        );
        let mut read = 0u64;
        while read < count {
            lx.skip_ws();
            let e_start = lx.pos;
            let entry = (|| {
                let offset = lx.uint()?;
                lx.skip_ws();
                let _generation = lx.uint()?;
                lx.skip_ws();
                match lx.word() {
                    b"n" => Some(Some(offset)),
                    b"f" => Some(None),
                    _ => None,
                }
            })();
            let n = first.saturating_add(read);
            let Some(entry) = entry else {
                // The rest of the line goes with it, and the table ends there.
                let end = line_end(lx.data, e_start)
                    .max(e_start + 1)
                    .min(lx.data.len());
                tree.error(
                    sub,
                    format!("cross-reference entry {n} cannot be read"),
                    span(e_start, end),
                );
                lx.pos = end;
                extend(tree, sub, lx.pos);
                break 'sections;
            };
            if ctx.xref_nodes < MAX_XREF_NODES {
                ctx.xref_nodes += 1;
                tree.add(
                    Some(sub),
                    format!("entry {n}"),
                    span(e_start, lx.pos),
                    NodeKind::Field,
                    Some(Value::Text(match entry {
                        Some(at) => format!("in use, at {at}"),
                        None => "free".into(),
                    })),
                );
            }
            read += 1;
        }
        extend(tree, sub, lx.pos);
        total += read;
    }
    tree.set_value(
        node,
        Some(Value::Text(format!(
            "{total} {}",
            if total == 1 { "entry" } else { "entries" }
        ))),
    );
    node
}

fn trailer(
    tree: &mut ParseTree,
    parent: NodeId,
    lx: &mut Lexer,
    start: usize,
    ctx: &mut Ctx,
) -> NodeId {
    let node = tree.add(
        Some(parent),
        "trailer",
        span(start, lx.pos),
        NodeKind::Container,
        None,
    );
    lx.skip_ws();
    let value_start = lx.pos;
    let next = ctx.items.at(lx.data, value_start + 1);
    match bounded_value(lx, next) {
        Some(Item {
            obj: obj @ Obj::Dict(_),
            ..
        }) => {
            emit_entries(tree, node, obj.entries());
            tree.set_value(
                node,
                Some(Value::Text(format!("{} entries", obj.entries().len()))),
            );
            ctx.trailers.push(obj);
        }
        _ => {
            tree.error(node, "the trailer cannot be read", span(value_start, next));
            lx.pos = next;
        }
    }
    node
}

fn startxref(
    tree: &mut ParseTree,
    parent: NodeId,
    lx: &mut Lexer,
    start: usize,
    ctx: &mut Ctx,
) -> NodeId {
    let node = tree.add(
        Some(parent),
        "startxref",
        span(start, lx.pos),
        NodeKind::Container,
        None,
    );
    lx.skip_ws();
    let at = lx.pos;
    match lx.uint() {
        Some(offset) => {
            let range = span(at, lx.pos);
            tree.add(
                Some(node),
                "offset",
                range,
                NodeKind::Field,
                Some(Value::U64(offset)),
            );
            tree.set_value(node, Some(Value::U64(offset)));
            ctx.startxrefs.push((node, offset, range));
        }
        None => {
            tree.error(node, "startxref has no offset", span(start, lx.pos));
        }
    }
    node
}

/// Every `startxref` must point at a cross-reference table or stream.
fn check_startxrefs(tree: &mut ParseTree, ctx: &Ctx) {
    let sections: std::collections::HashSet<u64> = ctx
        .xref_at
        .iter()
        .copied()
        .chain(
            ctx.objects
                .iter()
                .filter(|o| o.value.get("Type").and_then(Obj::name) == Some("XRef"))
                .map(|o| o.start),
        )
        .collect();
    for &(node, offset, range) in &ctx.startxrefs {
        // A linearized file's first trailer may say 0.
        let linearized_zero = offset == 0
            && ctx
                .objects
                .first()
                .is_some_and(|o| o.value.get("Linearized").is_some());
        if !sections.contains(&offset) && !linearized_zero {
            tree.warning(
                node,
                format!("startxref points to {offset}, where no cross-reference section starts"),
                range,
            );
        }
    }
}

#[cfg(test)]
mod tests;
