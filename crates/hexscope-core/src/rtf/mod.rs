//! RTF: groups in braces, control words after backslashes, and text. What
//! a document says about who made it lives in a few groups — `\info` (title,
//! author, the operator who saved it last, company, manager, dates, how
//! many versions and minutes of editing), `\*\generator`, `\*\template`
//! with its path on the author's computer, `\*\userprops` — and in the
//! names its revision table and comments carry. Text a tracked deletion
//! still holds is read from the runs marked `\deleted`.
//!
//! One pass over the bytes, without recursion: a group is pushed at `{` and
//! popped at `}`, and only the groups worth naming become nodes.

pub(crate) mod docs;

use crate::facts::DocumentFact;
use crate::model::{ByteRange, NodeId, NodeKind, ParseTree, Value};

/// What accepting tracked changes and deleting comments are called in a copy's list.
const ACCEPTED: &str = "tracked changes, accepted";
const COMMENTS: &str = "comments and who wrote them";
/// Groups deeper than this are not followed: no real document comes near.
const MAX_DEPTH: usize = 4096;
/// Longest text kept for one fact, in bytes.
const MAX_TEXT: usize = 512;
/// Authors named in a line, at most; the rest are counted.
const MAX_NAMES: usize = 4;

#[derive(Debug)]
pub struct RtfDocument {
    pub tree: ParseTree,
    pub facts: Vec<DocumentFact>,
    /// Where the groups a clean copy changes lie, and what it puts there.
    pub(crate) edits: Vec<Edit>,
}

/// A group a clean copy replaces, by its byte range, with what goes there.
#[derive(Debug, Clone)]
pub(crate) struct Edit {
    pub range: ByteRange,
    pub with: Vec<u8>,
    pub what: &'static str,
}

pub fn is_rtf(data: &[u8]) -> bool {
    data.starts_with(b"{\\rtf")
}

/// One group: where it starts and ends, and the control word it opens with.
#[derive(Debug, Clone)]
struct Group {
    start: usize,
    end: usize,
    depth: usize,
    /// The first control word, without its backslash.
    word: String,
}

/// Every group, in the order they open: each is written down at its `{`
/// and given its end at its `}`, so no sort is needed.
fn groups(data: &[u8]) -> (Vec<Group>, bool) {
    // The groups still open: where each is in `out`, and whether its first control word is known.
    let mut open: Vec<(usize, bool)> = Vec::new();
    let mut out: Vec<Group> = Vec::new();
    let mut i = 0;
    let mut balanced = true;
    while i < data.len() {
        match data[i] {
            b'{' => {
                if open.len() >= MAX_DEPTH {
                    balanced = false;
                    break;
                }
                out.push(Group {
                    start: i,
                    end: 0,
                    depth: open.len() + 1,
                    word: String::new(),
                });
                open.push((out.len() - 1, false));
                i += 1;
            }
            b'}' => {
                match open.pop() {
                    Some((at, _)) => out[at].end = i + 1,
                    None => balanced = false,
                }
                i += 1;
            }
            b'\\' => {
                let (word, next) = control(data, i);
                if let Some(top) = open.last_mut()
                    && !top.1
                {
                    match word {
                        // `\*` comes before the word that names the destination.
                        Control::Star => {}
                        Control::Word(w, _) => {
                            out[top.0].word = w.to_string();
                            top.1 = true;
                        }
                        _ => top.1 = true,
                    }
                }
                i = next;
            }
            b'\r' | b'\n' => i += 1,
            _ => {
                if let Some(top) = open.last_mut() {
                    top.1 = true;
                }
                i += 1;
            }
        }
    }
    let closed = balanced && open.is_empty();
    // A group never closed has no end: it is damage, said once, and not a group.
    out.retain(|g| g.end > 0);
    (out, closed)
}

/// What follows a backslash.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Control<'a> {
    /// A control word and its number, if it has one.
    Word(&'a str, Option<i32>),
    /// `\'hh`: a byte in the document's code page.
    Hex(u8),
    /// `\*`: the group that follows is a destination a reader may skip.
    Star,
    /// `\{`, `\}`, `\\`, `\~` and the like: a character of its own.
    Symbol(u8),
}

/// The control at `at` (a backslash) and where the next token starts.
fn control(data: &[u8], at: usize) -> (Control<'_>, usize) {
    let mut i = at + 1;
    let Some(&c) = data.get(i) else {
        return (Control::Symbol(b'\\'), i);
    };
    if c.is_ascii_alphabetic() {
        let start = i;
        while i < data.len() && data[i].is_ascii_alphabetic() && i - start < 32 {
            i += 1;
        }
        let word = std::str::from_utf8(&data[start..i]).unwrap_or("");
        let num_start = i;
        if data.get(i) == Some(&b'-') {
            i += 1;
        }
        while i < data.len() && data[i].is_ascii_digit() && i - num_start < 11 {
            i += 1;
        }
        let number = std::str::from_utf8(&data[num_start..i])
            .ok()
            .and_then(|n| n.parse::<i32>().ok());
        // One space ends a control word, and is part of it.
        if data.get(i) == Some(&b' ') {
            i += 1;
        }
        return (Control::Word(word, number), i);
    }
    if c == b'\'' {
        let hex = data
            .get(i + 1..i + 3)
            .and_then(|h| std::str::from_utf8(h).ok())
            .and_then(|h| u8::from_str_radix(h, 16).ok());
        return match hex {
            Some(b) => (Control::Hex(b), i + 3),
            None => (Control::Symbol(b'?'), i + 1),
        };
    }
    if c == b'*' {
        return (Control::Star, i + 1);
    }
    (Control::Symbol(c), i + 1)
}

/// A byte of a Windows code page as a character: 1251 (Cyrillic) or, for
/// any other, 1252, whose upper half is Latin-1's.
fn codepage(b: u8, page: u32) -> char {
    if b < 0x80 {
        return b as char;
    }
    if page == 1251 {
        return match b {
            0xC0..=0xFF => char::from_u32(0x0410 + (b - 0xC0) as u32).unwrap_or('?'),
            0xA8 => 'Ё',
            0xB8 => 'ё',
            0xAA => 'Є',
            0xBA => 'є',
            0xAF => 'Ї',
            0xBF => 'ї',
            0xB2 => 'І',
            0xB3 => 'і',
            0xA5 => 'Ґ',
            0xB4 => 'ґ',
            0xA0 => '\u{a0}',
            0x96 => '–',
            0x97 => '—',
            0xAB => '«',
            0xBB => '»',
            0xB9 => '№',
            _ => '?',
        };
    }
    match b {
        0x80 => '€',
        0x91 => '‘',
        0x92 => '’',
        0x93 => '“',
        0x94 => '”',
        0x96 => '–',
        0x97 => '—',
        _ if b < 0xA0 => '?',
        _ => b as char,
    }
}

/// The text of a piece of RTF: its characters, escapes decoded, control
/// words and nested destinations left out.
fn text(data: &[u8], page: u32) -> String {
    let mut out = String::new();
    let mut i = 0;
    let mut skip_fallback = 0usize;
    let mut uc = 1usize;
    let mut depth = 0usize;
    let mut hidden_from: Option<usize> = None;
    while i < data.len() && out.len() < MAX_TEXT * 2 {
        match data[i] {
            b'{' => {
                depth += 1;
                // A nested destination — `{\*\…}` — is not text.
                if hidden_from.is_none() && data.get(i + 1..i + 3) == Some(b"\\*") {
                    hidden_from = Some(depth);
                }
                i += 1;
            }
            b'}' => {
                if hidden_from == Some(depth) {
                    hidden_from = None;
                }
                depth = depth.saturating_sub(1);
                i += 1;
            }
            b'\\' => {
                let (c, next) = control(data, i);
                i = next;
                if hidden_from.is_some() {
                    continue;
                }
                match c {
                    Control::Hex(b) => {
                        if skip_fallback > 0 {
                            skip_fallback -= 1;
                        } else {
                            out.push(codepage(b, page));
                        }
                    }
                    Control::Word("u", Some(n)) => {
                        let code = if n < 0 { (n + 65536) as u32 } else { n as u32 };
                        out.push(char::from_u32(code).unwrap_or('?'));
                        skip_fallback = uc;
                    }
                    Control::Word("uc", Some(n)) => uc = n.max(0) as usize,
                    Control::Word("par" | "line" | "tab" | "cell", _) => out.push(' '),
                    Control::Symbol(b'~') => out.push('\u{a0}'),
                    Control::Symbol(b'_') => out.push('-'),
                    Control::Symbol(s @ (b'{' | b'}' | b'\\')) => out.push(s as char),
                    _ => {}
                }
            }
            b'\r' | b'\n' => i += 1,
            b => {
                i += 1;
                if hidden_from.is_some() {
                    continue;
                }
                if skip_fallback > 0 {
                    skip_fallback -= 1;
                    continue;
                }
                out.push(codepage(b, page));
            }
        }
    }
    let joined = out.split_whitespace().collect::<Vec<_>>().join(" ");
    cap(joined)
}

fn cap(mut s: String) -> String {
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

/// The inside of a group, after its opening control words (`{\*\generator ` → the rest).
fn body<'a>(data: &'a [u8], g: &Group) -> &'a [u8] {
    let inner = &data[g.start + 1..g.end - 1];
    let mut i = 0;
    loop {
        if inner.get(i) == Some(&b'\\') {
            let (c, next) = control(inner, i);
            match c {
                Control::Star => i = next,
                Control::Word(w, _) if w == g.word => return &inner[next..],
                _ => return &inner[i..],
            }
        } else {
            return &inner[i..];
        }
    }
}

/// A `{\creatim\yr2026\mo10\dy1\hr15\min40}` group as `2026-10-01 15:40`.
fn time(data: &[u8]) -> Option<String> {
    let (mut yr, mut mo, mut dy, mut hr, mut min) = (None, None, None, 0, 0);
    let mut i = 0;
    while i < data.len() {
        if data[i] == b'\\' {
            let (c, next) = control(data, i);
            if let Control::Word(w, Some(n)) = c {
                match w {
                    "yr" => yr = Some(n),
                    "mo" => mo = Some(n),
                    "dy" => dy = Some(n),
                    "hr" => hr = n,
                    "min" => min = n,
                    _ => {}
                }
            }
            i = next;
        } else {
            i += 1;
        }
    }
    let (yr, mo, dy) = (yr?, mo?, dy?);
    (yr > 0).then(|| format!("{yr:04}-{mo:02}-{dy:02} {hr:02}:{min:02}"))
}

/// The number a group's own control word carries: `{\version3}` is 3.
fn number(data: &[u8], g: &Group) -> Option<i32> {
    match control(data, g.start + 1).0 {
        Control::Word(w, n) if w == g.word => n,
        _ => None,
    }
}

/// `3 comments by Olena and Petro`: distinct names in order of appearance.
fn by(what: String, names: &[String]) -> String {
    let mut seen: Vec<&str> = Vec::new();
    for n in names {
        if !n.is_empty() && !seen.contains(&n.as_str()) {
            seen.push(n);
        }
    }
    let more = seen.len().saturating_sub(MAX_NAMES);
    seen.truncate(MAX_NAMES);
    let mut list = seen.join(", ");
    if more > 0 {
        list = format!("{list} and {more} more");
    } else if let Some(i) = list.rfind(", ") {
        list.replace_range(i..i + 2, " and ");
    }
    if list.is_empty() {
        what
    } else {
        cap(format!("{what} by {list}"))
    }
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// What `\info` holds, by the name of each group in it.
const INFO: &[(&str, &str, &str)] = &[
    ("title", "title", "title"),
    ("subject", "subject", "subject"),
    ("author", "author", "author"),
    ("operator", "editor", "operator"),
    ("manager", "manager", "manager"),
    ("company", "company", "company"),
    ("category", "category", "category"),
    ("keywords", "keywords", "keywords"),
    ("comment", "description", "note"),
    ("doccomm", "description", "document comment"),
    ("hlinkbase", "custom", "hyperlink base"),
];

/// The groups, outside `\info`, a node is made for, and their labels.
const DESTINATIONS: &[(&str, &str)] = &[
    ("fonttbl", "font table"),
    ("colortbl", "colour table"),
    ("stylesheet", "stylesheet"),
    ("info", "info"),
    ("generator", "generator"),
    ("template", "template"),
    ("userprops", "user properties"),
    ("revtbl", "revision table"),
    ("rsidtbl", "rsid table"),
    ("datastore", "data store"),
    ("listtable", "list table"),
    ("listoverridetable", "list overrides"),
    ("latentstyles", "latent styles"),
    ("themedata", "theme"),
    ("colorschememapping", "colour scheme"),
    ("xmlnstbl", "XML namespaces"),
    ("annotation", "comment"),
    ("atnauthor", "comment author"),
    ("atnid", "comment initials"),
];

pub fn parse_rtf(data: &[u8]) -> RtfDocument {
    let mut tree = ParseTree::new();
    let root = tree.add(
        None,
        "RTF",
        ByteRange::new(0, data.len() as u64),
        NodeKind::Container,
        None,
    );
    let (all, balanced) = groups(data);
    if !balanced {
        tree.error(
            root,
            "the braces do not balance: the document is cut short or damaged",
            ByteRange::new(data.len() as u64, 0),
        );
    }
    let page = header_number(data, "ansicpg").map_or(1252, |p| p as u32);
    let mut facts = Vec::new();
    let mut edits = Vec::new();

    // The text: everything after the header's tables, to the end.
    let header_end = all
        .iter()
        .filter(|g| {
            g.depth == 2
                && matches!(
                    g.word.as_str(),
                    "fonttbl"
                        | "colortbl"
                        | "stylesheet"
                        | "info"
                        | "generator"
                        | "template"
                        | "userprops"
                        | "revtbl"
                        | "rsidtbl"
                        | "listtable"
                        | "listoverridetable"
                        | "latentstyles"
                        | "xmlnstbl"
                        | "themedata"
                        | "colorschememapping"
                        | "datastore"
                        | "filetbl"
                        | "pgdsctbl"
                        | "defchp"
                        | "defpap"
                        | "mmathPr"
                )
        })
        .map(|g| g.end)
        .max()
        .unwrap_or(0);
    let body_node = (header_end > 0 && header_end < data.len()).then(|| {
        tree.add(
            Some(root),
            "text",
            ByteRange::new(header_end as u64, (data.len() - header_end) as u64),
            NodeKind::Container,
            None,
        )
    });

    let mut push = |kind: &'static str, text: Option<String>, node: NodeId| {
        if let Some(text) = text.filter(|t| !t.is_empty()) {
            facts.push(DocumentFact { kind, text, node });
        }
    };
    let (mut comments, mut authors, mut initials) = (0usize, Vec::new(), Vec::new());
    for g in &all {
        let Some(&(_, label)) = DESTINATIONS.iter().find(|(w, _)| *w == g.word) else {
            continue;
        };
        // Header tables at the top; comments wherever they stand.
        let top = g.depth == 2;
        let inside = matches!(g.word.as_str(), "annotation" | "atnauthor" | "atnid");
        if !top && !inside {
            continue;
        }
        let parent = if top || body_node.is_none() {
            root
        } else {
            body_node.unwrap_or(root)
        };
        let range = ByteRange::new(g.start as u64, (g.end - g.start) as u64);
        let value = match g.word.as_str() {
            "generator" | "template" | "atnauthor" | "atnid" => Some(Value::Text(
                text(body(data, g), page)
                    .trim_end_matches(';')
                    .trim()
                    .to_string(),
            )),
            _ => None,
        };
        let node = tree.add(
            Some(parent),
            label,
            range,
            if inside && g.word != "annotation" {
                NodeKind::Field
            } else {
                NodeKind::Container
            },
            value,
        );
        match g.word.as_str() {
            "info" => {
                info(data, g, &all, page, node, &mut tree, &mut push);
                edits.push(Edit { range, with: b"{\\info}".to_vec(), what: "document information: title, author, who saved it last, company, manager, dates, editing time" });
            }
            "generator" => {
                push(
                    "application",
                    Some(
                        text(body(data, g), page)
                            .trim_end_matches(';')
                            .trim()
                            .to_string(),
                    ),
                    node,
                );
                edits.push(Edit {
                    range,
                    with: Vec::new(),
                    what: "the program that wrote it",
                });
            }
            "template" => {
                push("template", Some(text(body(data, g), page)), node);
                edits.push(Edit {
                    range,
                    with: Vec::new(),
                    what: "the template it came from, and where it was",
                });
            }
            "userprops" => {
                push("custom", user_properties(data, g, &all, page), node);
                edits.push(Edit {
                    range,
                    with: Vec::new(),
                    what: "custom properties",
                });
            }
            "rsidtbl" => edits.push(Edit {
                range,
                with: Vec::new(),
                what: "the numbers that tie each editing session together",
            }),
            "datastore" => edits.push(Edit {
                range,
                with: Vec::new(),
                what: "data stored by the program that wrote it",
            }),
            "revtbl" => {
                let names = revision_names(data, g, &all, page);
                if names.iter().any(|n| n != "Unknown") {
                    let mut with = b"{\\*\\revtbl ".to_vec();
                    for _ in &names {
                        with.extend_from_slice(b"{Unknown;}");
                    }
                    with.push(b'}');
                    edits.push(Edit {
                        range,
                        with,
                        what: "who made the tracked changes",
                    });
                }
                let real: Vec<String> = names.into_iter().filter(|n| n != "Unknown").collect();
                let (ins, del) = (count_word(data, "revised"), count_word(data, "deleted"));
                if ins + del > 0 {
                    let what = match (ins, del) {
                        (i, 0) => plural(i, "insertion", "insertions"),
                        (0, d) => plural(d, "deletion", "deletions"),
                        (i, d) => format!(
                            "{} and {}",
                            plural(i, "insertion", "insertions"),
                            plural(d, "deletion", "deletions")
                        ),
                    };
                    push(
                        "tracked",
                        Some(format!("{}, not accepted yet", by(what, &real))),
                        node,
                    );
                }
            }
            "annotation" => {
                comments += 1;
                edits.push(Edit {
                    range,
                    with: Vec::new(),
                    what: COMMENTS,
                });
            }
            "atnauthor" | "atnid" => {
                let name = text(body(data, g), page);
                if g.word == "atnauthor" {
                    authors.push(name)
                } else {
                    initials.push(name)
                }
                edits.push(Edit {
                    range,
                    with: Vec::new(),
                    what: COMMENTS,
                });
            }
            _ => {}
        }
    }
    if comments > 0 {
        let node = body_node.unwrap_or(root);
        // Word writes a comment's initials and its author: the author names it,
        // and the initials only where there is no author.
        let named: Vec<String> = authors
            .into_iter()
            .filter(|a: &String| !a.is_empty())
            .collect();
        let names = if named.is_empty() {
            initials
                .into_iter()
                .filter(|a: &String| !a.is_empty())
                .collect()
        } else {
            named
        };
        push(
            "comments",
            Some(by(plural(comments, "comment", "comments"), &names)),
            node,
        );
    }
    if let Some(deleted) = deleted_text(data, page) {
        push(
            "deleted",
            Some(format!("“{deleted}”")),
            body_node.unwrap_or(root),
        );
    }
    // Three lists, each in the order of the file: one, without a sort.
    let edits = merge(merge(edits, accept_groups(data, &all)), accept_marks(data));
    RtfDocument { tree, facts, edits }
}

/// The edits that accept tracked changes and take the comments' marks out:
/// a run marked `\deleted` in a group of its own goes, with its text; the
/// marks of an insertion go and its text stays; the comment's reference
/// character and the bookmarks around what it is about go. A deletion that
/// shares its group with other text is left as it is, and the copy's check
/// says so.
fn accept_groups(data: &[u8], all: &[Group]) -> Vec<Edit> {
    let mut edits = Vec::new();
    for g in all {
        let inner = &data[g.start + 1..g.end - 1];
        let range = ByteRange::new(g.start as u64, (g.end - g.start) as u64);
        if matches!(g.word.as_str(), "atrfstart" | "atrfend" | "atnref") {
            edits.push(Edit {
                range,
                with: Vec::new(),
                what: COMMENTS,
            });
            continue;
        }
        if inner.contains(&b'{') {
            continue;
        }
        let mut i = 0;
        let mut deleted = false;
        while i < inner.len() && inner[i] == b'\\' {
            let (c, next) = control(inner, i);
            if let Control::Word(w, n) = c
                && w == "deleted"
            {
                deleted = n != Some(0);
            }
            i = next;
        }
        if deleted && count_in(inner, "deleted0") == 0 {
            edits.push(Edit {
                range,
                with: Vec::new(),
                what: ACCEPTED,
            });
        }
    }
    edits
}

/// An insertion's marks, and a comment's reference character, wherever they stand.
fn accept_marks(data: &[u8]) -> Vec<Edit> {
    let mut edits = Vec::new();
    let mut i = 0;
    while i < data.len() {
        if data[i] == b'\\' {
            let (c, next) = control(data, i);
            if let Control::Word(w, n) = c {
                let what = match w {
                    "revised" if n != Some(0) => Some(ACCEPTED),
                    "revauth" | "revdttm" => Some(ACCEPTED),
                    "chatn" => Some(COMMENTS),
                    _ => None,
                };
                if let Some(what) = what {
                    edits.push(Edit {
                        range: ByteRange::new(i as u64, (next - i) as u64),
                        with: Vec::new(),
                        what,
                    });
                }
            }
            i = next;
        } else {
            i += 1;
        }
    }
    edits
}

/// Two lists of edits, each in the order of the file, as one; an edit
/// starting where another does is the same one.
fn merge(a: Vec<Edit>, b: Vec<Edit>) -> Vec<Edit> {
    let mut out: Vec<Edit> = Vec::with_capacity(a.len() + b.len());
    let (mut a, mut b) = (a.into_iter().peekable(), b.into_iter().peekable());
    loop {
        let next = match (a.peek(), b.peek()) {
            (Some(x), Some(y)) if y.range.start < x.range.start => b.next(),
            (Some(_), _) => a.next(),
            (None, Some(_)) => b.next(),
            (None, None) => break,
        };
        if let Some(e) = next
            && out.last().is_none_or(|l| l.range.start != e.range.start)
        {
            out.push(e);
        }
    }
    out
}

/// How many times `\word` (with its number, as written) stands in a piece of RTF.
fn count_in(data: &[u8], word: &str) -> usize {
    let mut n = 0;
    let mut i = 0;
    while i < data.len() {
        if data[i] == b'\\' {
            let next = control(data, i).1;
            if data[i + 1..next].trim_ascii_end() == word.as_bytes() {
                n += 1;
            }
            i = next;
        } else {
            i += 1;
        }
    }
    n
}

/// The groups in `\info`, each as a node, and the facts they hold.
fn info(
    data: &[u8],
    info: &Group,
    all: &[Group],
    page: u32,
    node: NodeId,
    tree: &mut ParseTree,
    push: &mut impl FnMut(&'static str, Option<String>, NodeId),
) {
    let children: Vec<&Group> = all
        .iter()
        .filter(|g| g.depth == info.depth + 1 && g.start > info.start && g.end < info.end)
        .collect();
    for g in children {
        let range = ByteRange::new(g.start as u64, (g.end - g.start) as u64);
        let word = g.word.as_str();
        if let Some(&(_, kind, label)) = INFO.iter().find(|(w, _, _)| *w == word) {
            let said = text(body(data, g), page);
            let child = tree.add(
                Some(node),
                label,
                range,
                NodeKind::Field,
                Some(Value::Text(said.clone())),
            );
            push(kind, Some(said), child);
            continue;
        }
        let (label, kind) = match word {
            "creatim" => ("created", Some("created")),
            "revtim" => ("revised", Some("modified")),
            "printim" => ("printed", Some("printed")),
            "buptim" => ("backed up", None),
            "version" => ("version", Some("revisions")),
            "edmins" => ("editing minutes", Some("editing")),
            "nofpages" => ("pages", None),
            "nofwords" => ("words", None),
            "nofchars" => ("characters", None),
            "nofcharsws" => ("characters with spaces", None),
            "vern" => ("program version", None),
            "id" => ("id", None),
            _ => ("info item", None),
        };
        let value = match word {
            "creatim" | "revtim" | "printim" | "buptim" => time(&data[g.start..g.end]),
            _ => number(data, g).map(|n| n.to_string()),
        };
        let child = tree.add(
            Some(node),
            label,
            range,
            NodeKind::Field,
            value.clone().map(Value::Text),
        );
        let said = match (kind, value) {
            (Some("editing"), Some(v)) => v
                .parse::<u64>()
                .ok()
                .filter(|&m| m > 0)
                .map(crate::zip::office::duration),
            (Some("revisions"), Some(v)) => v
                .parse::<u64>()
                .ok()
                .filter(|&n| n > 1)
                .map(|n| n.to_string()),
            (Some(_), v) => v,
            _ => None,
        };
        if let Some(kind) = kind {
            push(kind, said, child);
        }
    }
}

/// `{\propname Client}\proptype30{\staticval Northwind}` pairs, as `Client: Northwind`.
fn user_properties(data: &[u8], g: &Group, all: &[Group], page: u32) -> Option<String> {
    let inner: Vec<&Group> = all
        .iter()
        .filter(|c| c.start > g.start && c.end < g.end)
        .collect();
    let mut pairs: Vec<String> = Vec::new();
    let mut name: Option<String> = None;
    for c in inner {
        match c.word.as_str() {
            "propname" => name = Some(text(body(data, c), page)),
            "staticval" => {
                let value = text(body(data, c), page);
                if let Some(n) = name.take() {
                    pairs.push(if value.is_empty() {
                        n
                    } else {
                        format!("{n}: {value}")
                    });
                }
            }
            _ => {}
        }
    }
    (!pairs.is_empty()).then(|| cap(pairs.join(" · ")))
}

/// The names in `{\*\revtbl {Unknown;}{Olena Koval;}}`.
fn revision_names(data: &[u8], g: &Group, all: &[Group], page: u32) -> Vec<String> {
    let inner: Vec<&Group> = all
        .iter()
        .filter(|c| c.depth == g.depth + 1 && c.start > g.start && c.end < g.end)
        .collect();
    inner
        .into_iter()
        .map(|c| {
            text(&data[c.start + 1..c.end - 1], page)
                .trim_end_matches(';')
                .trim()
                .to_string()
        })
        .collect()
}

/// How many times a control word stands in the document, `\deleted` but not `\deleted0`.
fn count_word(data: &[u8], word: &str) -> usize {
    let mut n = 0;
    let mut i = 0;
    while i < data.len() {
        if data[i] == b'\\' {
            let (c, next) = control(data, i);
            if let Control::Word(w, number) = c
                && w == word
                && number != Some(0)
            {
                n += 1;
            }
            i = next;
        } else {
            i += 1;
        }
    }
    n
}

/// The text of every run marked `\deleted`: what a tracked deletion still holds.
fn deleted_text(data: &[u8], page: u32) -> Option<String> {
    // Each group keeps the state it opened with: deleted or not, text or a destination.
    let mut stack: Vec<(bool, bool)> = vec![(false, false)];
    let mut out = String::new();
    let mut i = 0;
    let mut gap = false;
    while i < data.len() && out.len() < MAX_TEXT {
        let (deleted, skip) = *stack.last().unwrap_or(&(false, false));
        match data[i] {
            b'{' => {
                let starred = data.get(i + 1..i + 3) == Some(b"\\*");
                let named = matches!(control(data, i + 1).0, Control::Word(w, _) if matches!(w, "fonttbl" | "colortbl" | "stylesheet" | "info" | "pict" | "object" | "field" | "fldinst" | "header" | "footer"));
                if stack.len() < MAX_DEPTH {
                    stack.push((deleted, skip || starred || named));
                }
                i += 1;
            }
            b'}' => {
                if stack.len() > 1 {
                    stack.pop();
                }
                i += 1;
            }
            b'\\' => {
                let (c, next) = control(data, i);
                i = next;
                match c {
                    Control::Word("deleted", n) => {
                        if let Some(top) = stack.last_mut() {
                            top.0 = n != Some(0);
                        }
                        if n != Some(0) && !out.is_empty() && gap {
                            out.push_str(" … ");
                        }
                        gap = false;
                    }
                    Control::Word("plain", _) => {
                        if let Some(top) = stack.last_mut() {
                            top.0 = false;
                        }
                    }
                    Control::Hex(b) if deleted && !skip => out.push(codepage(b, page)),
                    Control::Word("u", Some(n)) if deleted && !skip => {
                        let code = if n < 0 { (n + 65536) as u32 } else { n as u32 };
                        out.push(char::from_u32(code).unwrap_or('?'));
                        // The fallback after it, one byte or one escape.
                        if data.get(i) == Some(&b'\\') && data.get(i + 1) == Some(&b'\'') {
                            i += 4;
                        } else if i < data.len()
                            && data[i] != b'\\'
                            && data[i] != b'{'
                            && data[i] != b'}'
                        {
                            i += 1;
                        }
                    }
                    Control::Word("par" | "line" | "tab", _) if deleted && !skip => out.push(' '),
                    _ => {}
                }
            }
            b'\r' | b'\n' => i += 1,
            b => {
                if deleted && !skip {
                    out.push(codepage(b, page));
                } else if !skip && b != b' ' {
                    gap = true;
                }
                i += 1;
            }
        }
    }
    let text = out.split_whitespace().collect::<Vec<_>>().join(" ");
    (!text.is_empty()).then(|| cap(text))
}

/// A number from the header's control words: `\ansicpg1251` is 1251.
fn header_number(data: &[u8], word: &str) -> Option<i32> {
    let head = &data[..data.len().min(512)];
    let mut i = 0;
    while i < head.len() {
        match head[i] {
            b'{' if i > 0 => return None,
            b'\\' => {
                let (c, next) = control(head, i);
                if let Control::Word(w, n) = c
                    && w == word
                {
                    return n;
                }
                i = next;
            }
            _ => i += 1,
        }
    }
    None
}

#[cfg(test)]
mod tests;
