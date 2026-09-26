//! A ZIP written anew from entries whose bytes are known: each with its
//! sizes in its local header (no data descriptors), a central directory,
//! and its end record. The clean copy and the repair both write archives
//! this way.

use super::ZipEntry;
use crate::model::ParseTree;

/// One entry to write.
pub(crate) struct Part<'a> {
    /// The name's bytes, as the original header held them.
    pub name: Vec<u8>,
    pub flags: u16,
    pub method: u16,
    /// DOS time and date, as the original header held them.
    pub time: [u8; 4],
    pub crc: u32,
    pub uncompressed: u64,
    /// The stored bytes: compressed with `method`.
    pub body: &'a [u8],
}

/// An entry's name bytes, time and extra field length, from its local
/// header; `None` when the header is not all there.
pub(crate) fn header_of(
    data: &[u8],
    tree: &ParseTree,
    e: &ZipEntry,
) -> Option<(Vec<u8>, [u8; 4], u16)> {
    let at = tree.try_get(e.node)?.range.start as usize;
    let header = data.get(at..at.checked_add(30)?)?;
    let name_len = u16::from_le_bytes([header[26], header[27]]) as usize;
    let extra = u16::from_le_bytes([header[28], header[29]]);
    let name = data.get(at + 30..at + 30 + name_len)?.to_vec();
    Some((
        name,
        [header[10], header[11], header[12], header[13]],
        extra,
    ))
}

fn le16(out: &mut Vec<u8>, v: u16) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn le32(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}

/// The archive's bytes; `None` past what a plain ZIP can say (4 GB, 65,535
/// entries).
pub(crate) fn assemble(parts: &[Part]) -> Option<Vec<u8>> {
    if parts.len() >= 0xFFFF {
        return None;
    }
    let mut out = Vec::new();
    let mut offsets = Vec::with_capacity(parts.len());
    for p in parts {
        offsets.push(u32::try_from(out.len()).ok()?);
        le32(&mut out, 0x0403_4B50);
        le16(&mut out, 20);
        // No data descriptor: the sizes are known and go in the header.
        le16(&mut out, p.flags & !(1 << 3));
        le16(&mut out, p.method);
        out.extend_from_slice(&p.time);
        le32(&mut out, p.crc);
        le32(&mut out, u32::try_from(p.body.len()).ok()?);
        le32(&mut out, u32::try_from(p.uncompressed).ok()?);
        le16(&mut out, u16::try_from(p.name.len()).ok()?);
        le16(&mut out, 0);
        out.extend_from_slice(&p.name);
        out.extend_from_slice(p.body);
    }
    let cd_start = u32::try_from(out.len()).ok()?;
    for (p, &offset) in parts.iter().zip(&offsets) {
        le32(&mut out, 0x0201_4B50);
        le16(&mut out, 20);
        le16(&mut out, 20);
        le16(&mut out, p.flags & !(1 << 3));
        le16(&mut out, p.method);
        out.extend_from_slice(&p.time);
        le32(&mut out, p.crc);
        le32(&mut out, p.body.len() as u32);
        le32(&mut out, p.uncompressed as u32);
        le16(&mut out, p.name.len() as u16);
        le16(&mut out, 0); // extra
        le16(&mut out, 0); // comment
        le16(&mut out, 0); // disk
        le16(&mut out, 0); // internal attributes
        le32(&mut out, 0); // external attributes
        le32(&mut out, offset);
        out.extend_from_slice(&p.name);
    }
    let cd_size = u32::try_from(out.len()).ok()? - cd_start;
    le32(&mut out, 0x0605_4B50);
    le16(&mut out, 0);
    le16(&mut out, 0);
    le16(&mut out, parts.len() as u16);
    le16(&mut out, parts.len() as u16);
    le32(&mut out, cd_size);
    le32(&mut out, cd_start);
    le16(&mut out, 0);
    Some(out)
}
