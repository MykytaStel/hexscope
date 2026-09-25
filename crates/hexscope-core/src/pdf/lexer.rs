//! PDF's object syntax (ISO 32000-1, 7.2–7.3): whitespace, comments, and the
//! eight kinds of value. Every read moves forward or fails, and nesting is
//! capped, so no input can make it loop or recurse without bound.

use crate::model::ByteRange;

/// Arrays and dictionaries nested deeper than this are not read.
const MAX_DEPTH: u32 = 32;

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Obj {
    Null,
    Bool(bool),
    Int(i64),
    Real(String),
    Str(Vec<u8>),
    Name(String),
    Array(Vec<Item>),
    Dict(Vec<Entry>),
    Ref(u32, u16),
}

/// A value and the bytes that spell it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Item {
    pub obj: Obj,
    pub range: ByteRange,
}

/// One key of a dictionary. `range` runs from the key to the end of its value.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Entry {
    pub key: String,
    pub value: Item,
    pub range: ByteRange,
}

impl Obj {
    pub fn entries(&self) -> &[Entry] {
        match self {
            Obj::Dict(e) => e,
            _ => &[],
        }
    }

    /// The value of `key`, when this is a dictionary that has it.
    pub fn get(&self, key: &str) -> Option<&Obj> {
        self.entries()
            .iter()
            .rev()
            .find(|e| e.key == key)
            .map(|e| &e.value.obj)
    }

    pub fn name(&self) -> Option<&str> {
        match self {
            Obj::Name(n) => Some(n),
            _ => None,
        }
    }

    pub fn int(&self) -> Option<i64> {
        match self {
            Obj::Int(i) => Some(*i),
            _ => None,
        }
    }
}

pub(crate) fn is_ws(b: u8) -> bool {
    matches!(b, 0 | b'\t' | b'\n' | 0x0C | b'\r' | b' ')
}

fn is_delim(b: u8) -> bool {
    matches!(
        b,
        b'(' | b')' | b'<' | b'>' | b'[' | b']' | b'{' | b'}' | b'/' | b'%'
    )
}

fn is_regular(b: u8) -> bool {
    !is_ws(b) && !is_delim(b)
}

pub(crate) struct Lexer<'a> {
    pub data: &'a [u8],
    pub pos: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(data: &'a [u8], pos: usize) -> Self {
        Self { data, pos }
    }

    fn peek(&self) -> Option<u8> {
        self.data.get(self.pos).copied()
    }

    pub fn at(&self, s: &[u8]) -> bool {
        self.data.get(self.pos..).is_some_and(|d| d.starts_with(s))
    }

    /// Skips whitespace and comments, stopping at `%%EOF`, which marks the
    /// end of a revision rather than being a comment to ignore.
    pub fn skip_ws(&mut self) {
        while let Some(b) = self.peek() {
            if is_ws(b) {
                self.pos += 1;
            } else if b == b'%' && !self.at(b"%%EOF") {
                while let Some(b) = self.peek() {
                    if b == b'\n' || b == b'\r' {
                        break;
                    }
                    self.pos += 1;
                }
            } else {
                break;
            }
        }
    }

    /// A run of regular characters: a keyword or a number.
    pub fn word(&mut self) -> &'a [u8] {
        let start = self.pos;
        while self.peek().is_some_and(is_regular) {
            self.pos += 1;
        }
        &self.data[start..self.pos]
    }

    /// The keyword at the cursor, consumed only when it is `kw`.
    pub fn keyword(&mut self, kw: &[u8]) -> bool {
        let save = self.pos;
        if self.word() == kw {
            true
        } else {
            self.pos = save;
            false
        }
    }

    /// A non-negative integer, or nothing (and the cursor unmoved).
    pub fn uint(&mut self) -> Option<u64> {
        let save = self.pos;
        let w = self.word();
        let n = (!w.is_empty() && w.len() <= 19 && w.iter().all(u8::is_ascii_digit))
            .then(|| std::str::from_utf8(w).ok()?.parse().ok())
            .flatten();
        if n.is_none() {
            self.pos = save;
        }
        n
    }

    /// `N G obj`, returning the object number and generation.
    pub fn object_header(&mut self) -> Option<(u32, u16)> {
        let save = self.pos;
        let header = (|| {
            let num = u32::try_from(self.uint()?).ok()?;
            self.skip_ws();
            let gen_ = u16::try_from(self.uint()?).ok()?;
            self.skip_ws();
            self.keyword(b"obj").then_some((num, gen_))
        })();
        if header.is_none() {
            self.pos = save;
        }
        header
    }

    /// One value, with whitespace before it skipped. `None` leaves the
    /// cursor where reading stopped.
    pub fn value(&mut self) -> Option<Item> {
        self.value_at(0)
    }

    fn value_at(&mut self, depth: u32) -> Option<Item> {
        if depth > MAX_DEPTH {
            return None;
        }
        self.skip_ws();
        let start = self.pos;
        let obj = match self.peek()? {
            b'/' => Obj::Name(self.name()?),
            b'(' => Obj::Str(self.literal()?),
            b'<' if self.at(b"<<") => {
                self.pos += 2;
                let mut entries = Vec::new();
                loop {
                    self.skip_ws();
                    if self.at(b">>") {
                        self.pos += 2;
                        break;
                    }
                    let key_start = self.pos;
                    if self.peek()? != b'/' {
                        return None;
                    }
                    let key = self.name()?;
                    let value = self.value_at(depth + 1)?;
                    let range = span(key_start, value.range.end() as usize);
                    entries.push(Entry { key, value, range });
                }
                Obj::Dict(entries)
            }
            b'<' => Obj::Str(self.hex()?),
            b'[' => {
                self.pos += 1;
                let mut items = Vec::new();
                loop {
                    self.skip_ws();
                    if self.peek()? == b']' {
                        self.pos += 1;
                        break;
                    }
                    items.push(self.value_at(depth + 1)?);
                }
                Obj::Array(items)
            }
            b'+' | b'-' | b'.' | b'0'..=b'9' => self.number()?,
            _ => match self.word() {
                b"true" => Obj::Bool(true),
                b"false" => Obj::Bool(false),
                b"null" => Obj::Null,
                _ => {
                    self.pos = start;
                    return None;
                }
            },
        };
        Some(Item {
            obj,
            range: span(start, self.pos),
        })
    }

    /// A number, or `N G R` when one follows.
    fn number(&mut self) -> Option<Obj> {
        let w = self.word();
        let text = std::str::from_utf8(w).ok()?;
        if let Ok(i) = text.parse::<i64>() {
            let after = self.pos;
            if let (Ok(num), true) = (u32::try_from(i), !text.starts_with(['+', '-'])) {
                self.skip_ws();
                if let Some(gen_) = self.uint().and_then(|g| u16::try_from(g).ok()) {
                    self.skip_ws();
                    if self.keyword(b"R") {
                        return Some(Obj::Ref(num, gen_));
                    }
                }
            }
            self.pos = after;
            return Some(Obj::Int(i));
        }
        let valid = !text.is_empty()
            && text.bytes().filter(|&b| b == b'.').count() <= 1
            && text
                .trim_start_matches(['+', '-'])
                .bytes()
                .all(|b| b.is_ascii_digit() || b == b'.')
            && text.bytes().any(|b| b.is_ascii_digit());
        valid.then(|| Obj::Real(text.to_string()))
    }

    /// `/Name`, with `#xx` escapes decoded.
    fn name(&mut self) -> Option<String> {
        self.pos += 1;
        let raw = self.word();
        let mut out = Vec::with_capacity(raw.len());
        let mut i = 0;
        while i < raw.len() {
            let b = raw[i];
            let hex = (b == b'#')
                .then(|| raw.get(i + 1..i + 3))
                .flatten()
                .and_then(|h| u8::from_str_radix(std::str::from_utf8(h).ok()?, 16).ok());
            match hex {
                Some(v) => {
                    out.push(v);
                    i += 3;
                }
                None => {
                    out.push(b);
                    i += 1;
                }
            }
        }
        Some(String::from_utf8_lossy(&out).into_owned())
    }

    /// `(literal)`: balanced parentheses, backslash escapes.
    fn literal(&mut self) -> Option<Vec<u8>> {
        self.pos += 1;
        let mut out = Vec::new();
        let mut depth = 1u32;
        loop {
            let b = self.peek()?;
            self.pos += 1;
            match b {
                b'(' => {
                    depth += 1;
                    out.push(b);
                }
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(out);
                    }
                    out.push(b);
                }
                b'\\' => {
                    let e = self.peek()?;
                    self.pos += 1;
                    match e {
                        b'n' => out.push(b'\n'),
                        b'r' => out.push(b'\r'),
                        b't' => out.push(b'\t'),
                        b'b' => out.push(0x08),
                        b'f' => out.push(0x0C),
                        b'0'..=b'7' => {
                            let mut v = u32::from(e - b'0');
                            for _ in 0..2 {
                                match self.peek() {
                                    Some(d @ b'0'..=b'7') => {
                                        v = v * 8 + u32::from(d - b'0');
                                        self.pos += 1;
                                    }
                                    _ => break,
                                }
                            }
                            out.push(v as u8);
                        }
                        // A backslash before a line break continues the line.
                        b'\r' => {
                            if self.peek() == Some(b'\n') {
                                self.pos += 1;
                            }
                        }
                        b'\n' => {}
                        other => out.push(other),
                    }
                }
                other => out.push(other),
            }
        }
    }

    /// `<hex>`: whitespace ignored, an odd final digit taken as followed by 0.
    fn hex(&mut self) -> Option<Vec<u8>> {
        self.pos += 1;
        let mut out = Vec::new();
        let mut high: Option<u8> = None;
        loop {
            let b = self.peek()?;
            self.pos += 1;
            let v = match b {
                b'>' => break,
                b'0'..=b'9' => b - b'0',
                b'a'..=b'f' => b - b'a' + 10,
                b'A'..=b'F' => b - b'A' + 10,
                b if is_ws(b) => continue,
                _ => return None,
            };
            match high.take() {
                Some(h) => out.push(h << 4 | v),
                None => high = Some(v),
            }
        }
        if let Some(h) = high {
            out.push(h << 4);
        }
        Some(out)
    }
}

fn span(start: usize, end: usize) -> ByteRange {
    ByteRange::new(start as u64, end.saturating_sub(start) as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(s: &[u8]) -> Option<Obj> {
        Lexer::new(s, 0).value().map(|i| i.obj)
    }

    #[test]
    fn reads_every_kind_of_value() {
        assert_eq!(read(b" 42"), Some(Obj::Int(42)));
        assert_eq!(read(b"-3.5"), Some(Obj::Real("-3.5".into())));
        assert_eq!(read(b"12 0 R"), Some(Obj::Ref(12, 0)));
        assert_eq!(read(b"12 0 obj"), Some(Obj::Int(12)));
        assert_eq!(read(b"/A#20B"), Some(Obj::Name("A B".into())));
        assert_eq!(
            read(b"(a (b) \\(c\\) \\101\\\nd)"),
            Some(Obj::Str(b"a (b) (c) Ad".to_vec()))
        );
        assert_eq!(read(b"<48 65 6c6>"), Some(Obj::Str(b"Hel`".to_vec())));
        assert_eq!(read(b"true"), Some(Obj::Bool(true)));
        assert_eq!(read(b"null"), Some(Obj::Null));
        let dict = read(b"<< /Type /Page /Kids [1 0 R 2 0 R] % note\n /N 3 >>").unwrap();
        assert_eq!(dict.get("Type").and_then(Obj::name), Some("Page"));
        assert_eq!(dict.get("N").and_then(Obj::int), Some(3));
        assert!(matches!(dict.get("Kids"), Some(Obj::Array(a)) if a.len() == 2));
    }

    #[test]
    fn broken_values_fail_without_looping() {
        for bad in [
            &b"<< /A"[..],
            b"(open",
            b"[1 2",
            b"<zz>",
            b"<< 1 2 >>",
            b"endobj",
            b"-",
            b".",
        ] {
            assert_eq!(read(bad), None, "{}", String::from_utf8_lossy(bad));
        }
        let deep = "[".repeat(100);
        assert_eq!(read(deep.as_bytes()), None);
    }

    #[test]
    fn an_object_header_is_three_tokens() {
        let mut l = Lexer::new(b"7 0 obj <<>>", 0);
        assert_eq!(l.object_header(), Some((7, 0)));
        let mut l = Lexer::new(b"7 0 R", 0);
        assert_eq!(l.object_header(), None);
        assert_eq!(l.pos, 0);
    }
}
