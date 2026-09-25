//! What a PDF says about who made it: the document information dictionary
//! (14.3.3) and the XMP metadata stream (14.3.2), read after the scan.

use super::lexer::{Lexer, Obj};
use super::{Ctx, ObjRec};
use crate::inflate::{NoTrace, inflate, zlib_decompress};
use crate::model::{NodeId, ParseTree, Value};
use crate::zip::DocumentFact;
use crate::zip::office::element_text;

/// Streams decompressed to read facts are capped at this.
const MAX_DECODED: u64 = 16 * 1024 * 1024;
/// And all of them together at this, so a file of many small streams that
/// each inflate to the cap costs no more than a few.
pub(super) const MAX_DECODED_TOTAL: u64 = 64 * 1024 * 1024;
/// Longest fact kept, in bytes.
const MAX_TEXT: usize = 512;

/// The order facts are shown in: who, what changed, then the rest.
const ORDER: [&str; 10] = [
    "author",
    "updates",
    "title",
    "subject",
    "keywords",
    "application",
    "producer",
    "created",
    "modified",
    "history",
];

pub(super) fn collect(
    data: &[u8],
    tree: &mut ParseTree,
    ctx: &Ctx,
    encrypted: bool,
) -> Vec<DocumentFact> {
    let mut facts = Vec::new();
    if encrypted {
        return facts;
    }
    let mut budget = MAX_DECODED_TOTAL;
    let trailer_ref = |key: &str| {
        ctx.trailers.iter().rev().find_map(|t| match t.get(key) {
            Some(Obj::Ref(n, _)) => Some(*n),
            _ => None,
        })
    };

    if let Some(num) = trailer_ref("Info") {
        match resolve(data, ctx, num, &mut budget) {
            Some(Found::Top(rec)) => {
                tree.set_value(rec.node, Some(Value::Text("document information".into())));
                info_facts(&rec.value, &mut facts, |key| {
                    rec.keys
                        .iter()
                        .rev()
                        .find(|(k, _)| k == key)
                        .map_or(rec.node, |&(_, n)| n)
                });
            }
            Some(Found::Packed(dict, node)) => info_facts(&dict, &mut facts, |_| node),
            None => {}
        }
    }

    // The catalog's metadata stream, or failing that the last one in the file.
    let catalog_xmp = trailer_ref("Root")
        .and_then(|n| match resolve(data, ctx, n, &mut budget)? {
            Found::Top(rec) => rec.value.get("Metadata").cloned(),
            Found::Packed(dict, _) => dict.get("Metadata").cloned(),
        })
        .and_then(|m| match m {
            Obj::Ref(n, _) => ctx.objects.iter().rev().find(|o| o.num == n),
            _ => None,
        });
    let xmp = catalog_xmp.or_else(|| {
        ctx.objects
            .iter()
            .rev()
            .find(|o| o.value.get("Type").and_then(Obj::name) == Some("Metadata"))
    });
    if let Some(rec) = xmp
        && let Some((range, node)) = rec.stream
        && let Some(bytes) = decode(data, &rec.value, range, &mut budget)
    {
        xmp_facts(&String::from_utf8_lossy(&bytes), node, &mut facts);
    }

    facts.sort_by_key(|f| ORDER.iter().position(|&k| k == f.kind));
    facts
}

pub(super) fn insert_update(facts: &mut Vec<DocumentFact>, fact: DocumentFact) {
    facts.push(fact);
    facts.sort_by_key(|f| ORDER.iter().position(|&k| k == f.kind));
}

enum Found<'a> {
    /// An object at the top level of the file.
    Top(&'a ObjRec),
    /// An object inside a compressed object stream, and that stream's node.
    Packed(Obj, NodeId),
}

/// The latest object numbered `num`, at the top level or packed in an
/// object stream (7.5.7).
fn resolve<'a>(data: &[u8], ctx: &'a Ctx, num: u32, budget: &mut u64) -> Option<Found<'a>> {
    if let Some(rec) = ctx.objects.iter().rev().find(|o| o.num == num) {
        return Some(Found::Top(rec));
    }
    ctx.objects.iter().rev().find_map(|rec| {
        let (bytes, packed) = unpack(data, rec, budget)?;
        let &(_, start, end) = packed.iter().find(|&&(n, ..)| n == num)?;
        let value = Lexer::new(&bytes[..end], start).value()?;
        Some(Found::Packed(value.obj, rec.stream?.1))
    })
}

/// One object in an object stream: its number, and the start and end of its
/// value in the decompressed bytes.
pub(super) type Packed = (u32, usize, usize);

/// An object stream's decompressed bytes, and where each object in it is:
/// its number, and the start and end of its value.
pub(super) fn unpack(
    data: &[u8],
    rec: &ObjRec,
    budget: &mut u64,
) -> Option<(Vec<u8>, Vec<Packed>)> {
    if rec.value.get("Type").and_then(Obj::name) != Some("ObjStm") {
        return None;
    }
    let (range, _) = rec.stream?;
    let bytes = decode(data, &rec.value, range, budget)?;
    let count = rec.value.get("N").and_then(Obj::int)?;
    let first = usize::try_from(rec.value.get("First").and_then(Obj::int)?).ok()?;
    let mut lx = Lexer::new(&bytes, 0);
    let mut at = Vec::new();
    for _ in 0..count {
        lx.skip_ws();
        let n = u32::try_from(lx.uint()?).ok()?;
        lx.skip_ws();
        let offset = usize::try_from(lx.uint()?).ok()?;
        let start = first.checked_add(offset).filter(|&s| s <= bytes.len())?;
        at.push((n, start));
    }
    // Each value runs to where the next one starts.
    let mut starts: Vec<usize> = at.iter().map(|&(_, s)| s).collect();
    starts.sort_unstable();
    let packed = at
        .into_iter()
        .map(|(n, s)| {
            let i = starts.partition_point(|&x| x <= s);
            (n, s, starts.get(i).copied().unwrap_or(bytes.len()))
        })
        .collect();
    Some((bytes, packed))
}

/// A stream's bytes, undone of Flate when that is its only filter.
/// Every call spends from `budget`, successful or not.
pub(super) fn decode(
    data: &[u8],
    dict: &Obj,
    range: crate::model::ByteRange,
    budget: &mut u64,
) -> Option<Vec<u8>> {
    let limit = MAX_DECODED.min(*budget);
    let raw = data.get(range.start as usize..range.end() as usize)?;
    let flate = match dict.get("Filter") {
        None => false,
        Some(Obj::Name(n)) => n == "FlateDecode",
        Some(Obj::Array(a)) => a.len() == 1 && a[0].obj.name() == Some("FlateDecode"),
        Some(_) => return None,
    };
    if !flate {
        let fits = raw.len() as u64 <= limit;
        *budget -= if fits { raw.len() as u64 } else { limit };
        return fits.then(|| raw.to_vec());
    }
    // Some writers get the Adler-32 wrong; the data is still good.
    let out = zlib_decompress(raw, limit, &mut NoTrace)
        .ok()
        .or_else(|| inflate(raw.get(2..)?, limit, &mut NoTrace).ok());
    *budget -= out.as_ref().map_or(limit, |o| o.len() as u64);
    out
}

fn info_facts(dict: &Obj, facts: &mut Vec<DocumentFact>, node_of: impl Fn(&str) -> NodeId) {
    const KEYS: [(&str, &str); 8] = [
        ("Title", "title"),
        ("Author", "author"),
        ("Subject", "subject"),
        ("Keywords", "keywords"),
        ("Creator", "application"),
        ("Producer", "producer"),
        ("CreationDate", "created"),
        ("ModDate", "modified"),
    ];
    for (key, kind) in KEYS {
        let Some(Obj::Str(raw)) = dict.get(key) else {
            continue;
        };
        let mut t = text(raw).trim().to_string();
        if kind == "created" || kind == "modified" {
            t = date(&t);
        }
        if !t.is_empty() {
            facts.push(DocumentFact {
                kind,
                text: cap(t),
                node: node_of(key),
            });
        }
    }
}

fn xmp_facts(xml: &str, node: NodeId, facts: &mut Vec<DocumentFact>) {
    let has = |facts: &Vec<DocumentFact>, kind| facts.iter().any(|f| f.kind == kind);
    let fields = [
        ("dc:creator", "author"),
        ("dc:title", "title"),
        ("xmp:CreatorTool", "application"),
        ("pdf:Producer", "producer"),
        ("xmp:CreateDate", "created"),
        ("xmp:ModifyDate", "modified"),
    ];
    for (name, kind) in fields {
        if has(facts, kind) {
            continue;
        }
        let Some(mut t) = xmp_text(xml, name) else {
            continue;
        };
        if kind == "created" || kind == "modified" {
            t = xmp_date(&t);
        }
        facts.push(DocumentFact {
            kind,
            text: cap(t),
            node,
        });
    }
    if let Some(history) = element_text(xml, "xmpMM:History") {
        let steps = history.matches("<rdf:li").count();
        if steps > 0 {
            facts.push(DocumentFact {
                kind: "history",
                text: format!(
                    "{steps} {} recorded",
                    if steps == 1 { "step" } else { "steps" }
                ),
                node,
            });
        }
    }
}

/// An XMP property as an element (its first list item, for lists) or as an
/// attribute of its description.
fn xmp_text(xml: &str, name: &str) -> Option<String> {
    if let Some(t) = element_text(xml, name) {
        if t.contains("<rdf:li") {
            return element_text(&t, "rdf:li");
        }
        return (!t.contains('<')).then_some(t);
    }
    let pattern = format!("{name}=\"");
    let at = xml.find(&pattern)? + pattern.len();
    let end = at + xml[at..].find('"')?;
    let t = xml[at..end].trim();
    (!t.is_empty()).then(|| t.to_string())
}

/// A PDF text string (7.9.2.2): UTF-16BE after a byte order mark, UTF-8
/// after its mark, PDFDocEncoding otherwise.
pub(super) fn text(raw: &[u8]) -> String {
    if let Some(rest) = raw.strip_prefix(&[0xFE, 0xFF]) {
        let units: Vec<u16> = rest
            .as_chunks::<2>()
            .0
            .iter()
            .map(|&c| u16::from_be_bytes(c))
            .collect();
        return String::from_utf16_lossy(&units);
    }
    if let Some(rest) = raw.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        return String::from_utf8_lossy(rest).into_owned();
    }
    raw.iter().map(|&b| pdf_doc_char(b)).collect()
}

/// PDFDocEncoding (Annex D.2): Latin-1, except for 0x18–0x1F and 0x80–0xA0.
fn pdf_doc_char(b: u8) -> char {
    const HIGH: [char; 33] = [
        '•', '†', '‡', '…', '—', '–', 'ƒ', '⁄', '‹', '›', '−', '‰', '„', '“', '”', '‘', '’', '‚',
        '™', 'ﬁ', 'ﬂ', 'Ł', 'Œ', 'Š', 'Ÿ', 'Ž', 'ı', 'ł', 'œ', 'š', 'ž', '\u{FFFD}', '€',
    ];
    const LOW: [char; 8] = ['˘', 'ˇ', 'ˆ', '˙', '˝', '˛', '˚', '˜'];
    match b {
        0x18..=0x1F => LOW[(b - 0x18) as usize],
        0x80..=0xA0 => HIGH[(b - 0x80) as usize],
        _ => char::from(b),
    }
}

pub(super) fn cap(mut s: String) -> String {
    if s.len() > MAX_TEXT {
        let mut end = MAX_TEXT;
        while !s.is_char_boundary(end) {
            end -= 1;
        }
        s.truncate(end);
        s.push('…');
    }
    s
}

/// `D:20260305174210+02'00'` as `2026-03-05 17:42 +02:00` (7.9.4). Whatever
/// is present is shown; anything not shaped like a date stays as written.
fn date(raw: &str) -> String {
    let s = raw.strip_prefix("D:").unwrap_or(raw);
    let digits = s.bytes().take_while(u8::is_ascii_digit).count();
    if digits < 4 {
        return raw.to_string();
    }
    let part = |a: usize, b: usize| s.get(a..b).filter(|_| digits >= b);
    let mut out = s[..4].to_string();
    if let Some(m) = part(4, 6) {
        out.push_str(&format!("-{m}"));
        if let Some(d) = part(6, 8) {
            out.push_str(&format!("-{d}"));
        }
    }
    if let (Some(h), Some(m)) = (part(8, 10), part(10, 12)) {
        out.push_str(&format!(" {h}:{m}"));
        let zone = &s[digits..];
        if zone.starts_with('Z') {
            out.push_str(" UTC");
        } else if let Some(sign @ ('+' | '-')) = zone.chars().next() {
            let z: String = zone[1..].chars().filter(char::is_ascii_digit).collect();
            if let (Some(zh), Some(zm)) = (z.get(0..2), z.get(2..4)) {
                out.push_str(&format!(" {sign}{zh}:{zm}"));
            } else if let Some(zh) = z.get(0..2) {
                out.push_str(&format!(" {sign}{zh}:00"));
            }
        }
    }
    out
}

/// `2026-03-02T09:14:00+02:00` as `2026-03-02 09:14 +02:00`.
fn xmp_date(raw: &str) -> String {
    let (Some(day), Some('T'), Some(time)) = (raw.get(..10), raw.chars().nth(10), raw.get(11..16))
    else {
        return raw.to_string();
    };
    let zone = raw
        .get(16..)
        .map(|z| z.trim_start_matches(|c: char| c == ':' || c == '.' || c.is_ascii_digit()))
        .unwrap_or("");
    match zone {
        "Z" => format!("{day} {time} UTC"),
        z if z.starts_with(['+', '-']) => format!("{day} {time} {z}"),
        _ => format!("{day} {time}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strings_decode_from_each_encoding() {
        assert_eq!(text(b"Olena"), "Olena");
        assert_eq!(text(&[0xFE, 0xFF, 0x04, 0x1E, 0x00, 0x21]), "О!");
        assert_eq!(text(&[0xEF, 0xBB, 0xBF, 0xC3, 0xA9]), "é");
        assert_eq!(text(&[0x93, 0x41, 0xA0, 0xE9]), "ﬁA€é");
        // An odd byte out after the mark is dropped, not a panic.
        assert_eq!(text(&[0xFE, 0xFF, 0x00]), "");
    }

    #[test]
    fn dates_read_whatever_is_there() {
        assert_eq!(date("D:20260305174210+02'00'"), "2026-03-05 17:42 +02:00");
        assert_eq!(date("D:20260411120000Z"), "2026-04-11 12:00 UTC");
        assert_eq!(date("D:2026"), "2026");
        assert_eq!(date("D:202603"), "2026-03");
        assert_eq!(date("D:20260305174210-05"), "2026-03-05 17:42 -05:00");
        assert_eq!(date("yesterday"), "yesterday");
        assert_eq!(date("D:2026é"), "2026");
        assert_eq!(
            xmp_date("2026-03-02T09:14:00+02:00"),
            "2026-03-02 09:14 +02:00"
        );
        assert_eq!(xmp_date("2026-03-02T09:14:00.5Z"), "2026-03-02 09:14 UTC");
        assert_eq!(xmp_date("2026"), "2026");
    }

    #[test]
    fn xmp_reads_elements_lists_and_attributes() {
        let xml = r#"<rdf:Description xmp:CreatorTool="Writer 2"><dc:creator><rdf:Seq><rdf:li>Olena</rdf:li></rdf:Seq></dc:creator><dc:title><rdf:Alt><rdf:li xml:lang="x-default">Plan</rdf:li></rdf:Alt></dc:title></rdf:Description>"#;
        assert_eq!(xmp_text(xml, "dc:creator").as_deref(), Some("Olena"));
        assert_eq!(xmp_text(xml, "dc:title").as_deref(), Some("Plan"));
        assert_eq!(
            xmp_text(xml, "xmp:CreatorTool").as_deref(),
            Some("Writer 2")
        );
        assert_eq!(xmp_text(xml, "pdf:Producer"), None);
    }
}
