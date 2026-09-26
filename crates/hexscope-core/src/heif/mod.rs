//! HEIF: HEIC photos from iPhones, and AVIF. A file is a tree of ISO base
//! media boxes; the picture and its metadata are *items*, listed in `iinf`,
//! located by `iloc`, and stored in `mdat` (or, small ones, in `idat`).
//!
//! Every size and offset comes from the file. Boxes are read inside their
//! container's bounds, nesting is capped, and item data is placed only where
//! it lies within the file.

pub(crate) mod docs;

use crate::bmff::{BoxBody, Fields, be, fourcc, walk};
use crate::exif::{PhotoFacts, parse_tiff_at};
use crate::model::{ByteRange, NodeId, NodeKind, ParseTree, Value};

/// Brands that say "HEIF image", as major or compatible brand.
const BRANDS: [&[u8; 4]; 10] = [
    b"heic", b"heix", b"heim", b"heis", b"hevc", b"hevx", b"mif1", b"msf1", b"avif", b"avis",
];
/// Boxes nested deeper than this are not opened.
const MAX_DEPTH: u32 = 8;
/// Items and locations read before stopping.
const MAX_ITEMS: usize = 10_000;

#[derive(Debug)]
pub struct HeifDocument {
    pub tree: ParseTree,
    /// The major brand, e.g. "heic" or "avif".
    pub brand: String,
    /// From the largest `ispe`: for a gridded picture, the grid's size.
    pub width: Option<u32>,
    pub height: Option<u32>,
    /// From the EXIF item; empty when there is none.
    pub facts: PhotoFacts,
    pub exif: Option<ExifItem>,
    /// The XMP item's bytes.
    pub xmp: Option<ByteRange>,
}

/// Where the EXIF item is, and where its TIFF block starts inside it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExifItem {
    pub data: ByteRange,
    pub tiff_start: u64,
}

/// A HEIF image: `ftyp` at byte 4, with a HEIF brand.
pub fn is_heif(data: &[u8]) -> bool {
    if data.get(4..8) != Some(b"ftyp") {
        return false;
    }
    let size = be(data, 0, 4).unwrap_or(0).min(data.len() as u64) as usize;
    let Some(brands) = data.get(8..size) else {
        return false;
    };
    brands
        .as_chunks::<4>()
        .0
        .iter()
        .enumerate()
        .filter(|&(i, _)| i != 1) // the minor version, not a brand
        .any(|(_, b)| BRANDS.contains(&b))
}

struct Item {
    id: u32,
    typ: String,
    content_type: String,
}

struct Loc {
    id: u32,
    method: u8,
    base: u64,
    extents: Vec<(u64, u64)>,
    node: NodeId,
}

struct Ref {
    typ: String,
    from: u32,
    to: Vec<u32>,
}

#[derive(Default)]
struct Ctx {
    brand: String,
    items: Vec<Item>,
    locs: Vec<Loc>,
    refs: Vec<Ref>,
    sizes: Vec<(u32, u32)>,
    primary: Option<u32>,
    /// Data start and node of `idat`, for construction method 1.
    idat: Option<(u64, NodeId, ByteRange)>,
    /// Data range and node of each `mdat`.
    mdat: Vec<(ByteRange, NodeId)>,
}

/// Parses a HEIF image. Never fails: damage is recorded as nodes.
pub fn parse_heif(data: &[u8]) -> HeifDocument {
    parse_heif_at(data, 0)
}

pub(crate) fn parse_heif_at(data: &[u8], depth: u8) -> HeifDocument {
    let mut tree = ParseTree::new();
    let len = data.len() as u64;
    let root = tree.add(
        None,
        "HEIF",
        ByteRange::new(0, len),
        NodeKind::Container,
        None,
    );
    let mut ctx = Ctx::default();
    walk(&mut tree, root, data, 0, len, 0, &mut ctx);

    let mut doc = HeifDocument {
        tree,
        brand: ctx.brand.clone(),
        width: None,
        height: None,
        facts: PhotoFacts::default(),
        exif: None,
        xmp: None,
    };
    place_items(&mut doc, data, &ctx, root, depth);

    if let Some(&(w, h)) = ctx.sizes.iter().max_by_key(|&&(w, h)| w as u64 * h as u64) {
        doc.width = Some(w);
        doc.height = Some(h);
    }
    let kind = match ctx.brand.as_str() {
        "avif" | "avis" => "AVIF",
        "heic" | "heix" | "heim" | "heis" | "hevc" | "hevx" => "HEIC",
        _ => "HEIF",
    };
    let n = ctx.items.len();
    doc.tree.set_value(
        root,
        Some(Value::Text(format!(
            "{kind} · {n} {}",
            if n == 1 { "item" } else { "items" }
        ))),
    );
    doc
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
        decode(tree, node, data, typ, span, depth, self)
    }
}

/// Decodes one box's body, `span` being `(body start, box end)`. `None` when
/// its fields do not fit in it.
fn decode(
    tree: &mut ParseTree,
    node: NodeId,
    data: &[u8],
    typ: &str,
    span: (u64, u64),
    depth: u32,
    ctx: &mut Ctx,
) -> Option<()> {
    let (body, end) = span;
    let deeper = depth + 1 < MAX_DEPTH;
    let mut f = Fields {
        tree,
        parent: node,
        data,
        at: body,
        end,
    };
    match typ {
        "ftyp" => {
            ctx.brand = f.cc("majorBrand")?;
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
        "meta" => {
            f.full_box()?;
            if deeper {
                walk(tree, node, data, body + 4, end, depth + 1, ctx);
            }
        }
        "dinf" | "iprp" | "ipco" => {
            if deeper {
                walk(tree, node, data, body, end, depth + 1, ctx);
            }
        }
        "dref" => {
            f.full_box()?;
            f.num("entryCount", 4)?;
            if deeper {
                let at = f.at;
                walk(tree, node, data, at, end, depth + 1, ctx);
            }
        }
        "hdlr" => {
            f.full_box()?;
            f.num("preDefined", 4)?;
            f.cc("handlerType")?;
            let start = f.take(12)?;
            f.tree.add(
                Some(node),
                "reserved",
                ByteRange::new(start, 12),
                NodeKind::Field,
                Some(Value::Bytes(12)),
            );
            if f.at < end {
                f.cstr("name")?;
            }
        }
        "pitm" => {
            let v = f.full_box()?;
            ctx.primary = Some(f.num("itemId", if v == 0 { 2 } else { 4 })? as u32);
        }
        "iinf" => {
            let v = f.full_box()?;
            f.num("entryCount", if v == 0 { 2 } else { 4 })?;
            if deeper {
                let at = f.at;
                walk(tree, node, data, at, end, depth + 1, ctx);
            }
        }
        "infe" => {
            let v = f.full_box()?;
            let (id, typ, content_type) = if v >= 2 {
                let id = f.num("itemId", if v == 2 { 2 } else { 4 })? as u32;
                f.num("protectionIndex", 2)?;
                let typ = f.cc("itemType")?;
                f.cstr("itemName")?;
                let ct = if typ == "mime" && f.at < end {
                    f.cstr("contentType")?
                } else {
                    String::new()
                };
                (id, typ, ct)
            } else {
                let id = f.num("itemId", 2)? as u32;
                f.num("protectionIndex", 2)?;
                f.cstr("itemName")?;
                let ct = if f.at < end {
                    f.cstr("contentType")?
                } else {
                    String::new()
                };
                (id, String::new(), ct)
            };
            if ctx.items.len() < MAX_ITEMS {
                ctx.items.push(Item {
                    id,
                    typ,
                    content_type,
                });
            }
        }
        "iref" => {
            let v = f.full_box()?;
            let id_size = if v == 0 { 2 } else { 4 };
            let mut pos = f.at;
            while pos + 8 <= end && ctx.refs.len() < MAX_ITEMS {
                let size = be(data, pos, 4)?;
                let typ = fourcc(data.get(pos as usize + 4..pos as usize + 8)?);
                if size < 8 || size > end - pos {
                    tree.error(
                        node,
                        format!("{typ} reference runs past the end of iref"),
                        ByteRange::new(pos, end - pos),
                    );
                    break;
                }
                let r = tree.add(
                    Some(node),
                    typ.clone(),
                    ByteRange::new(pos, size),
                    NodeKind::Container,
                    None,
                );
                let mut g = Fields {
                    tree,
                    parent: r,
                    data,
                    at: pos + 8,
                    end: pos + size,
                };
                let from = g.num("fromItemId", id_size)? as u32;
                let count = g.num("referenceCount", 2)?;
                let mut to = Vec::new();
                for _ in 0..count {
                    match g.num("toItemId", id_size) {
                        Some(t) => to.push(t as u32),
                        None => break,
                    }
                }
                tree.set_value(
                    r,
                    Some(Value::Text(format!(
                        "item {from} → {}",
                        to.iter()
                            .map(|t| t.to_string())
                            .collect::<Vec<_>>()
                            .join(", ")
                    ))),
                );
                ctx.refs.push(Ref { typ, from, to });
                pos += size;
            }
        }
        "ispe" => {
            f.full_box()?;
            let w = f.num("width", 4)? as u32;
            let h = f.num("height", 4)? as u32;
            ctx.sizes.push((w, h));
        }
        "irot" => {
            let start = f.take(1)?;
            let angle = (data.get(start as usize)? & 3) as u64 * 90;
            f.tree.add(
                Some(node),
                "angle",
                ByteRange::new(start, 1),
                NodeKind::Field,
                Some(Value::Text(format!("{angle}° anticlockwise"))),
            );
        }
        "imir" => {
            f.num("axis", 1)?;
        }
        "pixi" => {
            f.full_box()?;
            let n = f.num("channels", 1)?;
            for _ in 0..n {
                f.num("bitsPerChannel", 1)?;
            }
        }
        "colr" => {
            f.cc("colourType")?;
            f.rest("colourData");
        }
        "ipma" => {
            f.full_box()?;
            f.num("entryCount", 4)?;
            f.rest("associations");
        }
        "iloc" => iloc(&mut f, ctx)?,
        "idat" => {
            ctx.idat = Some((body, node, ByteRange::new(body, end - body)));
        }
        "mdat" => {
            ctx.mdat.push((ByteRange::new(body, end - body), node));
        }
        _ => {}
    }
    Some(())
}

fn iloc(f: &mut Fields, ctx: &mut Ctx) -> Option<()> {
    let v = f.full_box()?;
    let sizes = f.num("offsetSize·lengthSize", 1)? as u8;
    let more = f.num("baseOffsetSize·indexSize", 1)? as u8;
    let (off_size, len_size, base_size) = (sizes >> 4, sizes & 15, more >> 4);
    let index_size = if v == 1 || v == 2 { more & 15 } else { 0 };
    let valid = |n: u8| matches!(n, 0 | 4 | 8);
    if !valid(off_size) || !valid(len_size) || !valid(base_size) || !valid(index_size) {
        return None;
    }
    let count = f.num("itemCount", if v < 2 { 2 } else { 4 })?;
    let outer = f.parent;
    for _ in 0..count.min(MAX_ITEMS as u64) {
        let start = f.at;
        let node = f.tree.add(
            Some(outer),
            "item location",
            ByteRange::new(start, 0),
            NodeKind::Container,
            None,
        );
        f.parent = node;
        let mut id = None;
        let read = (|| {
            id = Some(f.num("itemId", if v < 2 { 2 } else { 4 })? as u32);
            let method = if v == 1 || v == 2 {
                (f.num("constructionMethod", 2)? & 15) as u8
            } else {
                0
            };
            f.num("dataReferenceIndex", 2)?;
            let base = f.var("baseOffset", base_size)?;
            let n = f.num("extentCount", 2)?;
            let mut extents = Vec::new();
            for _ in 0..n.min(MAX_ITEMS as u64) {
                if index_size > 0 {
                    f.var("extentIndex", index_size)?;
                }
                let off = f.var("extentOffset", off_size)?;
                let len = f.var("extentLength", len_size)?;
                extents.push((off, len));
            }
            Some((method, base, extents))
        })();
        f.parent = outer;
        // The node was added before its length was known; it gets its real
        // label and range whether or not every field could be read.
        let label = id.map_or_else(
            || "item location".to_string(),
            |id| format!("item {id} location"),
        );
        f.tree
            .relabel(node, label, ByteRange::new(start, f.at - start));
        let (method, base, extents) = read?;
        ctx.locs.push(Loc {
            id: id?,
            method,
            base,
            extents,
            node,
        });
    }
    Some(())
}

fn place_items(doc: &mut HeifDocument, data: &[u8], ctx: &Ctx, root: NodeId, depth: u8) {
    let len = data.len() as u64;
    let tiles: Vec<u32> = ctx
        .refs
        .iter()
        .filter(|r| r.typ == "dimg")
        .flat_map(|r| r.to.clone())
        .collect();
    let thumbs: Vec<u32> = ctx
        .refs
        .iter()
        .filter(|r| r.typ == "thmb")
        .map(|r| r.from)
        .collect();

    let mut placed: Vec<(u64, u64, u32, &Item, u32)> = Vec::new(); // start, len, id, item, extent number
    for loc in &ctx.locs {
        let Some(item) = ctx.items.iter().find(|i| i.id == loc.id) else {
            continue;
        };
        for (k, &(off, n)) in loc.extents.iter().enumerate() {
            let start = match loc.method {
                0 => loc.base.checked_add(off),
                1 => ctx
                    .idat
                    .and_then(|(s, _, _)| s.checked_add(loc.base)?.checked_add(off)),
                m => {
                    doc.tree.add(
                        Some(loc.node),
                        format!("construction method {m}: data held by other items, not read"),
                        ByteRange::new(0, 0),
                        NodeKind::Field,
                        None,
                    );
                    None
                }
            };
            let Some(start) = start else { continue };
            // A zero length means "to the end", which only a lone extent can say.
            let n = if n == 0 { len.saturating_sub(start) } else { n };
            if start.checked_add(n).is_none_or(|e| e > len) {
                doc.tree.error(
                    loc.node,
                    format!("item {}'s data lies past the end of the file", item.id),
                    doc.tree.get(loc.node).range,
                );
                continue;
            }
            placed.push((start, n, item.id, item, k as u32));
        }
    }
    // By position, ties in the order met.
    let mut order: Vec<(u64, usize)> = placed.iter().enumerate().map(|(i, p)| (p.0, i)).collect();
    order.sort_unstable();
    let placed: Vec<_> = order.into_iter().map(|(_, i)| placed[i]).collect();

    // The boxes item data can live in.
    let mut holders = ctx.mdat.clone();
    holders.extend(ctx.idat.map(|(_, node, r)| (r, node)));
    let mut xmp_node = None;
    for (start, n, id, item, part) in placed {
        let container = holders
            .iter()
            .find(|(r, _)| start >= r.start && start + n <= r.end())
            .map_or(root, |&(_, node)| node);
        let is_xmp = item.typ == "mime" && item.content_type.contains("rdf+xml");
        let what = if item.typ == "Exif" {
            "EXIF metadata".to_string()
        } else if is_xmp {
            "XMP metadata".to_string()
        } else if Some(id) == ctx.primary {
            if item.typ == "grid" {
                format!("the picture: a grid of {} tiles", tiles.len())
            } else {
                "the picture".to_string()
            }
        } else if thumbs.contains(&id) {
            "thumbnail".to_string()
        } else if let Some(k) = tiles.iter().position(|&t| t == id) {
            format!("tile {} of {}", k + 1, tiles.len())
        } else {
            item.typ.clone()
        };
        let label = format!(
            "item {id} · {}",
            if item.typ.is_empty() { "?" } else { &item.typ }
        );
        let label = if part > 0 {
            format!("{label} (part {})", part + 1)
        } else {
            label
        };
        let exif = item.typ == "Exif" && part == 0;
        let node = doc.tree.add(
            Some(container),
            label,
            ByteRange::new(start, n),
            if exif {
                NodeKind::Container
            } else {
                NodeKind::Field
            },
            Some(Value::Text(what)),
        );
        if exif {
            read_exif(doc, data, node, start, n, depth);
        } else if is_xmp && part == 0 {
            doc.xmp = Some(ByteRange::new(start, n));
            xmp_node = Some(node);
        }
    }
    // After EXIF, whichever came first: EXIF's facts win where both say.
    if let (Some(r), Some(node)) = (doc.xmp, xmp_node)
        && let Some(bytes) = data.get(r.start as usize..r.end() as usize)
    {
        let xmp = crate::exif::xmp::from_xmp(&String::from_utf8_lossy(bytes), node);
        doc.facts.fill_from(xmp);
    }
}

/// The EXIF item: a 4-byte offset to the TIFF header, usually `Exif\0\0`,
/// then the TIFF block the photo facts come from.
fn read_exif(doc: &mut HeifDocument, data: &[u8], node: NodeId, start: u64, n: u64, depth: u8) {
    let end = start + n;
    let Some(skip) = be(data, start, 4) else {
        return;
    };
    doc.tree.add(
        Some(node),
        "exifHeaderOffset",
        ByteRange::new(start, 4),
        NodeKind::Field,
        Some(Value::U64(skip)),
    );
    let Some(tiff) = (start + 4).checked_add(skip).filter(|&t| t < end) else {
        doc.tree.error(
            node,
            "the EXIF header offset points past the item",
            ByteRange::new(start, 4),
        );
        return;
    };
    if skip >= 6 && data.get(tiff as usize - 6..tiff as usize) == Some(b"Exif\0\0") {
        doc.tree.add(
            Some(node),
            "identifier",
            ByteRange::new(tiff - 6, 6),
            NodeKind::Field,
            Some(Value::Text("Exif".into())),
        );
    }
    let Some(block) = data.get(tiff as usize..end as usize) else {
        return;
    };
    doc.facts = parse_tiff_at(&mut doc.tree, node, block, tiff, depth + 1);
    doc.exif = Some(ExifItem {
        data: ByteRange::new(start, n),
        tiff_start: tiff,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn fixture(name: &str) -> Vec<u8> {
        std::fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures")
                .join(name),
        )
        .unwrap()
    }

    fn problems(tree: &ParseTree) -> Vec<String> {
        tree.nodes()
            .iter()
            .filter(|n| matches!(n.kind, NodeKind::Warning | NodeKind::Error))
            .map(|n| n.label.clone())
            .collect()
    }

    #[test]
    fn reads_the_three_kinds_of_file_sips_writes() {
        for (name, brand, w, h) in [
            ("photo.heic", "heic", 640, 480),
            ("photo-grid.heic", "heic", 4032, 3024),
            ("photo.avif", "avif", 640, 480),
        ] {
            let bytes = fixture(name);
            assert!(is_heif(&bytes), "{name}");
            let doc = parse_heif(&bytes);
            assert_eq!(problems(&doc.tree), Vec::<String>::new(), "{name}");
            assert_eq!(doc.brand, brand, "{name}");
            assert_eq!((doc.width, doc.height), (Some(w), Some(h)), "{name}");
            assert_eq!(
                doc.facts.camera.as_ref().map(|f| f.text.as_str()),
                Some("hexscope Sample Camera X1"),
                "{name}"
            );
            assert!(doc.exif.is_some() && doc.xmp.is_some(), "{name}");
        }
    }

    #[test]
    fn a_gridded_picture_lists_its_tiles() {
        let doc = parse_heif(&fixture("photo-grid.heic"));
        let tiles = doc
            .tree
            .nodes()
            .iter()
            .filter(|n| matches!(&n.value, Some(Value::Text(t)) if t.starts_with("tile ")))
            .count();
        assert!(tiles >= 12, "{tiles} tiles");
        assert!(doc.tree.nodes().iter().any(
            |n| matches!(&n.value, Some(Value::Text(t)) if t.starts_with("the picture: a grid of"))
        ));
    }

    #[test]
    fn every_truncation_is_survivable() {
        let bytes = fixture("photo.heic");
        for n in 0..=bytes.len() {
            let doc = parse_heif(&bytes[..n]);
            assert!(doc.tree.root().is_some());
        }
    }

    #[test]
    fn other_iso_files_are_not_heif() {
        let mut mp4 = vec![0, 0, 0, 20];
        mp4.extend_from_slice(b"ftypisom\0\0\0\0isomavc1");
        assert!(!is_heif(&mp4));
        assert!(!is_heif(b"not even close"));
    }
}
