use super::*;
use crate::summary::summarize;

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(format!(
        "{}/tests/fixtures/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

fn said(doc: &SvgDocument, kind: &str) -> String {
    doc.facts
        .iter()
        .find(|f| f.kind == kind)
        .map(|f| f.text.clone())
        .unwrap_or_default()
}

#[test]
fn a_drawing_is_known_by_its_first_element() {
    assert!(is_svg(b"<svg xmlns=\"http://www.w3.org/2000/svg\"/>"));
    assert!(is_svg(b"\xEF\xBB\xBF<?xml version=\"1.0\"?>\n<!-- made by hand -->\n<!DOCTYPE svg [ <!ENTITY a \"b\"> ]>\n<svg>"));
    assert!(!is_svg(b"<svgfoo/>"));
    assert!(!is_svg(b"<html><svg/></html>"));
    assert!(!is_svg(b"<?xml version=\"1.0\"?><plist/>"));
}

#[test]
fn a_namespace_prefixed_root_is_detected_and_cleaned() {
    let data = br#"<s:svg xmlns:s="http://www.w3.org/2000/svg" xmlns:inkscape="http://www.inkscape.org/namespaces/inkscape" inkscape:version="1"><s:rect/></s:svg>"#;
    assert_eq!(
        crate::document::parse(data).format(),
        crate::document::Format::Svg
    );

    let copy = crate::clean::clean(data).unwrap();
    let text = String::from_utf8(copy.bytes).unwrap();
    assert!(text.contains("<s:svg"));
    assert!(text.contains("<s:rect/>"));
    assert!(!text.contains("inkscape:version"));
}

fn has_reportable_photo_fact(summary: &crate::summary::Summary) -> bool {
    summary
        .facts
        .iter()
        .any(|(kind, _)| matches!(*kind, "location" | "camera" | "serial" | "owner" | "taken"))
}

fn webp_with_exif(tiff: &[u8]) -> Vec<u8> {
    let vp8x = [0x08, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    let vp8l = [0x2F, 0, 0, 0, 0];
    let mut body = b"WEBP".to_vec();
    for (kind, payload) in [(b"VP8X", &vp8x[..]), (b"VP8L", &vp8l[..]), (b"EXIF", tiff)] {
        body.extend_from_slice(kind);
        body.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        body.extend_from_slice(payload);
        if payload.len() % 2 != 0 {
            body.push(0);
        }
    }
    let mut file = b"RIFF".to_vec();
    file.extend_from_slice(&(body.len() as u32).to_le_bytes());
    file.extend_from_slice(&body);
    file
}

#[test]
fn embedded_raster_formats_are_reported_and_cleaned() {
    let tiff = fixture("photo.tif");
    let images = [
        ("photo.webp", "image/webp", webp_with_exif(&tiff)),
        ("photo.tif", "image/tiff", tiff),
        ("photo.heic", "image/heic", fixture("photo.heic")),
        ("photo.avif", "image/avif", fixture("photo.avif")),
    ];
    for (name, mime, image) in images {
        let before = crate::summary::summarize(&image);
        assert!(
            has_reportable_photo_fact(&before),
            "{name} ({}): {:?}",
            before.format,
            before.facts
        );

        let mut encoded = Vec::new();
        encode(&image, &mut encoded);
        let encoded = String::from_utf8(encoded).unwrap();
        let svg = format!("<svg><image href=\"data:{mime};base64,{encoded}\"/></svg>");
        let doc = parse_svg(svg.as_bytes());
        let reported = said(&doc, "photo");
        let reported_at = said(&doc, "photoplace");
        assert!(
            !reported.is_empty() || !reported_at.is_empty(),
            "{name}: {:?}",
            doc.facts
        );

        let copy = crate::clean::clean(svg.as_bytes()).unwrap();
        let text = String::from_utf8(copy.bytes).unwrap();
        assert!(text.contains("<svg>"), "{name}");
        assert!(text.contains("<image"), "{name}");
        let body = text
            .split_once(";base64,")
            .unwrap()
            .1
            .split('"')
            .next()
            .unwrap();
        let nested = crate::eml::base64(body.as_bytes());
        let after = crate::summary::summarize(&nested);
        assert_eq!(after.format, before.format, "{name}");
        assert!(
            !has_reportable_photo_fact(&after),
            "{name}: {:?}",
            after.facts
        );
    }
}

#[test]
fn inkscape_leaves_names_paths_a_photo_and_its_metadata() {
    let doc = parse_svg(&fixture("inkscape.svg"));
    assert_eq!(said(&doc, "original"), "poster-client-final-v3.svg");
    let links = said(&doc, "links");
    assert!(
        links.contains("/home/olena.koval/Work/Acme/poster-final.png"),
        "{links}"
    );
    assert!(
        links.contains("/home/olena.koval/Pictures/acme-logo.png"),
        "{links}"
    );
    assert!(said(&doc, "software").starts_with("Inkscape 0.92.5"));
    assert_eq!(said(&doc, "author"), "Olena Koval");
    assert_eq!(said(&doc, "title"), "Acme autumn poster");
    assert_eq!(said(&doc, "created"), "2026-09-14");
    assert_eq!(said(&doc, "keywords"), "acme, poster");
    let photo = said(&doc, "photoplace");
    assert!(
        photo.starts_with("picture 1: taken at 48.85840° N"),
        "{photo}"
    );
    assert!(!doc.tree.nodes().iter().any(|n| n.kind == NodeKind::Error));
}

#[test]
fn illustrator_leaves_its_name_and_its_own_copy_and_a_script_is_named() {
    let doc = parse_svg(&fixture("illustrator.svg"));
    assert_eq!(said(&doc, "software"), "Adobe Illustrator 27.5.0");
    assert!(said(&doc, "embedded").starts_with("Illustrator's own copy"));
    assert!(
        said(&doc, "scripts").starts_with("1 script and 1 event handler"),
        "{}",
        said(&doc, "scripts")
    );
    assert!(!doc.tree.nodes().iter().any(|n| n.kind == NodeKind::Error));
}

#[test]
fn a_clean_copy_keeps_the_drawing_and_says_nothing_of_its_author() {
    for name in ["inkscape.svg", "illustrator.svg"] {
        let data = fixture(name);
        let copy = crate::clean::clean(&data).unwrap();
        let text = String::from_utf8(copy.bytes.clone()).unwrap();
        for gone in [
            "olena",
            "Olena",
            "Acme autumn",
            "inkscape:",
            "sodipodi:",
            "<metadata",
            "Generator:",
            "i:pgf ",
            "eJzs",
        ] {
            assert!(!text.contains(gone), "{name}: {gone} stays");
        }
        // What the drawing draws stays, scripts included.
        for kept in ["<rect", "Autumn sale", "acme-logo.png", "<circle", "Badge"] {
            if String::from_utf8_lossy(&data).contains(kept) {
                assert!(text.contains(kept), "{name}: {kept} went");
            }
        }
        let after = summarize(&copy.bytes);
        assert_eq!(after.format, "svg");
        assert!(after.complete, "{name}: {:?}", after.problems);
        let kinds: Vec<&str> = after.facts.iter().map(|(k, _)| *k).collect();
        assert!(kinds.iter().all(|k| *k == "scripts"), "{name}: {kinds:?}");
    }
}

#[test]
fn the_photo_inside_is_cleaned_as_a_photo() {
    let copy = crate::clean::clean(&fixture("inkscape.svg")).unwrap();
    let text = String::from_utf8(copy.bytes).unwrap();
    let (_, rest) = text.split_once("data:image/jpeg;base64,").unwrap();
    let photo = crate::eml::base64(rest.split('"').next().unwrap().as_bytes());
    let facts = crate::jpeg::parse_jpeg(&photo).facts;
    assert!(facts.location.is_none() && facts.camera.is_none());
}

#[test]
fn a_cut_short_drawing_is_damage_and_gets_no_copy() {
    let data = fixture("inkscape.svg");
    let cut = &data[..data.len() / 2];
    let doc = parse_svg(cut);
    assert!(doc.tree.nodes().iter().any(|n| n.kind == NodeKind::Error));
    assert_eq!(crate::clean::clean(cut).unwrap_err(), CleanError::Damaged);
}

#[test]
fn base64_round_trips() {
    for bytes in [&b""[..], b"a", b"ab", b"abc", b"abcd", &[0xFF, 0x00, 0x80]] {
        let mut out = Vec::new();
        encode(bytes, &mut out);
        assert_eq!(crate::eml::base64(&out), bytes);
    }
}

#[test]
fn every_part_is_explained() {
    for name in ["inkscape.svg", "illustrator.svg"] {
        let doc = parse_svg(&fixture(name));
        for n in doc.tree.nodes() {
            assert!(
                docs::describe(&n.label, n.kind == NodeKind::Error).is_some(),
                "{name}: {}",
                n.label
            );
        }
    }
}

#[test]
fn any_cut_or_odd_bytes_are_read_without_a_panic() {
    let odd: [&[u8]; 6] = [
        b"<svg><!--></svg>",
        b"<svg><!---></svg>",
        b"<svg a='>' b=\"<\"/>",
        b"<svg><image href=\"data:image/jpeg;base64,!!!\"/></svg>",
        b"<svg inkscape:export-filename=\"C:\\Users\\o\\a.png\" sodipodi:docname=\"\"/>",
        b"<svg></g></svg>",
    ];
    for data in odd {
        let _ = parse_svg(data);
        let _ = crate::clean::clean(data);
    }
    for name in ["inkscape.svg", "illustrator.svg"] {
        let data = fixture(name);
        for cut in (0..data.len()).step_by(37) {
            let _ = parse_svg(&data[..cut]);
            let _ = crate::clean::clean(&data[..cut]);
        }
    }
}
