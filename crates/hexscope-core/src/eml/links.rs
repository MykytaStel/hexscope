//! What a message's links and files would do if opened. A link's words can
//! name one site while it goes to another — the heart of most phishing — and
//! an attachment can be a program, or a web page that opens any site, named
//! to look like a document. All of it is read from the message, offline:
//! nothing is looked up or visited.

/// Links read from one message, at most.
const MAX_LINKS: usize = 500;

/// A link in a web-page part: where it goes, and the words it shows.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Link {
    pub href: String,
    pub text: String,
}

/// `&amp;`, `&#64;` and the few others links are written with.
fn entities(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        let end = rest.find(';').filter(|e| *e <= 10);
        let decoded = end.and_then(|e| {
            let name = &rest[1..e];
            match name {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                "nbsp" => Some(' '),
                _ => {
                    let n = name.strip_prefix('#')?;
                    let v = match n.strip_prefix(['x', 'X']) {
                        Some(h) => u32::from_str_radix(h, 16).ok()?,
                        None => n.parse().ok()?,
                    };
                    char::from_u32(v)
                }
            }
        });
        match (decoded, end) {
            (Some(c), Some(e)) => {
                out.push(c);
                rest = &rest[e + 1..];
            }
            _ => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// An attribute's value in a tag's text, quoted or not.
fn attribute(tag: &str, name: &str) -> Option<String> {
    let lower = tag.to_ascii_lowercase();
    let mut from = 0;
    while let Some(i) = lower.get(from..)?.find(name) {
        let at = from + i;
        from = at + name.len();
        // A whole attribute name, then `=`.
        let before = lower
            .as_bytes()
            .get(at.wrapping_sub(1))
            .copied()
            .unwrap_or(b' ');
        if !before.is_ascii_whitespace() {
            continue;
        }
        let rest = tag.get(from..)?.trim_start();
        let Some(rest) = rest.strip_prefix('=') else {
            continue;
        };
        let rest = rest.trim_start();
        let value = match rest.chars().next()? {
            q @ ('"' | '\'') => rest[1..].split(q).next()?,
            _ => rest.split(|c: char| c.is_whitespace() || c == '>').next()?,
        };
        return Some(entities(value.trim()));
    }
    None
}

/// The text between tags, entities decoded and spaces folded.
fn words(html: &str) -> String {
    let mut out = String::new();
    let mut inside = false;
    for c in html.chars() {
        match c {
            '<' => inside = true,
            '>' => inside = false,
            _ if !inside => out.push(c),
            _ => {}
        }
    }
    entities(&out)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Every `<a href>` in a web page, with its words.
pub(super) fn links(html: &str) -> Vec<Link> {
    let lower = html.to_ascii_lowercase();
    let mut out = Vec::new();
    let mut at = 0;
    while out.len() < MAX_LINKS {
        let Some(i) = lower.get(at..).and_then(|s| s.find("<a")) else {
            break;
        };
        let start = at + i;
        at = start + 2;
        if !lower
            .as_bytes()
            .get(at)
            .is_some_and(|b| b.is_ascii_whitespace())
        {
            continue;
        }
        let Some(close) = lower[at..].find('>').map(|c| at + c) else {
            break;
        };
        let tag = &html[start..close];
        let end = lower[close..]
            .find("</a")
            .map_or(lower.len(), |e| close + e);
        at = end;
        if let Some(href) = attribute(tag, "href") {
            out.push(Link {
                href,
                text: words(&html[close + 1..end]),
            });
        }
    }
    out
}

/// The host a link goes to, lower case, for web links only.
pub(crate) fn host(href: &str) -> Option<String> {
    let lower = href.trim().to_ascii_lowercase();
    let rest = lower
        .strip_prefix("https://")
        .or_else(|| lower.strip_prefix("http://"))?;
    let authority = rest.split(['/', '?', '#']).next()?;
    // user:password@host:port
    let host = authority.rsplit('@').next()?.split(':').next()?;
    (!host.is_empty() && host.contains('.')).then(|| host.trim_end_matches('.').to_string())
}

/// The site a link's words name, when they name one: a web address, or a
/// bare domain such as `example-bank.com`.
pub(crate) fn named(text: &str) -> Option<String> {
    let t = text.trim().trim_end_matches(['/', '.', ',']);
    if let Some(h) = host(t) {
        return Some(h);
    }
    let first = t.split_whitespace().next()?;
    if first.len() != t.len() {
        return None;
    }
    let bare = first.to_ascii_lowercase();
    let bare = bare.split(['/', '?', '#']).next()?;
    let labels: Vec<&str> = bare.split('.').collect();
    let tld = labels.last()?;
    let ok = labels.len() >= 2
        && tld.len() >= 2
        && tld.chars().all(|c| c.is_ascii_alphabetic())
        && labels
            .iter()
            .all(|l| !l.is_empty() && l.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'));
    ok.then(|| bare.to_string())
}

/// A label written in punycode (RFC 3492), as its letters: `xn--exmple-bank-zij`
/// is “exаmple-bank”, its second letter Cyrillic.
pub(super) fn unpunycode(label: &str) -> Option<String> {
    const BASE: u32 = 36;
    let input = label.strip_prefix("xn--")?;
    let (basic, rest) = match input.rfind('-') {
        Some(i) => (&input[..i], &input[i + 1..]),
        None => ("", input),
    };
    let mut out: Vec<char> = basic.chars().collect();
    let (mut n, mut i, mut bias) = (128u32, 0u32, 72u32);
    let mut digits = rest.bytes().peekable();
    while digits.peek().is_some() {
        let old = i;
        let mut w = 1u32;
        let mut k = BASE;
        loop {
            let d = match digits.next()? {
                c @ b'a'..=b'z' => u32::from(c - b'a'),
                c @ b'0'..=b'9' => u32::from(c - b'0') + 26,
                _ => return None,
            };
            i = i.checked_add(d.checked_mul(w)?)?;
            let t = if k <= bias {
                1
            } else if k >= bias + 26 {
                26
            } else {
                k - bias
            };
            if d < t {
                break;
            }
            w = w.checked_mul(BASE - t)?;
            k += BASE;
        }
        let len = out.len() as u32 + 1;
        // Adapt the bias (RFC 3492 §6.1).
        let mut delta = if old == 0 {
            (i - old) / 700
        } else {
            (i - old) / 2
        };
        delta += delta / len;
        let mut k = 0;
        while delta > ((BASE - 1) * 26) / 2 {
            delta /= BASE - 1;
            k += BASE;
        }
        bias = k + (BASE * delta) / (delta + 38);
        n = n.checked_add(i / len)?;
        i %= len;
        out.insert(i as usize, char::from_u32(n)?);
        i += 1;
        if out.len() > 63 {
            return None;
        }
    }
    Some(out.into_iter().collect())
}

/// A host as it would be shown: each punycode label as its letters.
pub(crate) fn shown(host: &str) -> String {
    host.split('.')
        .map(|l| unpunycode(l).unwrap_or_else(|| l.to_string()))
        .collect::<Vec<_>>()
        .join(".")
}

/// What an attached file is, when opening it could do harm: why, in words.
pub(super) fn risky(name: &str) -> Option<String> {
    let lower = name.to_ascii_lowercase();
    let mut parts = lower.rsplit('.');
    let ext = parts.next()?;
    if ext == lower {
        return None;
    }
    let what = match ext {
        "html" | "htm" | "shtml" | "xhtml" | "svg" => {
            "a web page: opened, it can show a sign-in page for any site, or send you to one"
        }
        "exe" | "scr" | "com" | "pif" | "msi" | "bat" | "cmd" | "ps1" | "vbs" | "vbe" | "js"
        | "jse" | "wsf" | "hta" | "jar" | "app" | "command" | "sh" => {
            "a program: opened, it runs on your computer"
        }
        "lnk" | "url" => "a shortcut that opens a program or a site",
        "iso" | "img" | "vhd" | "vhdx" => {
            "a disk image, which can carry programs past your computer's warnings"
        }
        "docm" | "xlsm" | "pptm" => "an Office file with macros: programs that run when allowed",
        _ => return None,
    };
    // "invoice.pdf.html": named to look like something else.
    let disguise = parts
        .next()
        .filter(|prev| {
            matches!(
                *prev,
                "pdf" | "doc" | "docx" | "xls" | "xlsx" | "jpg" | "jpeg" | "png" | "txt" | "zip"
            )
        })
        .map(|prev| format!(" named to look like a .{prev},"))
        .unwrap_or_default();
    Some(format!("“{name}” is{disguise} {what}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_and_their_words_are_read() {
        let html = r#"<p>Hi <A class=x HREF="https://login.example.info/a?b=1&amp;c=2">www.example-bank.com</A>,
            <a href='mailto:x@y.z'>write</a> <a name=top>no</a> <a  href=https://example.org/>Our <b>site</b></a>
            <abbr>not a link</abbr>"#;
        let l = links(html);
        assert_eq!(l.len(), 3, "{l:?}");
        assert_eq!(l[0].href, "https://login.example.info/a?b=1&c=2");
        assert_eq!(l[0].text, "www.example-bank.com");
        assert_eq!(l[2].text, "Our site");
        assert_eq!(host(&l[0].href).as_deref(), Some("login.example.info"));
        assert_eq!(named(&l[0].text).as_deref(), Some("www.example-bank.com"));
        assert_eq!(named("Our site"), None);
        assert_eq!(
            named("https://example.org/path"),
            Some("example.org".into())
        );
        assert_eq!(
            host("https://user:pw@evil.example:8080/x").as_deref(),
            Some("evil.example")
        );
        assert_eq!(host("mailto:a@b.c"), None);
        assert_eq!(entities("a &#64; b &#x41; &bogus; &"), "a @ b A &bogus; &");
    }

    #[test]
    fn attachments_that_run_or_open_a_site_are_named() {
        assert!(
            risky("invoice.pdf.html")
                .unwrap()
                .starts_with("“invoice.pdf.html” is named to look like a .pdf, a web page")
        );
        assert_eq!(
            unpunycode("xn--exmple-bank-zij").as_deref(),
            Some("ex\u{430}mple-bank")
        );
        assert_eq!(
            unpunycode("xn--mnchen-3ya").as_deref(),
            Some("m\u{fc}nchen")
        );
        assert_eq!(unpunycode("xn--zzzzzzzzzzzzzzz"), None);
        assert_eq!(shown("www.xn--mnchen-3ya.de"), "www.m\u{fc}nchen.de");
        assert!(risky("setup.EXE").unwrap().contains("a program"));
        assert!(risky("report.docx").is_none());
        assert!(risky("photo.jpg").is_none());
        assert!(risky("html").is_none());
    }
}
