use super::*;
use crate::clean::clean;
use crate::model::NodeKind;

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

fn kinds(doc: &CfbDocument) -> Vec<&'static str> {
    doc.facts.iter().map(|f| f.kind).collect()
}

#[test]
fn an_old_word_file_says_who_wrote_it() {
    // Written by macOS's textutil: -convert doc -author "Olena Koval" -title Plan.
    let data = fixture("plan.doc");
    assert!(is_cfb(&data));
    let doc = parse_cfb(&data);
    assert_eq!(doc.kind, Kind::Word);
    assert_eq!(problems(&doc.tree), Vec::<String>::new());
    let fact = |k: &str| {
        doc.facts
            .iter()
            .find(|f| f.kind == k)
            .map(|f| f.text.as_str())
    };
    assert_eq!(fact("author"), Some("Olena Koval"));
    assert_eq!(fact("title"), Some("Plan"));
    assert_eq!(doc.tree.get(doc.facts[0].node).label, "SummaryInformation");
    // Every byte after the header belongs to one run of sectors, in order.
    let top: Vec<_> = doc
        .tree
        .get(0)
        .children
        .iter()
        .map(|&c| doc.tree.get(c).range)
        .collect();
    assert!(top.windows(2).all(|w| w[0].end() == w[1].start));
    assert_eq!(top.last().unwrap().end(), data.len() as u64);
}

#[test]
fn its_clean_copy_blanks_the_properties_and_moves_nothing() {
    let data = fixture("plan.doc");
    let copy = clean(&data).unwrap();
    assert_eq!(copy.bytes.len(), data.len());
    assert!(copy.removed[0].what.starts_with("document properties"));
    let again = parse_cfb(&copy.bytes);
    assert_eq!(again.kind, Kind::Word);
    assert_eq!(kinds(&again), Vec::<&str>::new());
    // The document itself is untouched.
    let c = Compound::open(&data).unwrap();
    let word = &c.entries[c.find(0, "WordDocument").unwrap()];
    let before = c.read(word, word.size);
    let c2 = Compound::open(&copy.bytes).unwrap();
    assert_eq!(
        c2.read(&c2.entries[c2.find(0, "WordDocument").unwrap()], word.size),
        before
    );
}

#[test]
fn an_outlook_message_is_checked_as_the_email_it_came_as() {
    // scripts/make-sample-msg.py: the fake bank email, as Outlook saves one.
    let data = fixture("phishing.msg");
    let doc = parse_cfb(&data);
    assert_eq!(doc.kind, Kind::Message);
    assert_eq!(problems(&doc.tree), Vec::<String>::new());
    let k = kinds(&doc);
    for want in [
        "sentfrom",
        "replyto",
        "authfail",
        "linkmismatch",
        "linkidn",
        "riskyfile",
    ] {
        assert!(k.contains(&want), "{want} in {k:?}");
    }
    let at = |kind: &str| {
        doc.tree
            .get(doc.facts.iter().find(|f| f.kind == kind).unwrap().node)
            .label
            .clone()
    };
    assert_eq!(at("replyto"), "internet headers");
    assert_eq!(at("linkmismatch"), "HTML");
    assert_eq!(at("riskyfile"), "attachment 1 › file");
    assert_eq!(doc.attachments, ["invoice.pdf.html"]);
    let file = attachment_bytes(&data, 0).unwrap();
    assert!(file.starts_with(b"<html><body>Sign in"));
    assert!(attachment_bytes(&data, 1).is_err());
    // A message is not rewritten.
    assert!(clean(&data).is_err());
}

#[test]
fn a_damaged_compound_file_says_so_and_never_panics() {
    let data = fixture("plan.doc");
    for cut in [8, 100, 511, 600, 5000, data.len() - 1] {
        let doc = parse_cfb(&data[..cut]);
        assert!(!doc.tree.is_empty());
    }
    let doc = parse_cfb(&data[..300]);
    assert_eq!(
        problems(&doc.tree),
        ["not readable: the header is cut short"]
    );
    // A table whose chains run in circles.
    let mut looped = data.clone();
    let fat = 512 * (1 + u32::from_le_bytes(looped[76..80].try_into().unwrap()) as usize);
    for i in 0..128 {
        looped[fat + 4 * i..fat + 4 * i + 4].copy_from_slice(&(i as u32).to_le_bytes());
    }
    let _ = parse_cfb(&looped);
    let _ = clean(&looped);
    // Every byte of the header turned, one at a time.
    for i in 0..512 {
        let mut d = data.clone();
        d[i] ^= 0xFF;
        let _ = parse_cfb(&d);
    }
}
