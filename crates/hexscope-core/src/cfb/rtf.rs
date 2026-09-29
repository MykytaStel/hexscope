//! A message's body as Outlook keeps it: RTF, compressed (MS-OXRTFCP), and
//! for a message that came as HTML, the HTML wrapped inside the RTF
//! (MS-OXRTFEX). Taken back to HTML, so its links can be checked as an
//! email's are; a message written in RTF gives its hyperlink fields as links.

/// The dictionary a compressed stream starts from.
const PREBUF: &[u8] = b"{\\rtf1\\ansi\\mac\\deff0\\deftab720{\\fonttbl;}{\\f0\\fnil \\froman \\fswiss \\fmodern \\fscript \\fdecor MS Sans SerifSymbolArialTimes New RomanCourier{\\colortbl\\red0\\green0\\blue0\r\n\\par \\pard\\plain\\f0\\fs20\\b\\i\\u\\tab\\tx";

/// Longest RTF written out.
const MAX_OUT: usize = 4 << 20;

/// `PR_RTF_COMPRESSED`'s bytes, decompressed; `None` when they are not a
/// compressed RTF stream.
pub(crate) fn decompress(b: &[u8]) -> Option<Vec<u8>> {
    let word = |at: usize| Some(u32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?));
    let raw = word(4)? as usize;
    let data = b.get(16..)?;
    match word(8)? {
        0x414C_454D => return data.get(..raw.min(data.len())).map(<[u8]>::to_vec),
        0x7546_5A4C => {}
        _ => return None,
    }
    let mut dict = [0u8; 4096];
    dict[..PREBUF.len()].copy_from_slice(PREBUF);
    let mut write = PREBUF.len();
    let mut out = Vec::with_capacity(raw.min(MAX_OUT));
    let mut at = 0;
    while at < data.len() {
        let control = data[at];
        at += 1;
        for bit in 0..8 {
            if out.len() >= MAX_OUT {
                return Some(out);
            }
            if control & (1 << bit) == 0 {
                let Some(&c) = data.get(at) else {
                    return Some(out);
                };
                at += 1;
                out.push(c);
                dict[write] = c;
                write = (write + 1) % 4096;
                continue;
            }
            let (Some(&hi), Some(&lo)) = (data.get(at), data.get(at + 1)) else {
                return Some(out);
            };
            at += 2;
            let offset = ((hi as usize) << 4) | (lo as usize >> 4);
            if offset == write {
                return Some(out);
            }
            for k in 0..(lo as usize & 0x0F) + 2 {
                let c = dict[(offset + k) % 4096];
                out.push(c);
                dict[write] = c;
                write = (write + 1) % 4096;
            }
        }
    }
    Some(out)
}

enum Token<'a> {
    Open,
    Close,
    /// `\*`: the group is a destination a reader may skip.
    Star,
    Word(&'a [u8], Option<i32>),
    Byte(u8),
    Text(u8),
}

/// RTF's tokens: groups, control words with their numbers, `\'hh` bytes, text.
fn tokens(rtf: &[u8]) -> impl Iterator<Item = Token<'_>> {
    let mut i = 0;
    std::iter::from_fn(move || {
        loop {
            let c = *rtf.get(i)?;
            i += 1;
            return Some(match c {
                b'{' => Token::Open,
                b'}' => Token::Close,
                b'\r' | b'\n' => continue,
                b'\\' => {
                    let &n = rtf.get(i)?;
                    if n == b'\'' {
                        let hex = std::str::from_utf8(rtf.get(i + 1..i + 3)?).ok()?;
                        i += 3;
                        Token::Byte(u8::from_str_radix(hex, 16).unwrap_or(b'?'))
                    } else if n.is_ascii_alphabetic() {
                        let start = i;
                        while rtf.get(i).is_some_and(u8::is_ascii_alphabetic) {
                            i += 1;
                        }
                        let word = &rtf[start..i];
                        let digits = i;
                        if rtf.get(i) == Some(&b'-') {
                            i += 1;
                        }
                        while rtf.get(i).is_some_and(u8::is_ascii_digit) {
                            i += 1;
                        }
                        let number = std::str::from_utf8(&rtf[digits..i])
                            .ok()
                            .and_then(|s| s.parse().ok());
                        if rtf.get(i) == Some(&b' ') {
                            i += 1;
                        }
                        Token::Word(word, number)
                    } else {
                        i += 1;
                        match n {
                            b'*' => Token::Star,
                            b'\r' | b'\n' => Token::Word(b"par", None),
                            b'~' => Token::Text(b' '),
                            _ => Token::Text(n),
                        }
                    }
                }
                c => Token::Text(c),
            });
        }
    })
}

#[derive(Clone, Copy, Default)]
struct Group {
    /// A `\*` destination, or a table: not text.
    skip: bool,
    /// Inside `{\*\htmltag …}`: HTML to keep.
    tag: bool,
    /// Between `\htmlrtf` and `\htmlrtf0`: RTF made for show only.
    shown_only: bool,
    /// Inside a hyperlink field's instruction or result.
    field: u8,
}

/// The HTML a message's RTF carries, or, for RTF written as RTF, its text
/// with each hyperlink field as a link. Bytes are Windows-1252, as
/// Outlook's are unless told otherwise.
pub(crate) fn to_html(rtf: &[u8]) -> String {
    let from_html = rtf.windows(10).any(|w| w == b"\\fromhtml1");
    let mut out = String::new();
    let mut stack = vec![Group::default()];
    let mut starred = false;
    let mut skip_next = 0usize;
    let mut fallback = 1usize;
    let mut instruction = String::new();
    for t in tokens(rtf) {
        let g = *stack.last().unwrap_or(&Group::default());
        match t {
            Token::Open => {
                stack.push(g);
                starred = false;
            }
            Token::Close => {
                let closed = stack.pop().unwrap_or_default();
                if closed.field == 2 && stack.last().is_some_and(|p| p.field != 2) {
                    out.push_str("</a>");
                }
                if stack.is_empty() {
                    stack.push(Group::default());
                }
            }
            Token::Star => starred = true,
            Token::Word(w, n) => {
                let Some(top) = stack.last_mut() else {
                    continue;
                };
                match w {
                    b"htmltag" if starred => {
                        top.tag = true;
                        top.skip = false;
                    }
                    b"fldinst" if !from_html => {
                        top.field = 1;
                        instruction.clear();
                    }
                    _ if starred => top.skip = true,
                    b"fonttbl" | b"colortbl" | b"stylesheet" | b"info" | b"pict" => top.skip = true,
                    b"htmlrtf" => top.shown_only = n != Some(0),
                    b"fldrslt" if !from_html => {
                        top.field = 2;
                        let url = instruction
                            .trim()
                            .strip_prefix("HYPERLINK")
                            .map(|u| u.trim().trim_matches('"').to_string());
                        if let Some(url) = url.filter(|u| !u.is_empty()) {
                            out.push_str(&format!("<a href=\"{}\">", url.replace('"', "%22")));
                        } else {
                            top.field = 3;
                        }
                    }
                    b"par" | b"line" => put(&mut out, &mut instruction, from_html, g, '\n'),
                    b"tab" => put(&mut out, &mut instruction, from_html, g, '\t'),
                    b"uc" => fallback = n.unwrap_or(1).max(0) as usize,
                    b"u" => {
                        if let Some(c) = n.and_then(|v| char::from_u32(v as u16 as u32)) {
                            put(&mut out, &mut instruction, from_html, g, c);
                        }
                        skip_next = fallback;
                    }
                    _ => {}
                }
                starred = false;
            }
            Token::Byte(b) | Token::Text(b) => {
                if skip_next > 0 {
                    skip_next -= 1;
                    continue;
                }
                put(&mut out, &mut instruction, from_html, g, windows_1252(b));
            }
        }
    }
    out
}

/// Writes `c` where the group says: nowhere for a destination or RTF made
/// for show, into a hyperlink field's instruction, or out.
fn put(out: &mut String, instruction: &mut String, from_html: bool, g: Group, c: char) {
    if g.skip || (from_html && g.shown_only && !g.tag) {
        return;
    }
    if g.field == 1 {
        instruction.push(c);
    } else {
        out.push(c);
    }
}

/// A Windows-1252 byte as a character: Latin-1, but for the printer's
/// marks at 0x80 to 0x9F.
fn windows_1252(b: u8) -> char {
    const HIGH: [char; 32] = [
        '€', '\u{81}', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\u{8D}', 'Ž',
        '\u{8F}', '\u{90}', '‘', '’', '“', '”', '•', '–', '—', '˜', '™', 'š', '›', 'œ', '\u{9D}',
        'ž', 'Ÿ',
    ];
    match b {
        0x80..=0x9F => HIGH[(b - 0x80) as usize],
        _ => b as char,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decompresses_the_specifications_example() {
        // MS-OXRTFCP 4.1: literals and a reference into the dictionary.
        let compressed = [
            0x2d, 0x00, 0x00, 0x00, 0x2b, 0x00, 0x00, 0x00, 0x4c, 0x5a, 0x46, 0x75, 0xf1, 0xc5,
            0xc7, 0xa7, 0x03, 0x00, 0x0a, 0x00, 0x72, 0x63, 0x70, 0x67, 0x31, 0x32, 0x35, 0x42,
            0x32, 0x0a, 0xf3, 0x20, 0x68, 0x65, 0x6c, 0x09, 0x00, 0x20, 0x62, 0x77, 0x05, 0xb0,
            0x6c, 0x64, 0x7d, 0x0a, 0x80, 0x0f, 0xa0,
        ];
        assert_eq!(
            decompress(&compressed).as_deref(),
            Some(&b"{\\rtf1\\ansi\\ansicpg1252\\pard hello world}\r\n"[..])
        );
        // Stored as it is.
        let mut stored = b"\x0e\0\0\0\x0a\0\0\0MELA\0\0\0\0".to_vec();
        stored.extend_from_slice(b"{\\rtf1 hi}");
        assert_eq!(decompress(&stored).as_deref(), Some(&b"{\\rtf1 hi}"[..]));
        // Cut anywhere, it stops.
        for cut in 0..compressed.len() {
            let _ = decompress(&compressed[..cut]);
        }
    }

    #[test]
    fn a_message_that_came_as_html_gives_its_html_back() {
        let rtf = br#"{\rtf1\ansi\fromhtml1 {\fonttbl{\f0 Arial;}}{\*\htmltag64 <p>}\htmlrtf {\htmlrtf0 Confirm {\*\htmltag84 <a href="https://login.example.info/">}\htmlrtf {\field{\*\fldinst HYPERLINK x}}\htmlrtf0 www.example-bank.com{\*\htmltag92 </a>}\htmlrtf }\htmlrtf0 caf\'e9{\*\htmltag72 </p>}}"#;
        let html = to_html(rtf);
        assert!(
            html.contains(
                r#"<p>Confirm <a href="https://login.example.info/">www.example-bank.com</a>"#
            ),
            "{html}"
        );
        assert!(html.contains("café"), "{html}");
        assert!(!html.contains("Arial"), "{html}");
    }

    #[test]
    fn rtf_written_as_rtf_gives_its_hyperlinks_as_links() {
        let rtf = br#"{\rtf1\ansi Pay here: {\field{\*\fldinst{HYPERLINK "https://login.example.info/pay"}}{\fldrslt{www.example-bank.com}}} today\par}"#;
        let html = to_html(rtf);
        assert_eq!(
            html,
            "Pay here: <a href=\"https://login.example.info/pay\">www.example-bank.com</a> today\n"
        );
    }
}
