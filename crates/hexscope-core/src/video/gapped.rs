//! A movie read without the bodies of its media data boxes: the picture and
//! sound in `mdat` can run to gigabytes, and nothing in them is needed to
//! say what the movie gives away or to make its clean copy.
//!
//! Each gap's box is walked as if its body were one stand-in byte; then
//! every position past a stand-in moves on by the length it stood for.

use super::{Scrub, VideoDocument, parse_video};
use crate::bmff::be;
use crate::model::{ByteRange, Value};

/// A box whose body was left out: where its header starts in the bytes
/// given, and how long the body was in the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Gap {
    pub at: u64,
    pub len: u64,
}

/// One left-out body, as the walk sees it.
#[derive(Debug, Clone, Copy)]
struct Hole {
    /// The box's header, in the bytes given.
    header_at: u64,
    header: u64,
    /// The box's size field as the file has it: its size, 1 for a 64-bit
    /// size after the type, or 0 for "to the end of the file".
    size32: u64,
    /// The stand-in byte, in the walked bytes.
    stand_in: u64,
    len: u64,
}

/// The bytes the walk reads: the bytes given, each gap's header shrunk to a
/// box of one stand-in byte.
pub(crate) struct Shrunk {
    pub(crate) bytes: Vec<u8>,
    holes: Vec<Hole>,
}

impl Shrunk {
    /// `None` when a gap is not a box header in `data` whose size is its
    /// header and `len`, or the gaps are out of order.
    pub(crate) fn new(data: &[u8], gaps: &[Gap]) -> Option<Self> {
        let mut bytes = Vec::with_capacity(data.len() + gaps.len());
        let mut holes = Vec::with_capacity(gaps.len());
        let mut copied = 0u64;
        for g in gaps {
            if g.at < copied || g.len == 0 {
                return None;
            }
            let size32 = be(data, g.at, 4)?;
            let header = if size32 == 1 { 16 } else { 8 };
            let size = match size32 {
                1 => be(data, g.at + 8, 8)?,
                0 => header + g.len,
                n => n,
            };
            let end = g.at.checked_add(header)?;
            let last = end == data.len() as u64;
            if size != header.checked_add(g.len)?
                || end > data.len() as u64
                || (size32 == 0 && !last)
            {
                return None;
            }
            bytes.extend_from_slice(data.get(copied as usize..end as usize)?);
            let box_at = bytes.len() - header as usize;
            if size32 == 1 {
                bytes[box_at + 8..box_at + 16].copy_from_slice(&(header + 1).to_be_bytes());
            } else {
                bytes[box_at..box_at + 4].copy_from_slice(&(header as u32 + 1).to_be_bytes());
            }
            holes.push(Hole {
                header_at: box_at as u64,
                header,
                size32,
                stand_in: bytes.len() as u64,
                len: g.len,
            });
            bytes.push(0);
            copied = end;
        }
        bytes.extend_from_slice(data.get(copied as usize..)?);
        Some(Self { bytes, holes })
    }

    /// Where a position in the walked bytes is in the file.
    fn in_file(&self, at: u64) -> u64 {
        let past: u64 = self
            .holes
            .iter()
            .take_while(|h| h.stand_in < at)
            .map(|h| h.len - 1)
            .fold(0, u64::saturating_add);
        at.saturating_add(past)
    }

    fn range_in_file(&self, r: ByteRange) -> ByteRange {
        let start = self.in_file(r.start);
        ByteRange::new(start, self.in_file(r.end()) - start)
    }

    /// Puts the file's own headers back into a copy of the walked bytes and
    /// takes the stand-ins out: the bytes given, changed where the copy
    /// changed them. `None` when a header or stand-in was changed.
    pub(crate) fn unshrink(&self, walked: &[u8]) -> Option<Vec<u8>> {
        if walked.len() != self.bytes.len() {
            return None;
        }
        let mut out = Vec::with_capacity(walked.len());
        let mut from = 0usize;
        for h in &self.holes {
            let (header_at, stand_in) = (h.header_at as usize, h.stand_in as usize);
            if walked.get(header_at..=stand_in)? != self.bytes.get(header_at..=stand_in)? {
                return None;
            }
            out.extend_from_slice(walked.get(from..header_at)?);
            out.extend_from_slice(&self.header(h));
            from = stand_in + 1;
        }
        out.extend_from_slice(walked.get(from..)?);
        Some(out)
    }

    /// A gap's header as the file has it.
    fn header(&self, h: &Hole) -> Vec<u8> {
        let at = h.header_at as usize;
        let mut header = self.bytes[at..at + h.header as usize].to_vec();
        if h.size32 == 1 {
            header[8..16].copy_from_slice(&(h.header + h.len).to_be_bytes());
        } else {
            header[..4].copy_from_slice(&(h.size32 as u32).to_be_bytes());
        }
        header
    }

    /// Moves a document of the walked bytes to the file's positions, and
    /// gives each gap's box its own size again.
    fn moved_to_file(&self, mut doc: VideoDocument) -> VideoDocument {
        for h in &self.holes {
            let Some(node) = doc
                .tree
                .nodes()
                .iter()
                .find(|n| n.parent == Some(0) && n.range.start == h.header_at)
            else {
                continue;
            };
            let size = h.header + h.len;
            let children = node.children.clone();
            doc.tree.set_value(node.id, Some(Value::Bytes(size)));
            for c in children {
                let value = match doc.tree.get(c).label.as_str() {
                    "size" => Value::U64(h.size32),
                    "largeSize" => Value::U64(size),
                    "body" => Value::Bytes(h.len),
                    _ => continue,
                };
                doc.tree.set_value(c, Some(value));
            }
        }
        for id in 0..doc.tree.len() as u32 {
            let range = self.range_in_file(doc.tree.get(id).range);
            doc.tree.set_range(id, range);
        }
        for s in &mut doc.scrub {
            *s = match *s {
                Scrub::Free {
                    at,
                    len,
                    header,
                    what,
                } => {
                    let r = self.range_in_file(ByteRange::new(at, len));
                    Scrub::Free {
                        at: r.start,
                        len: r.len,
                        header,
                        what,
                    }
                }
                Scrub::Zero { range, what } => Scrub::Zero {
                    range: self.range_in_file(range),
                    what,
                },
            };
        }
        doc
    }
}

/// Reads a movie from `data`, the file without the bodies of the boxes in
/// `gaps`. Positions in the document are the file's. Gaps that do not match
/// the boxes in `data` are ignored, and the bytes read as they are.
pub fn parse_video_gapped(data: &[u8], gaps: &[Gap]) -> VideoDocument {
    match Shrunk::new(data, gaps) {
        Some(s) => s.moved_to_file(parse_video(&s.bytes)),
        None => parse_video(data),
    }
}
