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
    assert!(doc.facts.is_empty());
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
