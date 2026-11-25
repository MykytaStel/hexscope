//! ISO base media boxes (ISO/IEC 14496-12), shared by HEIF images and MP4
//! and QuickTime movies: the box walk, and a reader for the big-endian
//! fields inside a box that adds a node for each.

use crate::model::{ByteRange, NodeId, NodeKind, ParseTree, Value};
use crate::reader::Reader;

/// What a format does with the body of each box it meets.
pub(crate) trait BoxBody {
    /// Decodes one box's body, `span` being `(body start, box end)`. `None`
    /// when its fields do not fit in it.
    fn decode(
        &mut self,
        tree: &mut ParseTree,
        node: NodeId,
        data: &[u8],
        typ: &str,
        span: (u64, u64),
        depth: u32,
    ) -> Option<()>;
}

pub(crate) fn be(data: &[u8], at: u64, n: usize) -> Option<u64> {
    if n > 8 || at > data.len() as u64 {
        return None;
    }
    let mut r = Reader::new(data);
    r.seek(at);
    let b = r.bytes(n).ok()?;
    Some(b.iter().fold(0u64, |v, &x| (v << 8) | x as u64))
}

/// A box type as text: four printable letters, QuickTime's `©` names
/// (0xA9 then three letters) as such, anything else in hex.
pub(crate) fn fourcc(b: &[u8]) -> String {
    let printable = |c: &u8| c.is_ascii_graphic() || *c == b' ';
    if b.len() == 4 && b.iter().all(printable) {
        String::from_utf8_lossy(b).into_owned()
    } else if b.len() == 4 && b[0] == 0xA9 && b[1..].iter().all(printable) {
        format!("©{}", String::from_utf8_lossy(&b[1..]))
    } else {
        format!(
            "0x{}",
            b.iter().map(|x| format!("{x:02X}")).collect::<String>()
        )
    }
}

/// Reads big-endian fields inside one box, adding a node for each.
pub(crate) struct Fields<'t, 'd> {
    pub tree: &'t mut ParseTree,
    pub parent: NodeId,
    pub data: &'d [u8],
    pub at: u64,
    pub end: u64,
}

impl Fields<'_, '_> {
    pub(crate) fn take(&mut self, n: u64) -> Option<u64> {
        let start = self.at;
        if start.checked_add(n)? > self.end.min(self.data.len() as u64) {
            return None;
        }
        self.at += n;
        Some(start)
    }

    pub(crate) fn num(&mut self, label: &str, n: usize) -> Option<u64> {
        if n > 8 {
            return None;
        }
        let start = self.take(n as u64)?;
        let v = be(self.data, start, n)?;
        self.tree.add(
            Some(self.parent),
            label,
            ByteRange::new(start, n as u64),
            NodeKind::Field,
            Some(Value::U64(v)),
        );
        Some(v)
    }

    /// Record a fixed-width opaque field without truncating it into a u64.
    pub(crate) fn opaque(&mut self, label: &str, n: u64) -> Option<()> {
        let start = self.take(n)?;
        self.tree.add(
            Some(self.parent),
            label,
            ByteRange::new(start, n),
            NodeKind::Field,
            Some(Value::Bytes(n)),
        );
        Some(())
    }

    /// An n-byte number that may be 0 bytes wide, as `iloc`'s are.
    pub(crate) fn var(&mut self, label: &str, n: u8) -> Option<u64> {
        if n == 0 {
            Some(0)
        } else {
            self.num(label, n as usize)
        }
    }

    pub(crate) fn cc(&mut self, label: &str) -> Option<String> {
        let start = self.take(4)?;
        let s = fourcc(self.data.get(start as usize..start as usize + 4)?);
        self.tree.add(
            Some(self.parent),
            label,
            ByteRange::new(start, 4),
            NodeKind::Field,
            Some(Value::Text(s.clone())),
        );
        Some(s)
    }

    /// A NUL-terminated string, or the rest of the box when it has no NUL.
    pub(crate) fn cstr(&mut self, label: &str) -> Option<String> {
        let start = usize::try_from(self.at).ok()?;
        let end = usize::try_from(self.end).ok()?;
        let rest = self.data.get(start..end)?;
        let text_len = rest.iter().position(|&b| b == 0).unwrap_or(rest.len());
        let n = if text_len < rest.len() {
            text_len + 1
        } else {
            text_len
        };
        let start = self.take(n as u64)?;
        let text = String::from_utf8_lossy(&rest[..text_len.min(512)]).into_owned();
        self.tree.add(
            Some(self.parent),
            label,
            ByteRange::new(start, n as u64),
            NodeKind::Field,
            Some(Value::Text(text.clone())),
        );
        Some(text)
    }

    pub(crate) fn full_box(&mut self) -> Option<u8> {
        let v = self.num("version", 1)? as u8;
        self.num("flags", 3)?;
        Some(v)
    }

    pub(crate) fn rest(&mut self, label: &str) {
        if self.at < self.end {
            let len = self.end - self.at;
            self.tree.add(
                Some(self.parent),
                label,
                ByteRange::new(self.at, len),
                NodeKind::Field,
                Some(Value::Bytes(len)),
            );
            self.at = self.end;
        }
    }
}

/// Reads the boxes in `[start, end)` under `parent`: each box's header as
/// fields, then its body through `body`. Damage ends the walk at that box.
pub(crate) fn walk<B: BoxBody + ?Sized>(
    tree: &mut ParseTree,
    parent: NodeId,
    data: &[u8],
    start: u64,
    end: u64,
    depth: u32,
    body: &mut B,
) {
    if start > end || end > data.len() as u64 {
        tree.error(
            parent,
            "box container range is outside the source",
            ByteRange::new(start.min(data.len() as u64), 0),
        );
        return;
    }
    let mut pos = start;
    while pos < end {
        if end - pos < 8 {
            tree.error(
                parent,
                "a box header is cut short",
                ByteRange::new(pos, end - pos),
            );
            return;
        }
        let size32 = be(data, pos, 4).unwrap_or(0);
        let typ = data.get(pos as usize + 4..pos as usize + 8).unwrap_or(&[]);
        let (size, header) = match size32 {
            1 => match be(data, pos + 8, 8) {
                Some(s) if pos + 16 <= end => (s, 16),
                _ => {
                    tree.error(
                        parent,
                        "a box's 64-bit size is cut short",
                        ByteRange::new(pos, end - pos),
                    );
                    return;
                }
            },
            0 => (end - pos, 8),
            n => (n, 8),
        };
        let name = fourcc(typ);
        if size < header {
            tree.error(
                parent,
                format!("{name} box size {size} is smaller than its header"),
                ByteRange::new(pos, 8),
            );
            return;
        }
        if size > end - pos {
            tree.error(
                parent,
                format!("{name} box runs past the end of its container"),
                ByteRange::new(pos, end - pos),
            );
            return;
        }
        let node = tree.add(
            Some(parent),
            name.clone(),
            ByteRange::new(pos, size),
            NodeKind::Container,
            Some(Value::Bytes(size)),
        );
        let mut f = Fields {
            tree,
            parent: node,
            data,
            at: pos,
            end: pos + size,
        };
        f.num("size", 4);
        f.cc("type");
        if size32 == 1 {
            f.num("largeSize", 8);
        }
        let span = (pos + header, pos + size);
        if body.decode(tree, node, data, &name, span, depth).is_none() {
            tree.error(
                node,
                format!("{name} box is shorter than its fields"),
                ByteRange::new(pos, size),
            );
        }
        pos += size;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_integer_reads_that_do_not_fit() {
        assert_eq!(be(&[0; 9], 0, 9), None);
        assert_eq!(be(&[0; 9], u64::MAX, 0), None);
        assert_eq!(be(&[1, 2], 0, 2), Some(258));
    }

    #[test]
    fn box_strings_keep_the_last_byte_without_a_terminator() {
        for (data, expected) in [
            (&b"abc"[..], "abc"),
            (&b"abc\0"[..], "abc"),
            (&b"\0"[..], ""),
            (&b""[..], ""),
        ] {
            let mut tree = ParseTree::new();
            let root = tree.add(
                None,
                "box",
                ByteRange::new(0, data.len() as u64),
                NodeKind::Container,
                None,
            );
            let mut fields = Fields {
                tree: &mut tree,
                parent: root,
                data,
                at: 0,
                end: data.len() as u64,
            };
            assert_eq!(fields.cstr("name").as_deref(), Some(expected));
            assert_eq!(fields.at, data.len() as u64);
        }
    }

    #[test]
    fn wide_fields_are_opaque_and_keep_their_full_range() {
        let mut tree = ParseTree::new();
        let root = tree.add(
            None,
            "box",
            ByteRange::new(0, 40),
            NodeKind::Container,
            None,
        );
        let mut fields = Fields {
            tree: &mut tree,
            parent: root,
            data: &[0; 40],
            at: 0,
            end: 40,
        };
        assert_eq!(fields.num("matrix", 36), None);
        assert_eq!(fields.at, 0);
        assert_eq!(fields.opaque("matrix", 36), Some(()));
        assert_eq!(fields.num("width", 4), Some(0));
        assert_eq!(fields.tree.get(1).range, ByteRange::new(0, 36));
        assert_eq!(fields.tree.get(1).value, Some(Value::Bytes(36)));
        assert_eq!(fields.at, 40);
    }

    #[test]
    fn fields_cannot_advance_past_the_source() {
        let mut tree = ParseTree::new();
        let mut fields = Fields {
            tree: &mut tree,
            parent: 0,
            data: &[1, 2],
            at: 1,
            end: 10,
        };
        assert_eq!(fields.take(2), None);
        assert_eq!(fields.at, 1);
    }
}
