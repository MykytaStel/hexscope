//! ZIP archives, read the way `unzip` reads them: find the end record, follow
//! it to the central directory, and from each central record to the local
//! header and data it points at. Every offset and size comes from the file,
//! so every one is checked before use.
//!
//! The tree runs in file order, but the central directory has to be read
//! before the entries it describes. It is read into a tree of its own and
//! grafted in at the end, together with the end records.

mod fields;
#[cfg(test)]
pub(crate) mod testing;

use crate::model::{ByteRange, NodeId, NodeKind, ParseTree, Value};
use crate::reader::Reader;
use fields::{Fields, Zip64Need, display, method_name};

pub const MAGIC: [u8; 4] = *b"PK\x03\x04";
/// An archive with no entries is only its end record.
const EMPTY_MAGIC: [u8; 4] = *b"PK\x05\x06";

const LOCAL_SIG: u32 = 0x0403_4B50;
const CENTRAL_SIG: u32 = 0x0201_4B50;
const EOCD_SIG: u32 = 0x0605_4B50;
const ZIP64_EOCD_SIG: u32 = 0x0606_4B50;
const ZIP64_LOCATOR_SIG: u32 = 0x0706_4B50;
const DESCRIPTOR_SIG: u32 = 0x0807_4B50;

const EOCD_LEN: u64 = 22;
const LOCATOR_LEN: u64 = 20;
/// A ZIP64 end record with no extensible data.
const ZIP64_EOCD_LEN: u64 = 56;
const LOCAL_LEN: u64 = 30;
const CENTRAL_LEN: u64 = 46;
/// The end record's comment length is a u16, so it is found within this
/// distance of the end of the file.
const MAX_COMMENT: u64 = 65_535;

/// Entries read before stopping.
const MAX_ENTRIES: usize = 100_000;
/// Entries whose header fields get nodes. Every field of every entry would
/// be millions of nodes for a large archive: far too much for a browser tab.
const DETAIL: usize = 2_000;

#[derive(Debug)]
pub struct ZipDocument {
    pub tree: ParseTree,
    pub entries: Vec<ZipEntry>,
}

/// One entry, as the central directory describes it and the local header
/// places it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZipEntry {
    /// The name as displayed: lossy UTF-8, capped in length.
    pub name: String,
    pub method: u16,
    pub flags: u16,
    pub crc32: u32,
    pub compressed: u64,
    pub uncompressed: u64,
    /// The entry's data in the file, clipped to the file's end.
    pub data: ByteRange,
    /// The entry's node: its local header, data and any data descriptor.
    pub node: NodeId,
}

impl ZipEntry {
    pub fn is_encrypted(&self) -> bool {
        self.flags & 1 != 0
    }

    pub fn is_dir(&self) -> bool {
        self.name.ends_with('/')
    }
}

/// An archive, a file that ends in one, or an empty archive.
pub fn is_zip(data: &[u8]) -> bool {
    data.starts_with(&MAGIC) || data.starts_with(&EMPTY_MAGIC) || find_eocd(data).is_some()
}

/// Parses a ZIP archive. Never fails: damage is recorded as nodes.
pub fn parse_zip(data: &[u8]) -> ZipDocument {
    let mut tree = ParseTree::new();
    let len = data.len() as u64;
    let root = tree.add(
        None,
        "ZIP",
        ByteRange::new(0, len),
        NodeKind::Container,
        None,
    );
    let mut entries = Vec::new();
    match find_eocd(data) {
        Some(at) => read_archive(&mut tree, root, data, at, &mut entries),
        None => {
            tree.error(
                root,
                "no end of central directory: the archive is incomplete",
                ByteRange::new(len, 0),
            );
            scan_local_headers(&mut tree, root, data, &mut entries);
        }
    }
    let n = entries.len();
    let count = format!("{n} {}", if n == 1 { "entry" } else { "entries" });
    tree.set_value(root, Some(Value::Text(count)));
    ZipDocument { tree, entries }
}

fn reader_at(data: &[u8], pos: u64) -> Reader<'_> {
    let mut r = Reader::new(data);
    r.seek(pos);
    r
}

fn u32_at(data: &[u8], pos: u64) -> Option<u32> {
    reader_at(data, pos).u32_le().ok()
}

/// The last end record whose comment fits in the file.
fn find_eocd(data: &[u8]) -> Option<u64> {
    let len = data.len() as u64;
    let last = len.checked_sub(EOCD_LEN)?;
    let first = last.saturating_sub(MAX_COMMENT);
    (first..=last).rev().find(|&p| {
        u32_at(data, p) == Some(EOCD_SIG)
            && reader_at(data, p + 20)
                .u16_le()
                .is_ok_and(|c| p + EOCD_LEN + c as u64 <= len)
    })
}

/// What the end records say about the central directory, and where each
/// claim sits so a problem with it can be pinned to its bytes.
struct End {
    entries: u64,
    entries_field: ByteRange,
    cd_size: u64,
    cd_size_field: ByteRange,
    cd_offset: u64,
    cd_offset_field: ByteRange,
    /// Where the central directory must end: the ZIP64 record, or the EOCD.
    cd_end: u64,
    multi_disk: bool,
    /// The values came from the ZIP64 record rather than the EOCD.
    from_zip64: bool,
}

/// A record placed after the entries: its own tree, its label and range.
struct Tail {
    tree: ParseTree,
    root: NodeId,
    label: &'static str,
    range: ByteRange,
}

impl Tail {
    fn new(label: &'static str, range: ByteRange) -> Self {
        let mut tree = ParseTree::new();
        let root = tree.add(None, label, range, NodeKind::Container, None);
        Self {
            tree,
            root,
            label,
            range,
        }
    }

    fn fields<'a>(&'a mut self, data: &'a [u8], pos: u64) -> Fields<'a, 'a> {
        Fields {
            tree: &mut self.tree,
            parent: self.root,
            r: reader_at(data, pos),
            detail: true,
        }
    }
}

/// A central directory record's claims about its entry.
struct Central {
    name: Vec<u8>,
    flags: u16,
    method: u16,
    crc: u32,
    compressed: u64,
    uncompressed: u64,
    /// Sizes came from a ZIP64 field, so a data descriptor holds 8-byte sizes.
    zip64: bool,
    /// Offset of the local header, as recorded.
    local: u64,
    /// The record's local-header offset field, blamed when it leads nowhere.
    offset_field: ByteRange,
    /// The record's node in the central directory's tree.
    record: Option<NodeId>,
}

fn read_archive(
    tree: &mut ParseTree,
    root: NodeId,
    data: &[u8],
    eocd_at: u64,
    entries: &mut Vec<ZipEntry>,
) {
    let len = data.len() as u64;
    let comment = reader_at(data, eocd_at + 20).u16_le().unwrap_or(0) as u64;
    let mut eocd = Tail::new(
        "end of central directory",
        ByteRange::new(eocd_at, EOCD_LEN + comment),
    );
    let Some(mut end) = read_eocd(&mut eocd, data, eocd_at) else {
        return;
    };

    let (mut locator, mut zip64) = (None, None);
    if let Some(loc_at) = eocd_at.checked_sub(LOCATOR_LEN)
        && u32_at(data, loc_at) == Some(ZIP64_LOCATOR_SIG)
    {
        let mut loc = Tail::new("ZIP64 locator", ByteRange::new(loc_at, LOCATOR_LEN));
        zip64 = read_zip64(&mut loc, data, loc_at, &mut end);
        locator = Some(loc);
    }

    if end.multi_disk {
        // A limit of this tool, not damage: said once, and nothing guessed.
        tree.add(
            Some(root),
            "spanned over several disks: only the last one's records are here",
            ByteRange::new(eocd_at, 0),
            NodeKind::Field,
            None,
        );
        place_tails(tree, root, [zip64, locator, Some(eocd)]);
        return;
    }

    // Where the directory actually is, measured back from its end record,
    // against where the record says it is. The difference is data before
    // the archive that its offsets do not count, as in a self-extractor.
    let (cd_start, shift) = match end.cd_end.checked_sub(end.cd_size) {
        Some(actual) if actual >= end.cd_offset => (actual, actual - end.cd_offset),
        Some(actual) => {
            let tail = if end.from_zip64 {
                zip64.as_mut()
            } else {
                Some(&mut eocd)
            };
            if let Some(t) = tail {
                t.tree.error(
                    t.root,
                    format!(
                        "central directory offset {} is past where it starts, {actual}",
                        end.cd_offset
                    ),
                    end.cd_offset_field,
                );
            }
            (actual, 0)
        }
        None => {
            let tail = if end.from_zip64 {
                zip64.as_mut()
            } else {
                Some(&mut eocd)
            };
            if let Some(t) = tail {
                t.tree.error(
                    t.root,
                    "central directory size is larger than the space before its end record",
                    end.cd_size_field,
                );
            }
            (end.cd_offset.min(end.cd_end), 0)
        }
    };
    let cd_limit = cd_start
        .saturating_add(end.cd_size)
        .min(end.cd_end)
        .min(len);

    let mut cd = Tail::new(
        "central directory",
        ByteRange::new(cd_start, cd_limit.saturating_sub(cd_start)),
    );
    let centrals = read_central_directory(&mut cd, data, cd_start, cd_limit);
    let n = centrals.len();
    cd.tree.set_value(
        cd.root,
        Some(Value::Text(format!(
            "{n} {}",
            if n == 1 { "record" } else { "records" }
        ))),
    );
    if n < MAX_ENTRIES && n as u64 != end.entries {
        let tail = if end.from_zip64 {
            zip64.as_mut()
        } else {
            Some(&mut eocd)
        };
        if let Some(t) = tail {
            t.tree.warning(
                t.root,
                format!(
                    "counts {} entries; the central directory holds {n}",
                    end.entries
                ),
                end.entries_field,
            );
        }
    }

    // Entries in the order their bytes appear.
    let mut placed: Vec<(u64, usize)> = Vec::new();
    for (i, c) in centrals.iter().enumerate() {
        match find_local(data, c.local, shift) {
            Some(at) => placed.push((at, i)),
            None => {
                if let Some(record) = c.record {
                    cd.tree.error(
                        record,
                        format!(
                            "points at offset {}, where there is no local header",
                            c.local
                        ),
                        c.offset_field,
                    );
                }
            }
        }
    }
    placed.sort_unstable();

    let first = placed.first().map_or(cd_start, |&(at, _)| at).min(cd_start);
    if first > 0 {
        let label = match crate::document::identify(data) {
            Some((name, _)) => format!("data before the archive — it looks like {name}"),
            None => "data before the archive".to_string(),
        };
        tree.warning(root, label, ByteRange::new(0, first));
    }

    let mut covered = first;
    let mut previous = String::new();
    for (k, &(at, i)) in placed.iter().enumerate() {
        let c = &centrals[i];
        if at > covered {
            unreferenced(tree, root, covered, at);
        }
        let Some(entry) = add_entry(tree, root, data, c, at, k < DETAIL) else {
            continue;
        };
        let range = tree.get(entry.node).range;
        if at < covered {
            tree.warning(
                entry.node,
                format!("overlaps {previous}: the same bytes belong to two entries"),
                ByteRange::new(at, covered.min(range.end()) - at),
            );
        }
        covered = covered.max(range.end());
        previous = entry.name.clone();
        entries.push(entry);
    }
    if cd_start > covered {
        unreferenced(tree, root, covered, cd_start);
    }

    place_tails(tree, root, [Some(cd), zip64, locator, Some(eocd)]);

    let archive_end = eocd_at + EOCD_LEN + comment;
    if archive_end < len {
        tree.warning(
            root,
            format!("{} bytes after the end of the archive", len - archive_end),
            ByteRange::new(archive_end, len - archive_end),
        );
    }
}

fn unreferenced(tree: &mut ParseTree, root: NodeId, from: u64, to: u64) {
    tree.warning(
        root,
        format!("{} bytes nothing points at", to - from),
        ByteRange::new(from, to - from),
    );
}

fn place_tails<const N: usize>(tree: &mut ParseTree, root: NodeId, tails: [Option<Tail>; N]) {
    for t in tails.into_iter().flatten() {
        let value = t.tree.get(t.root).value.clone();
        let node = tree.add(Some(root), t.label, t.range, NodeKind::Container, value);
        tree.graft(node, &t.tree, 0);
    }
}

fn read_eocd(t: &mut Tail, data: &[u8], at: u64) -> Option<End> {
    let mut f = t.fields(data, at);
    f.signature()?;
    let disk = f.u16("disk")?;
    let cd_disk = f.u16("centralDirectoryDisk")?;
    f.u16("entriesOnDisk")?;
    let entries_at = f.pos();
    let entries = f.u16("entries")?;
    let size_at = f.pos();
    let size = f.u32("centralDirectorySize")?;
    let offset_at = f.pos();
    let offset = f.u32("centralDirectoryOffset")?;
    let comment = f.u16("commentLength")?;
    if comment > 0 {
        f.text("comment", comment)?;
    }
    Some(End {
        entries: entries as u64,
        entries_field: ByteRange::new(entries_at, 2),
        cd_size: size as u64,
        cd_size_field: ByteRange::new(size_at, 4),
        cd_offset: offset as u64,
        cd_offset_field: ByteRange::new(offset_at, 4),
        cd_end: at,
        multi_disk: disk != 0 || cd_disk != 0,
        from_zip64: false,
    })
}

/// Reads the ZIP64 locator and the record it points at, which then speaks
/// for the central directory instead of the EOCD's saturated fields.
fn read_zip64(loc: &mut Tail, data: &[u8], loc_at: u64, end: &mut End) -> Option<Tail> {
    let mut f = loc.fields(data, loc_at);
    f.signature()?;
    f.u32("recordDisk")?;
    let pointer_at = f.pos();
    let declared = f.u64("recordOffset")?;
    f.u32("disks")?;

    // The declared offset, or — in an archive with data prepended — where the
    // record sits right before its locator.
    let at = [Some(declared), loc_at.checked_sub(ZIP64_EOCD_LEN)]
        .into_iter()
        .flatten()
        .find(|&p| u32_at(data, p) == Some(ZIP64_EOCD_SIG));
    let Some(at) = at else {
        loc.tree.error(
            loc.root,
            format!("points at offset {declared}, where there is no ZIP64 end record"),
            ByteRange::new(pointer_at, 8),
        );
        return None;
    };

    let size = reader_at(data, at + 4).u64_le().ok()?;
    let len = size.saturating_add(12).min(loc_at.saturating_sub(at));
    let mut t = Tail::new("ZIP64 end of central directory", ByteRange::new(at, len));
    let mut f = t.fields(data, at);
    f.signature()?;
    f.u64("recordSize")?;
    f.u16("versionMadeBy")?;
    f.u16("versionNeeded")?;
    let disk = f.u32("disk")?;
    let cd_disk = f.u32("centralDirectoryDisk")?;
    f.u64("entriesOnDisk")?;
    let entries_at = f.pos();
    let entries = f.u64("entries")?;
    let size_at = f.pos();
    let cd_size = f.u64("centralDirectorySize")?;
    let offset_at = f.pos();
    let cd_offset = f.u64("centralDirectoryOffset")?;

    *end = End {
        entries,
        entries_field: ByteRange::new(entries_at, 8),
        cd_size,
        cd_size_field: ByteRange::new(size_at, 8),
        cd_offset,
        cd_offset_field: ByteRange::new(offset_at, 8),
        cd_end: at,
        multi_disk: end.multi_disk || disk != 0 || cd_disk != 0,
        from_zip64: true,
    };
    Some(t)
}

fn read_central_directory(cd: &mut Tail, data: &[u8], start: u64, limit: u64) -> Vec<Central> {
    let mut out = Vec::new();
    let mut pos = start;
    while pos < limit {
        if out.len() >= MAX_ENTRIES {
            cd.tree.warning(
                cd.root,
                format!("stopped after {MAX_ENTRIES} entries"),
                ByteRange::new(pos, limit - pos),
            );
            break;
        }
        let detail = out.len() < DETAIL;
        match read_central(cd, data, pos, limit, detail) {
            Some((c, next)) => {
                out.push(c);
                pos = next;
            }
            None => break,
        }
    }
    out
}

fn read_central(
    cd: &mut Tail,
    data: &[u8],
    at: u64,
    limit: u64,
    detail: bool,
) -> Option<(Central, u64)> {
    let mut peek = reader_at(data, at);
    if peek.u32_le().ok() != Some(CENTRAL_SIG) {
        cd.tree.error(
            cd.root,
            "expected a central directory record here",
            ByteRange::new(at, (limit - at).min(4)),
        );
        return None;
    }
    peek.seek(at + 28);
    let lengths = (peek.u16_le(), peek.u16_le(), peek.u16_le());
    let (Ok(n), Ok(x), Ok(c)) = lengths else {
        cd.tree.error(
            cd.root,
            "central directory record truncated",
            ByteRange::new(at, limit - at),
        );
        return None;
    };
    let total = CENTRAL_LEN + n as u64 + x as u64 + c as u64;
    if at + total > limit {
        cd.tree.error(
            cd.root,
            "central directory record truncated",
            ByteRange::new(at, limit - at),
        );
        return None;
    }
    peek.seek(at + CENTRAL_LEN);
    let name = peek.bytes(n as usize).ok()?;

    let record = cd.tree.add(
        Some(cd.root),
        display(name),
        ByteRange::new(at, total),
        NodeKind::Container,
        None,
    );
    let mut f = Fields {
        tree: &mut cd.tree,
        parent: record,
        r: reader_at(data, at),
        detail,
    };
    f.signature()?;
    f.u16("versionMadeBy")?;
    f.u16("versionNeeded")?;
    let flags = f.flags()?;
    let method = f.method()?;
    f.modified()?;
    let crc = f.crc()?;
    let comp = f.u32("compressedSize")?;
    let uncomp = f.u32("uncompressedSize")?;
    f.u16("nameLength")?;
    f.u16("extraLength")?;
    f.u16("commentLength")?;
    f.u16("diskStart")?;
    f.u16("internalAttributes")?;
    f.u32("externalAttributes")?;
    let offset_at = f.pos();
    let offset = f.u32("localHeaderOffset")?;
    f.text("name", n)?;
    let z = f.extra(
        x,
        Zip64Need {
            uncompressed: uncomp == u32::MAX,
            compressed: comp == u32::MAX,
            offset: offset == u32::MAX,
        },
    )?;
    if c > 0 {
        f.text("comment", c)?;
    }

    let central = Central {
        name: name.to_vec(),
        flags,
        method,
        crc,
        compressed: z.compressed.unwrap_or(comp as u64),
        uncompressed: z.uncompressed.unwrap_or(uncomp as u64),
        zip64: z.compressed.is_some() || z.uncompressed.is_some(),
        local: z.offset.unwrap_or(offset as u64),
        offset_field: ByteRange::new(offset_at, 4),
        record: Some(record),
    };
    cd.tree
        .set_value(record, Some(Value::Text(summary(&central))));
    Some((central, at + total))
}

/// Where a local header really is: at its recorded offset, or shifted by
/// data the offsets do not count.
fn find_local(data: &[u8], recorded: u64, shift: u64) -> Option<u64> {
    let len = data.len() as u64;
    [recorded.checked_add(shift), Some(recorded)]
        .into_iter()
        .flatten()
        .find(|&at| at + LOCAL_LEN <= len && u32_at(data, at) == Some(LOCAL_SIG))
}

/// Adds an entry's local header, data and descriptor, comparing the local
/// header with the central record that led here.
fn add_entry(
    tree: &mut ParseTree,
    root: NodeId,
    data: &[u8],
    c: &Central,
    at: u64,
    detail: bool,
) -> Option<ZipEntry> {
    let len = data.len() as u64;
    let mut peek = reader_at(data, at + 26);
    let (n, x) = (peek.u16_le().unwrap_or(0), peek.u16_le().unwrap_or(0));
    let header_len = LOCAL_LEN + n as u64 + x as u64;
    if at + header_len > len {
        let node = tree.add(
            Some(root),
            display(&c.name),
            ByteRange::new(at, len - at),
            NodeKind::Container,
            None,
        );
        tree.error(node, "local header truncated", ByteRange::new(at, len - at));
        return None;
    }

    let data_start = at + header_len;
    let data_end = data_start.saturating_add(c.compressed);
    let clipped = data_end.min(len);
    let descriptor = if c.flags & (1 << 3) != 0 && data_end <= len {
        let signed = u32_at(data, data_end) == Some(DESCRIPTOR_SIG);
        let n = if signed { 4 } else { 0 } + 4 + if c.zip64 { 16 } else { 8 };
        if data_end + n <= len { n } else { 0 }
    } else {
        0
    };
    let end = clipped + descriptor;

    let node = tree.add(
        Some(root),
        display(&c.name),
        ByteRange::new(at, end - at),
        NodeKind::Container,
        Some(Value::Text(summary(c))),
    );
    let header = if detail {
        tree.add(
            Some(node),
            "local header",
            ByteRange::new(at, header_len),
            NodeKind::Container,
            None,
        )
    } else {
        node
    };

    let mut f = Fields {
        tree,
        parent: header,
        r: reader_at(data, at),
        detail,
    };
    // Every read below is inside the header, whose length was checked.
    f.signature();
    f.u16("versionNeeded");
    let flags = f.flags().unwrap_or(0);
    let method = f.method().unwrap_or(0);
    f.modified();
    let crc_at = f.pos();
    let crc = f.crc().unwrap_or(0);
    let sizes_at = f.pos();
    let comp = f.u32("compressedSize").unwrap_or(0);
    let uncomp = f.u32("uncompressedSize").unwrap_or(0);
    f.u16("nameLength");
    f.u16("extraLength");
    let name_at = f.pos();
    let name = f.text("name", n).unwrap_or_default();
    let z = f
        .extra(
            x,
            Zip64Need {
                uncompressed: uncomp == u32::MAX,
                compressed: comp == u32::MAX,
                offset: false,
            },
        )
        .unwrap_or_default();

    // Two records of one entry that disagree: which one an unzipper believes
    // decides what comes out — the trick behind APK signature bypasses.
    if name != c.name {
        tree.warning(
            header,
            format!(
                "name differs from the central directory's: {}",
                display(&c.name)
            ),
            ByteRange::new(name_at, n as u64),
        );
    }
    if method != c.method {
        tree.warning(
            header,
            format!(
                "method differs from the central directory's: {}",
                method_name(c.method)
            ),
            ByteRange::new(at + 8, 2),
        );
    }
    let deferred = flags & (1 << 3) != 0;
    if !deferred && crc != c.crc {
        tree.warning(
            header,
            "CRC-32 differs from the central directory's",
            ByteRange::new(crc_at, 4),
        );
    }
    let local_sizes = (
        z.compressed.unwrap_or(comp as u64),
        z.uncompressed.unwrap_or(uncomp as u64),
    );
    if !deferred && local_sizes != (c.compressed, c.uncompressed) {
        tree.warning(
            header,
            "sizes differ from the central directory's",
            ByteRange::new(sizes_at, 8),
        );
    }

    let data_node = tree.add(
        Some(node),
        "data",
        ByteRange::new(data_start, clipped - data_start),
        NodeKind::Field,
        Some(Value::Text(summary(c))),
    );
    if data_end > len {
        tree.error(
            data_node,
            "data runs past the end of the file",
            ByteRange::new(data_start, clipped - data_start),
        );
    }

    if descriptor > 0 {
        let parent = if detail {
            tree.add(
                Some(node),
                "data descriptor",
                ByteRange::new(clipped, descriptor),
                NodeKind::Container,
                None,
            )
        } else {
            node
        };
        let mut f = Fields {
            tree,
            parent,
            r: reader_at(data, clipped),
            detail,
        };
        if u32_at(data, clipped) == Some(DESCRIPTOR_SIG) {
            f.signature();
        }
        f.crc();
        if c.zip64 {
            f.u64("compressedSize");
            f.u64("uncompressedSize");
        } else {
            f.u32("compressedSize");
            f.u32("uncompressedSize");
        }
    }

    Some(ZipEntry {
        name: display(&c.name),
        method: c.method,
        flags: c.flags,
        crc32: c.crc,
        compressed: c.compressed,
        uncompressed: c.uncompressed,
        data: ByteRange::new(data_start, clipped - data_start),
        node,
    })
}

/// Without an end record there is no central directory: walk the local
/// headers from the start, for as long as each says where it ends.
fn scan_local_headers(
    tree: &mut ParseTree,
    root: NodeId,
    data: &[u8],
    entries: &mut Vec<ZipEntry>,
) {
    let len = data.len() as u64;
    let mut pos = 0u64;
    while entries.len() < MAX_ENTRIES && u32_at(data, pos) == Some(LOCAL_SIG) {
        let mut r = reader_at(data, pos + 6);
        let fixed = (
            r.u16_le(),
            r.u16_le(),
            r.u32_le().and_then(|_| r.u32_le()), // time and date, then crc
        );
        let (Ok(flags), Ok(method), Ok(crc)) = fixed else {
            break;
        };
        let sizes = (r.u32_le(), r.u32_le(), r.u16_le(), r.u16_le());
        let (Ok(comp), Ok(uncomp), Ok(n), Ok(x)) = sizes else {
            break;
        };
        if flags & (1 << 3) != 0 && comp == 0 {
            tree.error(
                root,
                "its sizes are in a data descriptor: without the central directory, where this entry ends is unknown",
                ByteRange::new(pos, (LOCAL_LEN + n as u64 + x as u64).min(len - pos)),
            );
            break;
        }
        let name = r.bytes(n as usize).map(<[u8]>::to_vec).unwrap_or_default();
        let c = Central {
            name,
            flags,
            method,
            crc,
            compressed: comp as u64,
            uncompressed: uncomp as u64,
            zip64: false,
            local: pos,
            offset_field: ByteRange::new(pos, 0),
            record: None,
        };
        let Some(entry) = add_entry(tree, root, data, &c, pos, entries.len() < DETAIL) else {
            break;
        };
        pos = tree.get(entry.node).range.end();
        entries.push(entry);
        if pos >= len {
            break;
        }
    }
}

fn summary(c: &Central) -> String {
    if c.name.ends_with(b"/") && c.uncompressed == 0 {
        return "folder".to_string();
    }
    let s = match c.method {
        0 => format!("stored · {}", human(c.uncompressed)),
        m => format!(
            "{} · {} → {}",
            method_name(m),
            human(c.compressed),
            human(c.uncompressed)
        ),
    };
    if c.flags & 1 != 0 {
        format!("encrypted · {s}")
    } else {
        s
    }
}

fn human(n: u64) -> String {
    const KB: f64 = 1024.0;
    let f = n as f64;
    if n < 1024 {
        format!("{n} B")
    } else if f < KB * KB {
        format!("{:.1} KB", f / KB)
    } else if f < KB * KB * KB {
        format!("{:.1} MB", f / (KB * KB))
    } else {
        format!("{:.1} GB", f / (KB * KB * KB))
    }
}

#[cfg(test)]
mod tests {
    use super::testing::{Archive, Descriptor, Entry, build};
    use super::*;
    use crate::crc32::crc32;

    fn sample() -> Vec<u8> {
        b"hexscope ".repeat(50)
    }

    fn two() -> Archive {
        Archive {
            entries: vec![
                Entry::new("hello.txt", b"hello hello hello hello", 0),
                Entry::new("dir/data.bin", &sample(), 8),
            ],
            ..Default::default()
        }
    }

    fn children(tree: &ParseTree, id: NodeId) -> Vec<String> {
        tree.get(id)
            .children
            .iter()
            .map(|&c| tree.get(c).label.clone())
            .collect()
    }

    fn problems(tree: &ParseTree) -> Vec<(NodeKind, String)> {
        tree.nodes()
            .iter()
            .filter(|n| matches!(n.kind, NodeKind::Warning | NodeKind::Error))
            .map(|n| (n.kind, n.label.clone()))
            .collect()
    }

    fn find<'t>(tree: &'t ParseTree, label: &str) -> &'t crate::model::Node {
        tree.nodes()
            .iter()
            .find(|n| n.label == label)
            .unwrap_or_else(|| panic!("no node {label}"))
    }

    #[test]
    fn reads_entries_in_file_order() {
        let b = build(&two());
        let doc = parse_zip(&b.bytes);
        let root = doc.tree.root().unwrap();
        assert_eq!(
            children(&doc.tree, root),
            [
                "hello.txt",
                "dir/data.bin",
                "central directory",
                "end of central directory"
            ]
        );
        assert_eq!(
            doc.tree.get(root).value,
            Some(Value::Text("2 entries".into()))
        );
        assert_eq!(problems(&doc.tree), []);

        let e = &doc.entries[1];
        assert_eq!(e.name, "dir/data.bin");
        assert_eq!(e.method, 8);
        assert_eq!(e.uncompressed, 450);
        assert_eq!(e.crc32, crc32(&sample()));
        assert_eq!(e.data.start, b.local[1] + 30 + 12);
        assert_eq!(e.data.len, e.compressed);
        assert_eq!(doc.entries[0].data.len, 23, "stored: data is the bytes");
    }

    #[test]
    fn reads_zip64_sizes_and_end_records() {
        let mut a = two();
        a.entries[1].zip64 = true;
        a.zip64_eocd = true;
        let doc = parse_zip(&build(&a).bytes);
        let root = doc.tree.root().unwrap();
        assert_eq!(
            children(&doc.tree, root),
            [
                "hello.txt",
                "dir/data.bin",
                "central directory",
                "ZIP64 end of central directory",
                "ZIP64 locator",
                "end of central directory"
            ]
        );
        assert_eq!(problems(&doc.tree), []);
        assert_eq!(doc.entries[1].uncompressed, 450);
    }

    #[test]
    fn reads_data_descriptors_with_and_without_a_signature() {
        let mut a = two();
        a.entries[0].descriptor = Descriptor::WithSignature;
        a.entries[1].descriptor = Descriptor::NoSignature;
        let b = build(&a);
        let doc = parse_zip(&b.bytes);
        assert_eq!(problems(&doc.tree), []);
        for e in &doc.entries {
            let kids = children(&doc.tree, e.node);
            assert_eq!(
                kids,
                ["local header", "data", "data descriptor"],
                "{}",
                e.name
            );
        }
        let first = doc.tree.get(doc.entries[0].node).range;
        assert_eq!(
            first.end(),
            b.local[1],
            "descriptor with signature is 16 bytes"
        );
        assert_eq!(doc.entries[1].crc32, crc32(&sample()));
    }

    #[test]
    fn reads_the_comment() {
        let mut a = two();
        a.comment = b"made by hexscope".to_vec();
        let doc = parse_zip(&build(&a).bytes);
        assert_eq!(problems(&doc.tree), []);
        assert_eq!(
            find(&doc.tree, "comment").value,
            Some(Value::Text("made by hexscope".into()))
        );
    }

    #[test]
    fn reads_an_empty_archive() {
        let doc = parse_zip(&build(&Archive::default()).bytes);
        assert_eq!(problems(&doc.tree), []);
        assert!(doc.entries.is_empty());
        let root = doc.tree.root().unwrap();
        assert_eq!(
            doc.tree.get(root).value,
            Some(Value::Text("0 entries".into()))
        );
    }

    fn warnings_and_errors(doc: &ZipDocument) -> Vec<String> {
        problems(&doc.tree).into_iter().map(|(_, l)| l).collect()
    }

    fn patch_u32(bytes: &mut [u8], at: u64, v: u32) {
        bytes[at as usize..at as usize + 4].copy_from_slice(&v.to_le_bytes());
    }

    #[test]
    fn names_data_before_the_archive() {
        let mut a = two();
        a.prefix = b"MZ".iter().copied().chain([0u8; 98]).collect();
        let b = build(&a);
        let doc = parse_zip(&b.bytes);
        assert_eq!(
            warnings_and_errors(&doc),
            ["data before the archive — it looks like a Windows executable"]
        );
        assert_eq!(doc.entries.len(), 2, "offsets are shifted by the prefix");
        assert_eq!(doc.entries[0].data.start, b.local[0] + 30 + 9);
        let first = doc.tree.get(doc.tree.root().unwrap()).children[0];
        assert_eq!(doc.tree.get(first).range, ByteRange::new(0, 100));
    }

    #[test]
    fn a_local_name_that_disagrees_is_a_warning_on_the_name() {
        let mut b = build(&two());
        b.bytes[b.local[0] as usize + 30] = b'H';
        let doc = parse_zip(&b.bytes);
        assert_eq!(
            warnings_and_errors(&doc),
            ["name differs from the central directory's: hello.txt"]
        );
        let w = find(
            &doc.tree,
            "name differs from the central directory's: hello.txt",
        );
        assert_eq!(w.range, ByteRange::new(b.local[0] + 30, 9));
    }

    #[test]
    fn overlapping_entries_and_unreferenced_bytes_are_named() {
        // The second record points at the first entry's local header: its
        // bytes are read twice, and the second entry's own bytes by nobody.
        let mut b = build(&two());
        patch_u32(&mut b.bytes, b.central[1] + 42, 0);
        let doc = parse_zip(&b.bytes);
        let found = warnings_and_errors(&doc);
        assert!(
            found.iter().any(|l| l.starts_with("overlaps hello.txt")),
            "{found:?}"
        );
        assert!(
            found.iter().any(|l| l.ends_with("bytes nothing points at")),
            "{found:?}"
        );
    }

    #[test]
    fn data_after_the_archive_is_named() {
        let mut b = build(&two());
        b.bytes.extend_from_slice(b"trailing");
        let doc = parse_zip(&b.bytes);
        assert_eq!(
            warnings_and_errors(&doc),
            ["8 bytes after the end of the archive"]
        );
    }

    #[test]
    fn a_wrong_entry_count_is_a_warning_on_the_count() {
        let mut b = build(&two());
        b.bytes[b.eocd as usize + 10] = 3;
        let doc = parse_zip(&b.bytes);
        assert_eq!(
            warnings_and_errors(&doc),
            ["counts 3 entries; the central directory holds 2"]
        );
        let w = find(&doc.tree, "counts 3 entries; the central directory holds 2");
        assert_eq!(w.range, ByteRange::new(b.eocd + 10, 2));
    }

    #[test]
    fn a_record_pointing_nowhere_is_an_error_on_its_offset() {
        let mut b = build(&two());
        patch_u32(&mut b.bytes, b.central[1] + 42, 0xFFFF_0000);
        let doc = parse_zip(&b.bytes);
        let label = format!(
            "points at offset {}, where there is no local header",
            0xFFFF_0000u32
        );
        // Its entry's own bytes are then read by nobody.
        assert_eq!(
            warnings_and_errors(&doc),
            ["71 bytes nothing points at".to_string(), label.clone()]
        );
        assert_eq!(
            find(&doc.tree, &label).range,
            ByteRange::new(b.central[1] + 42, 4)
        );
        assert_eq!(doc.entries.len(), 1);
    }

    #[test]
    fn without_an_end_record_local_headers_are_read_in_turn() {
        let b = build(&two());
        let cut = &b.bytes[..b.central[0] as usize];
        let doc = parse_zip(cut);
        assert_eq!(
            warnings_and_errors(&doc),
            ["no end of central directory: the archive is incomplete"]
        );
        assert_eq!(doc.entries.len(), 2);
        assert_eq!(doc.entries[1].crc32, crc32(&sample()));

        // Sizes deferred to a descriptor: the walk cannot know where it ends.
        let mut a = two();
        a.entries[1].descriptor = Descriptor::NoSignature;
        let b = build(&a);
        let doc = parse_zip(&b.bytes[..b.central[0] as usize]);
        assert_eq!(doc.entries.len(), 1);
        let found = warnings_and_errors(&doc);
        assert!(
            found[1].starts_with("its sizes are in a data descriptor"),
            "{found:?}"
        );
    }

    #[test]
    fn every_truncation_is_survivable() {
        let mut a = two();
        a.entries[1].zip64 = true;
        a.entries[0].descriptor = Descriptor::WithSignature;
        a.zip64_eocd = true;
        a.comment = b"note".to_vec();
        let b = build(&a);
        for n in 0..=b.bytes.len() {
            let doc = parse_zip(&b.bytes[..n]);
            assert!(doc.tree.root().is_some());
            for e in &doc.entries {
                assert!(e.data.end() <= n as u64, "data range clipped at {n}");
            }
        }
    }

    #[test]
    fn large_archives_stay_small_in_the_tree() {
        // Detailed entries cost about 34 nodes each; past the limit, 3.
        let entries = (0..5000)
            .map(|i| Entry::new(&format!("f{i}"), b"x", 0))
            .collect();
        let b = build(&Archive {
            entries,
            ..Default::default()
        });
        let doc = parse_zip(&b.bytes);
        assert_eq!(doc.entries.len(), 5000);
        assert_eq!(warnings_and_errors(&doc), Vec::<String>::new());
        assert!(
            doc.tree.len() < 20 * 5000,
            "{} nodes for 5000 entries",
            doc.tree.len()
        );
        // Past the detail limit, an entry is its data node alone.
        let last = doc.entries.last().unwrap();
        assert_eq!(children(&doc.tree, last.node), ["data"]);
    }
}
