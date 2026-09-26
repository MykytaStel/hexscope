//! WebAssembly modules (WebAssembly Core Specification, §5 Binary Format).
//!
//! A module is a header and a list of sections, each an id, a size and its
//! contents: the function types, what it imports from its host, its
//! functions' code, its data, what it exports. Custom sections carry the
//! rest, which engines ignore: the names of functions (for stack traces), the
//! compilers that made it, a link to a source map, DWARF debug info. Those
//! say how, and often where and by whom, the module was built; they are read
//! into facts, and the clean copy leaves them out. Data segments can also
//! hold the paths of the computer that built it, with its user's name in
//! them; those are part of the program, so they are found, and kept.

pub(crate) mod docs;
#[cfg(test)]
mod tests;

use crate::fixed::fixed;
use crate::model::{ByteRange, NodeId, NodeKind, ParseTree, Value};
use crate::zip::DocumentFact;

pub const MAGIC: [u8; 4] = *b"\0asm";
/// Entries listed at most per section; the rest are one node.
const MAX_ENTRIES: u32 = 20_000;
/// Function names read at most from the name section.
const MAX_NAMES: usize = 200_000;
/// Longest text kept from a name or a producer.
const MAX_TEXT: usize = 200;

/// A custom section the clean copy leaves out, and what it held.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Strip {
    pub range: ByteRange,
    pub what: &'static str,
}

#[derive(Debug)]
pub struct WasmDocument {
    pub tree: ParseTree,
    pub facts: Vec<DocumentFact>,
    pub strip: Vec<Strip>,
    /// A component (the component model), not a core module.
    pub component: bool,
    /// Functions, imported and defined.
    pub functions: u32,
}

pub fn is_wasm(data: &[u8]) -> bool {
    data.starts_with(&MAGIC)
}

/// The known sections, by id: their name, and their place in the order the
/// specification requires (§5.5.2; tags come from the exception-handling
/// proposal, between memories and globals).
fn section(id: u8) -> Option<(&'static str, u8)> {
    Some(match id {
        1 => ("type", 1),
        2 => ("import", 2),
        3 => ("function", 3),
        4 => ("table", 4),
        5 => ("memory", 5),
        13 => ("tag", 6),
        6 => ("global", 7),
        7 => ("export", 8),
        8 => ("start", 9),
        9 => ("element", 10),
        12 => ("data count", 11),
        10 => ("code", 12),
        11 => ("data", 13),
        _ => return None,
    })
}

/// Why an entry could not be read.
struct Short;
type R<T> = Result<T, Short>;

/// Reads a section's bytes, knowing where each value is in the file.
struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
    end: usize,
}

impl Reader<'_> {
    fn byte(&mut self) -> R<u8> {
        if self.pos >= self.end {
            return Err(Short);
        }
        self.pos += 1;
        Ok(self.data[self.pos - 1])
    }

    /// An unsigned LEB128 number of at most `bits` bits (§5.2.2).
    fn leb(&mut self, bits: u32) -> R<u64> {
        let mut v = 0u64;
        let mut shift = 0;
        loop {
            let b = self.byte()?;
            if shift < 64 {
                v |= ((b & 0x7F) as u64) << shift;
            }
            shift += 7;
            if b & 0x80 == 0 {
                return Ok(v);
            }
            if shift >= bits + 7 {
                return Err(Short);
            }
        }
    }

    fn u32(&mut self) -> R<u32> {
        self.leb(32).map(|v| v as u32)
    }

    fn skip(&mut self, n: usize) -> R<()> {
        if n > self.end - self.pos {
            return Err(Short);
        }
        self.pos += n;
        Ok(())
    }

    /// A length-prefixed UTF-8 name (§5.2.4), shortened for display.
    fn name(&mut self) -> R<String> {
        let n = self.u32()? as usize;
        let start = self.pos;
        self.skip(n)?;
        Ok(text(&self.data[start..self.pos]))
    }

    /// A value type, as the text format writes it (§5.3.1).
    fn valtype(&mut self) -> R<&'static str> {
        Ok(match self.byte()? {
            0x7F => "i32",
            0x7E => "i64",
            0x7D => "f32",
            0x7C => "f64",
            0x7B => "v128",
            0x70 => "funcref",
            0x6F => "externref",
            0x69 => "exnref",
            // Typed references (function references and GC proposals).
            0x63 | 0x64 => {
                self.leb(33)?;
                "ref"
            }
            0x6A..=0x74 => "ref",
            _ => return Err(Short),
        })
    }

    /// Limits: a minimum, and a maximum when the flag says so (§5.3.7).
    fn limits(&mut self) -> R<(u64, Option<u64>)> {
        let flags = self.byte()?;
        let min = self.leb(64)?;
        let max = if flags & 1 != 0 {
            Some(self.leb(64)?)
        } else {
            None
        };
        Ok((min, max))
    }

    /// A constant expression, up to its `end` (§5.4.9): the instructions a
    /// global's value, or a segment's offset, is computed with.
    fn expr(&mut self) -> R<()> {
        loop {
            match self.byte()? {
                0x0B => return Ok(()),
                0x41 => {
                    self.leb(32)?;
                }
                0x42 => {
                    self.leb(64)?;
                }
                0x43 => self.skip(4)?,
                0x44 => self.skip(8)?,
                0x23 | 0xD2 => {
                    self.u32()?;
                }
                0xD0 => {
                    self.leb(33)?;
                }
                // Extended constant expressions: add, sub, mul.
                0x6A..=0x6C | 0x7C..=0x7E => {}
                0xFD => {
                    // v128.const
                    if self.u32()? != 12 {
                        return Err(Short);
                    }
                    self.skip(16)?;
                }
                0xFB => {
                    // GC constant instructions: an opcode, then indices.
                    let op = self.u32()?;
                    match op {
                        0..=3 | 6..=9 | 26..=28 => {
                            for _ in 0..if matches!(op, 8 | 9) { 2 } else { 1 } {
                                self.u32()?;
                            }
                        }
                        _ => return Err(Short),
                    }
                }
                _ => return Err(Short),
            }
        }
    }
}

/// Bytes as display text: UTF-8, lossy, with control characters replaced,
/// at most [`MAX_TEXT`] characters.
fn text(bytes: &[u8]) -> String {
    let s: String = String::from_utf8_lossy(bytes)
        .chars()
        .map(|c| if c.is_control() { '\u{FFFD}' } else { c })
        .take(MAX_TEXT + 1)
        .collect();
    if s.chars().count() > MAX_TEXT {
        format!("{}…", s.chars().take(MAX_TEXT).collect::<String>())
    } else {
        s
    }
}

fn plural(n: u64, one: &str) -> String {
    format!("{n} {one}{}", if n == 1 { "" } else { "s" })
}

/// What the name section says, read before the tree is built so that each
/// function can be labelled with its name.
#[derive(Default)]
struct Names {
    module: Option<String>,
    /// Function names by index, in increasing order as the specification
    /// requires of a name map (§7.4.1); a module that breaks the order
    /// only loses some names.
    functions: Vec<(u32, String)>,
}

/// The sections: id, where the contents start, where they end (within the
/// file), and whether the size ran past it.
fn sections(data: &[u8]) -> Vec<(u8, usize, usize, usize, bool)> {
    let mut out = Vec::new();
    let mut r = Reader {
        data,
        pos: 8,
        end: data.len(),
    };
    while r.pos < data.len() {
        let at = r.pos;
        let Ok(id) = r.byte() else { break };
        let Ok(size) = r.u32() else {
            out.push((id, at, r.pos, data.len(), true));
            break;
        };
        let start = r.pos;
        let over = size as usize > data.len() - start;
        let end = if over {
            data.len()
        } else {
            start + size as usize
        };
        out.push((id, at, start, end, over));
        r.pos = end;
    }
    out
}

fn read_names(data: &[u8], start: usize, end: usize) -> Names {
    let mut names = Names::default();
    let mut r = Reader {
        data,
        pos: start,
        end,
    };
    let _ = (|| -> R<()> {
        r.name()?;
        while r.pos < end {
            let id = r.byte()?;
            let size = r.u32()? as usize;
            let sub_end = r.pos.checked_add(size).filter(|&e| e <= end).ok_or(Short)?;
            let mut s = Reader {
                data,
                pos: r.pos,
                end: sub_end,
            };
            match id {
                0 => names.module = Some(s.name()?),
                1 => {
                    let n = s.u32()?;
                    for _ in 0..n {
                        let idx = s.u32()?;
                        let name = s.name()?;
                        if names.functions.len() < MAX_NAMES {
                            names.functions.push((idx, name));
                        }
                    }
                }
                _ => {}
            }
            r.pos = sub_end;
        }
        Ok(())
    })();
    names
}

/// What a module can ask its host for, by the names WASI, the component
/// model's interfaces and wasm-bindgen give its imports: each pattern is
/// matched in the lower-cased module and function name.
const ABILITIES: [(&[&str], &str); 13] = [
    (
        &[
            "sock_",
            "fetch",
            "xmlhttprequest",
            "websocket",
            "sendbeacon",
            "wasi:sockets",
            "wasi:http",
        ],
        "reach the network",
    ),
    (
        &["path_", "fd_readdir", "wasi:filesystem"],
        "open files by name",
    ),
    (&["fd_write"], "write to files or the console"),
    (&["fd_read"], "read files or input"),
    (
        &["localstorage", "sessionstorage", "indexeddb", "cookie"],
        "keep data in the browser",
    ),
    (
        &["environ_", "wasi:cli/environment"],
        "read environment variables",
    ),
    (&["args_"], "read its command line"),
    (
        &["clock_", "date_now", "performance", "wasi:clocks"],
        "read the clock",
    ),
    (
        &[
            "random_get",
            "getrandomvalues",
            "randomfillsync",
            "wasi:random",
        ],
        "draw random numbers",
    ),
    (
        &["document", "createelement", "queryselector", "innerhtml"],
        "change the page it runs in",
    ),
    (&["clipboard"], "use the clipboard"),
    (&["geolocation"], "ask where you are"),
    (
        &["getusermedia", "mediadevices"],
        "use the camera or microphone",
    ),
];

/// The imports in words: what they let the module do, and from whom.
fn imports_fact(imports: &[(String, String)]) -> Option<String> {
    if imports.is_empty() {
        return None;
    }
    // Most telling first: the order of the table.
    let keys: Vec<String> = imports
        .iter()
        .map(|(m, n)| format!("{m} {n}").to_ascii_lowercase())
        .collect();
    let mut can: Vec<&str> = ABILITIES
        .iter()
        .filter(|(patterns, _)| keys.iter().any(|k| patterns.iter().any(|p| k.contains(p))))
        .map(|&(_, ability)| ability)
        .collect();
    let mut modules: Vec<&str> = Vec::new();
    for (m, _) in imports {
        if !modules.contains(&m.as_str()) {
            modules.push(m);
        }
    }
    let more = modules.len().saturating_sub(3);
    modules.truncate(3);
    let mut from = modules.join(", ");
    if more > 0 {
        from.push_str(&format!(" and {more} more"));
    }
    let count = plural(imports.len() as u64, "function");
    Some(if can.is_empty() {
        format!("{count} from {from}")
    } else {
        let last = can.pop().unwrap_or_default();
        let list = if can.is_empty() {
            last.to_string()
        } else {
            format!("{} and {last}", can.join(", "))
        };
        format!("{list} ({count} from {from})")
    })
}

/// State while the tree is built.
struct Ctx<'a> {
    data: &'a [u8],
    tree: ParseTree,
    names: Names,
    imported_functions: u32,
    defined_functions: u32,
    facts: Vec<DocumentFact>,
    strip: Vec<Strip>,
    /// User names seen in paths, and the node of the first place each was.
    users: Vec<(String, String, NodeId)>,
    debug: Option<(NodeId, u64, u32)>,
    /// Functions imported, by module and name, and the import section.
    imports: Vec<(String, String)>,
    import_node: Option<NodeId>,
}

impl Ctx<'_> {
    fn field(
        &mut self,
        parent: NodeId,
        label: impl Into<String>,
        from: usize,
        to: usize,
        value: Option<Value>,
    ) -> NodeId {
        self.tree.add(
            Some(parent),
            label,
            ByteRange::new(from as u64, (to - from) as u64),
            NodeKind::Field,
            value,
        )
    }

    fn fact(&mut self, kind: &'static str, text: String, node: NodeId) {
        if !self.facts.iter().any(|f| f.kind == kind) {
            self.facts.push(DocumentFact { kind, text, node });
        }
    }
}

pub fn parse_wasm(data: &[u8]) -> WasmDocument {
    let mut tree = ParseTree::new();
    let len = data.len();
    let root = tree.add(
        None,
        "module",
        ByteRange::new(0, len as u64),
        NodeKind::Container,
        None,
    );
    let mut doc = WasmDocument {
        tree,
        facts: Vec::new(),
        strip: Vec::new(),
        component: false,
        functions: 0,
    };
    if len < 8 {
        doc.tree.error(
            root,
            "the module header is cut short",
            ByteRange::new(0, len as u64),
        );
        return doc;
    }
    doc.tree.add(
        Some(root),
        "magic",
        ByteRange::new(0, 4),
        NodeKind::Field,
        Some(Value::Text("\\0asm".into())),
    );
    let version = u16::from_le_bytes([data[4], data[5]]);
    let layer = u16::from_le_bytes([data[6], data[7]]);
    doc.component = layer == 1;
    doc.tree.add(
        Some(root),
        "version",
        ByteRange::new(4, 4),
        NodeKind::Field,
        Some(if doc.component {
            Value::Text(format!("component, version {version}"))
        } else {
            Value::U64(u32::from_le_bytes([data[4], data[5], data[6], data[7]]) as u64)
        }),
    );
    if !doc.component && (version != 1 || layer != 0) {
        doc.tree.warning(
            root,
            "version is not 1, the only one engines run",
            ByteRange::new(4, 4),
        );
    }

    let found = sections(data);
    let names = found
        .iter()
        .find(|&&(id, _, start, end, _)| {
            id == 0 && {
                let mut r = Reader {
                    data,
                    pos: start,
                    end,
                };
                r.name().is_ok_and(|n| n == "name")
            }
        })
        .map(|&(_, _, start, end, _)| read_names(data, start, end))
        .unwrap_or_default();

    let mut ctx = Ctx {
        data,
        tree: doc.tree,
        names,
        imported_functions: 0,
        defined_functions: 0,
        facts: Vec::new(),
        strip: Vec::new(),
        users: Vec::new(),
        imports: Vec::new(),
        import_node: None,
        debug: None,
    };
    let mut last_rank = 0u8;
    for &(id, at, start, end, over) in &found {
        let known = if doc.component { None } else { section(id) };
        let label = match (doc.component, id, known) {
            (true, _, _) => format!("component section {id}"),
            (false, 0, _) => {
                let mut r = Reader {
                    data,
                    pos: start,
                    end,
                };
                format!("custom section · {}", r.name().unwrap_or_default())
            }
            (false, _, Some((name, _))) => format!("section · {name}"),
            (false, _, None) => format!("section {id}"),
        };
        let node = ctx.tree.add(
            Some(root),
            label,
            ByteRange::new(at as u64, (end - at) as u64),
            NodeKind::Container,
            None,
        );
        ctx.field(node, "id", at, at + 1, Some(Value::U64(id as u64)));
        if start > at + 1 {
            ctx.field(
                node,
                "size",
                at + 1,
                start,
                Some(Value::Bytes((end - start) as u64)),
            );
        }
        if over {
            ctx.tree.error(
                node,
                "a section runs past the end of the file",
                ByteRange::new(at as u64, (end - at) as u64),
            );
        }
        if doc.component {
            if end > start {
                ctx.field(
                    node,
                    "contents",
                    start,
                    end,
                    Some(Value::Bytes((end - start) as u64)),
                );
            }
            continue;
        }
        if id != 0 {
            match known {
                None => {
                    ctx.tree.error(
                        node,
                        format!("unknown section id {id}: engines refuse the module"),
                        ByteRange::new(at as u64, 1),
                    );
                    if end > start {
                        ctx.field(
                            node,
                            "contents",
                            start,
                            end,
                            Some(Value::Bytes((end - start) as u64)),
                        );
                    }
                    continue;
                }
                Some((name, rank)) => {
                    if rank <= last_rank {
                        ctx.tree.error(
                            node,
                            format!("the {name} section is out of order, or repeated: engines refuse the module"),
                            ByteRange::new(at as u64, 1),
                        );
                    }
                    last_rank = last_rank.max(rank);
                }
            }
        }
        let mut r = Reader {
            data,
            pos: start,
            end,
        };
        let result = if id == 0 {
            custom(&mut ctx, node, &mut r)
        } else {
            contents(&mut ctx, node, id, &mut r)
        };
        match result {
            Err(Short) => {
                ctx.tree.error(
                    node,
                    "the section ends in the middle of an entry",
                    ByteRange::new(r.pos.min(end) as u64, (end - r.pos.min(end)) as u64),
                );
            }
            Ok(summary) => {
                if let Some(s) = summary {
                    ctx.tree.set_value(node, Some(Value::Text(s)));
                }
                if r.pos < end {
                    ctx.tree.error(
                        node,
                        format!("{} bytes after the section's last entry", end - r.pos),
                        ByteRange::new(r.pos as u64, (end - r.pos) as u64),
                    );
                }
            }
        }
    }
    // Bytes that no section header could be read from.
    if let Some(&(_, _, _, end, _)) = found.last()
        && end < len
    {
        ctx.tree.error(
            root,
            "the file ends in a section header",
            ByteRange::new(end as u64, (len - end) as u64),
        );
    }

    // Who built it: user names in paths, from data and debug info.
    if let Some((user, path, node)) = ctx.users.first().cloned() {
        let others: Vec<&str> = ctx.users.iter().skip(1).map(|u| u.0.as_str()).collect();
        let mut text = format!("{user}, in paths like {path}");
        if !others.is_empty() {
            text.push_str(&format!(" (also {})", others.join(", ")));
        }
        ctx.fact("paths", text, node);
    }
    if let Some((node, bytes, n)) = ctx.debug {
        ctx.fact(
            "debug",
            format!("DWARF, {} in {}", human(bytes), plural(n as u64, "section")),
            node,
        );
    }

    // Most telling first: who built it, where its source is, then how.
    const ORDER: [&str; 9] = [
        "paths",
        "sourcemap",
        "debug",
        "debuginfo",
        "names",
        "language",
        "toolchain",
        "sdk",
        "imports",
    ];
    if let Some(node) = ctx.import_node
        && let Some(text) = imports_fact(&ctx.imports)
    {
        ctx.fact("imports", text, node);
    }
    let mut facts = std::mem::take(&mut ctx.facts);
    for kind in ORDER {
        if let Some(at) = facts.iter().position(|f| f.kind == kind) {
            ctx.facts.push(facts.remove(at));
        }
    }

    let functions = ctx.imported_functions + ctx.defined_functions;
    let mut summary = if doc.component {
        "WebAssembly component".to_string()
    } else {
        format!("WebAssembly · {}", plural(functions as u64, "function"))
    };
    if let Some(m) = &ctx.names.module {
        summary.push_str(&format!(" · “{m}”"));
    }
    ctx.tree.set_value(root, Some(Value::Text(summary)));
    doc.tree = ctx.tree;
    doc.facts = ctx.facts;
    doc.strip = ctx.strip;
    doc.functions = functions;
    doc
}

fn human(n: u64) -> String {
    match n {
        0..1024 => format!("{n} bytes"),
        1024..1_048_576 => format!("{} KB", fixed(n as f64 / 1024.0, 1)),
        _ => format!("{} MB", fixed(n as f64 / 1_048_576.0, 1)),
    }
}

/// Lists a vector's entries with `entry`, which reads one and returns its
/// label and value; past [`MAX_ENTRIES`], the rest is one node.
fn entries(
    ctx: &mut Ctx,
    node: NodeId,
    r: &mut Reader,
    mut entry: impl FnMut(&mut Ctx, &mut Reader, u32) -> R<(String, Option<Value>)>,
) -> R<u32> {
    let at = r.pos;
    let n = r.u32()?;
    ctx.field(node, "count", at, r.pos, Some(Value::U64(n as u64)));
    for i in 0..n {
        if i == MAX_ENTRIES {
            let rest = r.pos;
            // Read on, unlisted, to find the section's end.
            for j in i..n {
                entry(ctx, r, j)?;
            }
            ctx.field(
                node,
                format!("{} more entries", n - i),
                rest,
                r.pos,
                Some(Value::Bytes((r.pos - rest) as u64)),
            );
            break;
        }
        let start = r.pos;
        let (label, value) = entry(ctx, r, i)?;
        ctx.field(node, label, start, r.pos, value);
    }
    Ok(n)
}

/// A known section's entries, and a summary of them.
fn contents(ctx: &mut Ctx, node: NodeId, id: u8, r: &mut Reader) -> R<Option<String>> {
    Ok(Some(match id {
        1 => {
            let at = r.pos;
            let n = r.u32()?;
            ctx.field(node, "count", at, r.pos, Some(Value::U64(n as u64)));
            for i in 0..n {
                let start = r.pos;
                if r.data.get(r.pos) != Some(&0x60) {
                    // Recursive and sub types, from the GC proposal: named,
                    // not decoded.
                    ctx.field(
                        node,
                        "types in a newer form",
                        start,
                        r.end,
                        Some(Value::Bytes((r.end - start) as u64)),
                    );
                    r.pos = r.end;
                    break;
                }
                r.byte()?;
                let sig = |r: &mut Reader| -> R<String> {
                    let n = r.u32()?;
                    let mut out = Vec::new();
                    for _ in 0..n {
                        out.push(r.valtype()?);
                    }
                    Ok(out.join(", "))
                };
                let params = sig(r)?;
                let results = sig(r)?;
                if i < MAX_ENTRIES {
                    ctx.field(
                        node,
                        format!("type {i}"),
                        start,
                        r.pos,
                        Some(Value::Text(format!("({params}) → ({results})"))),
                    );
                }
            }
            plural(n as u64, "function type")
        }
        2 => {
            let n = entries(ctx, node, r, |ctx, r, _| {
                let module = r.name()?;
                let name = r.name()?;
                let what = match r.byte()? {
                    0 => {
                        ctx.imported_functions += 1;
                        if ctx.imports.len() < MAX_ENTRIES as usize {
                            ctx.imports.push((module.clone(), name.clone()));
                        }
                        format!("function, type {}", r.u32()?)
                    }
                    1 => {
                        let t = r.valtype()?;
                        let (min, max) = r.limits()?;
                        format!("table of {t}, {}", range(min, max))
                    }
                    2 => {
                        let (min, max) = r.limits()?;
                        format!("memory, {}", pages(min, max))
                    }
                    3 => {
                        let t = r.valtype()?;
                        let mutable = r.byte()? == 1;
                        format!("{}global {t}", if mutable { "mutable " } else { "" })
                    }
                    4 => {
                        r.byte()?;
                        format!("tag, type {}", r.u32()?)
                    }
                    _ => return Err(Short),
                };
                Ok((format!("import {module}.{name}"), Some(Value::Text(what))))
            })?;
            ctx.import_node = Some(node);
            plural(n as u64, "import")
        }
        3 => {
            let at = r.pos;
            let n = r.u32()?;
            ctx.field(node, "count", at, r.pos, Some(Value::U64(n as u64)));
            let from = r.pos;
            for _ in 0..n {
                r.u32()?;
            }
            if r.pos > from {
                ctx.field(
                    node,
                    "type indices",
                    from,
                    r.pos,
                    Some(Value::Bytes((r.pos - from) as u64)),
                );
            }
            ctx.defined_functions = n;
            plural(n as u64, "function")
        }
        4 => {
            let n = entries(ctx, node, r, |_, r, i| {
                // A table with an initial value starts 0x40 0x00 (§5.5.6).
                let init = r.data.get(r.pos) == Some(&0x40);
                if init {
                    r.skip(2)?;
                }
                let t = r.valtype()?;
                let (min, max) = r.limits()?;
                if init {
                    r.expr()?;
                }
                Ok((
                    format!("table {i}"),
                    Some(Value::Text(format!("{t}, {}", range(min, max)))),
                ))
            })?;
            plural(n as u64, "table")
        }
        5 => {
            let n = entries(ctx, node, r, |_, r, i| {
                let (min, max) = r.limits()?;
                Ok((format!("memory {i}"), Some(Value::Text(pages(min, max)))))
            })?;
            plural(n as u64, "memory")
        }
        13 => {
            let n = entries(ctx, node, r, |_, r, i| {
                r.byte()?;
                Ok((
                    format!("tag {i}"),
                    Some(Value::Text(format!("type {}", r.u32()?))),
                ))
            })?;
            plural(n as u64, "tag")
        }
        6 => {
            let n = entries(ctx, node, r, |_, r, i| {
                let t = r.valtype()?;
                let mutable = r.byte()? == 1;
                r.expr()?;
                Ok((
                    format!("global {i}"),
                    Some(Value::Text(format!(
                        "{}{t}",
                        if mutable { "mutable " } else { "" }
                    ))),
                ))
            })?;
            plural(n as u64, "global")
        }
        7 => {
            let n = entries(ctx, node, r, |ctx, r, _| {
                let name = r.name()?;
                let kind = r.byte()?;
                let idx = r.u32()?;
                let what = match kind {
                    0 => ctx.function_name(idx),
                    1 => format!("table {idx}"),
                    2 => format!("memory {idx}"),
                    3 => format!("global {idx}"),
                    4 => format!("tag {idx}"),
                    _ => return Err(Short),
                };
                Ok((format!("export “{name}”"), Some(Value::Text(what))))
            })?;
            plural(n as u64, "export")
        }
        8 => {
            let at = r.pos;
            let idx = r.u32()?;
            let name = ctx.function_name(idx);
            ctx.field(
                node,
                "start function",
                at,
                r.pos,
                Some(Value::Text(name.clone())),
            );
            format!("runs {name} first")
        }
        9 => {
            let at = r.pos;
            let n = r.u32()?;
            ctx.field(node, "count", at, r.pos, Some(Value::U64(n as u64)));
            if r.end > r.pos {
                ctx.field(
                    node,
                    "segments",
                    r.pos,
                    r.end,
                    Some(Value::Bytes((r.end - r.pos) as u64)),
                );
            }
            r.pos = r.end;
            plural(n as u64, "element segment")
        }
        12 => {
            let at = r.pos;
            let n = r.u32()?;
            ctx.field(node, "count", at, r.pos, Some(Value::U64(n as u64)));
            plural(n as u64, "data segment")
        }
        10 => {
            let first = ctx.imported_functions;
            let n = entries(ctx, node, r, |ctx, r, i| {
                let size = r.u32()? as usize;
                r.skip(size)?;
                let idx = first + i;
                Ok((
                    format!("function {}", ctx.function_name(idx)),
                    Some(Value::Bytes(size as u64)),
                ))
            })?;
            if n == 1 {
                "1 function body".to_string()
            } else {
                format!("{n} function bodies")
            }
        }
        11 => {
            let mut total = 0u64;
            let n = entries(ctx, node, r, |ctx, r, i| {
                let mode = r.u32()?;
                let at = match mode {
                    0 => {
                        r.expr()?;
                        "active".to_string()
                    }
                    1 => "passive".to_string(),
                    2 => {
                        let mem = r.u32()?;
                        r.expr()?;
                        format!("active, memory {mem}")
                    }
                    _ => return Err(Short),
                };
                let size = r.u32()? as usize;
                let start = r.pos;
                r.skip(size)?;
                total += size as u64;
                // The node for this segment is added after this returns:
                // it will be the next one.
                let next = ctx.tree.len() as NodeId;
                ctx.find_users(start, r.pos, next);
                Ok((
                    format!("data segment {i}"),
                    Some(Value::Text(format!("{}, {at}", human(size as u64)))),
                ))
            })?;
            format!("{}, {}", plural(n as u64, "segment"), human(total))
        }
        _ => return Ok(None),
    }))
}

impl Ctx<'_> {
    fn function_name(&self, idx: u32) -> String {
        let f = &self.names.functions;
        match f.binary_search_by_key(&idx, |x| x.0) {
            Ok(i) => format!("{idx} · {}", f[i].1),
            Err(_) => idx.to_string(),
        }
    }

    /// Looks for home directories in `data[from..to]` — /Users/…, /home/…,
    /// C:\Users\… — and keeps each user name once.
    fn find_users(&mut self, from: usize, to: usize, node: NodeId) {
        const HOMES: [&[u8]; 4] = [b"/Users/", b"/home/", b"C:\\Users\\", b"C:/Users/"];
        let bytes = &self.data[from..to];
        let mut i = 0;
        while i < bytes.len() && self.users.len() < 8 {
            let Some((home, at)) = HOMES
                .iter()
                .filter_map(|h| {
                    bytes[i..]
                        .windows(h.len())
                        .position(|w| w == *h)
                        .map(|p| (h, i + p))
                })
                .min_by_key(|&(_, p)| p)
            else {
                break;
            };
            let name_start = at + home.len();
            let name_end = bytes[name_start..]
                .iter()
                .position(|&b| !(b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-')))
                .map_or(bytes.len(), |p| name_start + p);
            let user = String::from_utf8_lossy(&bytes[name_start..name_end]).into_owned();
            // A home directory is followed by a separator; "Shared" and
            // placeholders are not anyone's.
            let separated = matches!(bytes.get(name_end), Some(b'/' | b'\\'));
            if separated
                && !user.is_empty()
                && user.len() <= 32
                && !matches!(
                    user.as_str(),
                    "Shared" | "runner" | "user" | "USER" | "username"
                )
                && !self.users.iter().any(|u| u.0 == user)
            {
                let path_end = bytes[at..]
                    .iter()
                    .position(|&b| !(0x20..0x7F).contains(&b) || b == b'"' || b == b'\'')
                    .map_or(bytes.len(), |p| at + p)
                    .min(at + 60);
                let mut path = String::from_utf8_lossy(&bytes[at..path_end]).into_owned();
                if path_end - at == 60 {
                    path.push('…');
                }
                self.users.push((user, path, node));
            }
            i = name_end.max(at + 1);
        }
    }
}

fn range(min: u64, max: Option<u64>) -> String {
    match max {
        Some(max) => format!("{min} to {max}"),
        None => format!("at least {min}"),
    }
}

/// Memory limits, in 64 KB pages (§2.3.8).
fn pages(min: u64, max: Option<u64>) -> String {
    let size = |p: u64| human(p.saturating_mul(65536));
    match max {
        Some(max) => format!(
            "{} ({}), up to {}",
            plural(min, "page"),
            size(min),
            size(max)
        ),
        None => format!("{} ({})", plural(min, "page"), size(min)),
    }
}

/// A custom section: its name, then what it holds, read for the sections
/// that say something about who made the module.
fn custom(ctx: &mut Ctx, node: NodeId, r: &mut Reader) -> R<Option<String>> {
    let at = r.pos;
    let name = r.name()?;
    ctx.field(node, "name", at, r.pos, Some(Value::Text(name.clone())));
    let (start, end) = (r.pos, r.end);
    let whole = ctx.tree.get(node).range;
    let strip = |ctx: &mut Ctx, what: &'static str| ctx.strip.push(Strip { range: whole, what });
    let summary = match name.as_str() {
        "name" => {
            let mut functions = 0u64;
            while r.pos < end {
                let at = r.pos;
                let id = r.byte()?;
                let size = r.u32()? as usize;
                let body = r.pos;
                r.skip(size)?;
                let (label, value) = match id {
                    0 => ("module name", ctx.names.module.clone().map(Value::Text)),
                    1 => {
                        let mut s = Reader {
                            data: r.data,
                            pos: body,
                            end: r.pos,
                        };
                        functions = s.u32().unwrap_or(0) as u64;
                        (
                            "function names",
                            Some(Value::Text(plural(functions, "name"))),
                        )
                    }
                    2 => ("local names", Some(Value::Bytes(size as u64))),
                    _ => ("other names", Some(Value::Bytes(size as u64))),
                };
                ctx.field(node, label, at, r.pos, value);
            }
            let mut text = plural(functions, "function name");
            if let Some(m) = &ctx.names.module {
                text = format!("module “{m}”, {text}");
            }
            ctx.fact("names", text.clone(), node);
            strip(ctx, "function names");
            text
        }
        "producers" => {
            let mut all = Vec::new();
            entries(ctx, node, r, |ctx, r, _| {
                let field = r.name()?;
                let n = r.u32()?;
                let mut tools = Vec::new();
                for _ in 0..n {
                    let tool = r.name()?;
                    let version = r.name()?;
                    tools.push(if version.is_empty() {
                        tool
                    } else {
                        format!("{tool} {version}")
                    });
                }
                let kind = match field.as_str() {
                    "language" => "language",
                    "sdk" => "sdk",
                    _ => "toolchain",
                };
                if !tools.is_empty() {
                    let next = ctx.tree.len() as NodeId;
                    all.push((kind, tools.join(" · "), next));
                }
                Ok((field, Some(Value::Text(tools.join(" · ")))))
            })?;
            for (kind, text, node) in all {
                ctx.fact(kind, text, node);
            }
            strip(ctx, "the names of the tools that built it");
            "the tools that built it".to_string()
        }
        "sourceMappingURL" | "external_debug_info" => {
            let at = r.pos;
            let url = r.name()?;
            let field = ctx.field(node, "URL", at, r.pos, Some(Value::Text(url.clone())));
            if name == "sourceMappingURL" {
                ctx.fact("sourcemap", url.clone(), field);
                strip(ctx, "a link to its source map");
            } else {
                ctx.fact("debuginfo", url.clone(), field);
                strip(ctx, "a link to its debug info");
            }
            url
        }
        "target_features" => {
            let mut list = Vec::new();
            entries(ctx, node, r, |_, r, _| {
                let prefix = r.byte()?;
                let feature = r.name()?;
                let f = format!("{}{feature}", prefix as char);
                list.push(f.clone());
                Ok(("feature".to_string(), Some(Value::Text(f))))
            })?;
            list.join(", ")
        }
        n if n.starts_with(".debug") => {
            ctx.field(
                node,
                "DWARF data",
                start,
                end,
                Some(Value::Bytes((end - start) as u64)),
            );
            r.pos = end;
            let next = ctx.tree.get(node).children.last().copied().unwrap_or(node);
            ctx.find_users(start, end, next);
            let d = ctx.debug.get_or_insert((node, 0, 0));
            d.1 += (end - start) as u64;
            d.2 += 1;
            strip(ctx, "DWARF debug info");
            human((end - start) as u64)
        }
        _ => {
            if end > start {
                ctx.field(
                    node,
                    "payload",
                    start,
                    end,
                    Some(Value::Bytes((end - start) as u64)),
                );
            }
            r.pos = end;
            human((end - start) as u64)
        }
    };
    Ok(Some(summary))
}
