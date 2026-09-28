use super::*;

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(format!(
        "{}/tests/fixtures/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

fn facts(doc: &EmlDocument) -> Vec<(&'static str, String)> {
    doc.facts.iter().map(|f| (f.kind, f.text.clone())).collect()
}

fn problems(tree: &ParseTree) -> Vec<String> {
    tree.nodes()
        .iter()
        .filter(|n| matches!(n.kind, NodeKind::Warning | NodeKind::Error))
        .map(|n| n.label.clone())
        .collect()
}

#[test]
fn an_ordinary_message_says_where_from_on_what_and_when() {
    let data = fixture("message.eml");
    assert!(is_eml(&data));
    let doc = parse_eml(&data);
    assert_eq!(
        facts(&doc),
        [
            (
                "sentfrom",
                "203.0.113.54, where the sender connected from, and 192.168.1.23, the computer's address on its own network, in the first server's Received line".to_string()
            ),
            ("computer", "MacBook-Pro-Olena.local".to_string()),
            ("mailer", "Apple Mail (2.3826.300.87)".to_string()),
            ("timezone", "UTC+03:00, the sender's clock when it was sent".to_string()),
            ("auth", "SPF, DKIM, DMARC passed: the sender's domain vouched for it".to_string()),
            ("attachments", "1 file: “IMG_2041.jpg”".to_string()),
        ]
    );
    assert!(problems(&doc.tree).is_empty(), "{:?}", problems(&doc.tree));
    // The subject is read as words, not as its encoding.
    assert!(matches!(
        &doc.tree.get(0).value,
        Some(Value::Text(t)) if t.contains("“Photos from Saturday — the view”")
    ));
    // The attached photo comes out whole, and is a photo with a place.
    let photo = attachment_bytes(&data, 0).unwrap();
    assert_eq!(
        photo,
        std::fs::read(format!(
            "{}/../../apps/web/public/samples/photo.jpg",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap()
    );
    assert!(attachment_bytes(&data, 1).is_err());
}

#[test]
fn a_forgery_is_named_by_its_replies_and_its_domain() {
    let doc = parse_eml(&fixture("phishing.eml"));
    let f = facts(&doc);
    assert!(f.contains(&(
        "replyto",
        "verify-account@example.info — not the sender's example-bank.com".to_string()
    )));
    assert!(f.contains(&(
        "returnpath",
        "bounces go to mailer.example.com, not the sender's example-bank.com".to_string()
    )));
    assert!(
        f.iter()
            .any(|(k, t)| *k == "authfail" && t.starts_with("SPF, DMARC failed"))
    );
    assert_eq!(
        problems(&doc.tree),
        [
            "replies go to another domain",
            "the sender's domain did not vouch for it"
        ]
    );
    assert!(
        f.contains(&(
            "sentfrom",
            "192.0.2.66, where the sender connected from, in the first server's Received line"
                .to_string()
        ))
    );
}

#[test]
fn encoded_words_and_encodings_read_as_text() {
    assert_eq!(decode_words("=?utf-8?B?0J/RgNC40LLRltGC?="), "Привіт");
    assert_eq!(
        decode_words("=?UTF-8?Q?caf=C3=A9_au_lait?="),
        "café au lait"
    );
    assert_eq!(decode_words("=?utf-8?Q?a?= =?utf-8?Q?b?= c"), "ab c");
    assert_eq!(decode_words("=?iso-8859-1?Q?na=EFve?="), "naïve");
    assert_eq!(decode_words("plain =? not a word"), "plain =? not a word");
    assert_eq!(base64(b"aGV4\r\nc2NvcGU="), b"hexscope");
    assert_eq!(quoted(b"a=3Db=\r\nc"), b"a=bc");
    assert_eq!(percent("na%C3%AFve.pdf"), "naïve.pdf");
}

#[test]
fn text_that_is_not_a_message_is_not_taken_for_one() {
    assert!(!is_eml(b"just some text\nwith lines\n"));
    assert!(!is_eml(b"Subject: only one header\n\nbody"));
    assert!(is_eml(b"From: a@b.c\nDate: now\n\nbody"));
    assert!(!is_eml(b"\x89PNG\r\n"));
}

#[test]
fn broken_messages_still_make_a_tree() {
    let cut = b"From: a@b.c\r\nDate: x\r\nContent-Type: multipart/mixed; boundary=b\r\n\r\n--b\r\nContent-Type: text/plain\r\n\r\nhello";
    let doc = parse_eml(cut);
    assert_eq!(problems(&doc.tree), ["no closing boundary"]);
    for data in [
        &b""[..],
        b"From:",
        b"From: a\n Date",
        b"Content-Type: multipart/mixed; boundary=\n\n--\n--\n",
        b"Received: from 999.1.1.1 [1.2.3]\nFrom: x@\nReply-To: @\n\n",
        b"Content-Type: multipart/x; boundary=a\n\n--a\nContent-Type: multipart/x; boundary=a\n\n--a\n",
    ] {
        let doc = parse_eml(data);
        assert!(!doc.tree.is_empty());
        let _ = attachment_bytes(data, 0);
    }
}

#[test]
fn ipv6_origins_and_forwarded_messages() {
    let msg = b"Received: from [IPv6:fe80::1c2b] (laptop.lan [2001:db8:85a3::8a2e:370:7334])\r\n\tby mx.example.net; Mon, 14 Sep 2026 18:33:01 +0000\r\nFrom: a@example.org\r\nDate: Mon, 14 Sep 2026 18:33:00 +0000\r\nContent-Type: multipart/mixed; boundary=b\r\n\r\n--b\r\nContent-Type: text/plain\r\n\r\nSee below.\r\n--b\r\nContent-Type: message/rfc822\r\n\r\nFrom: first@example.com\r\nSubject: the original\r\n\r\nHello\r\n--b--\r\n";
    let doc = parse_eml(msg);
    let f = facts(&doc);
    assert!(
        f.contains(&(
            "sentfrom",
            "2001:db8:85a3::8a2e:370:7334, where the sender connected from, and fe80::1c2b, the computer's address on its own network, in the first server's Received line".to_string()
        )),
        "{f:?}"
    );
    assert!(f.contains(&("computer", "laptop.lan".to_string())), "{f:?}");
    let fwd = doc
        .tree
        .nodes()
        .iter()
        .find(|n| n.label == "forwarded message")
        .expect("forwarded");
    // Its own headers, under it.
    let inner: Vec<&str> = doc
        .tree
        .nodes()
        .iter()
        .filter(|n| {
            n.parent
                .is_some_and(|p| doc.tree.get(p).parent == Some(fwd.id))
        })
        .map(|n| n.label.as_str())
        .collect();
    assert!(
        inner.contains(&"From") && inner.contains(&"Subject"),
        "{inner:?}"
    );
    assert!(problems(&doc.tree).is_empty());
}
