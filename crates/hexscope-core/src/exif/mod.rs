//! EXIF: the TIFF structure a camera writes into a JPEG's APP1 segment.
//!
//! Every offset in a TIFF block is relative to the block's start and comes
//! from the file, so every one is untrusted. Reads go through `Reader`, IFD
//! chains remember where they have been, and entry counts are capped by what
//! the block can actually hold.

use crate::fixed::fixed;
use crate::model::{ByteRange, NodeId, NodeKind, ParseTree, Value};
use crate::reader::Reader;
use tags::tag_name;

pub(crate) mod docs;
mod makernote;
mod tags;

/// IFDs followed before giving up: real files have five at most
/// (0, 1, Exif, GPS, Interop).
const MAX_IFDS: usize = 32;
/// Longest string rendered into a node value.
const MAX_TEXT: u64 = 512;
/// Most numbers listed in a value before summarising.
const MAX_LISTED: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ByteOrder {
    Little,
    Big,
}

/// What the metadata says about the photo and whoever took it. Each fact
/// names the node that holds it, so the interface can point at the bytes.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PhotoFacts {
    pub camera: Option<Fact>,
    pub lens: Option<Fact>,
    pub serial: Option<Fact>,
    pub owner: Option<Fact>,
    pub software: Option<Fact>,
    pub taken: Option<Fact>,
    pub location: Option<Location>,
    pub thumbnail: Option<Fact>,
    /// How many photos the camera had taken, from a maker's note.
    pub shutter: Option<Fact>,
    /// How long the phone had been on, from an iPhone's maker note.
    pub uptime: Option<Fact>,
    /// IDs tying the photo to its Live Photo video or its burst.
    pub linked: Option<Fact>,
    /// IFD0's Orientation, 1 to 8: how to turn the picture upright. Not a
    /// fact about anyone, but a clean copy must keep it.
    pub orientation: Option<u16>,
}

impl PhotoFacts {
    /// Takes whatever this set lacks from `other`, keeping what it already
    /// has: some tools write a second EXIF segment of their own.
    pub fn fill_from(&mut self, other: PhotoFacts) {
        self.camera = self.camera.take().or(other.camera);
        self.lens = self.lens.take().or(other.lens);
        self.serial = self.serial.take().or(other.serial);
        self.owner = self.owner.take().or(other.owner);
        self.software = self.software.take().or(other.software);
        self.taken = self.taken.take().or(other.taken);
        self.location = self.location.take().or(other.location);
        self.thumbnail = self.thumbnail.take().or(other.thumbnail);
        self.shutter = self.shutter.take().or(other.shutter);
        self.uptime = self.uptime.take().or(other.uptime);
        self.linked = self.linked.take().or(other.linked);
        self.orientation = self.orientation.take().or(other.orientation);
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Fact {
    pub text: String,
    pub node: NodeId,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Location {
    /// Decimal degrees, negative south of the equator.
    pub latitude: f64,
    /// Decimal degrees, negative west of Greenwich.
    pub longitude: f64,
    /// Metres, negative below sea level.
    pub altitude: Option<f64>,
    /// The GPS IFD.
    pub node: NodeId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Ifd {
    Zero,
    One,
    Exif,
    Gps,
    Interop,
    /// A maker's own IFD, inside its note.
    Maker,
}

impl Ifd {
    fn label(self) -> &'static str {
        match self {
            Ifd::Zero => "IFD0",
            Ifd::One => "IFD1 (thumbnail)",
            Ifd::Exif => "Exif IFD",
            Ifd::Gps => "GPS IFD",
            Ifd::Interop => "Interop IFD",
            Ifd::Maker => "maker's IFD",
        }
    }
}

/// One 12-byte IFD entry, with where its value lives in the TIFF block.
#[derive(Debug, Clone, Copy)]
struct Entry {
    tag: u16,
    typ: u16,
    count: u32,
    /// Offset of the value within the TIFF block: inline in the entry itself
    /// when it fits in four bytes, elsewhere otherwise.
    value_off: u64,
    size: u64,
    inline: bool,
}

struct Tiff<'a> {
    data: &'a [u8],
    base: u64,
    order: ByteOrder,
}

impl<'a> Tiff<'a> {
    fn len(&self) -> u64 {
        self.data.len() as u64
    }

    fn fits(&self, off: u64, len: u64) -> bool {
        off.checked_add(len).is_some_and(|end| end <= self.len())
    }

    fn at(&self, off: u64) -> Reader<'a> {
        let mut r = Reader::new(self.data);
        r.seek(off);
        r
    }

    fn range(&self, off: u64, len: u64) -> ByteRange {
        ByteRange::new(self.base + off, len)
    }

    fn u16(&self, r: &mut Reader) -> Option<u16> {
        match self.order {
            ByteOrder::Little => r.u16_le().ok(),
            ByteOrder::Big => r.u16_be().ok(),
        }
    }

    fn u32(&self, r: &mut Reader) -> Option<u32> {
        match self.order {
            ByteOrder::Little => r.u32_le().ok(),
            ByteOrder::Big => r.u32_be().ok(),
        }
    }

    fn entry(&self, off: u64) -> Option<Entry> {
        let mut r = self.at(off);
        let tag = self.u16(&mut r)?;
        let typ = self.u16(&mut r)?;
        let count = self.u32(&mut r)?;
        let size = type_size(typ).map_or(0, |s| s * count as u64);
        let inline = size <= 4;
        let value_off = if inline {
            off + 8
        } else {
            self.u32(&mut r)? as u64
        };
        Some(Entry {
            tag,
            typ,
            count,
            value_off,
            size,
            inline,
        })
    }

    /// The value's bytes, if they lie inside the block.
    fn value(&self, e: &Entry) -> Option<Reader<'a>> {
        self.fits(e.value_off, e.size).then(|| self.at(e.value_off))
    }

    fn text(&self, e: &Entry) -> Option<String> {
        let mut r = self.value(e)?;
        let raw = r.bytes(e.size.min(MAX_TEXT) as usize).ok()?;
        let end = raw.iter().position(|&b| b == 0).unwrap_or(raw.len());
        let mut rest = Reader::new(raw);
        let bytes = rest.bytes(end).ok()?;
        Some(String::from_utf8_lossy(bytes).trim().to_string())
    }

    /// Up to `max` values as numbers; rationals divided out. Empty for types
    /// that are not numeric or values outside the block.
    fn numbers(&self, e: &Entry, max: usize) -> Vec<f64> {
        let Some(mut r) = self.value(e) else {
            return Vec::new();
        };
        let n = (e.count as usize).min(max);
        let mut out = Vec::with_capacity(n);
        for _ in 0..n {
            let v = match e.typ {
                1 | 7 => r.u8().ok().map(|v| v as f64),
                6 => r.u8().ok().map(|v| v as i8 as f64),
                3 => self.u16(&mut r).map(|v| v as f64),
                8 => self.u16(&mut r).map(|v| v as i16 as f64),
                4 | 13 => self.u32(&mut r).map(|v| v as f64),
                9 => self.u32(&mut r).map(|v| v as i32 as f64),
                5 => self.rational(&mut r, false),
                10 => self.rational(&mut r, true),
                11 => self.u32(&mut r).map(|v| f32::from_bits(v) as f64),
                12 => {
                    let (a, b) = (self.u32(&mut r), self.u32(&mut r));
                    a.zip(b).map(|(a, b)| {
                        let bits = match self.order {
                            ByteOrder::Big => ((a as u64) << 32) | b as u64,
                            ByteOrder::Little => ((b as u64) << 32) | a as u64,
                        };
                        f64::from_bits(bits)
                    })
                }
                _ => None,
            };
            match v {
                Some(v) => out.push(v),
                None => break,
            }
        }
        out
    }

    fn rational(&self, r: &mut Reader, signed: bool) -> Option<f64> {
        let n = self.u32(r)?;
        let d = self.u32(r)?;
        let (n, d) = if signed {
            (n as i32 as f64, d as i32 as f64)
        } else {
            (n as f64, d as f64)
        };
        Some(if d == 0.0 { f64::NAN } else { n / d })
    }

    /// Rationals as their numerator and denominator, for display.
    fn fractions(&self, e: &Entry, max: usize) -> Vec<(i64, i64)> {
        let Some(mut r) = self.value(e) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for _ in 0..(e.count as usize).min(max) {
            let (Some(n), Some(d)) = (self.u32(&mut r), self.u32(&mut r)) else {
                break;
            };
            out.push(if e.typ == 10 {
                (n as i32 as i64, d as i32 as i64)
            } else {
                (n as i64, d as i64)
            });
        }
        out
    }

    fn raw(&self, e: &Entry, max: u64) -> Vec<u8> {
        self.value(e)
            .and_then(|mut r| r.bytes(e.size.min(max) as usize).ok().map(<[u8]>::to_vec))
            .unwrap_or_default()
    }
}

fn type_size(typ: u16) -> Option<u64> {
    match typ {
        1 | 2 | 6 | 7 => Some(1),
        3 | 8 => Some(2),
        // 13 is TIFF's IFD type: a four-byte offset, used by some writers
        // for sub-IFD pointers.
        4 | 9 | 11 | 13 => Some(4),
        5 | 10 | 12 => Some(8),
        _ => None,
    }
}

/// Formats a number without trailing noise: `35`, `2.8`, `0.0125`.
fn num(v: f64) -> String {
    if !v.is_finite() {
        return "undefined".to_string();
    }
    let s = fixed(v, 6);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" {
        "0".to_string()
    } else {
        s.to_string()
    }
}

/// Degrees, minutes, seconds as decimal degrees.
fn dms(v: &[f64]) -> Option<f64> {
    let d = *v.first()?;
    let m = v.get(1).copied().unwrap_or(0.0);
    let s = v.get(2).copied().unwrap_or(0.0);
    let total = d + m / 60.0 + s / 3600.0;
    total.is_finite().then_some(total)
}

fn render_dms(v: &[f64]) -> String {
    match v {
        [d, m, s, ..] => format!("{}° {}′ {}″", num(*d), num(*m), num(*s)),
        _ => v.iter().map(|x| num(*x)).collect::<Vec<_>>().join(", "),
    }
}

/// A text entry, kept until every IFD is read and the facts can be assembled.
struct TextEntry {
    ifd: Ifd,
    tag: u16,
    text: String,
    /// The bytes that spell the text out.
    node: NodeId,
}

/// Everything collected while walking, turned into [`PhotoFacts`] at the end.
#[derive(Default)]
struct Found {
    text: Vec<TextEntry>,
    lat: Option<(Vec<f64>, NodeId)>,
    lon: Option<Vec<f64>>,
    lat_ref: Option<String>,
    lon_ref: Option<String>,
    alt: Option<f64>,
    alt_below: bool,
    gps_node: Option<NodeId>,
    thumb_off: Option<u64>,
    thumb_len: Option<u64>,
    thumbnail: Option<Fact>,
    orientation: Option<u16>,
    /// From a maker's note.
    maker_serial: Vec<Fact>,
    maker_owner: Option<Fact>,
    shutter: Option<Fact>,
    uptime: Option<Fact>,
    linked: Vec<(&'static str, Fact)>,
}

impl Found {
    fn find_text(&self, hit: impl Fn(&TextEntry) -> bool) -> Option<(&str, NodeId)> {
        self.text
            .iter()
            .find(|e| !e.text.is_empty() && hit(e))
            .map(|e| (e.text.as_str(), e.node))
    }

    fn text_of(&self, ifd: Ifd, tag: u16) -> Option<(&str, NodeId)> {
        self.find_text(|e| e.tag == tag && e.ifd == ifd)
    }

    fn any_text(&self, tag: u16) -> Option<(&str, NodeId)> {
        self.find_text(|e| e.tag == tag)
    }

    fn facts(self) -> PhotoFacts {
        let fact = |v: Option<(&str, NodeId)>| {
            v.map(|(t, n)| Fact {
                text: t.to_string(),
                node: n,
            })
        };

        let camera = match (
            self.text_of(Ifd::Zero, 0x010F),
            self.text_of(Ifd::Zero, 0x0110),
        ) {
            (Some((make, _)), Some((model, node))) => Some(Fact {
                // Many cameras repeat the make inside the model.
                text: if model
                    .to_ascii_lowercase()
                    .starts_with(&make.to_ascii_lowercase())
                {
                    model.to_string()
                } else {
                    format!("{make} {model}")
                },
                node,
            }),
            (None, model) => fact(model),
            (make, None) => fact(make),
        };

        let lens = fact(self.any_text(0xA434));
        let serial = match (self.any_text(0xA431), self.any_text(0xA435)) {
            (Some((body, node)), Some((lens, _))) => Some(Fact {
                text: format!("body {body}, lens {lens}"),
                node,
            }),
            (Some((body, node)), None) => Some(Fact {
                text: body.to_string(),
                node,
            }),
            (None, Some((lens, node))) => Some(Fact {
                text: format!("lens {lens}"),
                node,
            }),
            (None, None) => None,
        };
        // A maker's note repeats, or adds, serial numbers; EXIF's come first.
        let serial = match (serial, self.maker_serial.first()) {
            (Some(s), _) => Some(s),
            (None, Some(first)) => {
                let mut all: Vec<&str> = Vec::new();
                for f in &self.maker_serial {
                    if !all.contains(&f.text.as_str()) {
                        all.push(&f.text);
                    }
                }
                Some(Fact {
                    text: all.join(", "),
                    node: first.node,
                })
            }
            (None, None) => None,
        };
        let owner =
            fact(self.any_text(0xA430).or(self.any_text(0x013B))).or(self.maker_owner.clone());
        // A Live Photo's ID first; each ID shortened, whole in its node.
        let live = |x: &&(&str, Fact)| x.0 == "Live Photo";
        let ids: Vec<&(&str, Fact)> = self
            .linked
            .iter()
            .filter(live)
            .chain(self.linked.iter().filter(|x| !live(x)))
            .collect();
        let linked = ids.first().map(|(_, first)| Fact {
            text: ids
                .iter()
                .map(|(what, f)| {
                    let short: String = f.text.chars().take(8).collect();
                    let more = if f.text.chars().count() > 8 {
                        "…"
                    } else {
                        ""
                    };
                    format!("{what} {short}{more}")
                })
                .collect::<Vec<_>>()
                .join(", "),
            node: first.node,
        });
        let software = fact(self.any_text(0x0131));
        let taken = self
            .any_text(0x9003)
            .or(self.any_text(0x0132))
            .map(|(t, node)| {
                // EXIF writes dates as "2026:06:14 18:32:07".
                let mut text = t.replacen(':', "-", 2);
                if let Some((offset, _)) = self.any_text(0x9011) {
                    text = format!("{text} {offset}");
                }
                Fact { text, node }
            });

        let location = match (&self.lat, &self.lon, self.gps_node) {
            (Some((lat, _)), Some(lon), Some(node)) => {
                let south = self.lat_ref.as_deref() == Some("S");
                let west = self.lon_ref.as_deref() == Some("W");
                dms(lat).zip(dms(lon)).map(|(la, lo)| Location {
                    latitude: if south { -la } else { la },
                    longitude: if west { -lo } else { lo },
                    altitude: self
                        .alt
                        .filter(|a| a.is_finite())
                        .map(|a| if self.alt_below { -a } else { a }),
                    node,
                })
            }
            _ => None,
        };

        PhotoFacts {
            camera,
            lens,
            serial,
            owner,
            software,
            taken,
            location,
            thumbnail: self.thumbnail,
            shutter: self.shutter,
            uptime: self.uptime,
            linked,
            orientation: self.orientation,
        }
    }
}

/// Human rendering of an entry's value.
fn render(t: &Tiff, ifd: Ifd, e: &Entry) -> String {
    let gps = ifd == Ifd::Gps;
    match e.typ {
        2 => t.text(e).unwrap_or_default(),
        1 | 6 | 7 => {
            let raw = t.raw(e, 64);
            let is_version = matches!(
                (ifd, e.tag),
                (Ifd::Exif, 0x9000 | 0xA000) | (Ifd::Interop, 0x0002)
            );
            if is_version && raw.len() == 4 && raw.iter().all(u8::is_ascii_digit) {
                // "0232" means version 2.32.
                let s: String = raw.iter().map(|&b| b as char).collect();
                return format!("{}.{}", s[..2].trim_start_matches('0'), &s[2..]);
            }
            if gps && e.tag == 0x00 && raw.len() == 4 {
                return raw.iter().map(u8::to_string).collect::<Vec<_>>().join(".");
            }
            if gps && e.tag == 0x05 {
                return match raw.first() {
                    Some(0) => "above sea level".into(),
                    Some(1) => "below sea level".into(),
                    _ => "unknown".into(),
                };
            }
            if e.count as usize <= MAX_LISTED && !raw.is_empty() {
                raw.iter()
                    .map(|b| format!("{b:02X}"))
                    .collect::<Vec<_>>()
                    .join(" ")
            } else {
                format!("{} bytes", e.size)
            }
        }
        5 | 10 => {
            let values = t.numbers(e, MAX_LISTED);
            if gps && matches!(e.tag, 0x02 | 0x04 | 0x14 | 0x16) {
                return render_dms(&values);
            }
            match (ifd, e.tag, values.as_slice(), t.fractions(e, 1).first()) {
                (_, 0x829A, [v], Some(&(n, d))) if *v > 0.0 && *v < 1.0 => {
                    if n == 1 {
                        format!("1/{d} s")
                    } else {
                        format!("1/{} s", num(1.0 / v))
                    }
                }
                (_, 0x829A, [v], _) => format!("{} s", num(*v)),
                (_, 0x829D, [v], _) => format!("f/{}", num(*v)),
                (_, 0x920A, [v], _) => format!("{} mm", num(*v)),
                (Ifd::Gps, 0x06, [v], _) => format!("{} m", num(*v)),
                (_, _, [v], Some(&(n, d))) if d != 0 && v.fract() != 0.0 => {
                    format!("{n}/{d} ({})", num(*v))
                }
                _ => list(&values, e.count),
            }
        }
        3 | 4 | 8 | 9 | 11 | 12 | 13 => {
            let values = t.numbers(e, MAX_LISTED);
            let named = match (e.tag, values.first().map(|v| *v as i64)) {
                (0x0112, Some(v)) => Some(match v {
                    1 => "normal",
                    3 => "rotated 180°",
                    6 => "rotated 90° clockwise",
                    8 => "rotated 90° counter-clockwise",
                    2 | 4 | 5 | 7 => "mirrored",
                    _ => "unknown",
                }),
                (0x0128 | 0xA210, Some(v)) => Some(match v {
                    1 => "no unit",
                    2 => "inches",
                    3 => "centimetres",
                    _ => "unknown",
                }),
                (0xA001, Some(1)) => Some("sRGB"),
                (0xA001, Some(0xFFFF)) => Some("uncalibrated"),
                (0x9209, Some(v)) => Some(if v & 1 == 1 { "fired" } else { "did not fire" }),
                _ => None,
            };
            match (named, values.first()) {
                (Some(name), Some(v)) => format!("{} ({name})", num(*v)),
                _ => list(&values, e.count),
            }
        }
        _ => format!("unknown type {}", e.typ),
    }
}

fn list(values: &[f64], count: u32) -> String {
    let shown: Vec<String> = values.iter().map(|v| num(*v)).collect();
    if count as usize > values.len() {
        format!("{}, … ({count} values)", shown.join(", "))
    } else {
        shown.join(", ")
    }
}

/// Parses a TIFF block — the part of APP1 after `Exif\0\0` — that starts at
/// file offset `base`, adding nodes under `parent`. Never panics: a damaged
/// block yields as much as can be read plus error nodes on the damage.
pub fn parse_tiff(tree: &mut ParseTree, parent: NodeId, data: &[u8], base: u64) -> PhotoFacts {
    parse_tiff_at(tree, parent, data, base, 0)
}

/// `depth` counts how many files deep this one is embedded, so a thumbnail
/// carrying its own thumbnail cannot recurse without end.
pub(crate) fn parse_tiff_at(
    tree: &mut ParseTree,
    parent: NodeId,
    data: &[u8],
    base: u64,
    depth: u8,
) -> PhotoFacts {
    let mut r = Reader::new(data);
    let order = match r.array::<2>() {
        Ok(ref b) if b == b"II" => ByteOrder::Little,
        Ok(ref b) if b == b"MM" => ByteOrder::Big,
        _ => {
            tree.error(
                parent,
                "not a TIFF header: byte order must be II or MM",
                ByteRange::new(base, data.len().min(2) as u64),
            );
            return PhotoFacts::default();
        }
    };
    let t = Tiff { data, base, order };

    let header = tree.add(
        Some(parent),
        "TIFF header",
        t.range(0, t.len().min(8)),
        NodeKind::Container,
        None,
    );
    tree.add(
        Some(header),
        "byteOrder",
        t.range(0, 2),
        NodeKind::Field,
        Some(Value::Enum {
            raw: if order == ByteOrder::Little {
                0x4949
            } else {
                0x4D4D
            },
            name: if order == ByteOrder::Little {
                "little-endian"
            } else {
                "big-endian"
            },
        }),
    );
    let Some(magic) = t.u16(&mut r) else {
        tree.error(header, "TIFF header truncated", t.range(2, t.len() - 2));
        return PhotoFacts::default();
    };
    tree.add(
        Some(header),
        "magic",
        t.range(2, 2),
        NodeKind::Field,
        Some(Value::U64(magic as u64)),
    );
    if magic != 42 {
        tree.warning(
            header,
            format!("TIFF magic is {magic}, expected 42"),
            t.range(2, 2),
        );
    }
    let Some(ifd0) = t.u32(&mut r) else {
        tree.error(header, "TIFF header truncated", t.range(4, t.len() - 4));
        return PhotoFacts::default();
    };
    tree.add(
        Some(header),
        "ifd0Offset",
        t.range(4, 4),
        NodeKind::Field,
        Some(Value::U64(ifd0 as u64)),
    );

    let first = Pending {
        off: ifd0 as u64,
        kind: Ifd::Zero,
        pointer: t.range(4, 4),
    };
    let mut walk = Walk {
        tree,
        parent,
        t,
        depth,
        queue: vec![first],
        found: Found::default(),
    };
    walk.run();
    walk.found.facts()
}

/// An IFD that some pointer leads to, waiting its turn.
#[derive(Clone, Copy)]
struct Pending {
    off: u64,
    kind: Ifd,
    /// The bytes of the pointer, blamed if the IFD cannot be read.
    pointer: ByteRange,
}

/// One pass over the IFDs of a TIFF block, following pointers breadth-first.
struct Walk<'w, 'a> {
    tree: &'w mut ParseTree,
    parent: NodeId,
    t: Tiff<'a>,
    depth: u8,
    queue: Vec<Pending>,
    found: Found,
}

impl Walk<'_, '_> {
    fn run(&mut self) {
        let mut visited: Vec<u64> = Vec::new();
        let mut i = 0;
        while let Some(&next) = self.queue.get(i) {
            i += 1;
            if i > MAX_IFDS {
                let label = format!("stopped after {MAX_IFDS} IFDs");
                self.tree.warning(self.parent, label, next.pointer);
                break;
            }
            if visited.contains(&next.off) {
                // A chain that leads back to an IFD already read would
                // otherwise be followed forever. Nothing is lost — that IFD
                // was read — so this is a warning, not an error.
                let label = format!("{} points back to an IFD already read", next.kind.label());
                self.tree.warning(self.parent, label, next.pointer);
                continue;
            }
            visited.push(next.off);
            self.ifd(next);
        }
    }

    fn ifd(&mut self, at: Pending) {
        let Pending { off, kind, pointer } = at;
        let (tree, t, parent, depth) = (&mut *self.tree, &self.t, self.parent, self.depth);
        let (queue, found) = (&mut self.queue, &mut self.found);
        let mut r = t.at(off);
        let Some(declared) = t.u16(&mut r).filter(|_| t.fits(off, 2)) else {
            tree.error(
                parent,
                format!(
                    "{} offset {off} is past the end of the EXIF data",
                    kind.label()
                ),
                pointer,
            );
            return;
        };

        // The count comes from the file; trust only as many entries as fit.
        let room = (t.len().saturating_sub(off + 2)) / 12;
        let count = (declared as u64).min(room);
        let has_next = t.fits(off + 2 + 12 * count, 4);
        let node = tree.add(
            Some(parent),
            kind.label(),
            t.range(off, 2 + 12 * count + if has_next { 4 } else { 0 }),
            NodeKind::Container,
            Some(Value::Text(format!(
                "{count} {}",
                if count == 1 { "entry" } else { "entries" }
            ))),
        );
        if kind == Ifd::Gps {
            found.gps_node = Some(node);
        }
        tree.add(
            Some(node),
            "entryCount",
            t.range(off, 2),
            NodeKind::Field,
            Some(Value::U64(declared as u64)),
        );
        if count < declared as u64 {
            tree.error(
                node,
                format!("claims {declared} entries, only {count} fit in the EXIF data"),
                t.range(off, 2),
            );
        }

        for n in 0..count {
            let eoff = off + 2 + 12 * n;
            let Some(e) = t.entry(eoff) else { break };
            let name = tag_name(kind, e.tag);
            let label = name.map_or_else(|| format!("tag 0x{:04X}", e.tag), str::to_string);
            let rendered = render(t, kind, &e);
            let entry = tree.add(
                Some(node),
                label.clone(),
                t.range(eoff, 12),
                NodeKind::Field,
                Some(Value::Text(rendered.clone())),
            );

            let mut value_node = None;
            if type_size(e.typ).is_none() {
                tree.warning(
                    entry,
                    format!("unknown value type {}", e.typ),
                    t.range(eoff + 2, 2),
                );
            } else if !e.inline {
                if t.fits(e.value_off, e.size) {
                    // A sibling of the IFDs rather than a child of the entry: the
                    // value's bytes lie outside the entry, and nodes must nest
                    // inside their parents for byte lookup to reach them.
                    value_node = Some(tree.add(
                        Some(parent),
                        format!("{label} value"),
                        t.range(e.value_off, e.size),
                        NodeKind::Field,
                        Some(Value::Text(rendered.clone())),
                    ));
                } else {
                    tree.error(
                        entry,
                        format!(
                            "value at offset {} runs past the end of the EXIF data",
                            e.value_off
                        ),
                        t.range(eoff + 8, 4),
                    );
                }
            }

            // A maker's note, in the maker's own format.
            if (kind, e.tag) == (Ifd::Exif, 0x927C)
                && let Some(note) = value_node
            {
                let make = found
                    .text_of(Ifd::Zero, 0x010F)
                    .map(|(m, _)| m.to_string())
                    .unwrap_or_default();
                makernote::parse(tree, note, t, e.value_off, e.size, &make, found);
            }

            let value_field = t.range(eoff + 8, 4);
            let target = || t.numbers(&e, 1).first().map(|v| *v as u64);
            match (kind, e.tag) {
                (Ifd::Zero, 0x8769) => {
                    if let Some(o) = target() {
                        queue.push(Pending {
                            off: o,
                            kind: Ifd::Exif,
                            pointer: value_field,
                        });
                    }
                }
                (Ifd::Zero, 0x8825) => {
                    if let Some(o) = target() {
                        queue.push(Pending {
                            off: o,
                            kind: Ifd::Gps,
                            pointer: value_field,
                        });
                    }
                }
                (Ifd::Exif, 0xA005) => {
                    if let Some(o) = target() {
                        queue.push(Pending {
                            off: o,
                            kind: Ifd::Interop,
                            pointer: value_field,
                        });
                    }
                }
                (Ifd::One, 0x0201) => found.thumb_off = target(),
                (Ifd::One, 0x0202) => found.thumb_len = target(),
                (Ifd::Gps, 0x01) => found.lat_ref = t.text(&e),
                (Ifd::Gps, 0x02) => found.lat = Some((t.numbers(&e, 3), entry)),
                (Ifd::Gps, 0x03) => found.lon_ref = t.text(&e),
                (Ifd::Gps, 0x04) => found.lon = Some(t.numbers(&e, 3)),
                (Ifd::Gps, 0x05) => found.alt_below = t.raw(&e, 1).first() == Some(&1),
                (Ifd::Gps, 0x06) => found.alt = t.numbers(&e, 1).first().copied(),
                (Ifd::Zero, 0x0112) => {
                    found.orientation = t.numbers(&e, 1).first().map(|&v| v as u16);
                }
                _ => {}
            }
            if e.typ == 2 {
                // Point a fact at the bytes that spell it out, not at the entry
                // that merely says where they are.
                found.text.push(TextEntry {
                    ifd: kind,
                    tag: e.tag,
                    text: rendered,
                    node: value_node.unwrap_or(entry),
                });
            }
        }

        if has_next {
            let noff = off + 2 + 12 * count;
            let mut nr = t.at(noff);
            let next = t.u32(&mut nr).unwrap_or(0);
            tree.add(
                Some(node),
                "nextIFDOffset",
                t.range(noff, 4),
                NodeKind::Field,
                Some(Value::U64(next as u64)),
            );
            // EXIF uses exactly one link: IFD0 to the thumbnail's IFD1.
            if kind == Ifd::Zero && next != 0 {
                queue.push(Pending {
                    off: next as u64,
                    kind: Ifd::One,
                    pointer: t.range(noff, 4),
                });
            }
        }

        if kind == Ifd::One
            && let (Some(o), Some(len)) = (found.thumb_off, found.thumb_len)
        {
            if t.fits(o, len) && len > 0 {
                let thumb = tree.add(
                    Some(parent),
                    "thumbnail",
                    t.range(o, len),
                    NodeKind::Container,
                    Some(Value::Bytes(len)),
                );
                // A thumbnail is a JPEG in its own right, and sometimes keeps what
                // an edit removed from the main image: parse it, one level deep.
                let mut r = t.at(o);
                if depth == 0
                    && let Ok(bytes) = r.bytes(len as usize)
                    && bytes.starts_with(&crate::jpeg::MAGIC)
                {
                    let inner = crate::jpeg::parse_jpeg_at(bytes, depth + 1);
                    tree.graft(thumb, &inner.tree, t.base + o);
                }
                found.thumbnail = Some(Fact {
                    text: format!("embedded JPEG, {len} bytes"),
                    node: thumb,
                });
            } else {
                tree.error(
                    node,
                    "thumbnail runs past the end of the EXIF data",
                    t.range(off, 2),
                );
            }
        }
    }
}

#[cfg(test)]
pub(crate) mod testing;

/// The parser's tag table, for the test that checks every tag is explained.
#[cfg(test)]
pub(crate) fn tag_name_for_tests(ifd: Ifd, tag: u16) -> Option<&'static str> {
    tags::tag_name(ifd, tag)
}

#[cfg(test)]
mod tests {
    use super::testing::{Spec, V, build};
    use super::*;

    fn parse(data: &[u8]) -> (ParseTree, PhotoFacts) {
        let mut tree = ParseTree::new();
        let root = tree.add(
            None,
            "APP1",
            ByteRange::new(0, data.len() as u64),
            NodeKind::Container,
            None,
        );
        // A non-zero base checks that ranges come out in file coordinates.
        let facts = parse_tiff(&mut tree, root, data, 1000);
        (tree, facts)
    }

    fn camera(order: ByteOrder) -> Spec {
        let mut s = Spec::new(order);
        s.ifd0 = vec![
            (0x010F, V::Ascii("Canon")),
            (0x0110, V::Ascii("Canon EOS R5")),
            (0x0112, V::Short(vec![6])),
            (0x0131, V::Ascii("Firmware 1.8.1")),
        ];
        s.exif = vec![
            (0x829A, V::Rational(vec![(1, 125)])),
            (0x829D, V::Rational(vec![(28, 10)])),
            (0x9003, V::Ascii("2026:06:14 18:32:07")),
            (0x9011, V::Ascii("+02:00")),
            (0x9000, V::Undefined(b"0232".to_vec())),
            (0xA431, V::Ascii("032021001234")),
            (0xA434, V::Ascii("RF24-105mm F4 L IS USM")),
        ];
        s
    }

    fn with_gps(mut s: Spec, north: bool, east: bool) -> Spec {
        // The Eiffel Tower: 48° 51′ 30.24″ N, 2° 17′ 40.2″ E, 35 m.
        s.gps = vec![
            (0x01, V::Ascii(if north { "N" } else { "S" })),
            (0x02, V::Rational(vec![(48, 1), (51, 1), (3024, 100)])),
            (0x03, V::Ascii(if east { "E" } else { "W" })),
            (0x04, V::Rational(vec![(2, 1), (17, 1), (402, 10)])),
            (0x05, V::Byte(vec![0])),
            (0x06, V::Rational(vec![(35, 1)])),
        ];
        s
    }

    fn node_named<'t>(tree: &'t ParseTree, label: &str) -> &'t crate::model::Node {
        tree.nodes()
            .iter()
            .find(|n| n.label == label)
            .unwrap_or_else(|| panic!("no node {label}"))
    }

    #[test]
    fn reads_camera_facts_in_both_byte_orders() {
        for order in [ByteOrder::Little, ByteOrder::Big] {
            let (tree, facts) = parse(&build(camera(order)));
            assert_eq!(
                facts.camera.as_ref().unwrap().text,
                "Canon EOS R5",
                "{order:?}"
            );
            assert_eq!(facts.serial.as_ref().unwrap().text, "032021001234");
            assert_eq!(facts.lens.as_ref().unwrap().text, "RF24-105mm F4 L IS USM");
            assert_eq!(facts.software.as_ref().unwrap().text, "Firmware 1.8.1");
            assert_eq!(
                facts.taken.as_ref().unwrap().text,
                "2026-06-14 18:32:07 +02:00"
            );
            assert!(facts.location.is_none());

            let value = |label| node_named(&tree, label).value.clone();
            assert_eq!(value("ExposureTime"), Some(Value::Text("1/125 s".into())));
            assert_eq!(value("FNumber"), Some(Value::Text("f/2.8".into())));
            assert_eq!(
                value("Orientation"),
                Some(Value::Text("6 (rotated 90° clockwise)".into()))
            );
            assert_eq!(value("ExifVersion"), Some(Value::Text("2.32".into())));
            // The fact links to the bytes that spell it out.
            assert_eq!(tree.get(facts.camera.unwrap().node).label, "Model value");
        }
    }

    #[test]
    fn decodes_gps_to_decimal_degrees() {
        let (tree, facts) = parse(&build(with_gps(Spec::new(ByteOrder::Big), true, true)));
        let loc = facts.location.expect("location");
        assert!((loc.latitude - 48.8584).abs() < 1e-4, "{}", loc.latitude);
        assert!((loc.longitude - 2.2945).abs() < 1e-4, "{}", loc.longitude);
        assert_eq!(loc.altitude, Some(35.0));
        assert_eq!(tree.get(loc.node).label, "GPS IFD");
        assert_eq!(
            node_named(&tree, "GPSLatitude").value,
            Some(Value::Text("48° 51′ 30.24″".into()))
        );

        let (_, facts) = parse(&build(with_gps(Spec::new(ByteOrder::Little), false, false)));
        let loc = facts.location.unwrap();
        assert!(
            loc.latitude < 0.0 && loc.longitude < 0.0,
            "south and west are negative"
        );
    }

    #[test]
    fn values_stored_elsewhere_sit_on_their_own_bytes() {
        let data = build(camera(ByteOrder::Little));
        let (tree, _) = parse(&data);
        let value = node_named(&tree, "Model value");
        let start = (value.range.start - 1000) as usize;
        let end = (value.range.end() - 1000) as usize;
        assert_eq!(&data[start..end], b"Canon EOS R5\0");
        // The entry itself covers exactly its 12 bytes.
        assert_eq!(node_named(&tree, "Model").range.len, 12);
    }

    #[test]
    fn an_ifd_that_links_to_itself_is_reported_and_ends() {
        let mut s = camera(ByteOrder::Little);
        s.ifd0_next = 8; // IFD0's own offset
        let (tree, _) = parse(&build(s));
        assert!(
            tree.nodes()
                .iter()
                .any(|n| n.kind == NodeKind::Warning && n.label.contains("points back"))
        );
    }

    #[test]
    fn offsets_past_the_end_are_errors_not_panics() {
        let mut s = camera(ByteOrder::Big);
        s.ifd0_next = 0xFFFF_FFF0;
        let mut data = build(s);
        let (tree, _) = parse(&data);
        assert!(
            tree.nodes()
                .iter()
                .any(|n| n.kind == NodeKind::Error && n.label.contains("past the end"))
        );

        // Cut the block so out-of-line values no longer fit.
        data.truncate(data.len() - 20);
        let (tree, _) = parse(&data);
        assert!(
            tree.nodes()
                .iter()
                .any(|n| n.kind == NodeKind::Error && n.label.contains("runs past the end"))
        );
    }

    #[test]
    fn a_huge_entry_count_is_capped_by_the_data() {
        let mut data = build(camera(ByteOrder::Little));
        data[8] = 0xFF;
        data[9] = 0xFF; // IFD0 claims 65,535 entries
        let (tree, _) = parse(&data);
        assert!(
            tree.nodes()
                .iter()
                .any(|n| n.label.starts_with("claims 65535 entries"))
        );
    }

    #[test]
    fn a_bad_byte_order_is_an_error() {
        let (tree, facts) = parse(b"XX\x00\x2a\x00\x00\x00\x08");
        assert_eq!(facts, PhotoFacts::default());
        assert!(tree.nodes().iter().any(|n| n.kind == NodeKind::Error));
    }

    #[test]
    fn every_truncation_of_a_real_block_is_survivable() {
        let data = build(with_gps(camera(ByteOrder::Big), true, true));
        for len in 0..=data.len() {
            let (tree, _) = parse(&data[..len]);
            assert!(!tree.is_empty());
        }
    }

    #[test]
    fn never_panics_on_scrambled_blocks() {
        let base = build(with_gps(camera(ByteOrder::Little), true, true));
        for seed in 0..2000u32 {
            let mut data = base.clone();
            let mut x = seed.wrapping_mul(2654435761).wrapping_add(1);
            for _ in 0..6 {
                x ^= x << 13;
                x ^= x >> 17;
                x ^= x << 5;
                let i = (x as usize) % data.len();
                data[i] = (x >> 8) as u8;
            }
            let _ = parse(&data);
        }
    }
}
