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
        if start.checked_add(n)? > self.end {
            return None;
        }
        self.at += n;
        Some(start)
    }

    pub(crate) fn num(&mut self, label: &str, n: usize) -> Option<u64> {
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
        let rest = self.data.get(self.at as usize..self.end as usize)?;
        let n = rest
            .iter()
            .position(|&b| b == 0)
            .map_or(rest.len(), |p| p + 1);
        let start = self.take(n as u64)?;
        let text: String =
            String::from_utf8_lossy(&rest[..n.saturating_sub(1).min(512)]).into_owned();
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
