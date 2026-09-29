//! Compound files (MS-CFB): a file system inside a file, its sectors chained
//! by a table. Word, Excel and PowerPoint 97–2003 keep their parts in one,
//! and so does Outlook's saved message. What such a file gives away is in a
//! few of its streams: who wrote it and for which company, the template's
//! path, when; and, in a message, the headers it came with.

pub(crate) mod docs;
mod msg;
mod props;
#[cfg(test)]
mod tests;

use crate::model::{ByteRange, NodeId, NodeKind, ParseTree, Value};
use crate::zip::DocumentFact;

pub const MAGIC: [u8; 8] = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];
const MAX_REGULAR: u32 = 0xFFFF_FFFA;
const FREE: u32 = 0xFFFF_FFFF;
const MINI: u64 = 64;
/// Streams shorter than this live in the mini stream.
const CUTOFF: u64 = 4096;
const ENTRY: u64 = 128;
const MAX_ENTRIES: usize = 100_000;

pub fn is_cfb(data: &[u8]) -> bool {
    data.starts_with(&MAGIC)
}

/// What the compound file holds, told by the streams at its top.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Word,
    Excel,
    PowerPoint,
    Message,
    Other,
}

impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Kind::Word => "Word 97–2003 document",
            Kind::Excel => "Excel 97–2003 workbook",
            Kind::PowerPoint => "PowerPoint 97–2003 presentation",
            Kind::Message => "Outlook message",
            Kind::Other => "compound file",
        }
    }

    /// What the page and the command line call the format: a message is
    /// read as an email is, an Office file as a document.
    pub fn format(self) -> &'static str {
        match self {
            Kind::Message => "msg",
            Kind::Word | Kind::Excel | Kind::PowerPoint => "office97",
            Kind::Other => "cfb",
        }
    }

    pub fn is_office(self) -> bool {
        matches!(self, Kind::Word | Kind::Excel | Kind::PowerPoint)
    }
}

#[derive(Debug)]
pub struct CfbDocument {
    pub tree: ParseTree,
    pub kind: Kind,
    pub facts: Vec<DocumentFact>,
    /// A message's attached files, by name.
    pub attachments: Vec<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct Entry {
    pub(crate) name: String,
    /// 1 a storage, 2 a stream, 5 the root.
    pub(crate) kind: u8,
    siblings: [u32; 2],
    child: u32,
    start: u32,
    pub(crate) size: u64,
    /// The storage it is in; `None` for the root and for entries no storage reaches.
    pub(crate) parent: Option<usize>,
}

/// A compound file opened: its tables read, its entries listed.
pub(crate) struct Compound<'a> {
    data: &'a [u8],
    shift: u32,
    fat: Vec<u32>,
    minifat: Vec<u32>,
    pub(crate) entries: Vec<Entry>,
    fat_sectors: Vec<u32>,
    difat_chain: Vec<u32>,
    dir_chain: Vec<u32>,
    minifat_chain: Vec<u32>,
    mini_chain: Vec<u32>,
}

fn le(data: &[u8], at: u64, n: usize) -> Option<u64> {
    let at = usize::try_from(at).ok()?;
    let b = data.get(at..at.checked_add(n)?)?;
    Some(b.iter().rev().fold(0u64, |v, &x| (v << 8) | u64::from(x)))
}

/// UTF-16LE text, an odd last byte left out.
pub(crate) fn utf16(b: &[u8]) -> String {
    let mut units = Vec::with_capacity(b.len() / 2);
    for p in b.as_chunks::<2>().0 {
        units.push(u16::from_le_bytes(*p));
    }
    String::from_utf16_lossy(&units)
}

fn le32(data: &[u8], at: u64) -> u32 {
    le(data, at, 4).map_or(FREE, |v| v as u32)
}

/// Appends `n` little-endian sector numbers from `at`; with `regular`, only
/// those that name a sector.
fn words(data: &[u8], at: u64, n: u64, out: &mut Vec<u32>, regular: bool) {
    for i in 0..n {
        let w = le32(data, at + 4 * i);
        if !regular || w <= MAX_REGULAR {
            out.push(w);
        }
    }
}

/// Each sector's place in `chain`, for sectors `0..count`.
fn places(chain: &[u32], count: usize) -> Vec<Option<usize>> {
    let mut out = vec![None; count];
    for (i, &s) in chain.iter().enumerate() {
        if let Some(p) = out.get_mut(s as usize) {
            *p = Some(i);
        }
    }
    out
}

/// A chain of sectors through `table`, from `start`, each sector once.
fn chain(table: &[u32], start: u32) -> Vec<u32> {
    let mut out = Vec::new();
    let mut seen = vec![false; table.len()];
    let mut s = start;
    while s <= MAX_REGULAR {
        let Some(been) = seen.get_mut(s as usize) else {
            break;
        };
        if *been {
            break;
        }
        *been = true;
        out.push(s);
        s = table[s as usize];
    }
    out
}

impl<'a> Compound<'a> {
    pub(crate) fn open(data: &'a [u8]) -> Result<Self, &'static str> {
        if !is_cfb(data) || data.len() < 512 {
            return Err("the header is cut short");
        }
        let at16 = |at| le(data, at, 2).unwrap_or(0);
        let shift = match (at16(26), at16(30)) {
            (3, 9) => 9,
            (4, 12) => 12,
            _ => return Err("its sector size is not one the format allows"),
        };
        if at16(32) != 6 {
            return Err("its mini sector size is not one the format allows");
        }
        let sector = 1u64 << shift;
        let sectors = (data.len() as u64).saturating_sub(sector).div_ceil(sector);
        let mut c = Compound {
            data,
            shift,
            fat: Vec::new(),
            minifat: Vec::new(),
            entries: Vec::new(),
            fat_sectors: Vec::new(),
            difat_chain: Vec::new(),
            dir_chain: Vec::new(),
            minifat_chain: Vec::new(),
            mini_chain: Vec::new(),
        };

        // Which sectors hold the table: 109 in the header, the rest in a
        // chain of their own.
        words(data, 76, 109, &mut c.fat_sectors, true);
        let per = sector / 4 - 1;
        let mut next = le32(data, 68);
        while next <= MAX_REGULAR
            && (c.difat_chain.len() as u64) < sectors
            && !c.difat_chain.contains(&next)
        {
            c.difat_chain.push(next);
            let Some(at) = c.offset(next) else { break };
            words(data, at, per, &mut c.fat_sectors, true);
            next = le32(data, at + 4 * per);
        }
        c.fat_sectors.truncate(sectors as usize);
        for &s in &c.fat_sectors {
            let Some(at) = c.offset(s) else { continue };
            words(data, at, sector / 4, &mut c.fat, false);
        }
        c.fat.truncate(sectors as usize);

        c.dir_chain = chain(&c.fat, le32(data, 48));
        for &s in &c.dir_chain {
            let Some(at) = c.offset(s) else { continue };
            for k in 0..sector / ENTRY {
                if c.entries.len() >= MAX_ENTRIES {
                    break;
                }
                c.entries.push(c.entry(at + k * ENTRY));
            }
        }
        if c.entries.first().map(|e| e.kind) != Some(5) {
            return Err("it has no root entry");
        }
        c.minifat_chain = chain(&c.fat, le32(data, 60));
        for &s in &c.minifat_chain {
            let Some(at) = c.offset(s) else { continue };
            words(data, at, sector / 4, &mut c.minifat, false);
        }
        c.mini_chain = chain(&c.fat, c.entries[0].start);
        c.find_parents();
        Ok(c)
    }

    fn sector(&self) -> u64 {
        1 << self.shift
    }

    /// Sectors after the header, the last one perhaps cut short.
    fn sectors(&self) -> usize {
        let size = self.sector();
        (self.data.len() as u64).saturating_sub(size).div_ceil(size) as usize
    }

    /// Where sector `s` starts in the file, if it is in the file.
    fn offset(&self, s: u32) -> Option<u64> {
        let at = (u64::from(s) + 1) << self.shift;
        (at < self.data.len() as u64).then_some(at)
    }

    fn entry(&self, at: u64) -> Entry {
        let d = self.data;
        let len = (le(d, at + 64, 2).unwrap_or(0) as usize / 2)
            .saturating_sub(1)
            .min(31);
        let name = d
            .get(at as usize..at as usize + 2 * len)
            .map_or_else(String::new, utf16);
        let size = le(d, at + 120, 8).unwrap_or(0);
        Entry {
            name,
            kind: d.get(at as usize + 66).copied().unwrap_or(0),
            siblings: [le32(d, at + 68), le32(d, at + 72)],
            child: le32(d, at + 76),
            start: le32(d, at + 116),
            // Version 3 files may leave junk in the high half.
            size: if self.shift == 9 {
                size & 0xFFFF_FFFF
            } else {
                size
            },
            parent: None,
        }
    }

    /// Each storage's children hang from it as a tree of siblings.
    fn find_parents(&mut self) {
        let mut seen = vec![false; self.entries.len()];
        seen[0] = true;
        let mut stack = vec![(self.entries[0].child, 0usize)];
        while let Some((i, parent)) = stack.pop() {
            let Some(e) = self.entries.get(i as usize) else {
                continue;
            };
            if std::mem::replace(&mut seen[i as usize], true) || e.kind == 0 {
                continue;
            }
            let (siblings, child, kind) = (e.siblings, e.child, e.kind);
            self.entries[i as usize].parent = Some(parent);
            stack.push((siblings[0], parent));
            stack.push((siblings[1], parent));
            if kind == 1 {
                stack.push((child, i as usize));
            }
        }
    }

    /// Where a stream's bytes are in the file, in order.
    pub(crate) fn pieces(&self, e: &Entry) -> Vec<(u64, u64)> {
        let mut out: Vec<(u64, u64)> = Vec::new();
        let mut left = e.size;
        let file = self.data.len() as u64;
        let mut push = |at: u64, len: u64| match out.last_mut() {
            Some(last) if last.0 + last.1 == at => last.1 += len,
            _ => out.push((at, len)),
        };
        if e.kind == 2 && e.size < CUTOFF {
            for m in chain(&self.minifat, e.start) {
                let pos = u64::from(m) * MINI;
                let Some(at) = self
                    .mini_chain
                    .get((pos >> self.shift) as usize)
                    .and_then(|&s| self.offset(s))
                else {
                    break;
                };
                let at = at + (pos & (self.sector() - 1));
                let len = MINI.min(left).min(file.saturating_sub(at));
                if len == 0 {
                    break;
                }
                push(at, len);
                left -= len;
            }
        } else {
            for s in chain(&self.fat, e.start) {
                let Some(at) = self.offset(s) else { break };
                let len = self.sector().min(left).min(file - at);
                if len == 0 {
                    break;
                }
                push(at, len);
                left -= len;
            }
        }
        out
    }

    /// A stream's bytes, at most `max` of them.
    pub(crate) fn read(&self, e: &Entry, max: u64) -> Vec<u8> {
        let mut out = Vec::new();
        for (at, len) in self.pieces(e) {
            let len = len.min(max - out.len() as u64);
            out.extend_from_slice(&self.data[at as usize..(at + len) as usize]);
            if out.len() as u64 >= max {
                break;
            }
        }
        out
    }

    /// The entry named `name` directly under `parent`.
    pub(crate) fn find(&self, parent: usize, name: &str) -> Option<usize> {
        self.entries
            .iter()
            .position(|e| e.parent == Some(parent) && e.name == name)
    }

    fn kind(&self) -> Kind {
        let top = |name: &str| self.find(0, name).is_some();
        if top("WordDocument") {
            Kind::Word
        } else if top("Workbook") || top("Book") {
            Kind::Excel
        } else if top("PowerPoint Document") {
            Kind::PowerPoint
        } else if top("__properties_version1.0")
            || self
                .entries
                .iter()
                .any(|e| e.parent == Some(0) && e.name.starts_with("__substg1.0_"))
        {
            Kind::Message
        } else {
            Kind::Other
        }
    }

    /// An entry's name as a person would read it: without the control
    /// character some names start with, a message's streams by what they hold.
    pub(crate) fn label(&self, i: usize, kind: Kind) -> String {
        let e = &self.entries[i];
        let name = e.name.trim_start_matches(|c: char| c < ' ');
        let own = if kind == Kind::Message {
            msg::label(name).unwrap_or_else(|| name.to_string())
        } else {
            name.to_string()
        };
        match e.parent {
            Some(p) if p != 0 => format!("{} › {own}", self.label(p, kind)),
            _ => own,
        }
    }
}

/// What a sector is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Owner {
    Unclaimed,
    Free,
    Fat,
    Difat,
    Directory,
    MiniFat,
    MiniStream,
    Stream(usize),
}

pub fn parse_cfb(data: &[u8]) -> CfbDocument {
    let mut tree = ParseTree::new();
    let len = data.len() as u64;
    let root = tree.add(
        None,
        "compound file",
        ByteRange::new(0, len),
        NodeKind::Container,
        None,
    );
    let c = match Compound::open(data) {
        Ok(c) => c,
        Err(why) => {
            header(&mut tree, root, data, 512);
            tree.error(
                root,
                format!("not readable: {why}"),
                ByteRange::new(0, len.min(512)),
            );
            return CfbDocument {
                tree,
                kind: Kind::Other,
                facts: Vec::new(),
                attachments: Vec::new(),
            };
        }
    };
    let kind = c.kind();
    tree.set_value(root, Some(Value::Text(kind.name().into())));
    header(&mut tree, root, data, c.sector());
    let firsts = sectors(&mut tree, root, &c, kind);

    let node_of = |i: Option<usize>| i.and_then(|i| firsts[i]).unwrap_or(root);
    let mut facts = Vec::new();
    let mut attachments = Vec::new();
    for name in ["\u{5}SummaryInformation", "\u{5}DocumentSummaryInformation"] {
        let Some(i) = c.find(0, name) else { continue };
        let stream = c.read(&c.entries[i], props::MAX);
        for (kind, text) in props::facts(&stream) {
            facts.push(DocumentFact {
                kind,
                text,
                node: node_of(Some(i)),
            });
        }
    }
    if kind == Kind::Message {
        let m = msg::read(&c);
        for (kind, text, at) in m.facts {
            facts.push(DocumentFact {
                kind,
                text,
                node: node_of(at),
            });
        }
        for (name, _) in m.attachments {
            attachments.push(name);
        }
    }
    CfbDocument {
        tree,
        kind,
        facts,
        attachments,
    }
}

/// The header's fields.
fn header(tree: &mut ParseTree, root: NodeId, data: &[u8], sector: u64) {
    let len = (data.len() as u64).min(sector);
    let node = tree.add(
        Some(root),
        "header",
        ByteRange::new(0, len),
        NodeKind::Container,
        None,
    );
    let fields: [(&str, u64, usize); 18] = [
        ("signature", 0, 8),
        ("CLSID", 8, 16),
        ("minorVersion", 24, 2),
        ("majorVersion", 26, 2),
        ("byteOrder", 28, 2),
        ("sectorShift", 30, 2),
        ("miniSectorShift", 32, 2),
        ("reserved", 34, 6),
        ("directorySectors", 40, 4),
        ("fatSectors", 44, 4),
        ("firstDirectorySector", 48, 4),
        ("transactionSignature", 52, 4),
        ("miniStreamCutoff", 56, 4),
        ("firstMiniFatSector", 60, 4),
        ("miniFatSectors", 64, 4),
        ("firstDifatSector", 68, 4),
        ("difatSectors", 72, 4),
        ("DIFAT", 76, 436),
    ];
    for (label, at, n) in fields {
        if at + n as u64 > len {
            break;
        }
        let value = match n {
            2 | 4 => le(data, at, n).map(Value::U64),
            _ => Some(Value::Bytes(n as u64)),
        };
        tree.add(
            Some(node),
            label,
            ByteRange::new(at, n as u64),
            NodeKind::Field,
            value,
        );
    }
}

/// Every sector after the header, as runs of sectors for the same thing,
/// in file order; the directory's entries and the mini stream's streams
/// under theirs. Returns each entry's first node.
/// What each sector after the header is for, and whose each of the mini
/// stream's sectors is.
fn owners(c: &Compound) -> (Vec<Owner>, MiniMap) {
    let count = c.sectors();
    let mut owner = vec![Owner::Unclaimed; count];
    for (s, o) in owner.iter_mut().enumerate() {
        if c.fat.get(s) == Some(&FREE) {
            *o = Owner::Free;
        }
    }
    let mut claim = |list: &[u32], who: Owner| {
        for &s in list {
            if let Some(o) = owner.get_mut(s as usize) {
                *o = who;
            }
        }
    };
    claim(&c.fat_sectors, Owner::Fat);
    claim(&c.difat_chain, Owner::Difat);
    claim(&c.dir_chain, Owner::Directory);
    claim(&c.minifat_chain, Owner::MiniFat);
    claim(&c.mini_chain, Owner::MiniStream);
    let mut map = MiniMap {
        places: places(&c.mini_chain, count),
        owner: vec![None; c.minifat.len()],
    };
    for (i, e) in c.entries.iter().enumerate() {
        if e.kind != 2 {
            continue;
        }
        if e.size >= CUTOFF {
            claim(&chain(&c.fat, e.start), Owner::Stream(i));
        } else {
            for m in chain(&c.minifat, e.start) {
                if let Some(o) = map.owner.get_mut(m as usize) {
                    *o = Some(i);
                }
            }
        }
    }
    (owner, map)
}

fn sectors(tree: &mut ParseTree, root: NodeId, c: &Compound, kind: Kind) -> Vec<Option<NodeId>> {
    let size = c.sector();
    let (owner, map) = owners(c);
    let count = owner.len();
    let dir_places = places(&c.dir_chain, count);
    let mut firsts = vec![None; c.entries.len()];
    let mut s = 0;
    while s < count {
        let who = owner[s];
        let mut end = s + 1;
        while end < count && owner[end] == who {
            end += 1;
        }
        let at = (s as u64 + 1) * size;
        let run = ByteRange::new(at, ((end - s) as u64 * size).min(c.data.len() as u64 - at));
        let label = match who {
            Owner::Unclaimed => "unclaimed sectors".to_string(),
            Owner::Free => "free sectors".to_string(),
            Owner::Fat => "FAT".to_string(),
            Owner::Difat => "DIFAT".to_string(),
            Owner::Directory => "directory".to_string(),
            Owner::MiniFat => "mini FAT".to_string(),
            Owner::MiniStream => "mini stream".to_string(),
            Owner::Stream(i) => c.label(i, kind),
        };
        // Sectors in no stream can hold anything; nothing reads them.
        let kind_of = if who == Owner::Unclaimed {
            NodeKind::Warning
        } else {
            NodeKind::Container
        };
        let node = tree.add(Some(root), label, run, kind_of, Some(Value::Bytes(run.len)));
        match who {
            Owner::Stream(i) => {
                firsts[i].get_or_insert(node);
            }
            Owner::Directory => entries(tree, node, c, kind, s..end, &dir_places),
            Owner::MiniStream => minis(tree, node, c, kind, s..end, &map, &mut firsts),
            _ => {}
        }
        s = end;
    }
    firsts
}

/// The directory's entries in sectors `run`, each with its fields.
fn entries(
    tree: &mut ParseTree,
    parent: NodeId,
    c: &Compound,
    kind: Kind,
    run: std::ops::Range<usize>,
    places: &[Option<usize>],
) {
    let per = c.sector() / ENTRY;
    for s in run {
        let Some(pos) = places[s] else { continue };
        let Some(base) = c.offset(s as u32) else {
            continue;
        };
        for k in 0..per {
            let i = pos * per as usize + k as usize;
            let at = base + k * ENTRY;
            let Some(e) = c.entries.get(i) else { break };
            let (label, value) = match e.kind {
                0 => ("unused entry".to_string(), None),
                1 => (c.label(i, kind), Some(Value::Text("storage".into()))),
                5 => ("root".to_string(), Some(Value::Text("root storage".into()))),
                _ => (
                    c.label(i, kind),
                    Some(Value::Text(format!("stream · {} bytes", e.size))),
                ),
            };
            let node = tree.add(
                Some(parent),
                label,
                ByteRange::new(at, ENTRY),
                NodeKind::Container,
                value,
            );
            if e.kind == 0 {
                continue;
            }
            let fields: [(&str, u64, usize); 4] = [
                ("name", 0, 64),
                ("type", 66, 1),
                ("startSector", 116, 4),
                ("size", 120, 8),
            ];
            for (label, off, n) in fields {
                let value = match label {
                    "name" => Value::Text(e.name.clone()),
                    _ => Value::U64(le(c.data, at + off, n).unwrap_or(0)),
                };
                tree.add(
                    Some(node),
                    label,
                    ByteRange::new(at + off, n as u64),
                    NodeKind::Field,
                    Some(value),
                );
            }
        }
    }
}

/// The mini stream's sectors: each one's place in its chain, and the
/// stream each of its 64-byte sectors belongs to.
struct MiniMap {
    places: Vec<Option<usize>>,
    owner: Vec<Option<usize>>,
}

/// The small streams in the mini stream's sectors `run`.
fn minis(
    tree: &mut ParseTree,
    parent: NodeId,
    c: &Compound,
    kind: Kind,
    run: std::ops::Range<usize>,
    map: &MiniMap,
    firsts: &mut [Option<NodeId>],
) {
    let per = c.sector() / MINI;
    let mut pending: Option<(usize, u64, u64)> = None;
    let mut flush = |tree: &mut ParseTree, p: Option<(usize, u64, u64)>| {
        if let Some((i, at, len)) = p {
            let node = tree.add(
                Some(parent),
                c.label(i, kind),
                ByteRange::new(at, len),
                NodeKind::Container,
                Some(Value::Bytes(len)),
            );
            firsts[i].get_or_insert(node);
        }
    };
    for s in run {
        let Some(pos) = map.places[s] else { continue };
        let Some(base) = c.offset(s as u32) else {
            continue;
        };
        for k in 0..per {
            let at = base + k * MINI;
            if at >= c.data.len() as u64 {
                break;
            }
            let m = pos * per as usize + k as usize;
            match (map.owner.get(m).copied().flatten(), pending) {
                (Some(i), Some((j, start, len))) if i == j && start + len == at => {
                    pending = Some((j, start, len + MINI));
                }
                (who, p) => {
                    flush(tree, p);
                    pending = who.map(|i| (i, at, MINI));
                }
            }
        }
    }
    flush(tree, pending);
}

/// The part of `pieces`, a stream's places in the file, that holds its
/// bytes `start..end`.
fn in_file(pieces: &[(u64, u64)], start: u64, end: u64, out: &mut Vec<(u64, u64)>) {
    let mut pos = 0;
    for &(at, len) in pieces {
        let (from, to) = (start.max(pos), end.min(pos + len));
        if from < to {
            out.push((at + from - pos, to - from));
        }
        pos += len;
    }
}

/// What a clean copy overwrites with zeros, and what each is: the texts and
/// times in the property streams, and the sectors no stream uses, where
/// deleted parts of a document can remain.
pub(crate) fn blanks(data: &[u8]) -> Vec<(&'static str, Vec<(u64, u64)>)> {
    let Ok(c) = Compound::open(data) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let streams = [
        (
            "\u{5}SummaryInformation",
            "document properties: title, author, last editor, template, dates, editing time",
        ),
        (
            "\u{5}DocumentSummaryInformation",
            "document properties: company, manager, and any added",
        ),
    ];
    for (name, what) in streams {
        let Some(i) = c.find(0, name) else { continue };
        let e = &c.entries[i];
        let pieces = c.pieces(e);
        let mut spans = Vec::new();
        for (a, b) in props::blanks(&c.read(e, props::MAX)) {
            in_file(&pieces, a as u64, b as u64, &mut spans);
        }
        if !spans.is_empty() {
            out.push((what, spans));
        }
    }
    let size = c.sector();
    let mut unused = Vec::new();
    for (s, who) in owners(&c).0.into_iter().enumerate() {
        let at = (s as u64 + 1) * size;
        let bytes = &data[at as usize..(at + size).min(data.len() as u64) as usize];
        if matches!(who, Owner::Free | Owner::Unclaimed) && bytes.iter().any(|&b| b != 0) {
            unused.push((at, bytes.len() as u64));
        }
    }
    if !unused.is_empty() {
        out.push((
            "sectors no stream uses, where deleted parts can remain",
            unused,
        ));
    }
    out
}

/// A message's attached file, by its place in [`CfbDocument::attachments`].
pub fn attachment_bytes(data: &[u8], index: usize) -> Result<Vec<u8>, &'static str> {
    let c = Compound::open(data).map_err(|_| "the message cannot be read")?;
    let m = msg::read(&c);
    let (_, entry) = m
        .attachments
        .get(index)
        .ok_or("there is no such attachment")?;
    let e = entry
        .map(|i| &c.entries[i])
        .ok_or("the attachment holds no file")?;
    Ok(c.read(e, e.size))
}
