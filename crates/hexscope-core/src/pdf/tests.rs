use super::*;
use crate::model::Node;

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(format!(
        "{}/tests/fixtures/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

fn problems(tree: &ParseTree) -> Vec<String> {
    tree.nodes()
        .iter()
        .filter(|n| matches!(n.kind, NodeKind::Warning | NodeKind::Error))
        .map(|n| n.label.clone())
        .collect()
}

fn facts(doc: &PdfDocument) -> Vec<(&str, &str)> {
    doc.facts
        .iter()
        .map(|f| (f.kind, f.text.as_str()))
        .collect()
}

/// Whether text a rewrite wrote as a hex string is in the bytes.
fn hex_has(bytes: &[u8], text: &str) -> bool {
    let hex: String = text.bytes().map(|b| format!("{b:02X}")).collect();
    super::find(bytes, hex.as_bytes()).is_some()
}

fn find<'t>(tree: &'t ParseTree, label: &str) -> Vec<&'t Node> {
    tree.nodes().iter().filter(|n| n.label == label).collect()
}

#[test]
fn a_document_with_an_update_keeps_both_versions() {
    let doc = parse_pdf(&fixture("report.pdf"));
    assert_eq!(problems(&doc.tree), Vec::<String>::new());
    assert_eq!(doc.version.as_deref(), Some("1.7"));
    assert_eq!(doc.revisions, 2);
    let root = doc.tree.get(0);
    let labels: Vec<_> = root
        .children
        .iter()
        .map(|&c| doc.tree.get(c).label.as_str())
        .collect();
    assert_eq!(labels, ["revision 1", "revision 2"]);
    // Object 4, the page's text, twice: the salary line and its replacement.
    assert_eq!(find(&doc.tree, "object 4 0").len(), 2);
    assert_eq!(find(&doc.tree, "object 7 0").len(), 2);
    assert_eq!(
        facts(&doc),
        [
            ("author", "Olena Koval"),
            (
                "updates",
                "once after it was first saved; the earlier version is still inside"
            ),
            // The line the update replaced, which no page shows any more.
            ("earlier", "“Salary: 4,200 EUR a month”"),
            ("title", "Quarterly plan - v2"),
            ("application", "Sample Writer 3.1"),
            ("producer", "hexscope sample generator"),
            ("created", "2026-03-02 09:14 +02:00"),
            ("modified", "2026-03-05 17:42 +02:00"),
            ("history", "3 steps recorded"),
        ]
    );
    // The facts point at the latest information dictionary's keys.
    let author = doc.tree.get(doc.facts[0].node);
    assert_eq!(author.label, "/Author");
    let info = doc.tree.get(author.parent.unwrap());
    assert_eq!(info.value, Some(Value::Text("document information".into())));
    assert_eq!(doc.tree.get(info.parent.unwrap()).label, "revision 2");
    assert_eq!(doc.tree.get(doc.facts[1].node).label, "revision 2");
}

#[test]
fn every_byte_of_a_clean_file_belongs_to_something() {
    for name in ["report.pdf", "compact.pdf"] {
        let data = fixture(name);
        let doc = parse_pdf(&data);
        let slices = crate::map::composition(&doc.tree, crate::Format::Pdf, data.len() as u64);
        let hidden: Vec<_> = slices
            .iter()
            .filter(|s| s.role == crate::map::Role::Hidden)
            .collect();
        assert!(hidden.is_empty(), "{name}: {hidden:?}");
        assert!(
            slices.iter().any(|s| s.role == crate::map::Role::Metadata),
            "{name}"
        );
        assert!(
            slices.iter().any(|s| s.role == crate::map::Role::Content),
            "{name}"
        );
    }
}

#[test]
fn information_packed_in_an_object_stream_is_read() {
    let doc = parse_pdf(&fixture("compact.pdf"));
    assert_eq!(problems(&doc.tree), Vec::<String>::new());
    assert_eq!(doc.revisions, 1);
    assert_eq!(
        facts(&doc),
        [
            ("author", "Taras Melnyk"),
            ("title", "Quarterly plan"),
            ("application", "Sample Writer 3.1"),
            ("producer", "hexscope sample generator"),
            ("created", "2026-04-11 12:00 UTC"),
            ("history", "3 steps recorded"),
        ]
    );
    let packed = doc.tree.get(doc.facts[0].node);
    assert_eq!(packed.label, "stream data");
    let stream = doc.tree.get(packed.parent.unwrap());
    assert_eq!(stream.value, Some(Value::Text("object stream".into())));
    // The title came from the compressed XMP, which the dictionary lacked.
    let xmp = doc
        .tree
        .get(doc.tree.get(doc.facts[1].node).parent.unwrap());
    assert_eq!(xmp.value, Some(Value::Text("XMP metadata".into())));
}

/// A JPEG that says where, with what camera, and whose.
fn photo() -> Vec<u8> {
    use crate::exif::ByteOrder;
    use crate::exif::testing::{Spec, V, build};
    let mut s = Spec::new(ByteOrder::Big);
    s.ifd0 = vec![(0x0110, V::Ascii("Canon EOS R5"))];
    s.exif = vec![(0xA431, V::Ascii("HX-000042"))];
    s.gps = vec![
        (0x01, V::Ascii("S")),
        (0x02, V::Rational(vec![(33, 1), (51, 1), (0, 1)])),
        (0x03, V::Ascii("E")),
        (0x04, V::Rational(vec![(151, 1), (12, 1), (0, 1)])),
    ];
    crate::jpeg::testing::jpeg_with_exif(Some(&build(s)))
}

/// A PDF of these objects, numbered from 1, with a correct cross-reference
/// table; the first is the catalog.
fn pdf_of(objects: &[Vec<u8>]) -> Vec<u8> {
    let mut out = b"%PDF-1.7\n".to_vec();
    let mut at = Vec::new();
    for (i, body) in objects.iter().enumerate() {
        at.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n", i + 1).as_bytes());
        out.extend_from_slice(body);
        out.extend_from_slice(b"\nendobj\n");
    }
    let xref = out.len();
    let size = objects.len() + 1;
    out.extend_from_slice(format!("xref\n0 {size}\n0000000000 65535 f\r\n").as_bytes());
    for a in at {
        out.extend_from_slice(format!("{a:010} 00000 n\r\n").as_bytes());
    }
    out.extend_from_slice(
        format!("trailer\n<< /Size {size} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n").as_bytes(),
    );
    out
}

fn stream(dict: &str, bytes: &[u8]) -> Vec<u8> {
    [
        format!("<< {dict} /Length {} >>\nstream\n", bytes.len()).as_bytes(),
        bytes,
        b"\nendstream",
    ]
    .concat()
}

/// One page showing `jpeg`.
fn pdf_with_photo(jpeg: &[u8]) -> Vec<u8> {
    pdf_of(&[
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec(),
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 8 8] /Resources << /XObject << /Im1 4 0 R >> >> >>".to_vec(),
        stream(
            "/Type /XObject /Subtype /Image /Width 8 /Height 8 /ColorSpace /DeviceGray /BitsPerComponent 8 /Filter /DCTDecode",
            jpeg,
        ),
    ])
}

#[test]
fn the_redacted_sample_still_holds_what_its_boxes_cover() {
    let doc = parse_pdf(&fixture("redacted.pdf"));
    assert_eq!(
        facts(&doc),
        [
            (
                "covered",
                "page 1: “Olena Koval · +380 67 123 4567 · EUR 48,000”"
            ),
            (
                "covered",
                "page 2: an area marked for redaction was never applied: “Petro Ivanenko”"
            ),
            (
                "hiddentext",
                "page 1, in white on white: “Internal: the client would accept EUR 60,000 if pushed.”"
            ),
            ("comments", "1 comment by Olena Koval"),
            ("title", "Settlement agreement - redacted"),
            ("producer", "hexscope sample generator"),
        ]
    );
    // The clean copy takes the covered text out, applies the mark, and
    // leaves the rest where it was.
    let clean = crate::clean::clean(&fixture("redacted.pdf")).unwrap();
    let after = parse_pdf(&clean.bytes);
    assert_eq!(problems(&after.tree), Vec::<String>::new());
    // Comments are part of the document: they stay.
    assert_eq!(facts(&after), [("comments", "1 comment by Olena Koval")]);
    for gone in [
        "Olena Koval) Tj",
        "4567",
        "48,000",
        "Ivanenko",
        "60,000",
        "4F6C656E61",
    ] {
        assert!(
            super::find(&clean.bytes, gone.as_bytes()).is_none(),
            "{gone}"
        );
    }
    for kept in ["Claimant:", "Settlement agreement", "Signed in Kyiv"] {
        assert!(
            super::find(&clean.bytes, kept.as_bytes()).is_some() || hex_has(&clean.bytes, kept),
            "{kept}"
        );
    }
    let what: Vec<_> = clean.removed.iter().map(|r| r.what.as_str()).collect();
    assert!(what[0].starts_with("Text under black boxes"), "{what:?}");
}

#[test]
fn a_filled_form_and_attached_files_are_named() {
    let pdf = pdf_of(&[
        b"<< /Type /Catalog /Pages 2 0 R /AcroForm << /Fields [4 0 R 5 0 R 6 0 R] >> /Names << /EmbeddedFiles << /Names [(a) 7 0 R] >> >> >>".to_vec(),
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec(),
        b"<< /Type /Page /Parent 2 0 R /Annots [8 0 R] >>".to_vec(),
        b"<< /T (Name) /V (Olena Koval) /FT /Tx >>".to_vec(),
        b"<< /T (Agree) /V /Off /FT /Btn >>".to_vec(),
        b"<< /T (Contact) /Kids [9 0 R] >>".to_vec(),
        b"<< /Type /Filespec /F (salaries.xlsx) /UF (salaries.xlsx) >>".to_vec(),
        b"<< /Type /Annot /Subtype /FileAttachment /FS << /F (notes.txt) >> >>".to_vec(),
        b"<< /T (Phone) /V <FEFF002B0033> >>".to_vec(),
    ]);
    let doc = parse_pdf(&pdf);
    assert_eq!(
        facts(&doc),
        [
            (
                "form",
                "2 fields filled in: Name: “Olena Koval”, Contact.Phone: “+3”"
            ),
            ("attachments", "2 files: “salaries.xlsx”, “notes.txt”"),
        ]
    );
}

#[test]
fn text_under_a_black_box_and_an_unapplied_redaction_are_found() {
    let covered = b"BT /F1 12 Tf 72 700 Td (Salary: 4200 UAH) Tj ET 0 0 0 rg 70 695 140 16 re f";
    let marked = b"BT /F1 12 Tf 72 700 Td (Visible) Tj ET";
    let deflated = {
        use std::io::Write;
        let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::best());
        z.write_all(covered).unwrap();
        z.finish().unwrap()
    };
    let pdf = pdf_of(&[
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        b"<< /Type /Pages /Kids [3 0 R 4 0 R] /Count 2 >>".to_vec(),
        b"<< /Type /Page /Parent 2 0 R /Contents [5 0 R] >>".to_vec(),
        b"<< /Type /Page /Parent 2 0 R /Contents 6 0 R /Annots [7 0 R] >>".to_vec(),
        stream("/Filter /FlateDecode", &deflated),
        stream("", marked),
        b"<< /Type /Annot /Subtype /Redact /Rect [70 695 210 711] >>".to_vec(),
    ]);
    let doc = parse_pdf(&pdf);
    assert_eq!(
        facts(&doc),
        [
            ("covered", "page 1: “Salary: 4200 UAH”"),
            (
                "covered",
                "page 2: an area marked for redaction was never applied: “Visible”"
            ),
        ]
    );
    assert_eq!(
        problems(&doc.tree),
        [
            "text under a black box",
            "marked for redaction, never redacted"
        ]
    );
    let warning = doc.tree.get(doc.facts[0].node);
    assert_eq!(
        warning.value,
        Some(Value::Text("“Salary: 4200 UAH”".into()))
    );
    assert_eq!(doc.tree.get(warning.parent.unwrap()).label, "object 5 0");
}

#[test]
fn a_photo_in_a_pdf_says_where_it_was_taken() {
    let doc = parse_pdf(&pdf_with_photo(&photo()));
    assert_eq!(problems(&doc.tree), Vec::<String>::new());
    assert_eq!(
        facts(&doc),
        [(
            "photoplace",
            "the image in object 4: taken at 33.85000° S, 151.20000° E · Canon EOS R5 · serial HX-000042"
        )]
    );
    assert_eq!(doc.tree.get(doc.facts[0].node).label, "stream data");
    // A picture with nothing to say is not listed.
    let plain = parse_pdf(&pdf_with_photo(&crate::jpeg::testing::jpeg_with_exif(None)));
    assert_eq!(facts(&plain), []);
}

#[test]
fn the_clean_copy_blanks_a_photo_and_keeps_its_picture() {
    let jpeg = photo();
    let cleaned = crate::clean::clean(&pdf_with_photo(&jpeg)).unwrap();
    let doc = parse_pdf(&cleaned.bytes);
    assert_eq!(problems(&doc.tree), Vec::<String>::new());
    assert_eq!(facts(&doc), []);
    assert!(
        cleaned.removed[0]
            .what
            .starts_with("EXIF and XMP of the photos"),
        "{:?}",
        cleaned.removed
    );
    // The same length, and the same picture after the metadata.
    let stream = find(&doc.tree, "stream data")[0].range;
    let after = &cleaned.bytes[stream.start as usize..stream.end() as usize];
    assert_eq!(after.len(), jpeg.len());
    let sos = jpeg.windows(2).position(|w| w == [0xFF, 0xDA]).unwrap();
    assert_eq!(after[sos..], jpeg[sos..]);
    assert_eq!(
        crate::jpeg::parse_jpeg(after).facts,
        Default::default(),
        "nothing left to reveal"
    );
}

#[test]
fn blanking_stops_at_anything_it_cannot_follow() {
    use super::clean::blank_photo;
    // Not a JPEG, a length past the end, a length under two.
    for mut bytes in [
        b"GIF89a".to_vec(),
        vec![0xFF, 0xD8, 0xFF, 0xE1, 0x40, 0x00, 1, 2],
        vec![0xFF, 0xD8, 0xFF, 0xE1, 0x00, 0x01, 1, 2],
    ] {
        let before = bytes.clone();
        assert_eq!(blank_photo(&mut bytes), 0);
        assert_eq!(bytes, before);
    }
    // Fill bytes, a segment kept, one blanked, and nothing after the scan.
    let mut bytes = vec![
        0xFF, 0xD8, 0xFF, 0xFF, 0xE0, 0x00, 0x04, 7, 7, 0xFF, 0xE1, 0x00, 0x04, 9, 9, 0xFF, 0xDA,
        0xFF, 0xE1,
    ];
    assert_eq!(blank_photo(&mut bytes), 2);
    assert_eq!(bytes[7..9], [7, 7]);
    assert_eq!(bytes[13..15], [0, 0]);
    assert_eq!(bytes[17..], [0xFF, 0xE1]);
}

#[test]
fn what_runs_or_hides_is_marked() {
    let pdf = b"%PDF-1.4
1 0 obj << /Type /Catalog /OpenAction 2 0 R /Names << /EmbeddedFiles 3 0 R >> >> endobj
2 0 obj << /S /JavaScript /JS (app.alert\\(1\\)) >> endobj
3 0 obj << /Type /EmbeddedFile /Length 3 >> stream
abc
endstream endobj
4 0 obj << /S /Launch /F (calc.exe) >> endobj
trailer << /Root 1 0 R >>
startxref
0
%%EOF
";
    let doc = parse_pdf(pdf);
    let p = problems(&doc.tree);
    for expected in [
        "the document runs JavaScript",
        "a file carried inside the document",
        "an action that starts a program",
        "startxref points to 0, where no cross-reference section starts",
    ] {
        assert!(p.iter().any(|l| l == expected), "{expected} in {p:?}");
    }
}

#[test]
fn a_stream_with_the_wrong_length_is_still_read() {
    let pdf = b"%PDF-1.4
1 0 obj << /Length 99 >> stream
hello
endstream
endobj
2 0 obj << /Length 3 0 R >> stream
indirect
endstream
endobj
3 0 obj 8 endobj
";
    let doc = parse_pdf(pdf);
    let data: Vec<_> = find(&doc.tree, "stream data")
        .iter()
        .map(|n| n.range.len)
        .collect();
    assert_eq!(data, [5, 8]);
    let p = problems(&doc.tree);
    assert!(p.contains(&"the stream's /Length says 99 bytes, but it holds 5".to_string()));
    assert!(p.contains(&"the file ends without %%EOF: it was cut short".to_string()));
}

#[test]
fn junk_is_skipped_to_the_next_object() {
    let pdf = b"MZ polyglot%PDF-1.4
1 0 obj (a) endobj
@@@ garbage @@@
2 0 obj (b) endobj
trailer << /Size 3 >>
startxref
0
%%EOF
appended secret";
    let doc = parse_pdf(pdf);
    let p = problems(&doc.tree);
    assert!(
        p.contains(&"data before the PDF header".to_string()),
        "{p:?}"
    );
    assert!(
        p.contains(&"bytes that are not part of any object".to_string()),
        "{p:?}"
    );
    assert!(
        p.contains(&"data after the end of the document".to_string()),
        "{p:?}"
    );
    assert_eq!(find(&doc.tree, "object 2 0").len(), 1);
}

#[test]
fn unclosed_objects_by_the_thousand_are_read_in_one_pass() {
    // Each would otherwise be read to the end of the file: quadratic time.
    for opener in ["(", "<<", "[", "<< /Length 5 >> stream\n"] {
        let mut pdf = b"%PDF-1.4\n".to_vec();
        for i in 0..50_000 {
            pdf.extend_from_slice(format!("{i} 0 obj {opener}\nstartxref 1\n").as_bytes());
        }
        let t = std::time::Instant::now();
        let doc = parse_pdf(&pdf);
        assert!(t.elapsed().as_secs() < 5, "{opener}: {:?}", t.elapsed());
        // A stream with no endstream runs to the end; anything else is
        // left behind for the next object.
        if !opener.contains("stream") {
            assert!(doc.tree.len() > 50_000, "{opener}");
        }
    }
}

#[test]
fn an_encrypted_document_gives_no_strings() {
    let pdf = b"%PDF-1.4
1 0 obj << /Author (\x8f\x12\xa0) >> endobj
trailer << /Info 1 0 R /Encrypt 2 0 R >>
startxref
0
%%EOF
";
    let doc = parse_pdf(pdf);
    assert!(doc.encrypted);
    // Its encryption dictionary is missing: nothing can be read, and it
    // says only that.
    assert_eq!(doc.lock, Some(crypt::Lock::Unknown));
    assert_eq!(
        facts(&doc),
        [("encryption", "in a way hexscope does not read")]
    );
}

#[test]
fn every_cut_of_the_fixtures_is_survivable() {
    for name in ["report.pdf", "compact.pdf"] {
        let data = fixture(name);
        for cut in 0..data.len() {
            let doc = parse_pdf(&data[..cut]);
            let tree = &doc.tree;
            for n in tree.nodes() {
                assert!(
                    n.range.end() <= cut as u64,
                    "{name} cut at {cut}: {:?}",
                    n.label
                );
                if let Some(p) = n.parent {
                    let pr = tree.get(p).range;
                    assert!(
                        n.range.start >= pr.start && n.range.end() <= pr.end(),
                        "{name} cut at {cut}: {:?} outside {:?}",
                        n.label,
                        tree.get(p).label
                    );
                }
            }
            // A cut just after a revision's %%EOF leaves an earlier, whole
            // version of the document.
            let at_revision_end = data[..cut].trim_ascii_end().ends_with(b"%%EOF");
            if cut > 0 && !at_revision_end {
                assert!(
                    !problems(tree).is_empty(),
                    "{name} cut at {cut} looks whole"
                );
            }
        }
    }
}

fn contains(hay: &[u8], needle: &[u8]) -> bool {
    hay.windows(needle.len()).any(|w| w == needle)
}

#[test]
fn a_clean_copy_is_one_version_with_nothing_to_tell() {
    for name in ["report.pdf", "compact.pdf"] {
        let data = fixture(name);
        let cleaned = crate::clean::clean(&data).unwrap();
        let doc = parse_pdf(&cleaned.bytes);
        assert_eq!(problems(&doc.tree), Vec::<String>::new(), "{name}");
        assert_eq!(doc.revisions, 1, "{name}");
        assert_eq!(facts(&doc), [], "{name}");
        let what: Vec<_> = cleaned.removed.iter().map(|r| r.what.as_str()).collect();
        assert!(
            what[0].starts_with("Document information"),
            "{name}: {what:?}"
        );
        assert!(what[1].starts_with("XMP"), "{name}: {what:?}");
    }
}

#[test]
fn a_clean_copy_drops_the_version_an_edit_replaced() {
    let data = fixture("report.pdf");
    let (doc, ctx) = parse_with(&data);
    let pages: Vec<_> = ctx.objects.iter().filter(|o| o.num == 4).collect();
    let (old, new) = (pages[0].stream.unwrap().0, pages[1].stream.unwrap().0);
    let bytes = |r: ByteRange| &data[r.start as usize..r.end() as usize];
    let cleaned = crate::clean::clean(&data).unwrap();
    assert!(
        !contains(&cleaned.bytes, bytes(old)),
        "the salary line is gone"
    );
    assert!(
        contains(&cleaned.bytes, bytes(new)),
        "the current page stays"
    );
    assert!(!contains(&cleaned.bytes, b"Olena"));
    assert_eq!(doc.edits, 1);
    assert!(
        cleaned
            .removed
            .iter()
            .any(|r| r.what.starts_with("Earlier versions") && r.bytes > 0)
    );
}

#[test]
fn an_encrypted_document_is_not_rewritten() {
    let pdf = b"%PDF-1.4
1 0 obj << /Type /Catalog >> endobj
trailer << /Root 1 0 R /Encrypt 2 0 R >>
startxref
0
%%EOF
";
    assert_eq!(
        crate::clean::clean(pdf),
        Err(crate::clean::CleanError::Locked)
    );
}

#[test]
fn an_encrypted_document_that_opens_without_a_password_is_read() {
    for (name, scheme) in [
        ("encrypted-rc4-40.pdf", "RC4, 40-bit"),
        ("encrypted-rc4-128.pdf", "RC4"),
        ("encrypted-aes-128.pdf", "AES-128"),
        ("encrypted-aes-256.pdf", "AES-256"),
    ] {
        let doc = parse_pdf(&fixture(name));
        assert_eq!(problems(&doc.tree), Vec::<String>::new(), "{name}");
        assert_eq!(doc.lock, Some(crypt::Lock::Open { scheme }), "{name}");
        let f = facts(&doc);
        assert!(f.contains(&("author", "Mariia Bondar")), "{name}: {f:?}");
        assert!(f.contains(&("title", "Payroll 2026")), "{name}: {f:?}");
        assert!(
            f.contains(&("created", "2026-05-12 10:00 UTC")),
            "{name}: {f:?}"
        );
        // From the encrypted XMP.
        assert!(
            f.contains(&("application", "Sample Writer 3.1")),
            "{name}: {f:?}"
        );
        assert!(
            f.last().unwrap().1.contains("opens without a password"),
            "{name}"
        );
        // The tree shows the string decrypted, too.
        let author = doc.tree.get(doc.facts[0].node);
        assert_eq!(
            author.value,
            Some(Value::Text("“Mariia Bondar”".into())),
            "{name}"
        );
    }
}

#[test]
fn a_document_that_needs_a_password_says_so_and_nothing_more() {
    let doc = parse_pdf(&fixture("encrypted-password.pdf"));
    assert_eq!(doc.lock, Some(crypt::Lock::Password { scheme: "AES-256" }));
    let f = facts(&doc);
    assert_eq!(f.len(), 1, "{f:?}");
    assert_eq!(f[0].0, "encryption");
}

#[test]
fn the_iso_standard_itself_opens_when_present() {
    // ISO 32000-1 as Adobe publishes it: 22 MB, RC4 with a 40-bit key. Not
    // in the repository; run with the file at this path to check a real one.
    let Ok(data) = std::fs::read("/tmp/PDF32000_2008.pdf") else {
        return;
    };
    let doc = parse_pdf(&data);
    assert_eq!(
        doc.lock,
        Some(crypt::Lock::Open {
            scheme: "RC4, 40-bit"
        })
    );
    assert!(facts(&doc).contains(&("author", "Jim King")));
}
