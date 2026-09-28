//! Email saved as a file (`.eml`): header lines, then a body that MIME may
//! split into parts, some of them attached files (RFC 5322, RFC 2045–2047).
//!
//! What a message gives away sits in its header: the servers it passed
//! through stamp a `Received` line each, the first of them with the address
//! the sender's computer connected from; the mail program names itself; the
//! date says the sender's time zone; the message's ID can carry the name of
//! the computer that wrote it. And whether the sender's domain vouched for
//! it, or replies go somewhere else, is what tells a forgery.

pub(crate) mod docs;
pub(crate) mod links;
#[cfg(test)]
mod tests;

use crate::model::{ByteRange, NodeId, NodeKind, ParseTree, Value};
use crate::zip::DocumentFact;

/// Headers read per part, parts per message, and how deep parts nest.
const MAX_HEADERS: usize = 500;
const MAX_PARTS: usize = 200;
const MAX_DEPTH: usize = 8;
/// Characters of a header's value shown, at most.
const MAX_SHOWN: usize = 300;
/// Names and addresses quoted in a fact, at most.
const MAX_QUOTED: usize = 4;
/// Web-page text read for its links, at most, across the message.
const MAX_HTML: usize = 4 * 1024 * 1024;

#[derive(Debug)]
pub struct EmlDocument {
    pub tree: ParseTree,
    pub facts: Vec<DocumentFact>,
    /// Attached files by name, in the order they come.
    pub attachments: Vec<String>,
}

/// One header line, unfolded: its name as written, its value, and where
/// it lies, continuation lines and all.
#[derive(Debug, Clone)]
struct Header {
    name: String,
    value: String,
    start: usize,
    end: usize,
    node: NodeId,
}

fn header<'a>(hs: &'a [Header], name: &str) -> Option<&'a Header> {
    hs.iter().find(|h| h.name.eq_ignore_ascii_case(name))
}

/// Whether a file reads as a message: header lines from its first byte, one
/// of them a sender, a date or a message ID, with nothing but text before
/// the body.
pub fn is_eml(data: &[u8]) -> bool {
    let head = data.get(..data.len().min(8192)).unwrap_or(&[]);
    let Ok(text) = std::str::from_utf8(head)
        .or_else(|e| std::str::from_utf8(head.get(..e.valid_up_to()).unwrap_or(&[])))
    else {
        return false;
    };
    let mut lines = text.split('\n').map(|l| l.strip_suffix('\r').unwrap_or(l));
    let Some(first) = lines.next() else {
        return false;
    };
    let is_field = |l: &str| {
        l.split_once(':').is_some_and(|(name, _)| {
            !name.is_empty()
                && name.len() < 80
                && name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        })
    };
    // Some mailboxes start each message with an "From " envelope line.
    if !is_field(first) && !first.starts_with("From ") {
        return false;
    }
    let mut known = 0;
    for l in std::iter::once(first).chain(lines).take(200) {
        if l.is_empty() {
            break;
        }
        if l.starts_with(' ') || l.starts_with('\t') || l.starts_with("From ") {
            continue;
        }
        if !is_field(l) {
            return false;
        }
        let name = l.split(':').next().unwrap_or("").to_ascii_lowercase();
        if matches!(
            name.as_str(),
            "from"
                | "date"
                | "message-id"
                | "received"
                | "return-path"
                | "mime-version"
                | "subject"
        ) {
            known += 1;
        }
    }
    known >= 2
}

/// The end of the line starting at `at`, past its line break.
fn line_end(data: &[u8], at: usize, to: usize) -> usize {
    data.get(at..to)
        .and_then(|s| s.iter().position(|&b| b == b'\n'))
        .map_or(to, |i| at + i + 1)
}

/// Reads header lines from `from` up to the empty line; returns them and
/// where the body starts.
fn read_headers(
    data: &[u8],
    from: usize,
    to: usize,
    tree: &mut ParseTree,
    parent: NodeId,
) -> (Vec<Header>, usize) {
    let mut out: Vec<Header> = Vec::new();
    let mut at = from;
    while at < to {
        let end = line_end(data, at, to);
        let line = data.get(at..end).unwrap_or(&[]);
        let bare = trim_eol(line);
        if bare.is_empty() {
            return finish(tree, parent, out, end);
        }
        let first = line.first().copied().unwrap_or(0);
        if (first == b' ' || first == b'\t') && !out.is_empty() {
            if let Some(last) = out.last_mut() {
                last.value.push(' ');
                last.value.push_str(String::from_utf8_lossy(bare).trim());
                last.end = end;
            }
        } else if let Some(colon) = bare.iter().position(|&b| b == b':') {
            if out.len() >= MAX_HEADERS {
                at = end;
                continue;
            }
            let name = String::from_utf8_lossy(bare.get(..colon).unwrap_or(&[]))
                .trim()
                .to_string();
            let value = String::from_utf8_lossy(bare.get(colon + 1..).unwrap_or(&[]))
                .trim()
                .to_string();
            out.push(Header {
                name,
                value,
                start: at,
                end,
                node: 0,
            });
        } else if at == from && bare.starts_with(b"From ") {
            // An mbox envelope line: not a header, part of the file.
        } else {
            // Not a header: the body starts here, with no empty line before it.
            return finish(tree, parent, out, at);
        }
        at = end;
    }
    finish(tree, parent, out, to)
}

fn finish(
    tree: &mut ParseTree,
    parent: NodeId,
    mut hs: Vec<Header>,
    body: usize,
) -> (Vec<Header>, usize) {
    for h in &mut hs {
        let shown = decode_words(&h.value);
        let shown: String = shown.chars().take(MAX_SHOWN).collect();
        h.node = tree.add(
            Some(parent),
            h.name.clone(),
            ByteRange::new(h.start as u64, (h.end - h.start) as u64),
            NodeKind::Field,
            Some(Value::Text(shown)),
        );
    }
    (hs, body)
}

fn trim_eol(line: &[u8]) -> &[u8] {
    let line = line.strip_suffix(b"\n").unwrap_or(line);
    line.strip_suffix(b"\r").unwrap_or(line)
}

/// A header value's main part and its parameters, names in lower case:
/// `multipart/mixed; boundary="x"`.
fn params(value: &str) -> (String, Vec<(String, String)>) {
    let mut parts = value.split(';');
    let main = parts.next().unwrap_or("").trim().to_ascii_lowercase();
    let list = parts
        .filter_map(|p| {
            let (k, v) = p.split_once('=')?;
            Some((
                k.trim().to_ascii_lowercase(),
                v.trim().trim_matches('"').to_string(),
            ))
        })
        .collect();
    (main, list)
}

fn param<'a>(list: &'a [(String, String)], name: &str) -> Option<&'a str> {
    list.iter()
        .find(|(k, _)| k == name)
        .map(|(_, v)| v.as_str())
}

/// An attachment's name: `filename`, or RFC 2231's `filename*`, or the
/// type's `name`.
fn file_name(disposition: &[(String, String)], content: &[(String, String)]) -> Option<String> {
    if let Some(v) = param(disposition, "filename*").or(param(content, "name*")) {
        // charset'language'percent-encoded
        let raw = v.splitn(3, '\'').nth(2).unwrap_or(v);
        return Some(percent(raw));
    }
    param(disposition, "filename")
        .or(param(content, "name"))
        .map(decode_words)
        .filter(|n| !n.is_empty())
}

fn percent(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while let Some(&c) = b.get(i) {
        if c == b'%'
            && let (Some(h), Some(l)) = (
                b.get(i + 1).and_then(|x| hex(*x)),
                b.get(i + 2).and_then(|x| hex(*x)),
            )
        {
            out.push(h * 16 + l);
            i += 3;
            continue;
        }
        out.push(c);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

/// RFC 2047's encoded words, `=?utf-8?B?…?=` and `=?utf-8?Q?…?=`, as text.
/// Other character sets are read as UTF-8, and Latin-1 byte for byte.
pub(crate) fn decode_words(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    let mut last_was_word = false;
    while let Some(i) = rest.find("=?") {
        let before = rest.get(..i).unwrap_or("");
        let after = rest.get(i + 2..).unwrap_or("");
        let word = after.split_once('?').and_then(|(charset, r)| {
            let (enc, r) = r.split_once('?')?;
            let (text, tail) = r.split_once("?=")?;
            let bytes = match enc {
                "B" | "b" => base64(text.as_bytes()),
                "Q" | "q" => quoted(text.replace('_', " ").as_bytes()),
                _ => return None,
            };
            let latin = charset.eq_ignore_ascii_case("iso-8859-1")
                || charset.eq_ignore_ascii_case("latin1");
            let t = if latin {
                bytes.iter().map(|&b| b as char).collect()
            } else {
                String::from_utf8_lossy(&bytes).into_owned()
            };
            Some((t, tail))
        });
        match word {
            Some((t, tail)) => {
                // Space between two encoded words is not part of the text.
                if !(last_was_word && before.trim().is_empty()) {
                    out.push_str(before);
                }
                out.push_str(&t);
                rest = tail;
                last_was_word = true;
            }
            None => {
                out.push_str(rest.get(..i + 2).unwrap_or(""));
                rest = after;
                last_was_word = false;
            }
        }
    }
    out.push_str(rest);
    out
}

/// Base64, skipping anything that is not of its alphabet.
pub(crate) fn base64(input: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(input.len() / 4 * 3);
    let mut acc = 0u32;
    let mut bits = 0;
    for &c in input {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            b'=' => break,
            _ => continue,
        };
        acc = (acc << 6) | v as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    out
}

/// Quoted-printable: `=XX` is a byte, `=` at a line's end joins it to the next.
pub(crate) fn quoted(input: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(input.len());
    let mut i = 0;
    while let Some(&c) = input.get(i) {
        if c == b'=' {
            match (input.get(i + 1), input.get(i + 2)) {
                (Some(b'\r'), Some(b'\n')) => {
                    i += 3;
                    continue;
                }
                (Some(b'\n'), _) => {
                    i += 2;
                    continue;
                }
                (Some(&h), Some(&l)) => {
                    if let (Some(h), Some(l)) = (hex(h), hex(l)) {
                        out.push(h * 16 + l);
                        i += 3;
                        continue;
                    }
                }
                _ => {}
            }
        }
        out.push(c);
        i += 1;
    }
    out
}

/// An attached file: its name, where its encoded bytes lie, and how they
/// are encoded.
#[derive(Debug, Clone)]
struct Attached {
    name: String,
    start: usize,
    end: usize,
    encoding: String,
}

struct Walk {
    parts: usize,
    attached: Vec<Attached>,
    /// Every part's headers, for the checks after.
    top: Vec<Header>,
    /// Each web-page part's text, decoded, and its node: for its links.
    html: Vec<(String, NodeId)>,
}

/// Reads one entity — headers, then a body that may be parts — between
/// `from` and `to`, under `parent`.
fn entity(
    data: &[u8],
    from: usize,
    to: usize,
    tree: &mut ParseTree,
    parent: NodeId,
    depth: usize,
    w: &mut Walk,
) {
    let head = tree.add(
        Some(parent),
        "headers",
        ByteRange::new(from as u64, 0),
        NodeKind::Container,
        None,
    );
    let (hs, body) = read_headers(data, from, to, tree, head);
    tree.set_range(head, ByteRange::new(from as u64, (body - from) as u64));
    let (ctype, cparams) = params(header(&hs, "Content-Type").map_or("text/plain", |h| &h.value));
    let (disp, dparams) = params(header(&hs, "Content-Disposition").map_or("", |h| &h.value));
    let encoding = header(&hs, "Content-Transfer-Encoding")
        .map_or("7bit".to_string(), |h| h.value.trim().to_ascii_lowercase());
    if depth == 0 {
        w.top = hs.clone();
    }
    let len = (to.saturating_sub(body)) as u64;
    if ctype.starts_with("multipart/")
        && let Some(boundary) = param(&cparams, "boundary").filter(|b| !b.is_empty())
        && depth < MAX_DEPTH
    {
        let parts = tree.add(
            Some(parent),
            "parts",
            ByteRange::new(body as u64, len),
            NodeKind::Container,
            Some(Value::Text(ctype.clone())),
        );
        multipart(data, body, to, boundary, tree, parts, depth, w);
        return;
    }
    // A forwarded message: headers and a body of its own, shown, not judged.
    if ctype == "message/rfc822" && depth < MAX_DEPTH {
        let inner = tree.add(
            Some(parent),
            "forwarded message",
            ByteRange::new(body as u64, len),
            NodeKind::Container,
            None,
        );
        entity(data, body, to, tree, inner, depth + 1, w);
        return;
    }
    let name = file_name(&dparams, &cparams);
    let is_file = disp == "attachment" || (name.is_some() && !ctype.starts_with("text/"));
    if is_file {
        let name = name.unwrap_or_else(|| "attachment".to_string());
        tree.add(
            Some(parent),
            "attachment",
            ByteRange::new(body as u64, len),
            NodeKind::Field,
            Some(Value::Text(format!("“{name}” · {ctype} · {encoding}"))),
        );
        w.attached.push(Attached {
            name,
            start: body,
            end: to,
            encoding,
        });
    } else {
        let node = tree.add(
            Some(parent),
            "body",
            ByteRange::new(body as u64, len),
            NodeKind::Field,
            Some(Value::Text(format!("{ctype} · {len} bytes"))),
        );
        // A web page's links are read after: decoded, within bounds.
        let held: usize = w.html.iter().map(|h| h.0.len()).sum();
        if ctype == "text/html"
            && let Some(raw) = data.get(body..to)
            && held + raw.len() <= MAX_HTML
        {
            let bytes = match encoding.as_str() {
                "base64" => base64(raw),
                "quoted-printable" => quoted(raw),
                _ => raw.to_vec(),
            };
            w.html
                .push((String::from_utf8_lossy(&bytes).into_owned(), node));
        }
    }
}

/// The parts between a multipart body's boundary lines (RFC 2046 §5.1.1).
#[allow(clippy::too_many_arguments)]
fn multipart(
    data: &[u8],
    from: usize,
    to: usize,
    boundary: &str,
    tree: &mut ParseTree,
    parent: NodeId,
    depth: usize,
    w: &mut Walk,
) {
    let delim = format!("--{boundary}");
    let delim = delim.as_bytes();
    // Every line that starts with the delimiter, and whether it closes.
    let mut marks: Vec<(usize, usize, bool)> = Vec::new();
    let mut at = from;
    while at < to {
        let end = line_end(data, at, to);
        let line = trim_eol(data.get(at..end).unwrap_or(&[]));
        if let Some(rest) = line.strip_prefix(delim) {
            let closing = rest.starts_with(b"--");
            marks.push((at, end, closing));
            if closing {
                break;
            }
        }
        at = end;
    }
    let mut n = 0;
    for pair in marks.windows(2) {
        let [(_, start, closing), (next, ..)] = pair else {
            continue;
        };
        if *closing || w.parts >= MAX_PARTS {
            break;
        }
        w.parts += 1;
        n += 1;
        // The line break before a delimiter belongs to the delimiter.
        let mut end = *next;
        if data.get(end.saturating_sub(2)..end) == Some(b"\r\n") {
            end -= 2;
        } else if data.get(end.saturating_sub(1)..end) == Some(b"\n") {
            end -= 1;
        }
        let end = end.max(*start);
        let part = tree.add(
            Some(parent),
            format!("part {n}"),
            ByteRange::new(*start as u64, (end - start) as u64),
            NodeKind::Container,
            None,
        );
        entity(data, *start, end, tree, part, depth + 1, w);
    }
    // A last part with no closing delimiter runs to the end.
    if let Some(&(_, start, false)) = marks.last()
        && w.parts < MAX_PARTS
        && start < to
    {
        w.parts += 1;
        let part = tree.add(
            Some(parent),
            format!("part {}", n + 1),
            ByteRange::new(start as u64, (to - start) as u64),
            NodeKind::Container,
            None,
        );
        tree.warning(part, "no closing boundary", ByteRange::new(to as u64, 0));
        entity(data, start, to, tree, part, depth + 1, w);
    }
}

pub fn parse_eml(data: &[u8]) -> EmlDocument {
    let mut tree = ParseTree::new();
    let root = tree.add(
        None,
        "email",
        ByteRange::new(0, data.len() as u64),
        NodeKind::Container,
        None,
    );
    let mut w = Walk {
        parts: 0,
        attached: Vec::new(),
        top: Vec::new(),
        html: Vec::new(),
    };
    entity(data, 0, data.len(), &mut tree, root, 0, &mut w);
    let subject = header(&w.top, "Subject").map(|h| decode_words(&h.value));
    let from = header(&w.top, "From").map(|h| decode_words(&h.value));
    let summary = match (from, subject) {
        (Some(f), Some(s)) => format!("from {f} · “{s}”"),
        (Some(f), None) => format!("from {f}"),
        (None, Some(s)) => format!("“{s}”"),
        (None, None) => "a message".to_string(),
    };
    tree.set_value(
        root,
        Some(Value::Text(summary.chars().take(MAX_SHOWN).collect())),
    );
    let facts = check(&mut tree, &w);
    EmlDocument {
        tree,
        facts,
        attachments: w.attached.iter().map(|a| a.name.clone()).collect(),
    }
}

/// An attached file's bytes, decoded.
pub fn attachment_bytes(data: &[u8], index: usize) -> Result<Vec<u8>, &'static str> {
    let mut tree = ParseTree::new();
    let root = tree.add(
        None,
        "email",
        ByteRange::new(0, 0),
        NodeKind::Container,
        None,
    );
    let mut w = Walk {
        parts: 0,
        attached: Vec::new(),
        top: Vec::new(),
        html: Vec::new(),
    };
    entity(data, 0, data.len(), &mut tree, root, 0, &mut w);
    let a = w.attached.get(index).ok_or("there is no such attachment")?;
    let raw = data
        .get(a.start..a.end)
        .ok_or("the attachment is cut short")?;
    Ok(match a.encoding.as_str() {
        "base64" => base64(raw),
        "quoted-printable" => quoted(raw),
        _ => raw.to_vec(),
    })
}

/// The first IPv4 address in text, such as `[203.0.113.7]`.
fn ipv4_in(s: &str) -> Option<[u8; 4]> {
    ipv4_all(s).into_iter().next()
}

/// Every IPv4 address in text, in order.
fn ipv4_all(s: &str) -> Vec<[u8; 4]> {
    let mut out = Vec::new();
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b.get(i).is_some_and(u8::is_ascii_digit)
            && (i == 0
                || !b
                    .get(i - 1)
                    .is_some_and(|c| c.is_ascii_digit() || *c == b'.'))
        {
            let mut parts = [0u16; 4];
            let mut k = 0;
            let mut j = i;
            let mut digits = 0;
            while let Some(&c) = b.get(j) {
                if c.is_ascii_digit() && digits < 3 {
                    parts[k] = parts[k] * 10 + (c - b'0') as u16;
                    digits += 1;
                } else if c == b'.' && digits > 0 && k < 3 {
                    k += 1;
                    digits = 0;
                } else {
                    break;
                }
                j += 1;
            }
            let after_ok = !b.get(j).is_some_and(|c| c.is_ascii_digit() || *c == b'.');
            if k == 3 && digits > 0 && after_ok && parts.iter().all(|&p| p <= 255) {
                out.push(parts.map(|p| p as u8));
            }
            i = j.max(i + 1);
        } else {
            i += 1;
        }
    }
    out
}

/// IPv6 addresses written in brackets, as servers write them: `[2001:db8::1]`
/// or `[IPv6:2001:db8::1]`.
fn ipv6_all(s: &str) -> Vec<String> {
    s.split('[')
        .skip(1)
        .filter_map(|t| t.split(']').next())
        .map(|t| t.strip_prefix("IPv6:").unwrap_or(t))
        .filter(|t| {
            t.contains(':')
                && t.len() >= 2
                && t.len() <= 45
                && t.bytes()
                    .all(|b| b.is_ascii_hexdigit() || b == b':' || b == b'.')
        })
        .map(str::to_ascii_lowercase)
        .collect()
}

/// An IPv6 address only its own network reaches: link-local, unique local, loopback.
fn private6(ip: &str) -> bool {
    ip == "::1"
        || ip.starts_with("fe8")
        || ip.starts_with("fe9")
        || ip.starts_with("fea")
        || ip.starts_with("feb")
        || ip.starts_with("fc")
        || ip.starts_with("fd")
}

/// Addresses no one reaches from the internet: a home or office network,
/// the computer itself.
fn private(ip: [u8; 4]) -> bool {
    matches!(ip, [10, ..] | [127, ..] | [192, 168, ..] | [169, 254, ..])
        || (ip[0] == 172 && (16..32).contains(&ip[1]))
        || (ip[0] == 100 && (64..128).contains(&ip[1]))
}

/// The domain of an address in a header: `Olena <olena@example.org>`.
fn domain(value: &str) -> Option<String> {
    let at = value.rfind('@')?;
    let rest = value.get(at + 1..)?;
    let d: String = rest
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '.' || *c == '-')
        .collect();
    (!d.is_empty()).then(|| d.to_ascii_lowercase())
}

/// A domain without its subdomains, for comparing who sends and who is
/// replied to: `mail.example.org` is `example.org`.
fn base(d: &str) -> &str {
    let last = d.rfind('.').unwrap_or(0);
    match d.get(..last).and_then(|h| h.rfind('.')) {
        Some(i) => d.get(i + 1..).unwrap_or(d),
        None => d,
    }
}

fn quote(items: &[String]) -> String {
    let q: Vec<String> = items
        .iter()
        .take(MAX_QUOTED)
        .map(|n| format!("“{n}”"))
        .collect();
    let more = items.len().saturating_sub(MAX_QUOTED);
    if more > 0 {
        format!("{} and {more} more", q.join(", "))
    } else {
        q.join(", ")
    }
}

/// What the message gives away, and the signs of a forgery, from its top
/// headers.
fn check(tree: &mut ParseTree, w: &Walk) -> Vec<DocumentFact> {
    let hs = &w.top;
    let mut facts = Vec::new();
    let mut fact = |kind: &'static str, text: String, node: NodeId| {
        facts.push(DocumentFact { kind, text, node });
    };

    // The first server writes the last Received line, with the address the
    // sender connected from; webmail may name it in X-Originating-IP.
    let received: Vec<&Header> = hs
        .iter()
        .filter(|h| h.name.eq_ignore_ascii_case("Received"))
        .collect();
    // In a Received line, the address the server saw is the one to trust;
    // the one the computer gave for itself may be its own network's.
    let origin = header(hs, "X-Originating-IP")
        .and_then(|h| ipv4_in(&h.value).map(|ip| (vec![ip], h, "X-Originating-IP")))
        .or_else(|| {
            received.iter().rev().find_map(|h| {
                // Only the "from" half: "by" is the receiving server.
                let from = h.value.split(" by ").next().unwrap_or("");
                let all = ipv4_all(from);
                (!all.is_empty() || !ipv6_all(from).is_empty()).then_some((all, *h, "Received"))
            })
        });
    if let Some((all, h, from)) = origin {
        let show = |[a, b, c, d]: [u8; 4]| format!("{a}.{b}.{c}.{d}");
        let from_half = h.value.split(" by ").next().unwrap_or("");
        let six = ipv6_all(from_half);
        let public: Option<String> = all
            .iter()
            .rev()
            .find(|ip| !private(**ip))
            .map(|ip| show(*ip))
            .or_else(|| six.iter().rev().find(|ip| !private6(ip)).cloned());
        let inner: Option<String> = all
            .iter()
            .find(|ip| private(**ip))
            .map(|ip| show(*ip))
            .or_else(|| six.iter().find(|ip| private6(ip)).cloned());
        let text = match (public, inner) {
            (Some(p), Some(i)) => format!(
                "{p}, where the sender connected from, and {i}, the computer's address on its own network, in the first server's {from} line"
            ),
            (Some(p), None) => {
                format!("{p}, where the sender connected from, in the first server's {from} line")
            }
            (None, Some(i)) => format!(
                "{i}, an address on the sender's own network, in the first server's {from} line"
            ),
            (None, None) => String::new(),
        };
        if !text.is_empty() {
            fact("sentfrom", text, h.node);
        }
    }

    // The sending computer's own name: in the first server's "from" host,
    // or in the message's ID.
    let local = |host: &str| {
        let host = host
            .trim_matches(|c| c == '[' || c == ']' || c == '(' || c == ')' || c == '<' || c == '>');
        let lower = host.to_ascii_lowercase();
        (lower.ends_with(".local")
            || lower.ends_with(".lan")
            || lower.ends_with(".home")
            || lower.ends_with(".localdomain")
            || lower.contains("macbook")
            || lower.contains("-pc")
            || lower.contains("desktop-")
            || lower.contains("laptop"))
        .then(|| host.to_string())
    };
    let computer = received
        .last()
        .and_then(|h| {
            let from = h.value.strip_prefix("from ")?;
            from.split_whitespace()
                .take(3)
                .find_map(local)
                .map(|n| (n, h.node))
        })
        .or_else(|| {
            let h = header(hs, "Message-ID")?;
            let host = h.value.rsplit('@').next()?;
            local(host).map(|n| (n, h.node))
        });
    if let Some((name, node)) = computer {
        fact("computer", name, node);
    }

    for name in ["X-Mailer", "User-Agent"] {
        if let Some(h) = header(hs, name) {
            fact("mailer", decode_words(&h.value), h.node);
            break;
        }
    }

    // The date's offset is the sender's clock: "+0300".
    if let Some(h) = header(hs, "Date") {
        let zone = h.value.split_whitespace().find(|t| {
            t.len() == 5
                && (t.starts_with('+') || t.starts_with('-'))
                && t.bytes().skip(1).all(|b| b.is_ascii_digit())
        });
        if let Some(z) = zone {
            let (sign, hh, mm) = (
                z.get(..1).unwrap_or("+"),
                z.get(1..3).unwrap_or("00"),
                z.get(3..5).unwrap_or("00"),
            );
            fact(
                "timezone",
                format!("UTC{sign}{hh}:{mm}, the sender's clock when it was sent"),
                h.node,
            );
        }
    }

    // Replies that go to another domain than the sender's: a sign of a
    // message that is not what it says.
    let sender = header(hs, "From").and_then(|h| domain(&h.value));
    if let (Some(from), Some(h)) = (&sender, header(hs, "Reply-To"))
        && let Some(to) = domain(&h.value)
        && base(&to) != base(from)
    {
        let (start, len) = (h.start as u64, (h.end - h.start) as u64);
        tree.warning(
            h.node,
            "replies go to another domain",
            ByteRange::new(start, len),
        );
        fact(
            "replyto",
            format!("{} — not the sender's {from}", decode_words(&h.value)),
            h.node,
        );
    }
    if let (Some(from), Some(h)) = (&sender, header(hs, "Return-Path"))
        && let Some(back) = domain(&h.value)
        && base(&back) != base(from)
    {
        fact(
            "returnpath",
            format!("bounces go to {back}, not the sender's {from}"),
            h.node,
        );
    }

    // What the receiving server found when it asked the sender's domain.
    if let Some(h) = header(hs, "Authentication-Results") {
        let v = h.value.to_ascii_lowercase();
        let failed: Vec<&str> = ["spf", "dkim", "dmarc"]
            .into_iter()
            .filter(|m| {
                [format!("{m}=fail"), format!("{m}=softfail")]
                    .iter()
                    .any(|f| v.contains(f.as_str()))
            })
            .collect();
        if !failed.is_empty() {
            let (start, len) = (h.start as u64, (h.end - h.start) as u64);
            tree.warning(
                h.node,
                "the sender's domain did not vouch for it",
                ByteRange::new(start, len),
            );
            fact(
                "authfail",
                format!(
                    "{} failed: the domain it claims to come from did not vouch for it",
                    failed.join(", ").to_ascii_uppercase()
                ),
                h.node,
            );
        } else if v.contains("=pass") {
            let passed: Vec<&str> = ["spf", "dkim", "dmarc"]
                .into_iter()
                .filter(|m| v.contains(&format!("{m}=pass")))
                .collect();
            fact(
                "auth",
                format!(
                    "{} passed: the sender's domain vouched for it",
                    passed.join(", ").to_ascii_uppercase()
                ),
                h.node,
            );
        }
    }

    if !w.attached.is_empty() {
        let names: Vec<String> = w.attached.iter().map(|a| a.name.clone()).collect();
        let n = names.len();
        let node = tree
            .nodes()
            .iter()
            .find(|x| x.label == "attachment")
            .map_or(0, |x| x.id);
        fact(
            "attachments",
            format!(
                "{n} {}: {}",
                if n == 1 { "file" } else { "files" },
                quote(&names)
            ),
            node,
        );
    }

    // What its links and files would do if opened.
    let mut sites: Vec<String> = Vec::new();
    let mut elsewhere: Vec<String> = Vec::new();
    let mut lookalike: Vec<String> = Vec::new();
    let mut first: Option<NodeId> = None;
    let mut count = 0;
    for (html, node) in &w.html {
        for l in links::links(html) {
            let Some(to) = links::host(&l.href) else {
                continue;
            };
            count += 1;
            first.get_or_insert(*node);
            if !sites.contains(&to) {
                sites.push(to.clone());
            }
            if let Some(says) = links::named(&l.text)
                && base(says.trim_start_matches("www.")) != base(&to)
            {
                let line = format!("a link reads “{}” and goes to {to}", l.text);
                if !elsewhere.contains(&line) {
                    elsewhere.push(line);
                }
            }
            if to.split('.').any(|label| label.starts_with("xn--")) && !lookalike.contains(&to) {
                lookalike.push(to);
            }
        }
    }
    if let Some(node) = first {
        let range = tree.get(node).range;
        let shown: Vec<String> = sites.iter().take(MAX_QUOTED).cloned().collect();
        let more = sites.len().saturating_sub(MAX_QUOTED);
        fact(
            "weblinks",
            format!(
                "{count} {} to {}{}",
                if count == 1 { "link," } else { "links," },
                shown.join(", "),
                if more > 0 {
                    format!(" and {more} more")
                } else {
                    String::new()
                }
            ),
            node,
        );
        if !elsewhere.is_empty() {
            tree.warning(node, "a link goes somewhere other than it says", range);
            let more = elsewhere.len().saturating_sub(2);
            fact(
                "linkmismatch",
                format!(
                    "{}{}",
                    elsewhere[..elsewhere.len().min(2)].join("; "),
                    if more > 0 {
                        format!("; and {more} more")
                    } else {
                        String::new()
                    }
                ),
                node,
            );
        }
        if !lookalike.is_empty() {
            tree.warning(node, "a link to an address in lookalike letters", range);
            fact(
                "linkidn",
                lookalike
                    .iter()
                    .map(|h| format!("{h}, shown as “{}”: letters of other alphabets that pass for Latin ones", links::shown(h)))
                    .collect::<Vec<_>>()
                    .join("; "),
                node,
            );
        }
    }
    let risky: Vec<String> = w
        .attached
        .iter()
        .filter_map(|a| links::risky(&a.name))
        .collect();
    if !risky.is_empty()
        && let Some(node) = tree
            .nodes()
            .iter()
            .find(|x| x.label == "attachment")
            .map(|x| x.id)
    {
        let range = tree.get(node).range;
        tree.warning(node, "an attachment that runs or opens a site", range);
        fact("riskyfile", risky.join("; "), node);
    }
    facts
}
