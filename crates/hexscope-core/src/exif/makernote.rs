//! Makers' notes: the block EXIF leaves to the camera's maker (tag 0x927C),
//! in a format of each maker's own. Most are a TIFF IFD like EXIF's, with a
//! header before it and offsets counted from a place of the maker's
//! choosing. None is published; the layouts and tag names here are those
//! ExifTool documents, which every tool that reads them follows.
//!
//! What they hold that EXIF does not: more serial numbers, the owner's name,
//! how many photos the camera has taken, and on an iPhone, IDs that tie a
//! photo to its Live Photo video and the rest of its burst, and how long the
//! phone had been on — which ties together photos taken between restarts.

use super::{ByteOrder, Fact, Found, Ifd, Tiff, render, type_size};
use crate::model::{ByteRange, NodeId, NodeKind, ParseTree, Value};

/// Entries read at most from a maker's IFD.
const MAX_ENTRIES: u64 = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Maker {
    Apple,
    Canon,
    Nikon,
    Fujifilm,
}

impl Maker {
    fn name(self) -> &'static str {
        match self {
            Maker::Apple => "Apple",
            Maker::Canon => "Canon",
            Maker::Nikon => "Nikon",
            Maker::Fujifilm => "Fujifilm",
        }
    }
}

/// The names ExifTool gives each maker's tags, for the ones read here.
pub(super) fn tag_name(maker: Maker, tag: u16) -> Option<&'static str> {
    Some(match (maker, tag) {
        (Maker::Apple, 0x0001) => "MakerNoteVersion",
        (Maker::Apple, 0x0003) => "RunTime",
        (Maker::Apple, 0x0008) => "AccelerationVector",
        (Maker::Apple, 0x000A) => "HDRImageType",
        (Maker::Apple, 0x000B) => "BurstUUID",
        (Maker::Apple, 0x000C) => "FocusDistanceRange",
        (Maker::Apple, 0x000F) => "OISMode",
        (Maker::Apple, 0x0011) => "ContentIdentifier",
        (Maker::Apple, 0x0014) => "ImageCaptureType",
        (Maker::Apple, 0x0015) => "ImageUniqueID",
        (Maker::Apple, 0x0017) => "LivePhotoVideoIndex",
        (Maker::Apple, 0x002E) => "CameraType",
        (Maker::Canon, 0x0001) => "CanonCameraSettings",
        (Maker::Canon, 0x0002) => "CanonFocalLength",
        (Maker::Canon, 0x0004) => "CanonShotInfo",
        (Maker::Canon, 0x0006) => "CanonImageType",
        (Maker::Canon, 0x0007) => "CanonFirmwareVersion",
        (Maker::Canon, 0x0008) => "FileNumber",
        (Maker::Canon, 0x0009) => "OwnerName",
        (Maker::Canon, 0x000C) => "SerialNumber",
        (Maker::Canon, 0x000D) => "CanonCameraInfo",
        (Maker::Canon, 0x0010) => "CanonModelID",
        (Maker::Canon, 0x0095) => "LensModel",
        (Maker::Canon, 0x0096) => "InternalSerialNumber",
        (Maker::Nikon, 0x0001) => "MakerNoteVersion",
        (Maker::Nikon, 0x0002) => "ISO",
        (Maker::Nikon, 0x0004) => "Quality",
        (Maker::Nikon, 0x0005) => "WhiteBalance",
        (Maker::Nikon, 0x0007) => "FocusMode",
        (Maker::Nikon, 0x001D) => "SerialNumber",
        (Maker::Nikon, 0x0084) => "Lens",
        (Maker::Nikon, 0x0091) => "ShotInfo",
        (Maker::Nikon, 0x0098) => "LensData",
        (Maker::Nikon, 0x00A0) => "SerialNumber",
        (Maker::Nikon, 0x00A7) => "ShutterCount",
        (Maker::Fujifilm, 0x0000) => "Version",
        (Maker::Fujifilm, 0x0010) => "InternalSerialNumber",
        (Maker::Fujifilm, 0x1000) => "Quality",
        (Maker::Fujifilm, 0x1001) => "Sharpness",
        (Maker::Fujifilm, 0x1002) => "WhiteBalance",
        (Maker::Fujifilm, 0x1003) => "Saturation",
        _ => return None,
    })
}

/// Every tag named above, for the test that checks each is explained.
#[cfg(test)]
pub(super) fn all_names() -> Vec<&'static str> {
    let mut out = Vec::new();
    for maker in [Maker::Apple, Maker::Canon, Maker::Nikon, Maker::Fujifilm] {
        for tag in 0..=u16::MAX {
            if let Some(n) = tag_name(maker, tag) {
                out.push(n);
            }
        }
    }
    out
}

/// Whose note this is, the view its offsets count from, where its IFD is in
/// that view, and how long its header is.
fn locate<'a>(t: &Tiff<'a>, off: u64, len: u64, make: &str) -> Option<(Maker, Tiff<'a>, u64, u64)> {
    let start = usize::try_from(off).ok()?;
    let end = usize::try_from(off.checked_add(len)?).ok()?;
    let bytes = t.data.get(start..end)?;
    let base = t.base + off;
    // "Apple iOS\0", a version, "MM", then the IFD; offsets from the note's start.
    if bytes.starts_with(b"Apple iOS\0") {
        let m = Tiff {
            data: bytes,
            base,
            order: ByteOrder::Big,
        };
        return Some((Maker::Apple, m, 14, 14));
    }
    // "Nikon\0", a version, then a TIFF header of its own; offsets from it.
    if bytes.starts_with(b"Nikon\0") {
        let inner = bytes.get(10..)?;
        let order = match inner.get(..2)? {
            b"II" => ByteOrder::Little,
            b"MM" => ByteOrder::Big,
            _ => return None,
        };
        let m = Tiff {
            data: inner,
            base: base + 10,
            order,
        };
        let ifd = m.u32(&mut m.at(4))? as u64;
        return Some((Maker::Nikon, m, ifd, 18));
    }
    // "FUJIFILM", then where the IFD is; little-endian, from the note's start.
    if bytes.starts_with(b"FUJIFILM") {
        let ifd = u32::from_le_bytes(bytes.get(8..12)?.try_into().ok()?) as u64;
        let m = Tiff {
            data: bytes,
            base,
            order: ByteOrder::Little,
        };
        return Some((Maker::Fujifilm, m, ifd, 12));
    }
    // Canon: no header; an IFD whose offsets count from EXIF's own start.
    if make.trim().to_ascii_lowercase().starts_with("canon") {
        let m = Tiff {
            data: t.data,
            base: t.base,
            order: t.order,
        };
        return Some((Maker::Canon, m, off, 0));
    }
    None
}

fn inside(inner: ByteRange, outer: ByteRange) -> bool {
    inner.start >= outer.start && inner.end() <= outer.end()
}

/// Reads the maker's note at `off` in `t` — `len` bytes, shown by `note` —
/// adding its IFD under `note` and what it reveals to `found`.
pub(super) fn parse(
    tree: &mut ParseTree,
    note: NodeId,
    t: &Tiff,
    off: u64,
    len: u64,
    make: &str,
    found: &mut Found,
) {
    let Some((maker, m, ifd, header)) = locate(t, off, len, make) else {
        return;
    };
    let whole = t.range(off, len);
    tree.set_value(
        note,
        Some(Value::Text(format!("{}'s format", maker.name()))),
    );
    if header > 0 {
        tree.add(
            Some(note),
            "maker header",
            t.range(off, header),
            NodeKind::Field,
            Some(Value::Text(maker.name().to_string())),
        );
    }
    let mut r = m.at(ifd);
    let declared = m.u16(&mut r).filter(|_| m.fits(ifd, 2));
    let Some(declared) = declared.filter(|_| inside(m.range(ifd, 2), whole)) else {
        tree.warning(
            note,
            "the maker's IFD is outside its note",
            t.range(off, header.min(len)),
        );
        return;
    };
    let room = m.len().saturating_sub(ifd + 2) / 12;
    let count = (declared as u64).min(room).min(MAX_ENTRIES);
    let mut span = m.range(ifd, 2 + 12 * count);
    if !inside(span, whole) {
        // Read only the entries inside the note.
        let fit = whole.end().saturating_sub(span.start + 2) / 12;
        span = m.range(ifd, 2 + 12 * fit.min(count));
    }
    let count = (span.len - 2) / 12;
    let node = tree.add(
        Some(note),
        format!("{} IFD", maker.name()),
        span,
        NodeKind::Container,
        Some(Value::Text(format!(
            "{count} {}",
            if count == 1 { "entry" } else { "entries" }
        ))),
    );
    tree.add(
        Some(node),
        "entryCount",
        m.range(ifd, 2),
        NodeKind::Field,
        Some(Value::U64(declared as u64)),
    );
    for n in 0..count {
        let eoff = ifd + 2 + 12 * n;
        let Some(e) = m.entry(eoff) else { break };
        let label =
            tag_name(maker, e.tag).map_or_else(|| format!("tag 0x{:04X}", e.tag), str::to_string);
        let special = match (maker, e.tag) {
            (Maker::Apple, 0x0003) => uptime(&m.raw(&e, 4096)),
            (Maker::Nikon, 0x00A7) => m
                .numbers(&e, 1)
                .first()
                .map(|&v| format!("{} photos", thousands(v as u64))),
            _ => None,
        };
        let rendered = special
            .clone()
            .unwrap_or_else(|| render(&m, Ifd::Maker, &e));
        let entry = tree.add(
            Some(node),
            label.clone(),
            m.range(eoff, 12),
            NodeKind::Field,
            Some(Value::Text(rendered.clone())),
        );
        // A value stored elsewhere in the note gets a node of its own, as
        // EXIF's do; one outside the note is shown in its entry only.
        let mut holder = entry;
        if type_size(e.typ).is_some() && !e.inline && m.fits(e.value_off, e.size) {
            let at = m.range(e.value_off, e.size);
            if inside(at, whole) && (at.end() <= span.start || at.start >= span.end()) {
                holder = tree.add(
                    Some(note),
                    format!("{label} value"),
                    at,
                    NodeKind::Field,
                    Some(Value::Text(rendered.clone())),
                );
            }
        }
        let text = rendered.trim().to_string();
        if text.is_empty() {
            continue;
        }
        let fact = || Fact {
            text: text.clone(),
            node: holder,
        };
        match (maker, e.tag) {
            (Maker::Apple, 0x0003) if special.is_some() => {
                found.uptime = Some(Fact {
                    text: text.trim_start_matches("on for ").to_string(),
                    node: holder,
                });
            }
            (Maker::Apple, 0x0011) => found.linked.push(("Live Photo", fact())),
            (Maker::Apple, 0x000B) => found.linked.push(("burst", fact())),
            (Maker::Canon, 0x0009) => found.maker_owner = Some(fact()),
            (Maker::Canon, 0x000C | 0x0096)
            | (Maker::Nikon, 0x001D | 0x00A0)
            | (Maker::Fujifilm, 0x0010) => found.maker_serial.push(fact()),
            (Maker::Nikon, 0x00A7) if special.is_some() => found.shutter = Some(fact()),
            _ => {}
        }
    }
}

fn thousands(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// Apple's RunTime: a binary property list of a CMTime — a value and its
/// time scale — counting from when the phone was last switched on. Read as
/// "on for 3 days, 4 hours".
fn uptime(bytes: &[u8]) -> Option<String> {
    let dict = bplist_dict(bytes)?;
    let get = |k: &str| dict.iter().find(|(key, _)| key == k).map(|(_, v)| *v);
    let (value, scale) = (get("value")?, get("timescale")?);
    if scale <= 0 || value < 0 {
        return None;
    }
    let s = value / scale;
    let (d, h, m) = (s / 86_400, s % 86_400 / 3600, s % 3600 / 60);
    let unit = |n: i64, one: &str| format!("{n} {one}{}", if n == 1 { "" } else { "s" });
    Some(match (d, h) {
        (0, 0) => format!("on for {}", unit(m, "minute")),
        (0, _) => format!("on for {}, {}", unit(h, "hour"), unit(m, "minute")),
        _ => format!("on for {}, {}", unit(d, "day"), unit(h, "hour")),
    })
}

/// The integer entries of a binary property list's top dictionary (Apple's
/// bplist00 format): enough for RunTime, and never a panic on anything else.
fn bplist_dict(b: &[u8]) -> Option<Vec<(String, i64)>> {
    if !b.starts_with(b"bplist00") || b.len() < 40 {
        return None;
    }
    let trailer = &b[b.len() - 32..];
    let int_size = trailer[6] as usize;
    let ref_size = trailer[7] as usize;
    let be = |s: &[u8]| s.iter().fold(0u64, |acc, &x| (acc << 8) | x as u64);
    let objects = be(&trailer[8..16]) as usize;
    let top = be(&trailer[16..24]) as usize;
    let table = be(&trailer[24..32]) as usize;
    if !(1..=8).contains(&int_size)
        || !(1..=8).contains(&ref_size)
        || top >= objects
        || objects > 4096
    {
        return None;
    }
    let offset = |i: usize| -> Option<usize> {
        let at = table.checked_add(i.checked_mul(int_size)?)?;
        Some(be(b.get(at..at + int_size)?) as usize)
    };
    // A count stored in the marker's low nibble, or after it when that is 15.
    let count = |at: usize| -> Option<(usize, usize)> {
        let low = (b.get(at)? & 0x0F) as usize;
        if low < 15 {
            return Some((low, at + 1));
        }
        let marker = *b.get(at + 1)?;
        let n = 1usize << (marker & 0x0F);
        (marker >> 4 == 1 && n <= 8).then_some(())?;
        Some((be(b.get(at + 2..at + 2 + n)?) as usize, at + 2 + n))
    };
    let int = |i: usize| -> Option<i64> {
        let at = offset(i)?;
        let marker = *b.get(at)?;
        let n = 1usize << (marker & 0x0F);
        (marker >> 4 == 1 && n <= 8).then_some(())?;
        Some(be(b.get(at + 1..at + 1 + n)?) as i64)
    };
    let string = |i: usize| -> Option<String> {
        let at = offset(i)?;
        (*b.get(at)? >> 4 == 5).then_some(())?;
        let (n, from) = count(at)?;
        Some(String::from_utf8_lossy(b.get(from..from + n)?).into_owned())
    };
    let at = offset(top)?;
    (*b.get(at)? >> 4 == 0xD).then_some(())?;
    let (n, from) = count(at)?;
    let mut out = Vec::new();
    for k in 0..n.min(64) {
        let key = be(b.get(from + k * ref_size..from + (k + 1) * ref_size)?) as usize;
        let val = be(b.get(from + (n + k) * ref_size..from + (n + k + 1) * ref_size)?) as usize;
        if let (Some(key), Some(v)) = (string(key), int(val)) {
            out.push((key, v));
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A bplist00 of a CMTime, as an iPhone writes RunTime.
    pub(crate) fn runtime(value: u64, timescale: u64) -> Vec<u8> {
        let mut b = b"bplist00".to_vec();
        let mut offsets = Vec::new();
        // 0: the dict, keys 1-4, values 5-8.
        offsets.push(b.len());
        b.push(0xD4);
        b.extend([1, 2, 3, 4, 5, 6, 7, 8]);
        for key in ["flags", "value", "timescale", "epoch"] {
            offsets.push(b.len());
            b.push(0x50 | key.len() as u8);
            b.extend(key.as_bytes());
        }
        for v in [1u64, value, timescale, 0] {
            offsets.push(b.len());
            b.push(0x13);
            b.extend(v.to_be_bytes());
        }
        let table = b.len();
        for o in &offsets {
            b.push(*o as u8);
        }
        b.extend([0; 6]);
        b.push(1);
        b.push(1);
        b.extend((offsets.len() as u64).to_be_bytes());
        b.extend(0u64.to_be_bytes());
        b.extend((table as u64).to_be_bytes());
        b
    }

    #[test]
    fn runtime_is_how_long_the_phone_was_on() {
        // 3 days, 4 hours and a bit, in nanoseconds.
        let ns = (3 * 86_400 + 4 * 3600 + 125) * 1_000_000_000u64;
        assert_eq!(
            uptime(&runtime(ns, 1_000_000_000)).as_deref(),
            Some("on for 3 days, 4 hours")
        );
        assert_eq!(
            uptime(&runtime(2 * 3600 + 5 * 60, 1)).as_deref(),
            Some("on for 2 hours, 5 minutes")
        );
        // Anything else is not read, and does not panic.
        for cut in 0..runtime(5, 1).len() {
            assert!(uptime(&runtime(5, 1)[..cut]).is_none());
        }
        assert!(uptime(b"bplist00 but nothing after it at all, not even a trailer").is_none());
    }

    use crate::exif::testing::{Spec, V, build, encode, u16b, u32b};
    use crate::exif::{PhotoFacts, parse_tiff};

    /// A maker's note: `header`, an IFD of `entries`, then their values,
    /// with offsets counted from `origin` bytes before the note's start
    /// (negative: after it).
    fn note(header: &[u8], order: ByteOrder, entries: &[(u16, V)], origin: i64) -> Vec<u8> {
        let mut out = header.to_vec();
        let mut values = Vec::new();
        let mut values_at = header.len() + 2 + 12 * entries.len() + 4;
        out.extend(u16b(order, entries.len() as u16));
        for (tag, v) in entries {
            let (typ, count, bytes) = encode(order, v);
            out.extend(u16b(order, *tag));
            out.extend(u16b(order, typ));
            out.extend(u32b(order, count));
            if bytes.len() <= 4 {
                let mut inline = bytes.clone();
                inline.resize(4, 0);
                out.extend(inline);
            } else {
                out.extend(u32b(order, (values_at as i64 + origin) as u32));
                values_at += bytes.len();
                values.extend(bytes);
            }
        }
        out.extend(u32b(order, 0));
        out.extend(values);
        out
    }

    fn read(tiff: &[u8]) -> (ParseTree, PhotoFacts) {
        let mut tree = ParseTree::new();
        let root = tree.add(
            None,
            "APP1",
            ByteRange::new(0, tiff.len() as u64),
            NodeKind::Container,
            None,
        );
        let facts = parse_tiff(&mut tree, root, tiff, 100);
        (tree, facts)
    }

    fn photo(make: &'static str, maker_note: Vec<u8>) -> Vec<u8> {
        let mut s = Spec::new(ByteOrder::Big);
        s.ifd0 = vec![(0x010F, V::Ascii(make))];
        s.exif = vec![(0x927C, V::Undefined(maker_note))];
        build(s)
    }

    fn labels(tree: &ParseTree) -> Vec<String> {
        tree.nodes().iter().map(|n| n.label.clone()).collect()
    }

    fn problems(tree: &ParseTree) -> Vec<String> {
        tree.nodes()
            .iter()
            .filter(|n| matches!(n.kind, NodeKind::Warning | NodeKind::Error))
            .map(|n| n.label.clone())
            .collect()
    }

    #[test]
    fn an_iphone_note_ties_the_photo_to_its_video_and_says_how_long_the_phone_was_on() {
        let ns = (3 * 86_400 + 4 * 3600) * 1_000_000_000u64;
        let apple = note(
            b"Apple iOS\0\0\x01MM",
            ByteOrder::Big,
            &[
                (0x0001, V::Long(vec![14])),
                (0x0003, V::Undefined(runtime(ns, 1_000_000_000))),
                (0x000B, V::Ascii("5C6D7E8F-0000-4000-8000-000000000002")),
                (0x0011, V::Ascii("1A2B3C4D-0000-4000-8000-000000000001")),
            ],
            0,
        );
        let (tree, facts) = read(&photo("Apple", apple));
        assert!(problems(&tree).is_empty(), "{:?}", problems(&tree));
        let l = labels(&tree);
        for want in [
            "maker header",
            "Apple IFD",
            "RunTime",
            "BurstUUID",
            "ContentIdentifier",
            "ContentIdentifier value",
        ] {
            assert!(l.iter().any(|x| x == want), "{want} in {l:?}");
        }
        assert_eq!(facts.uptime.unwrap().text, "3 days, 4 hours");
        assert_eq!(
            facts.linked.unwrap().text,
            "Live Photo 1A2B3C4D…, burst 5C6D7E8F…"
        );
    }

    #[test]
    fn a_canon_note_names_its_owner_and_serial_numbers() {
        let entries = || {
            vec![
                (0x0009, V::Ascii("Jane Doe")),
                (0x000C, V::Long(vec![123_456_789])),
                (0x0096, V::Ascii("ZA1234567")),
            ]
        };
        // Canon counts offsets from EXIF's start: find where the note lands,
        // then build it again for that place.
        let first = photo("Canon", note(b"", ByteOrder::Big, &entries(), 0));
        let probe = note(b"", ByteOrder::Big, &entries(), 0);
        let at = first
            .windows(probe.len())
            .position(|w| w == probe.as_slice())
            .unwrap();
        let mut tiff = first.clone();
        tiff[at..at + probe.len()].copy_from_slice(&note(
            b"",
            ByteOrder::Big,
            &entries(),
            at as i64,
        ));
        let (tree, facts) = read(&tiff);
        assert!(problems(&tree).is_empty(), "{:?}", problems(&tree));
        assert!(labels(&tree).iter().any(|x| x == "Canon IFD"));
        assert_eq!(facts.owner.unwrap().text, "Jane Doe");
        assert_eq!(facts.serial.unwrap().text, "123456789, ZA1234567");
    }

    #[test]
    fn a_nikon_note_counts_the_photos_the_camera_has_taken() {
        let mut header = b"Nikon\0\x02\x10\0\0".to_vec();
        header.extend(b"MM\0\x2A\0\0\0\x08");
        let nikon = note(
            &header,
            ByteOrder::Big,
            &[
                (0x001D, V::Ascii("3001234")),
                (0x00A7, V::Long(vec![12_345])),
            ],
            -10,
        );
        let (tree, facts) = read(&photo("NIKON CORPORATION", nikon));
        assert!(problems(&tree).is_empty(), "{:?}", problems(&tree));
        assert_eq!(facts.shutter.unwrap().text, "12,345 photos");
        assert_eq!(facts.serial.unwrap().text, "3001234");
    }

    #[test]
    fn a_fujifilm_note_holds_its_internal_serial() {
        let mut header = b"FUJIFILM".to_vec();
        header.extend(12u32.to_le_bytes());
        let fuji = note(
            &header,
            ByteOrder::Little,
            &[
                (0x0000, V::Undefined(b"0130".to_vec())),
                (0x0010, V::Ascii("FF02B1234567 5935323635351112")),
            ],
            0,
        );
        let (tree, facts) = read(&photo("FUJIFILM", fuji));
        assert!(problems(&tree).is_empty(), "{:?}", problems(&tree));
        assert_eq!(facts.serial.unwrap().text, "FF02B1234567 5935323635351112");
    }

    #[test]
    fn exif_serials_come_first_and_damage_is_survived() {
        let fuji = |n: u16| {
            let mut header = b"FUJIFILM".to_vec();
            header.extend(12u32.to_le_bytes());
            let mut b = note(
                &header,
                ByteOrder::Little,
                &[(0x0010, V::Ascii("INTERNAL-1"))],
                0,
            );
            b[12..14].copy_from_slice(&n.to_le_bytes());
            b
        };
        let mut s = Spec::new(ByteOrder::Big);
        s.ifd0 = vec![(0x010F, V::Ascii("FUJIFILM"))];
        s.exif = vec![
            (0xA431, V::Ascii("BODY-9")),
            (0x927C, V::Undefined(fuji(1))),
        ];
        let (_, facts) = read(&build(s));
        assert_eq!(facts.serial.unwrap().text, "BODY-9");
        // An entry count far past the note, and notes cut anywhere.
        let (tree, _) = read(&photo("FUJIFILM", fuji(60_000)));
        assert!(!tree.is_empty());
        let whole = fuji(1);
        for cut in 0..whole.len() {
            let (tree, _) = read(&photo("FUJIFILM", whole[..cut].to_vec()));
            assert!(!tree.is_empty());
        }
        let (tree, facts) = read(&photo("Apple", b"Apple iOS\0\0\x01MM\xFF".to_vec()));
        assert!(
            problems(&tree)
                .iter()
                .any(|p| p.contains("outside its note"))
        );
        assert!(facts.uptime.is_none());
    }

    #[test]
    fn random_notes_under_every_header_never_panic() {
        let mut x = 0x9E37_79B9_7F4A_7C15u64;
        let mut next = move || {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            x
        };
        let headers: [&[u8]; 4] = [
            b"Apple iOS\0\0\x01MM",
            b"Nikon\0\x02\x10\0\0MM\0\x2A\0\0\0\x08",
            b"FUJIFILM\x0C\0\0\0",
            b"",
        ];
        for i in 0..3000 {
            let mut blob = headers[i % 4].to_vec();
            let n = (next() % 200) as usize;
            // Small numbers, so entry counts and offsets often land inside.
            blob.extend((0..n).map(|_| (next() % 24) as u8));
            let make = if i % 4 == 3 { "Canon" } else { "x" };
            let tiff = photo(make, blob);
            let (tree, _) = read(&tiff);
            // `read` puts the block at file offset 100.
            let end = 100 + tiff.len() as u64;
            assert!(
                tree.nodes()
                    .iter()
                    .skip(1)
                    .all(|n| n.range.start >= 100 && n.range.end() <= end)
            );
        }
    }

    #[test]
    fn every_maker_tag_is_explained() {
        for name in all_names() {
            assert!(crate::exif::docs::describe(name, false).is_some(), "{name}");
        }
    }

    #[test]
    fn shutter_counts_read_with_separators() {
        assert_eq!(thousands(1_234_567), "1,234,567");
        assert_eq!(thousands(999), "999");
    }
}
