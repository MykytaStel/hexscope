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
    data_reference_index: u16,
    base: u64,
    extents: Vec<Extent>,
    node: NodeId,
}

struct Extent {
    index: u64,
    offset: u64,
    length: u64,
}

struct Ref {
    typ: String,
    from: u32,
    to: Vec<u32>,
}

#[derive(Default)]
struct SequenceTrack {
    node: NodeId,
    handler: String,
    timescale: Option<u64>,
    sample_count: u64,
    sample_ticks: u64,
}

#[derive(Default)]
struct Ctx {
    brand: String,
    brands: Vec<String>,
    items: Vec<Item>,
    locs: Vec<Loc>,
    refs: Vec<Ref>,
    sizes: Vec<(u32, u32)>,
    primary: Option<u32>,
    /// Data start and node of `idat`, for construction method 1.
    idat: Option<(u64, NodeId, ByteRange)>,
    /// Data range and node of each `mdat`.
    mdat: Vec<(ByteRange, NodeId)>,
    sequence_tracks: Vec<SequenceTrack>,
    current_track: Option<usize>,
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

    if ctx.brands.iter().any(|brand| brand == "avis") {
        for track in &ctx.sequence_tracks {
            let timescale = track.timescale.unwrap_or(0);
            let value = if track.handler == "pict" && timescale > 0 && track.sample_count > 0 {
                format!(
                    "AV1 image sequence · {} · {} samples · {} ticks at {} ticks/s",
                    track.handler, track.sample_count, track.sample_ticks, timescale,
                )
            } else if track.handler == "pict" {
                "AV1 image sequence · pict · timing not fully read".to_string()
            } else {
                format!(
                    "AVIS track · {}",
                    if track.handler.is_empty() {
                        "unknown handler"
                    } else {
                        &track.handler
                    }
                )
            };
            tree.set_value(track.node, Some(Value::Text(value)));
        }
    }

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
            ctx.brands.push(ctx.brand.clone());
            f.num("minorVersion", 4)?;
            let brands: Vec<String> = data
                .get(f.at as usize..end as usize)?
                .as_chunks::<4>()
                .0
                .iter()
                .map(|c| fourcc(c))
                .collect();
            let start = f.at;
            ctx.brands.extend(brands.iter().cloned());
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
        "moov" | "trak" | "mdia" | "minf" | "stbl" | "edts" | "mvex" | "iprp" | "ipco" => {
            if deeper {
                if typ == "trak" {
                    if ctx.sequence_tracks.len() < MAX_ITEMS {
                        ctx.sequence_tracks.push(SequenceTrack {
                            node,
                            ..SequenceTrack::default()
                        });
                        ctx.current_track = Some(ctx.sequence_tracks.len() - 1);
                    } else {
                        ctx.current_track = None;
                    }
                }
                walk(tree, node, data, body, end, depth + 1, ctx);
            }
        }
        "dinf" => {
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
            let handler = f.cc("handlerType")?;
            if let Some(track) = ctx
                .current_track
                .and_then(|i| ctx.sequence_tracks.get_mut(i))
            {
                track.handler = handler;
            }
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
        "mdhd" => {
            let version = f.full_box()?;
            if version > 1 {
                return None;
            }
            let time_size = if version == 1 { 8 } else { 4 };
            f.num("creationTime", time_size)?;
            f.num("modificationTime", time_size)?;
            let timescale = f.num("timescale", 4)?;
            let duration = f.num("duration", time_size)?;
            f.num("language", 2)?;
            f.num("preDefined", 2)?;
            if let Some(track) = ctx
                .current_track
                .and_then(|i| ctx.sequence_tracks.get_mut(i))
            {
                track.timescale = Some(timescale);
                if track.sample_count == 0 {
                    track.sample_ticks = duration;
                }
            }
        }
        "stts" => {
            f.full_box()?;
            let count = f.num("entryCount", 4)?;
            if count > MAX_ITEMS as u64 {
                f.tree.warning(
                    node,
                    "sample timing table exceeds the item limit",
                    ByteRange::new(body, end - body),
                );
            }
            let mut samples = 0u64;
            let mut ticks = 0u64;
            for _ in 0..count.min(MAX_ITEMS as u64) {
                let sample_count = f.num("sampleCount", 4)?;
                let sample_delta = f.num("sampleDelta", 4)?;
                samples = samples.checked_add(sample_count)?;
                ticks = ticks.checked_add(sample_count.checked_mul(sample_delta)?)?;
            }
            if let Some(track) = ctx
                .current_track
                .and_then(|i| ctx.sequence_tracks.get_mut(i))
            {
                track.sample_count = samples;
                track.sample_ticks = ticks;
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
            let colour_type = f.cc("colourType")?;
            if matches!(colour_type.as_str(), "prof" | "rICC") {
                let profile_at = f.at;
                let profile = data.get(profile_at as usize..end as usize)?;
                let source = ByteRange::new(profile_at, end - profile_at);
                f.rest("profile data");
                crate::icc::add_profile(f.tree, node, profile, source, |offset, len| {
                    ByteRange::new(profile_at + offset, len)
                });
            } else {
                f.rest("colourData");
            }
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
            let data_reference_index = f.num("dataReferenceIndex", 2)? as u16;
            let base = f.var("baseOffset", base_size)?;
            let n = f.num("extentCount", 2)?;
            let mut extents = Vec::new();
            for _ in 0..n.min(MAX_ITEMS as u64) {
                let index = if index_size > 0 {
                    f.var("extentIndex", index_size)?
                } else {
                    1 // ISO/IEC 14496-12: index_size == 0 implies index 1.
                };
                let off = f.var("extentOffset", off_size)?;
                let len = f.var("extentLength", len_size)?;
                extents.push(Extent {
                    index,
                    offset: off,
                    length: len,
                });
            }
            Some((method, data_reference_index, base, extents))
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
        let (method, data_reference_index, base, extents) = read?;
        ctx.locs.push(Loc {
            id: id?,
            method,
            data_reference_index,
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
        for (k, extent) in loc.extents.iter().enumerate() {
            let range = match loc.method {
                0 => loc
                    .base
                    .checked_add(extent.offset)
                    .map(|start| (start, extent.length)),
                1 => ctx
                    .idat
                    .and_then(|(s, _, _)| s.checked_add(loc.base)?.checked_add(extent.offset))
                    .map(|start| (start, extent.length)),
                2 => match method2_extent(ctx, loc, extent, len) {
                    Ok(range) => Some((range.start, range.len)),
                    Err(reason) => {
                        doc.tree.add(
                            Some(loc.node),
                            format!("construction method 2: {reason}"),
                            ByteRange::new(0, 0),
                            NodeKind::Field,
                            None,
                        );
                        None
                    }
                },
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
            let Some((start, mut n)) = range else {
                continue;
            };
            // A zero length means "to the end", which only a lone extent can say.
            if n == 0 {
                n = len.saturating_sub(start);
            }
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

/// Resolves the deliberately narrow, zero-copy subset of item-offset extents.
/// Chained locations, implicit lengths and items assembled from multiple
/// extents remain visible as unread instead of being guessed at.
fn method2_extent(
    ctx: &Ctx,
    loc: &Loc,
    extent: &Extent,
    file_len: u64,
) -> Result<ByteRange, &'static str> {
    if loc.data_reference_index != 0 {
        return Err("external data references are not read");
    }
    let ref_index = usize::try_from(
        extent
            .index
            .checked_sub(1)
            .ok_or("extent index 0 is reserved")?,
    )
    .map_err(|_| "extent reference index is too large")?;
    let target = ctx
        .refs
        .iter()
        .filter(|r| r.typ == "iloc" && r.from == loc.id)
        .flat_map(|r| r.to.iter())
        .nth(ref_index)
        .copied()
        .ok_or("no matching iloc item reference was found")?;
    let source = ctx
        .locs
        .iter()
        .find(|source| source.id == target)
        .ok_or("the referenced item has no location")?;
    if source.method > 1 || source.data_reference_index != 0 {
        return Err("the referenced item is not stored directly in this file");
    }
    if source.extents.len() != 1 {
        return Err("the referenced item does not have one extent");
    }
    let source_extent = &source.extents[0];
    if source_extent.length == 0 {
        return Err("the referenced item has an implicit extent length");
    }
    let source_start = match source.method {
        0 => source.base.checked_add(source_extent.offset),
        1 => ctx.idat.and_then(|(start, _, _)| {
            start
                .checked_add(source.base)?
                .checked_add(source_extent.offset)
        }),
        _ => None,
    }
    .ok_or("the referenced item's location overflows")?;
    let source_end = source_start
        .checked_add(source_extent.length)
        .ok_or("the referenced item's length overflows")?;
    let in_item_data = ctx
        .mdat
        .iter()
        .any(|(r, _)| source_start >= r.start && source_end <= r.end())
        || ctx
            .idat
            .is_some_and(|(_, _, r)| source_start >= r.start && source_end <= r.end());
    if source_end > file_len || !in_item_data {
        return Err("the referenced item's extent is outside bounded item data");
    }
    if extent.length == 0 {
        return Err("an explicit extent length is required for item-offset reads");
    }
    let item_offset = loc
        .base
        .checked_add(extent.offset)
        .ok_or("the item offset overflows")?;
    let item_end = item_offset
        .checked_add(extent.length)
        .ok_or("the item extent length overflows")?;
    if item_end > source_extent.length {
        return Err("the requested range runs past the referenced item");
    }
    let start = source_start
        .checked_add(item_offset)
        .ok_or("the resolved item offset overflows")?;
    Ok(ByteRange::new(start, extent.length))
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

    fn bx(typ: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let mut out = ((body.len() + 8) as u32).to_be_bytes().to_vec();
        out.extend_from_slice(typ);
        out.extend_from_slice(body);
        out
    }

    fn avif_sequence_fixture() -> Vec<u8> {
        let ftyp = b"avis\0\0\0\0avismif1".to_vec();

        let mut mdhd = vec![0; 4];
        mdhd.extend_from_slice(&0u32.to_be_bytes()); // creation time
        mdhd.extend_from_slice(&0u32.to_be_bytes()); // modification time
        mdhd.extend_from_slice(&1000u32.to_be_bytes()); // media timescale
        mdhd.extend_from_slice(&400u32.to_be_bytes()); // media duration
        mdhd.extend_from_slice(&[0; 4]); // language and pre-defined

        let mut hdlr = vec![0; 4];
        hdlr.extend_from_slice(&0u32.to_be_bytes());
        hdlr.extend_from_slice(b"pict");
        hdlr.extend_from_slice(&[0; 12]);
        hdlr.push(0);

        let mut stts = vec![0; 4];
        stts.extend_from_slice(&2u32.to_be_bytes());
        stts.extend_from_slice(&2u32.to_be_bytes());
        stts.extend_from_slice(&100u32.to_be_bytes());
        stts.extend_from_slice(&1u32.to_be_bytes());
        stts.extend_from_slice(&200u32.to_be_bytes());

        let stbl = bx(b"stbl", &bx(b"stts", &stts));
        let minf = bx(b"minf", &stbl);
        let mut mdia_body = bx(b"mdhd", &mdhd);
        mdia_body.extend_from_slice(&bx(b"hdlr", &hdlr));
        mdia_body.extend_from_slice(&minf);
        let track = bx(b"trak", &bx(b"mdia", &mdia_body));
        let moov = bx(b"moov", &track);

        let mut out = bx(b"ftyp", &ftyp);
        out.extend_from_slice(&moov);
        out
    }

    #[test]
    fn avis_sequence_tracks_expose_handler_and_sample_timing() {
        let doc = parse_heif(&avif_sequence_fixture());
        let track = doc.tree.nodes().iter().find(|n| n.label == "trak").unwrap();
        let value = match &track.value {
            Some(Value::Text(value)) => value,
            other => panic!("expected sequence summary, got {other:?}"),
        };
        assert!(value.contains("AV1 image sequence · pict"), "{value}");
        assert!(value.contains("3 samples"), "{value}");
        assert!(value.contains("400 ticks at 1000 ticks/s"), "{value}");
        assert!(problems(&doc.tree).is_empty(), "{:?}", problems(&doc.tree));
    }

    #[test]
    fn every_avif_sequence_truncation_is_survivable() {
        let bytes = avif_sequence_fixture();
        for n in 0..=bytes.len() {
            assert!(parse_heif(&bytes[..n]).tree.root().is_some(), "at {n}");
        }
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
