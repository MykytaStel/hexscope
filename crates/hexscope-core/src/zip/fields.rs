//! Reading a ZIP header field and adding its node are one step here, so a
//! header's layout is written down once.

use crate::model::{ByteRange, NodeId, NodeKind, ParseTree, Value};
use crate::reader::Reader;

/// Longest name or comment kept for display, in bytes.
pub(super) const MAX_NAME: usize = 1024;

/// What a ZIP64 extra field is expected to hold: only the values whose
/// header fields are saturated appear, in this order.
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct Zip64Need {
    pub uncompressed: bool,
    pub compressed: bool,
    pub offset: bool,
}

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct Zip64Values {
    pub uncompressed: Option<u64>,
    pub compressed: Option<u64>,
    pub offset: Option<u64>,
}

/// Reads fields at a cursor, adding a node for each under `parent`. With
/// `detail` off, values are still read but no nodes are added: large archives
/// keep their size in check that way.
pub(super) struct Fields<'t, 'd> {
    pub tree: &'t mut ParseTree,
    pub parent: NodeId,
    pub r: Reader<'d>,
    pub detail: bool,
}

impl Fields<'_, '_> {
    pub fn pos(&self) -> u64 {
        self.r.pos()
    }

    fn node(&mut self, label: &str, start: u64, value: Value) {
        if self.detail {
            let range = ByteRange::new(start, self.r.pos() - start);
            self.tree.add(
                Some(self.parent),
                label,
                range,
                NodeKind::Field,
                Some(value),
            );
        }
    }

    pub fn u16(&mut self, label: &str) -> Option<u16> {
        let s = self.r.pos();
        let v = self.r.u16_le().ok()?;
        self.node(label, s, Value::U64(v as u64));
        Some(v)
    }

    pub fn u32(&mut self, label: &str) -> Option<u32> {
        let s = self.r.pos();
        let v = self.r.u32_le().ok()?;
        self.node(label, s, Value::U64(v as u64));
        Some(v)
    }

    pub fn u64(&mut self, label: &str) -> Option<u64> {
        let s = self.r.pos();
        let v = self.r.u64_le().ok()?;
        self.node(label, s, Value::U64(v));
        Some(v)
    }

    pub fn signature(&mut self) -> Option<u32> {
        let s = self.r.pos();
        let b = self.r.array::<4>().ok()?;
        let text = format!("PK {:02X} {:02X}", b[2], b[3]);
        self.node("signature", s, Value::Text(text));
        Some(u32::from_le_bytes(b))
    }

    pub fn flags(&mut self) -> Option<u16> {
        let s = self.r.pos();
        let v = self.r.u16_le().ok()?;
        let mut meaning = Vec::new();
        if v & 1 != 0 {
            meaning.push("encrypted");
        }
        if v & (1 << 3) != 0 {
            meaning.push("sizes in a data descriptor");
        }
        if v & (1 << 11) != 0 {
            meaning.push("UTF-8 names");
        }
        let text = if meaning.is_empty() {
            format!("0x{v:04X}")
        } else {
            format!("0x{v:04X} · {}", meaning.join(", "))
        };
        self.node("flags", s, Value::Text(text));
        Some(v)
    }

    pub fn method(&mut self) -> Option<u16> {
        let s = self.r.pos();
        let v = self.r.u16_le().ok()?;
        let value = Value::Enum {
            raw: v as u64,
            name: method_name(v),
        };
        self.node("method", s, value);
        Some(v)
    }

    /// DOS time then date: two fields that only mean something together.
    pub fn modified(&mut self) -> Option<()> {
        let s = self.r.pos();
        let time = self.r.u16_le().ok()?;
        let date = self.r.u16_le().ok()?;
        self.node("modified", s, Value::Text(dos_datetime(time, date)));
        Some(())
    }

    pub fn crc(&mut self) -> Option<u32> {
        let s = self.r.pos();
        let v = self.r.u32_le().ok()?;
        self.node("crc32", s, Value::Text(format!("{v:08X}")));
        Some(v)
    }

    /// A name or comment: the raw bytes, and a display form capped in length.
    pub fn text(&mut self, label: &str, len: u16) -> Option<Vec<u8>> {
        let s = self.r.pos();
        let raw = self.r.bytes(len as usize).ok()?;
        self.node(label, s, Value::Text(display(raw)));
        Some(raw.to_vec())
    }

    /// The extra block: a run of (id, size, data) fields. Returns what a
    /// ZIP64 field held of what `need` asked for.
    pub fn extra(&mut self, len: u16, need: Zip64Need) -> Option<Zip64Values> {
        let start = self.r.pos();
        let end = start + len as u64;
        let block = self.r.bytes(len as usize).ok()?;
        let mut found = Zip64Values::default();
        if len == 0 {
            return Some(found);
        }

        let outer = self.parent;
        if self.detail {
            self.parent = self.tree.add(
                Some(outer),
                "extra fields",
                ByteRange::new(start, len as u64),
                NodeKind::Container,
                Some(Value::Bytes(len as u64)),
            );
        }
        let mut r = Reader::new(block);
        while r.remaining() >= 4 {
            let at = start + r.pos();
            let (Ok(id), Ok(size)) = (r.u16_le(), r.u16_le()) else {
                break;
            };
            let Ok(body) = r.bytes(size as usize) else {
                let range = ByteRange::new(at, end - at);
                self.tree
                    .warning(self.parent, "extra field runs past the extra block", range);
                break;
            };
            if id == 0x0001 {
                found = zip64_values(body, need);
            }
            if self.detail {
                let range = ByteRange::new(at, 4 + size as u64);
                self.tree.add(
                    Some(self.parent),
                    extra_name(id),
                    range,
                    NodeKind::Field,
                    Some(Value::Bytes(size as u64)),
                );
            }
        }
        self.parent = outer;
        Some(found)
    }
}

fn zip64_values(body: &[u8], need: Zip64Need) -> Zip64Values {
    let mut r = Reader::new(body);
    let mut take = |wanted: bool| wanted.then(|| r.u64_le().ok()).flatten();
    Zip64Values {
        uncompressed: take(need.uncompressed),
        compressed: take(need.compressed),
        offset: take(need.offset),
    }
}

pub(super) fn display(raw: &[u8]) -> String {
    let shown = &raw[..raw.len().min(MAX_NAME)];
    let mut s = String::from_utf8_lossy(shown).into_owned();
    if raw.len() > MAX_NAME {
        s.push('…');
    }
    s
}

pub(super) fn method_name(m: u16) -> &'static str {
    match m {
        0 => "stored",
        1 => "shrunk",
        6 => "imploded",
        8 => "deflate",
        9 => "Deflate64",
        12 => "bzip2",
        14 => "LZMA",
        93 => "zstd",
        95 => "xz",
        98 => "PPMd",
        99 => "AES-encrypted",
        _ => "unknown method",
    }
}

fn extra_name(id: u16) -> String {
    match id {
        0x0001 => "ZIP64 sizes".into(),
        0x000A => "NTFS times".into(),
        0x5455 => "extended timestamp".into(),
        0x7875 => "Unix owner".into(),
        0x7075 => "Unicode path".into(),
        0x6375 => "Unicode comment".into(),
        0x9901 => "AES encryption".into(),
        0xCAFE => "JAR marker".into(),
        0xD935 => "Android alignment".into(),
        _ => format!("extra 0x{id:04X}"),
    }
}

fn dos_datetime(time: u16, date: u16) -> String {
    let year = 1980 + (date >> 9);
    let month = (date >> 5) & 0x0F;
    let day = date & 0x1F;
    let hour = time >> 11;
    let minute = (time >> 5) & 0x3F;
    let second = (time & 0x1F) * 2;
    format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dos_dates_read_as_calendar_dates() {
        let time = (18 << 11) | (47 << 5) | 1;
        let date = ((2026 - 1980) << 9) | (9 << 5) | 23;
        assert_eq!(dos_datetime(time, date), "2026-09-23 18:47:02");
    }

    #[test]
    fn long_names_are_capped_for_display() {
        let long = vec![b'a'; MAX_NAME + 10];
        let shown = display(&long);
        assert!(shown.ends_with('…'));
        assert_eq!(shown.chars().count(), MAX_NAME + 1);
    }
}
