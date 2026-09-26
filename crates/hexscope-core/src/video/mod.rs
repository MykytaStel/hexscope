//! MP4 and QuickTime movies: the same tree of boxes as HEIF, with the
//! picture and sound in `mdat` and everything about them in `moov`.
//!
//! What a phone says about a video lives in three places, depending on who
//! wrote it: an iPhone's QuickTime `mdta` metadata (a `keys` box naming
//! each item, an `ilst` box holding them), the `©xyz` text an Android phone
//! puts in the user data, and 3GPP's `loci` box. All three can hold where
//! it was recorded. Each is read, and each is noted for the clean copy.

pub(crate) mod docs;

use crate::bmff::{BoxBody, Fields, be, fourcc, walk};
use crate::exif::{Fact, Location, PhotoFacts};
use crate::fixed::fixed;
use crate::model::{ByteRange, NodeId, NodeKind, ParseTree, Value};

/// Boxes nested deeper than this are not opened.
const MAX_DEPTH: u32 = 12;
/// Longest text kept from a metadata item.
const MAX_TEXT: usize = 256;
/// Box types a QuickTime file without `ftyp` may start with.
const QUICKTIME_FIRST: [&[u8; 4]; 6] = [b"moov", b"mdat", b"wide", b"free", b"skip", b"pnot"];

/// One change the clean copy makes, found while reading.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scrub {
    /// The box at `at` becomes a `free` box of the same size, its body
    /// zeroed after its `header` bytes.
    Free {
        at: u64,
        len: u64,
        header: u64,
        what: &'static str,
    },
    /// These bytes become zero.
    Zero {
        range: ByteRange,
        what: &'static str,
    },
}

#[derive(Debug)]
pub struct VideoDocument {
    pub tree: ParseTree,
    /// "MP4" or "QuickTime".
    pub kind: &'static str,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub duration: Option<f64>,
    pub facts: PhotoFacts,
    pub scrub: Vec<Scrub>,
}

/// An MP4 or QuickTime movie: `ftyp` first, or one of QuickTime's first
/// boxes with a size that fits. HEIF, which also has `ftyp`, is told apart
/// before this is asked.
pub fn is_video(data: &[u8]) -> bool {
    if data.get(4..8) == Some(b"ftyp") {
        return true;
    }
    let (Some(size), Some(typ)) = (be(data, 0, 4), data.get(4..8)) else {
        return false;
    };
    QUICKTIME_FIRST.iter().any(|t| t.as_slice() == typ)
        && (size == 1 || (8..=data.len() as u64).contains(&size))
}

#[derive(Default)]
struct Ctx {
    brand: String,
    /// The `mdta` keys, by their 1-based index.
    keys: Vec<String>,
    make: Option<Fact>,
    model: Option<Fact>,
    facts: PhotoFacts,
    scrub: Vec<Scrub>,
    width: u32,
    height: u32,
    duration: Option<f64>,
    /// `mvhd`'s creation time, for when nothing else says when.
    created: Option<Fact>,
}

pub fn parse_video(data: &[u8]) -> VideoDocument {
    let mut tree = ParseTree::new();
    let len = data.len() as u64;
    let root = tree.add(
        None,
        "movie",
        ByteRange::new(0, len),
        NodeKind::Container,
        None,
    );
    let mut ctx = Ctx::default();
    walk(&mut tree, root, data, 0, len, 0, &mut ctx);

    let mut facts = ctx.facts;
    facts.camera = match (ctx.make, ctx.model) {
        (Some(make), Some(model)) if !model.text.starts_with(&make.text) => Some(Fact {
            text: format!("{} {}", make.text, model.text),
            node: model.node,
        }),
        (make, model) => model.or(make),
    };
    if facts.taken.is_none() {
        facts.taken = ctx.created;
    }
    let kind = if ctx.brand.is_empty() || ctx.brand.starts_with("qt") {
        "QuickTime"
    } else {
        "MP4"
    };
    let (width, height) = (ctx.width > 0).then_some((ctx.width, ctx.height)).unzip();
    let mut summary = kind.to_string();
    if let (Some(w), Some(h)) = (width, height) {
        summary.push_str(&format!(" · {w}×{h}"));
    }
    if let Some(d) = ctx.duration {
        summary.push_str(&format!(" · {} s", fixed(d, 1)));
    }
    tree.set_value(root, Some(Value::Text(summary)));
    VideoDocument {
        tree,
        kind,
        width,
        height,
        duration: ctx.duration,
        facts,
        scrub: ctx.scrub,
    }
}

/// Seconds since 1904-01-01 UTC, the movie epoch, as a date.
fn movie_time(secs: u64) -> String {
    let days = (secs / 86_400) as i64 - 24_107;
    let rem = secs % 86_400;
    // Howard Hinnant's days-to-civil.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02} UTC",
        rem / 3600,
        rem / 60 % 60,
        rem % 60
    )
}

/// `2026-06-14T18:32:07+0200` as `2026-06-14 18:32:07 +02:00`; anything
/// not shaped like that stays as written.
fn date(raw: &str) -> String {
    let b = raw.as_bytes();
    let shaped = b.len() >= 19 && b[4] == b'-' && b[10] == b'T' && b[13] == b':' && b[16] == b':';
    let (Some(day), Some(time), Some(zone)) = (raw.get(..10), raw.get(11..19), raw.get(19..))
    else {
        return raw.to_string();
    };
    if !shaped {
        return raw.to_string();
    }
    let zone = match zone {
        "Z" => " UTC".to_string(),
        z if z.len() == 5 && z.starts_with(['+', '-']) => format!(" {}:{}", &z[..3], &z[3..]),
        z if z.len() == 6 && z.starts_with(['+', '-']) => format!(" {z}"),
        _ => String::new(),
    };
    format!("{day} {time}{zone}")
}

/// An ISO 6709 point, as phones write it: `+48.8584+002.2945+035.000/`.
fn iso6709(text: &str) -> Option<(f64, f64, Option<f64>)> {
    let s = text.trim().trim_end_matches('/');
    let mut parts = Vec::new();
    let mut start = 0;
    for (i, c) in s.char_indices().skip(1) {
        if c == '+' || c == '-' {
            parts.push(&s[start..i]);
            start = i;
        }
        if c == '/' || c.is_ascii_alphabetic() {
            break;
        }
    }
    parts.push(&s[start..]);
    let num = |p: &str| decimal(p);
    let lat = num(parts.first()?)?;
    let lon = num(parts.get(1)?)?;
    ((-90.0..=90.0).contains(&lat) && (-180.0..=180.0).contains(&lon))
        .then(|| (lat, lon, parts.get(2).and_then(|p| num(p))))
}

/// `+048.8584` as 48.8584: a sign, digits, and at most one point. Written
/// out rather than `str::parse`, whose float parser is a sizeable part of a
/// WebAssembly build for the one place it would be used.
fn decimal(s: &str) -> Option<f64> {
    let (sign, digits) = match s.as_bytes().first()? {
        b'-' => (-1.0, &s[1..]),
        b'+' => (1.0, &s[1..]),
        _ => (1.0, s),
    };
    let (whole, frac) = digits.split_once('.').unwrap_or((digits, ""));
    if whole.is_empty() || whole.len() > 9 || frac.len() > 9 {
        return None;
    }
    let int = |t: &str| -> Option<u64> {
        t.bytes().try_fold(0u64, |v, b| {
            b.is_ascii_digit().then(|| v * 10 + u64::from(b - b'0'))
        })
    };
    let value = int(whole)? as f64 + int(frac)? as f64 / 10f64.powi(frac.len() as i32);
    Some(sign * value)
}

fn text(bytes: &[u8]) -> String {
    let s = String::from_utf8_lossy(bytes);
    let s = s.trim_matches(char::from(0)).trim();
    s.chars().take(MAX_TEXT).collect()
}

impl Ctx {
    /// Keeps a metadata text as a fact, and marks its box for the copy.
    fn note(&mut self, key: &str, value: &str, node: NodeId) {
        let fact = || {
            Some(Fact {
                text: value.to_string(),
                node,
            })
        };
        if value.is_empty() {
            return;
        }
        let key = key.to_ascii_lowercase();
        if key.ends_with("location.iso6709") || key == "©xyz" {
            if let Some((latitude, longitude, altitude)) = iso6709(value) {
                self.facts.location.get_or_insert(Location {
                    latitude,
                    longitude,
                    altitude,
                    node,
                });
            }
        } else if key.ends_with(".make") || key == "©mak" {
            self.make = self.make.take().or_else(fact);
        } else if key.ends_with(".model") || key == "©mod" {
            self.model = self.model.take().or_else(fact);
        } else if key.ends_with(".software") || key == "©swr" || key == "©too" {
            self.facts.software = self.facts.software.take().or_else(fact);
        } else if key.ends_with(".creationdate") || key == "©day" {
            let when = Fact {
                text: date(value),
                node,
            };
            self.facts.taken.get_or_insert(when);
        } else if key.ends_with(".author") || key == "©aut" || key == "©art" {
            self.facts.owner = self.facts.owner.take().or_else(fact);
        }
    }

    fn free(&mut self, tree: &ParseTree, node: NodeId, key: &str) {
        let n = tree.get(node);
        let header = if n.children.iter().any(|&c| tree.get(c).label == "largeSize") {
            16
        } else {
            8
        };
        let key = key.to_ascii_lowercase();
        let what = if key.contains("location") || key == "©xyz" || key == "loci" {
            "the location"
        } else if key.ends_with(".make")
            || key.ends_with(".model")
            || key == "©mak"
            || key == "©mod"
        {
            "the camera's make and model"
        } else if key.ends_with(".software") || key == "©swr" || key == "©too" {
            "the software"
        } else if key.ends_with(".creationdate") || key == "©day" {
            "when it was recorded"
        } else if key == "xmp" {
            "XMP: editing history and author"
        } else {
            "other metadata"
        };
        self.scrub.push(Scrub::Free {
            at: n.range.start,
            len: n.range.len,
            header,
            what,
        });
    }

    /// Creation and modification times, shown as dates and zeroed in the copy.
    fn times(&mut self, f: &mut Fields, version: u8, is_movie: bool) -> Option<()> {
        let n = if version == 1 { 8 } else { 4 };
        for label in ["creationTime", "modificationTime"] {
            let at = f.at;
            let secs = f.num(label, n)?;
            let node = f.tree.get(f.parent).children.last().copied();
            if secs > 0
                && let Some(node) = node
            {
                let when = movie_time(secs);
                f.tree.set_value(node, Some(Value::Text(when.clone())));
                self.scrub.push(Scrub::Zero {
                    range: ByteRange::new(at, n as u64),
                    what: "the times it was made and changed",
                });
                if is_movie && label == "creationTime" && self.created.is_none() {
                    self.created = Some(Fact { text: when, node });
                }
            }
        }
        Some(())
    }
}

impl BoxBody for Ctx {
    fn decode(
        &mut self,
        tree: &mut ParseTree,
        node: NodeId,
        data: &[u8],
        typ: &str,
        span: (u64, u64),
        depth: u32,
    ) -> Option<()> {
        let (body, end) = span;
        let deeper = depth + 1 < MAX_DEPTH;
        let parent_label = tree
            .get(node)
            .parent
            .map(|p| tree.get(p).label.clone())
            .unwrap_or_default();
        let mut f = Fields {
            tree,
            parent: node,
            data,
            at: body,
            end,
        };
        match typ {
            "ftyp" => {
                self.brand = f.cc("majorBrand")?;
                f.num("minorVersion", 4)?;
                let brands: Vec<String> = data
                    .get(f.at as usize..end as usize)?
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|c| fourcc(c))
                    .collect();
                let start = f.at;
                f.tree.add(
                    Some(node),
                    "compatibleBrands",
                    ByteRange::new(start, end - start),
                    NodeKind::Field,
                    Some(Value::Text(brands.join(", "))),
                );
            }
            "moov" | "trak" | "mdia" | "minf" | "stbl" | "dinf" | "edts" | "udta" | "mvex"
            | "moof" | "traf" | "tref" | "tapt" | "sinf" | "schi"
                if deeper =>
            {
                walk(f.tree, node, data, body, end, depth + 1, self);
            }
            "meta" if deeper => {
                // QuickTime's meta is a plain container; ISO's is a full box.
                let plain = data.get(body as usize + 4..body as usize + 8) == Some(b"hdlr");
                if !plain {
                    f.full_box()?;
                }
                walk(f.tree, node, data, f.at, end, depth + 1, self);
            }
            "hdlr" => {
                f.full_box()?;
                f.num("preDefined", 4)?;
                f.cc("handlerType")?;
                if f.at + 12 <= end {
                    f.num("reserved", 12)?;
                }
                f.cstr("name");
            }
            "keys" => {
                f.full_box()?;
                let count = f.num("entryCount", 4)?;
                for i in 1..=count {
                    let at = f.at;
                    let size = be(data, at, 4)?;
                    if size < 8 || at + size > end {
                        return None;
                    }
                    let key = text(data.get(at as usize + 8..(at + size) as usize)?);
                    let ns = fourcc(data.get(at as usize + 4..at as usize + 8)?);
                    f.tree.add(
                        Some(node),
                        format!("key {i}"),
                        ByteRange::new(at, size),
                        NodeKind::Field,
                        Some(Value::Text(format!("{ns} · {key}"))),
                    );
                    self.keys.push(key);
                    f.at = at + size;
                }
            }
            "ilst" => {
                // Each item is named by a key's index (mdta) or a four-letter
                // code (iTunes-style), and holds a `data` box.
                let mut at = body;
                while at + 8 <= end {
                    let size = be(data, at, 4)?;
                    if size < 8 || at + size > end {
                        return None;
                    }
                    let raw = data.get(at as usize + 4..at as usize + 8)?;
                    // Space a writer, or a clean copy, left free.
                    if raw == b"free" || raw == b"skip" {
                        let spare = f.tree.add(
                            Some(node),
                            fourcc(raw),
                            ByteRange::new(at, size),
                            NodeKind::Container,
                            Some(Value::Bytes(size)),
                        );
                        let mut g = Fields {
                            tree: f.tree,
                            parent: spare,
                            data,
                            at,
                            end: at + size,
                        };
                        g.num("size", 4)?;
                        g.cc("type")?;
                        g.rest("body");
                        at += size;
                        continue;
                    }
                    let index = be(data, at + 4, 4)?;
                    let key = if raw[0] == 0 {
                        self.keys
                            .get((index as usize).wrapping_sub(1))
                            .cloned()
                            .unwrap_or_else(|| format!("key {index}"))
                    } else {
                        fourcc(raw)
                    };
                    let item = f.tree.add(
                        Some(node),
                        key.clone(),
                        ByteRange::new(at, size),
                        NodeKind::Container,
                        None,
                    );
                    let mut g = Fields {
                        tree: f.tree,
                        parent: item,
                        data,
                        at,
                        end: at + size,
                    };
                    g.num("size", 4)?;
                    g.cc("type")?;
                    let mut value = String::new();
                    while g.at + 16 <= at + size {
                        let d = g.at;
                        let dsize = be(data, d, 4)?;
                        if dsize < 16 || d + dsize > at + size {
                            return None;
                        }
                        let dnode = g.tree.add(
                            Some(item),
                            fourcc(data.get(d as usize + 4..d as usize + 8)?),
                            ByteRange::new(d, dsize),
                            NodeKind::Container,
                            None,
                        );
                        let mut h = Fields {
                            tree: g.tree,
                            parent: dnode,
                            data,
                            at: d,
                            end: d + dsize,
                        };
                        h.num("size", 4)?;
                        h.cc("type")?;
                        let kind = h.num("typeIndicator", 4)?;
                        h.num("locale", 4)?;
                        let bytes = data.get(h.at as usize..(d + dsize) as usize)?;
                        let shown = if kind & 0xFF_FFFF == 1 {
                            value = text(bytes);
                            Value::Text(value.clone())
                        } else {
                            Value::Bytes(bytes.len() as u64)
                        };
                        h.tree.add(
                            Some(dnode),
                            "value",
                            ByteRange::new(h.at, d + dsize - h.at),
                            NodeKind::Field,
                            Some(shown),
                        );
                        g.at = d + dsize;
                        g.tree.set_value(item, Some(Value::Text(value.clone())));
                    }
                    self.note(&key, &value, item);
                    self.free(g.tree, item, &key);
                    at += size;
                }
            }
            t if t.starts_with('©') && parent_label == "udta" => {
                // QuickTime user data text: a length, a language, the text.
                let n = f.num("textLength", 2)?;
                f.num("language", 2)?;
                let start = f.at;
                let bytes = data.get(start as usize..(start + n).min(end) as usize)?;
                let value = text(bytes);
                f.tree.add(
                    Some(node),
                    "text",
                    ByteRange::new(start, bytes.len() as u64),
                    NodeKind::Field,
                    Some(Value::Text(value.clone())),
                );
                f.at = start + bytes.len() as u64;
                f.rest("body");
                f.tree.set_value(node, Some(Value::Text(value.clone())));
                self.note(t, &value, node);
                self.free(f.tree, node, t);
            }
            "loci" => {
                // 3GPP TS 26.244: a name, a role, then 16.16 fixed-point
                // longitude, latitude and altitude.
                f.full_box()?;
                f.num("language", 2)?;
                f.cstr("name")?;
                f.num("role", 1)?;
                let fixed = |v: u64| (v as u32 as i32) as f64 / 65_536.0;
                let lon = fixed(f.num("longitude", 4)?);
                let lat = fixed(f.num("latitude", 4)?);
                let alt = fixed(f.num("altitude", 4)?);
                f.cstr("astronomicalBody");
                f.cstr("additionalNotes");
                f.rest("body");
                f.tree.set_value(
                    node,
                    Some(Value::Text(format!(
                        "{}, {}",
                        crate::fixed::fixed(lat, 5),
                        crate::fixed::fixed(lon, 5)
                    ))),
                );
                if (-90.0..=90.0).contains(&lat) && (-180.0..=180.0).contains(&lon) {
                    self.facts.location.get_or_insert(Location {
                        latitude: lat,
                        longitude: lon,
                        altitude: Some(alt),
                        node,
                    });
                }
                self.free(f.tree, node, "loci");
            }
            "uuid" => {
                let id = data.get(body as usize..body as usize + 16)?;
                f.num("userType", 16)?;
                f.rest("body");
                // Adobe's XMP packet: BE7ACFCB-97A9-42E8-9C71-999491E3AFAC.
                if id == b"\xBE\x7A\xCF\xCB\x97\xA9\x42\xE8\x9C\x71\x99\x94\x91\xE3\xAF\xAC" {
                    f.tree
                        .set_value(node, Some(Value::Text("XMP metadata".into())));
                    self.free(f.tree, node, "xmp");
                }
            }
            "mvhd" => {
                let v = f.full_box()?;
                self.times(&mut f, v, true)?;
                let scale = f.num("timescale", 4)?;
                let duration = f.num("duration", if v == 1 { 8 } else { 4 })?;
                if scale > 0 {
                    self.duration = Some(duration as f64 / scale as f64);
                }
                f.rest("body");
            }
            "tkhd" => {
                let v = f.full_box()?;
                self.times(&mut f, v, false)?;
                f.num("trackId", 4)?;
                f.num("reserved", 4)?;
                f.num("duration", if v == 1 { 8 } else { 4 })?;
                f.num("layout", 16)?;
                f.num("matrix", 36)?;
                let w = f.num("width", 4)? >> 16;
                let h = f.num("height", 4)? >> 16;
                if w * h > self.width as u64 * self.height as u64 {
                    (self.width, self.height) = (w as u32, h as u32);
                }
            }
            "mdhd" => {
                let v = f.full_box()?;
                self.times(&mut f, v, false)?;
                f.num("timescale", 4)?;
                f.num("duration", if v == 1 { 8 } else { 4 })?;
                f.num("language", 2)?;
                f.num("quality", 2)?;
            }
            _ => f.rest("body"),
        }
        Some(())
    }
}

#[cfg(test)]
mod tests;
