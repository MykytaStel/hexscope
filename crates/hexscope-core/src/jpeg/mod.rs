//! JPEG: marker segments, then entropy-coded scan data, until EOI.
//!
//! Pixels are not decoded. What matters here is the structure, the image
//! dimensions, and the EXIF block most cameras and phones write into APP1.

pub mod blocks;
pub(crate) mod docs;

use crate::exif::{PhotoFacts, parse_tiff_at};
use crate::model::{ByteRange, NodeId, NodeKind, ParseTree, Value};
use crate::reader::Reader;

/// SOI followed by the start of the next marker: how every JPEG begins.
pub const MAGIC: [u8; 3] = [0xFF, 0xD8, 0xFF];

#[derive(Debug)]
pub struct JpegDocument {
    pub tree: ParseTree,
    /// From the first SOF segment.
    pub width: Option<u16>,
    pub height: Option<u16>,
    /// From the first APP1 EXIF segment; empty when there is none.
    pub facts: PhotoFacts,
    pub has_exif: bool,
}

fn marker_name(m: u8) -> String {
    match m {
        0xC4 => "DHT".into(),
        0xC8 => "JPG".into(),
        0xCC => "DAC".into(),
        0xC0..=0xCF => format!("SOF{}", m - 0xC0),
        0xD0..=0xD7 => format!("RST{}", m - 0xD0),
        0xD8 => "SOI".into(),
        0xD9 => "EOI".into(),
        0xDA => "SOS".into(),
        0xDB => "DQT".into(),
        0xDC => "DNL".into(),
        0xDD => "DRI".into(),
        0xE0..=0xEF => format!("APP{}", m - 0xE0),
        0xFE => "COM".into(),
        0x01 => "TEM".into(),
        _ => format!("marker 0x{m:02X}"),
    }
}

fn is_sof(m: u8) -> bool {
    matches!(m, 0xC0..=0xCF) && !matches!(m, 0xC4 | 0xC8 | 0xCC)
}

/// What an APPn segment holds, from the identifier its payload starts with.
fn app_kind(payload: &[u8]) -> Option<&'static str> {
    const KNOWN: [(&[u8], &str); 8] = [
        (b"Exif\0\0", "EXIF"),
        (b"http://ns.adobe.com/xap/1.0/\0", "XMP"),
        (b"JFIF\0", "JFIF"),
        (b"JFXX\0", "JFXX"),
        (b"ICC_PROFILE\0", "ICC"),
        (b"MPF\0", "MPF"),
        (b"Photoshop 3.0\0", "Photoshop"),
        (b"Adobe", "Adobe"),
    ];
    KNOWN
        .iter()
        .find(|(prefix, _)| payload.starts_with(prefix))
        .map(|(_, name)| *name)
}

/// The first position at or after `from` where a segment plausibly begins: a
/// marker that can start one, followed by a length that fits in the file — or
/// EOI. Used to resume after damage instead of abandoning the rest of the file.
fn next_segment(data: &[u8], from: u64) -> Option<u64> {
    let end = data.len() as u64;
    let mut p = from;
    while p + 2 <= end {
        let mut r = Reader::new(data);
        r.seek(p);
        if let (Ok(0xFF), Ok(m)) = (r.u8(), r.u8()) {
            if m == 0xD9 {
                return Some(p);
            }
            if matches!(m, 0xC0..=0xFE)
                && !matches!(m, 0xD0..=0xD8)
                && let Ok(len) = r.u16_be()
                && len >= 2
                && p + 2 + len as u64 <= end
            {
                return Some(p);
            }
        }
        p += 1;
    }
    None
}

/// Parses a JPEG. Never fails: damage becomes error nodes on the bytes.
pub fn parse_jpeg(data: &[u8]) -> JpegDocument {
    parse_jpeg_at(data, 0)
}

/// `depth` is how many files deep this JPEG is embedded; see `parse_tiff_at`.
pub(crate) fn parse_jpeg_at(data: &[u8], depth: u8) -> JpegDocument {
    let mut tree = ParseTree::new();
    let root = tree.add(
        None,
        "JPEG",
        ByteRange::new(0, data.len() as u64),
        NodeKind::Container,
        None,
    );
    let mut doc = JpegDocument {
        tree: ParseTree::new(),
        width: None,
        height: None,
        facts: PhotoFacts::default(),
        has_exif: false,
    };

    let mut r = Reader::new(data);
    match r.array::<2>() {
        Ok([0xFF, 0xD8]) => {
            tree.add(
                Some(root),
                "SOI",
                ByteRange::new(0, 2),
                NodeKind::Field,
                None,
            );
        }
        _ => {
            tree.error(
                root,
                "not a JPEG: it must start with FF D8",
                ByteRange::new(0, data.len().min(2) as u64),
            );
            doc.tree = tree;
            return doc;
        }
    }

    let mut saw_eoi = false;
    while r.remaining() > 0 {
        let start = r.pos();
        let Ok(first) = r.u8() else { break };
        if first != 0xFF {
            // No marker, so no length to skip by: look for the next segment
            // rather than give up on the rest of the file.
            if let Some(next) = next_segment(data, start + 1) {
                tree.error(
                    root,
                    format!("expected a marker, found 0x{first:02X}: skipped to the next segment"),
                    ByteRange::new(start, next - start),
                );
                r.seek(next);
                continue;
            }
            tree.error(
                root,
                format!("expected a marker, found 0x{first:02X}"),
                ByteRange::new(start, data.len() as u64 - start),
            );
            break;
        }
        // Any number of 0xFF fill bytes may precede the marker code.
        let mut marker = 0xFF;
        while marker == 0xFF {
            match r.u8() {
                Ok(m) => marker = m,
                Err(_) => break,
            }
        }
        if marker == 0xFF {
            tree.error(
                root,
                "the file ends inside a marker",
                ByteRange::new(start, r.pos() - start),
            );
            break;
        }

        match marker {
            0xD9 => {
                tree.add(
                    Some(root),
                    "EOI",
                    ByteRange::new(start, r.pos() - start),
                    NodeKind::Field,
                    None,
                );
                saw_eoi = true;
                break;
            }
            // Markers that stand alone, without a length.
            0xD8 | 0x01 | 0xD0..=0xD7 => {
                tree.add(
                    Some(root),
                    marker_name(marker),
                    ByteRange::new(start, r.pos() - start),
                    NodeKind::Field,
                    None,
                );
                continue;
            }
            _ => {}
        }

        let name = marker_name(marker);
        let Ok(len) = r.u16_be() else {
            tree.error(
                root,
                format!("{name} segment truncated before its length"),
                ByteRange::new(start, data.len() as u64 - start),
            );
            break;
        };
        if len < 2 {
            tree.error(
                root,
                format!("{name} segment length {len} is less than 2"),
                ByteRange::new(start, r.pos() - start),
            );
            break;
        }
        let payload_start = r.pos();
        let Ok(payload) = r.bytes(len as usize - 2) else {
            // Either the file is truncated or the length is corrupt. A later
            // segment that fits tells them apart.
            if let Some(next) = next_segment(data, start + 2) {
                tree.error(
                    root,
                    format!("{name} segment length {len} is wrong: skipped to the next segment"),
                    ByteRange::new(start, next - start),
                );
                r.seek(next);
                continue;
            }
            tree.error(
                root,
                format!("{name} segment runs past the end of the file"),
                ByteRange::new(start, data.len() as u64 - start),
            );
            break;
        };

        let label = match (marker, app_kind(payload)) {
            (0xE0..=0xEF, Some(kind)) => format!("{name} · {kind}"),
            _ => name,
        };
        let node = tree.add(
            Some(root),
            label,
            ByteRange::new(start, r.pos() - start),
            NodeKind::Container,
            Some(Value::Bytes(payload.len() as u64)),
        );
        tree.add(
            Some(node),
            "marker",
            ByteRange::new(payload_start - 4, 2),
            NodeKind::Field,
            None,
        );
        tree.add(
            Some(node),
            "length",
            ByteRange::new(payload_start - 2, 2),
            NodeKind::Field,
            Some(Value::U64(len as u64)),
        );

        decode_segment(
            &mut tree,
            node,
            marker,
            payload,
            payload_start,
            &mut doc,
            depth,
        );

        if marker == 0xDA {
            // Scan data follows SOS, unframed: it ends at the first marker
            // that is neither a stuffed 0xFF (FF 00) nor a restart marker.
            let scan_start = r.pos();
            let rest = r.rest();
            let end = rest
                .windows(2)
                .position(|w| matches!(w, [0xFF, m] if *m != 0x00 && !(0xD0..=0xD7).contains(m)))
                .unwrap_or(rest.len());
            r.seek(scan_start + end as u64);
            tree.add(
                Some(root),
                "scan data",
                ByteRange::new(scan_start, end as u64),
                NodeKind::Container,
                Some(Value::Bytes(end as u64)),
            );
        }
    }

    if !saw_eoi {
        tree.warning(
            root,
            "no EOI marker: the image never ends",
            ByteRange::new(data.len() as u64, 0),
        );
    } else if r.remaining() > 0 {
        // Some phones append data after EOI; so do files crafted to be two
        // formats at once. Either way it is worth seeing.
        let extra = r.remaining() as u64;
        tree.add(
            Some(root),
            "data after the end of the image",
            ByteRange::new(r.pos(), extra),
            NodeKind::Warning,
            Some(Value::Bytes(extra)),
        );
    }

    doc.tree = tree;
    doc
}

fn field(tree: &mut ParseTree, parent: NodeId, label: &str, start: u64, len: u64, value: Value) {
    tree.add(
        Some(parent),
        label,
        ByteRange::new(start, len),
        NodeKind::Field,
        Some(value),
    );
}

fn decode_segment(
    tree: &mut ParseTree,
    node: NodeId,
    marker: u8,
    payload: &[u8],
    at: u64,
    doc: &mut JpegDocument,
    depth: u8,
) {
    let mut p = Reader::new(payload);
    if is_sof(marker) {
        let (Ok(precision), Ok(height), Ok(width), Ok(components)) =
            (p.u8(), p.u16_be(), p.u16_be(), p.u8())
        else {
            tree.error(
                node,
                "SOF segment truncated",
                ByteRange::new(at, payload.len() as u64),
            );
            return;
        };
        field(tree, node, "precision", at, 1, Value::U64(precision as u64));
        field(tree, node, "height", at + 1, 2, Value::U64(height as u64));
        field(tree, node, "width", at + 3, 2, Value::U64(width as u64));
        field(
            tree,
            node,
            "components",
            at + 5,
            1,
            Value::U64(components as u64),
        );
        if doc.width.is_none() {
            doc.width = Some(width);
            doc.height = Some(height);
        }
        return;
    }

    match marker {
        0xE0 if app_kind(payload) == Some("JFIF") => {
            let (Ok(_), Ok(major), Ok(minor), Ok(units), Ok(xd), Ok(yd)) =
                (p.bytes(5), p.u8(), p.u8(), p.u8(), p.u16_be(), p.u16_be())
            else {
                return;
            };
            field(tree, node, "identifier", at, 5, Value::Text("JFIF".into()));
            field(
                tree,
                node,
                "version",
                at + 5,
                2,
                Value::Text(format!("{major}.{minor:02}")),
            );
            let unit = match units {
                1 => "dots per inch",
                2 => "dots per centimetre",
                _ => "aspect ratio only",
            };
            field(
                tree,
                node,
                "units",
                at + 7,
                1,
                Value::Enum {
                    raw: units as u64,
                    name: unit,
                },
            );
            field(tree, node, "xDensity", at + 8, 2, Value::U64(xd as u64));
            field(tree, node, "yDensity", at + 10, 2, Value::U64(yd as u64));
        }
        0xE1 if app_kind(payload) == Some("EXIF") => {
            field(tree, node, "identifier", at, 6, Value::Text("Exif".into()));
            let _ = p.bytes(6);
            let tiff = p.rest();
            let facts = parse_tiff_at(tree, node, tiff, at + 6, depth);
            doc.facts.fill_from(facts);
            doc.has_exif = true;
        }
        0xE1 if app_kind(payload) == Some("XMP") => {
            // XMP is XML: show it as text rather than an opaque blob.
            const ID: u64 = 29;
            let _ = p.bytes(ID as usize);
            let xml = String::from_utf8_lossy(p.rest());
            let text: String = xml.trim().chars().take(4000).collect();
            let shown = if xml.trim().chars().count() > 4000 {
                format!("{text}…")
            } else {
                text
            };
            field(tree, node, "identifier", at, ID, Value::Text("XMP".into()));
            field(
                tree,
                node,
                "packet",
                at + ID,
                payload.len() as u64 - ID,
                Value::Text(shown),
            );
        }
        0xFE => {
            let text = String::from_utf8_lossy(p.rest()).trim().to_string();
            field(
                tree,
                node,
                "comment",
                at,
                payload.len() as u64,
                Value::Text(text),
            );
        }
        0xDA => {
            if let Ok(n) = p.u8() {
                field(tree, node, "components", at, 1, Value::U64(n as u64));
            }
        }
        0xDD => {
            if let Ok(interval) = p.u16_be() {
                field(
                    tree,
                    node,
                    "restartInterval",
                    at,
                    2,
                    Value::U64(interval as u64),
                );
            }
        }
        _ => {}
    }
}

#[cfg(test)]
pub(crate) mod testing {
    /// A minimal JPEG around the given APP1 EXIF block: SOI, APP0 JFIF,
    /// APP1, DQT, SOF0, SOS with a little scan data, EOI.
    pub fn jpeg_with_exif(tiff: Option<&[u8]>) -> Vec<u8> {
        let seg = |marker: u8, payload: &[u8]| {
            let mut out = vec![0xFF, marker];
            out.extend_from_slice(&((payload.len() + 2) as u16).to_be_bytes());
            out.extend_from_slice(payload);
            out
        };
        let mut out = vec![0xFF, 0xD8];
        out.extend(seg(0xE0, b"JFIF\0\x01\x02\x01\x00\x48\x00\x48\x00\x00"));
        if let Some(tiff) = tiff {
            let mut app1 = b"Exif\0\0".to_vec();
            app1.extend_from_slice(tiff);
            out.extend(seg(0xE1, &app1));
        }
        out.extend(seg(0xDB, &[0u8; 65]));
        // 8-bit, 480 high, 640 wide, one component.
        out.extend(seg(0xC0, &[8, 0x01, 0xE0, 0x02, 0x80, 1, 1, 0x11, 0]));
        out.extend(seg(0xDA, &[1, 1, 0, 0, 63, 0]));
        // Scan data with a stuffed FF 00 and a restart marker inside it.
        out.extend_from_slice(&[0x12, 0xFF, 0x00, 0x34, 0xFF, 0xD0, 0x56]);
        out.extend_from_slice(&[0xFF, 0xD9]);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::testing::jpeg_with_exif;
    use super::*;
    use crate::exif::ByteOrder;
    use crate::exif::testing::{Spec, V, build};

    fn labels(doc: &JpegDocument) -> Vec<String> {
        let root = doc.tree.root().unwrap();
        doc.tree
            .get(root)
            .children
            .iter()
            .map(|&c| doc.tree.get(c).label.clone())
            .collect()
    }

    fn tiff() -> Vec<u8> {
        let mut s = Spec::new(ByteOrder::Little);
        s.ifd0 = vec![(0x010F, V::Ascii("Google")), (0x0110, V::Ascii("Pixel 8"))];
        s.gps = vec![
            (0x01, V::Ascii("N")),
            (0x02, V::Rational(vec![(50, 1), (27, 1), (1, 1)])),
            (0x03, V::Ascii("E")),
            (0x04, V::Rational(vec![(30, 1), (31, 1), (24, 1)])),
        ];
        build(s)
    }

    #[test]
    fn walks_every_segment_to_eoi() {
        let doc = parse_jpeg(&jpeg_with_exif(Some(&tiff())));
        assert_eq!(
            labels(&doc),
            [
                "SOI",
                "APP0 · JFIF",
                "APP1 · EXIF",
                "DQT",
                "SOF0",
                "SOS",
                "scan data",
                "EOI"
            ]
        );
        assert_eq!((doc.width, doc.height), (Some(640), Some(480)));
        assert!(doc.tree.nodes().iter().all(|n| n.kind != NodeKind::Error));
    }

    #[test]
    fn scan_data_skips_stuffing_and_restart_markers() {
        let doc = parse_jpeg(&jpeg_with_exif(None));
        let scan = doc
            .tree
            .nodes()
            .iter()
            .find(|n| n.label == "scan data")
            .unwrap();
        assert_eq!(scan.range.len, 7, "12 FF00 34 FFD0 56 is all scan data");
    }

    #[test]
    fn reads_the_photo_facts_from_exif() {
        let data = jpeg_with_exif(Some(&tiff()));
        let doc = parse_jpeg(&data);
        assert!(doc.has_exif);
        assert_eq!(doc.facts.camera.as_ref().unwrap().text, "Google Pixel 8");
        let loc = doc.facts.location.unwrap();
        assert!((loc.latitude - 50.45028).abs() < 1e-4);
        assert!((loc.longitude - 30.52333).abs() < 1e-4);

        // EXIF node ranges are in file coordinates: the Model value's bytes in
        // the file are the string itself.
        let model = doc
            .tree
            .nodes()
            .iter()
            .find(|n| n.label == "Model value")
            .unwrap();
        let (s, e) = (model.range.start as usize, model.range.end() as usize);
        assert_eq!(&data[s..e], b"Pixel 8\0");
    }

    #[test]
    fn a_truncated_file_reports_where_it_stops() {
        let data = jpeg_with_exif(Some(&tiff()));
        let doc = parse_jpeg(&data[..40]);
        assert!(
            doc.tree
                .nodes()
                .iter()
                .any(|n| n.kind == NodeKind::Error && n.label.contains("past the end"))
        );
        assert!(labels(&doc).iter().any(|l| l.starts_with("no EOI")));
    }

    #[test]
    fn data_after_eoi_is_flagged() {
        let appended = b"PK\x03\x04 a hidden zip";
        let mut data = jpeg_with_exif(None);
        data.extend_from_slice(appended);
        let doc = parse_jpeg(&data);
        let extra = doc
            .tree
            .nodes()
            .iter()
            .find(|n| n.label == "data after the end of the image")
            .unwrap();
        assert_eq!(extra.kind, NodeKind::Warning);
        assert_eq!(extra.range.len, appended.len() as u64);
    }

    #[test]
    fn garbage_between_segments_is_skipped_not_fatal() {
        let clean = jpeg_with_exif(None);
        // Insert junk just before SOF0.
        let sof = clean.windows(2).position(|w| w == [0xFF, 0xC0]).unwrap();
        let mut data = clean[..sof].to_vec();
        data.extend_from_slice(b"junk");
        data.extend_from_slice(&clean[sof..]);

        let doc = parse_jpeg(&data);
        let l = labels(&doc);
        assert!(
            l.contains(&"SOF0".to_string()) && l.contains(&"EOI".to_string()),
            "{l:?}"
        );
        let err = doc
            .tree
            .nodes()
            .iter()
            .find(|n| n.kind == NodeKind::Error)
            .unwrap();
        assert_eq!((err.range.start, err.range.len), (sof as u64, 4));
        assert_eq!(doc.width, Some(640), "the frame header is still read");
    }

    #[test]
    fn a_corrupt_segment_length_costs_one_segment() {
        let mut data = jpeg_with_exif(None);
        // DQT's length, made far too large.
        let dqt = data.windows(2).position(|w| w == [0xFF, 0xDB]).unwrap();
        data[dqt + 2] = 0x7F;
        let doc = parse_jpeg(&data);
        let l = labels(&doc);
        assert!(
            l.contains(&"SOF0".to_string()) && l.contains(&"EOI".to_string()),
            "{l:?}"
        );
        assert!(
            doc.tree
                .nodes()
                .iter()
                .any(|n| n.label.starts_with("DQT segment length"))
        );
    }

    #[test]
    fn a_second_exif_segment_fills_what_the_first_lacks() {
        let mut first = Spec::new(ByteOrder::Big);
        first.ifd0 = vec![(0x0110, V::Ascii("Camera One"))];
        let with_gps = tiff();

        let mut data = jpeg_with_exif(Some(&build(first)));
        // Insert a second APP1 EXIF right after the first one.
        let sof = data.windows(2).position(|w| w == [0xFF, 0xDB]).unwrap();
        let mut app1 = vec![0xFF, 0xE1];
        app1.extend_from_slice(&((with_gps.len() + 8) as u16).to_be_bytes());
        app1.extend_from_slice(b"Exif\0\0");
        app1.extend_from_slice(&with_gps);
        data.splice(sof..sof, app1);

        let doc = parse_jpeg(&data);
        assert_eq!(
            doc.facts.camera.as_ref().unwrap().text,
            "Camera One",
            "the first segment wins"
        );
        assert!(
            doc.facts.location.is_some(),
            "the location comes from the second"
        );
    }

    #[test]
    fn an_xmp_packet_is_shown_as_text() {
        let seg = |payload: &[u8]| {
            let mut out = vec![0xFF, 0xE1];
            out.extend_from_slice(&((payload.len() + 2) as u16).to_be_bytes());
            out.extend_from_slice(payload);
            out
        };
        let mut xmp = b"http://ns.adobe.com/xap/1.0/\0".to_vec();
        xmp.extend_from_slice(b"<x:xmpmeta><rdf:RDF/></x:xmpmeta>");
        let mut data = vec![0xFF, 0xD8];
        data.extend(seg(&xmp));
        data.extend_from_slice(&[0xFF, 0xD9]);

        let doc = parse_jpeg(&data);
        let packet = doc
            .tree
            .nodes()
            .iter()
            .find(|n| n.label == "packet")
            .unwrap();
        assert_eq!(
            packet.value,
            Some(Value::Text("<x:xmpmeta><rdf:RDF/></x:xmpmeta>".into()))
        );
        // SOI, then APP1 marker and length, then the 29-byte identifier.
        assert_eq!(packet.range.start, 2 + 4 + 29);
    }

    #[test]
    fn every_truncation_is_survivable() {
        let data = jpeg_with_exif(Some(&tiff()));
        for len in 0..=data.len() {
            assert!(!parse_jpeg(&data[..len]).tree.is_empty());
        }
    }
}
