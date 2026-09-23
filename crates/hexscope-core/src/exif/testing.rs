//! Builds TIFF blocks byte by byte, in either byte order, so tests can
//! state exactly what a camera wrote.
use super::ByteOrder;

pub enum V {
    Ascii(&'static str),
    Byte(Vec<u8>),
    Short(Vec<u16>),
    Long(Vec<u32>),
    Rational(Vec<(u32, u32)>),
    Undefined(Vec<u8>),
}

pub struct Spec {
    pub order: ByteOrder,
    pub ifd0: Vec<(u16, V)>,
    pub exif: Vec<(u16, V)>,
    pub gps: Vec<(u16, V)>,
    /// Overrides IFD0's next-IFD pointer, for building loops.
    pub ifd0_next: u32,
}

impl Spec {
    pub fn new(order: ByteOrder) -> Self {
        Self {
            order,
            ifd0: Vec::new(),
            exif: Vec::new(),
            gps: Vec::new(),
            ifd0_next: 0,
        }
    }
}

fn u16b(o: ByteOrder, v: u16) -> [u8; 2] {
    match o {
        ByteOrder::Little => v.to_le_bytes(),
        ByteOrder::Big => v.to_be_bytes(),
    }
}

fn u32b(o: ByteOrder, v: u32) -> [u8; 4] {
    match o {
        ByteOrder::Little => v.to_le_bytes(),
        ByteOrder::Big => v.to_be_bytes(),
    }
}

fn encode(o: ByteOrder, v: &V) -> (u16, u32, Vec<u8>) {
    match v {
        V::Ascii(s) => {
            let mut b = s.as_bytes().to_vec();
            b.push(0);
            (2, b.len() as u32, b)
        }
        V::Byte(b) => (1, b.len() as u32, b.clone()),
        V::Undefined(b) => (7, b.len() as u32, b.clone()),
        V::Short(x) => (
            3,
            x.len() as u32,
            x.iter().flat_map(|v| u16b(o, *v)).collect(),
        ),
        V::Long(x) => (
            4,
            x.len() as u32,
            x.iter().flat_map(|v| u32b(o, *v)).collect(),
        ),
        V::Rational(x) => (
            5,
            x.len() as u32,
            x.iter()
                .flat_map(|(n, d)| u32b(o, *n).into_iter().chain(u32b(o, *d)))
                .collect(),
        ),
    }
}

/// Layout: header, IFD0, Exif IFD, GPS IFD, then every out-of-line value.
pub fn build(spec: Spec) -> Vec<u8> {
    let o = spec.order;
    let mut ifd0: Vec<(u16, V)> = spec.ifd0;
    let has_exif = !spec.exif.is_empty();
    let has_gps = !spec.gps.is_empty();
    if has_exif {
        ifd0.push((0x8769, V::Long(vec![0])));
    }
    if has_gps {
        ifd0.push((0x8825, V::Long(vec![0])));
    }
    let size = |n: usize| 2 + 12 * n as u32 + 4;
    let off0 = 8u32;
    let off_exif = off0 + size(ifd0.len());
    let off_gps = off_exif + if has_exif { size(spec.exif.len()) } else { 0 };
    let mut values_at = off_gps + if has_gps { size(spec.gps.len()) } else { 0 };
    for (tag, v) in &mut ifd0 {
        match tag {
            0x8769 => *v = V::Long(vec![off_exif]),
            0x8825 => *v = V::Long(vec![off_gps]),
            _ => {}
        }
    }

    let mut out = Vec::new();
    out.extend_from_slice(if o == ByteOrder::Little { b"II" } else { b"MM" });
    out.extend_from_slice(&u16b(o, 42));
    out.extend_from_slice(&u32b(o, off0));
    let mut values = Vec::new();

    let mut write_ifd = |out: &mut Vec<u8>, entries: &[(u16, V)], next: u32| {
        out.extend_from_slice(&u16b(o, entries.len() as u16));
        for (tag, v) in entries {
            let (typ, count, bytes) = encode(o, v);
            out.extend_from_slice(&u16b(o, *tag));
            out.extend_from_slice(&u16b(o, typ));
            out.extend_from_slice(&u32b(o, count));
            if bytes.len() <= 4 {
                let mut inline = bytes.clone();
                inline.resize(4, 0);
                out.extend_from_slice(&inline);
            } else {
                out.extend_from_slice(&u32b(o, values_at));
                values_at += bytes.len() as u32;
                values.extend_from_slice(&bytes);
            }
        }
        out.extend_from_slice(&u32b(o, next));
    };
    write_ifd(&mut out, &ifd0, spec.ifd0_next);
    if has_exif {
        write_ifd(&mut out, &spec.exif, 0);
    }
    if has_gps {
        write_ifd(&mut out, &spec.gps, 0);
    }
    out.extend_from_slice(&values);
    out
}
