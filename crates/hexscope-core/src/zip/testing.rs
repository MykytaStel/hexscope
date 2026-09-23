//! Builds ZIP archives byte by byte, so tests can state exactly what an
//! archiver wrote — including the things archivers are not supposed to write.

use crate::crc32::crc32;
use flate2::Compression;
use flate2::write::DeflateEncoder;
use std::io::Write;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Descriptor {
    None,
    WithSignature,
    NoSignature,
}

#[derive(Clone)]
pub struct Entry {
    pub name: String,
    pub data: Vec<u8>,
    /// 0 stores, 8 deflates; anything else stores the bytes under that number.
    pub method: u16,
    /// Extra flag bits, OR'ed in (bit 0 marks the entry encrypted).
    pub flags: u16,
    pub descriptor: Descriptor,
    /// Sizes in a ZIP64 extra field, with 0xFFFFFFFF in the header fields.
    pub zip64: bool,
}

impl Entry {
    pub fn new(name: &str, data: &[u8], method: u16) -> Self {
        Self {
            name: name.to_string(),
            data: data.to_vec(),
            method,
            flags: 0,
            descriptor: Descriptor::None,
            zip64: false,
        }
    }
}

#[derive(Clone, Default)]
pub struct Archive {
    pub entries: Vec<Entry>,
    pub comment: Vec<u8>,
    /// Bytes before the archive. Offsets are written relative to the
    /// archive's start, as an unadjusted self-extractor has them.
    pub prefix: Vec<u8>,
    pub zip64_eocd: bool,
}

/// The archive and where its records landed, as file offsets.
pub struct Built {
    pub bytes: Vec<u8>,
    pub local: Vec<u64>,
    pub central: Vec<u64>,
    pub eocd: u64,
}

// 2026-09-23 18:47:02 in DOS form.
const DOS_TIME: u16 = (18 << 11) | (47 << 5) | 1;
const DOS_DATE: u16 = ((2026 - 1980) << 9) | (9 << 5) | 23;

fn stored_form(e: &Entry) -> Vec<u8> {
    if e.method == 8 {
        let mut enc = DeflateEncoder::new(Vec::new(), Compression::best());
        enc.write_all(&e.data).unwrap();
        enc.finish().unwrap()
    } else {
        e.data.clone()
    }
}

fn u16le(out: &mut Vec<u8>, v: u16) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn u32le(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn u64le(out: &mut Vec<u8>, v: u64) {
    out.extend_from_slice(&v.to_le_bytes());
}

pub fn build(a: &Archive) -> Built {
    let mut out = a.prefix.clone();
    let base = out.len() as u64;
    let mut local = Vec::new();
    let mut records = Vec::new();

    for e in &a.entries {
        let body = stored_form(e);
        let crc = crc32(&e.data);
        let flags = e.flags
            | if e.descriptor == Descriptor::None {
                0
            } else {
                1 << 3
            };
        let deferred = e.descriptor != Descriptor::None;
        let rel = out.len() as u64 - base;
        local.push(out.len() as u64);

        let zip64_extra = |sizes: bool| {
            let mut x = Vec::new();
            u16le(&mut x, 1);
            u16le(&mut x, 16);
            u64le(&mut x, if sizes { e.data.len() as u64 } else { 0 });
            u64le(&mut x, if sizes { body.len() as u64 } else { 0 });
            x
        };
        let (comp32, uncomp32) = if e.zip64 {
            (u32::MAX, u32::MAX)
        } else if deferred {
            (0, 0)
        } else {
            (body.len() as u32, e.data.len() as u32)
        };
        let local_extra = if e.zip64 {
            zip64_extra(!deferred)
        } else {
            Vec::new()
        };

        u32le(&mut out, 0x0403_4B50);
        u16le(&mut out, if e.zip64 { 45 } else { 20 });
        u16le(&mut out, flags);
        u16le(&mut out, e.method);
        u16le(&mut out, DOS_TIME);
        u16le(&mut out, DOS_DATE);
        u32le(&mut out, if deferred { 0 } else { crc });
        u32le(&mut out, comp32);
        u32le(&mut out, uncomp32);
        u16le(&mut out, e.name.len() as u16);
        u16le(&mut out, local_extra.len() as u16);
        out.extend_from_slice(e.name.as_bytes());
        out.extend_from_slice(&local_extra);
        out.extend_from_slice(&body);

        if deferred {
            if e.descriptor == Descriptor::WithSignature {
                u32le(&mut out, 0x0807_4B50);
            }
            u32le(&mut out, crc);
            if e.zip64 {
                u64le(&mut out, body.len() as u64);
                u64le(&mut out, e.data.len() as u64);
            } else {
                u32le(&mut out, body.len() as u32);
                u32le(&mut out, e.data.len() as u32);
            }
        }

        let central_extra = if e.zip64 {
            zip64_extra(true)
        } else {
            Vec::new()
        };
        let (ccomp, cuncomp) = if e.zip64 {
            (u32::MAX, u32::MAX)
        } else {
            (body.len() as u32, e.data.len() as u32)
        };
        let mut r = Vec::new();
        u32le(&mut r, 0x0201_4B50);
        u16le(&mut r, 0x031E);
        u16le(&mut r, if e.zip64 { 45 } else { 20 });
        u16le(&mut r, flags);
        u16le(&mut r, e.method);
        u16le(&mut r, DOS_TIME);
        u16le(&mut r, DOS_DATE);
        u32le(&mut r, crc);
        u32le(&mut r, ccomp);
        u32le(&mut r, cuncomp);
        u16le(&mut r, e.name.len() as u16);
        u16le(&mut r, central_extra.len() as u16);
        u16le(&mut r, 0);
        u16le(&mut r, 0);
        u16le(&mut r, 0);
        u32le(&mut r, 0);
        u32le(&mut r, rel as u32);
        r.extend_from_slice(e.name.as_bytes());
        r.extend_from_slice(&central_extra);
        records.push(r);
    }

    let cd_rel = out.len() as u64 - base;
    let mut central = Vec::new();
    for r in &records {
        central.push(out.len() as u64);
        out.extend_from_slice(r);
    }
    let cd_size = out.len() as u64 - base - cd_rel;
    let n = a.entries.len() as u64;

    if a.zip64_eocd {
        let record_rel = out.len() as u64 - base;
        u32le(&mut out, 0x0606_4B50);
        u64le(&mut out, 44);
        u16le(&mut out, 45);
        u16le(&mut out, 45);
        u32le(&mut out, 0);
        u32le(&mut out, 0);
        u64le(&mut out, n);
        u64le(&mut out, n);
        u64le(&mut out, cd_size);
        u64le(&mut out, cd_rel);
        u32le(&mut out, 0x0706_4B50);
        u32le(&mut out, 0);
        u64le(&mut out, record_rel);
        u32le(&mut out, 1);
    }

    let eocd = out.len() as u64;
    u32le(&mut out, 0x0605_4B50);
    u16le(&mut out, 0);
    u16le(&mut out, 0);
    let (n16, size32, off32) = if a.zip64_eocd {
        (u16::MAX, u32::MAX, u32::MAX)
    } else {
        (n as u16, cd_size as u32, cd_rel as u32)
    };
    u16le(&mut out, n16);
    u16le(&mut out, n16);
    u32le(&mut out, size32);
    u32le(&mut out, off32);
    u16le(&mut out, a.comment.len() as u16);
    out.extend_from_slice(&a.comment);

    Built {
        bytes: out,
        local,
        central,
        eocd,
    }
}
