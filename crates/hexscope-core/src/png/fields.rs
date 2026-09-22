#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ByteRange, NodeKind, ParseTree, Value};
    use crate::png::chunks::Chunk;

    fn fake_chunk<'a>(kind: &[u8; 4], data: &'a [u8]) -> Chunk<'a> {
        Chunk {
            range: ByteRange::new(0, (data.len() + 12) as u64),
            kind: *kind,
            data,
            data_range: ByteRange::new(8, data.len() as u64),
            declared_crc: 0,
            actual_crc: 0,
        }
    }

    #[test]
    fn decodes_ihdr_fields() {
        // 1920x1080, 8-bit, colour type 6 (RGBA), no interlace.
        let data = [0, 0, 7, 128, 0, 0, 4, 56, 8, 6, 0, 0, 0];
        let chunk = fake_chunk(b"IHDR", &data);
        let mut tree = ParseTree::new();
        let root = tree.add(None, "PNG", ByteRange::new(0, 0), NodeKind::Container, None);

        let ihdr = decode_ihdr(&chunk, &mut tree, root).expect("valid IHDR");

        assert_eq!(ihdr.width, 1920);
        assert_eq!(ihdr.height, 1080);
        assert_eq!(ihdr.bit_depth, 8);
        assert_eq!(ihdr.color_type, 6);
        assert_eq!(ihdr.filter_distance(), 4);
        assert_eq!(ihdr.stride(), Some(1920 * 4));

        let labels: Vec<&str> = tree
            .get(root)
            .children
            .iter()
            .map(|&id| tree.get(id).label.as_str())
            .collect();
        assert_eq!(
            labels,
            [
                "width",
                "height",
                "bitDepth",
                "colorType",
                "compression",
                "filter",
                "interlace"
            ]
        );

        let color_node = tree.get(tree.get(root).children[3]);
        assert_eq!(
            color_node.value,
            Some(Value::Enum {
                raw: 6,
                name: "RGBA"
            })
        );
        // Field ranges point at the real file offsets, not at chunk-local ones.
        assert_eq!(
            tree.get(tree.get(root).children[0]).range,
            ByteRange::new(8, 4)
        );
    }

    #[test]
    fn stride_and_filter_distance_differ_below_eight_bits() {
        // 32 px of 1-bit greyscale is 4 bytes per scanline, not 32. Treating
        // the filter distance as the stride reported every such file as
        // truncated.
        let ihdr = Ihdr {
            width: 32,
            height: 32,
            bit_depth: 1,
            color_type: 0,
            interlace: 0,
        };
        assert_eq!(ihdr.filter_distance(), 1);
        assert_eq!(ihdr.stride(), Some(4));

        // 4-bit palette, 33 px: 16.5 bytes rounds up to 17.
        let palette = Ihdr {
            width: 33,
            height: 1,
            bit_depth: 4,
            color_type: 3,
            interlace: 0,
        };
        assert_eq!(palette.filter_distance(), 1);
        assert_eq!(palette.stride(), Some(17));

        // At 8 bits and above the two coincide per pixel.
        let rgb = Ihdr {
            width: 10,
            height: 1,
            bit_depth: 8,
            color_type: 2,
            interlace: 0,
        };
        assert_eq!(rgb.filter_distance(), 3);
        assert_eq!(rgb.stride(), Some(30));
    }

    #[test]
    fn marks_a_truncated_ihdr_without_panicking() {
        let chunk = fake_chunk(b"IHDR", &[0, 0, 7]);
        let mut tree = ParseTree::new();
        let root = tree.add(None, "PNG", ByteRange::new(0, 0), NodeKind::Container, None);

        assert!(decode_ihdr(&chunk, &mut tree, root).is_none());
        let child = tree.get(tree.get(root).children[0]);
        assert_eq!(child.kind, NodeKind::Error);
        assert_eq!(child.label, "IHDR truncated");
    }

    #[test]
    fn decodes_text_keyword_and_value() {
        let chunk = fake_chunk(b"tEXt", b"Author\0Ada");
        let mut tree = ParseTree::new();
        let root = tree.add(
            None,
            "tEXt",
            ByteRange::new(0, 0),
            NodeKind::Container,
            None,
        );

        decode_text(&chunk, &mut tree, root);

        let children = &tree.get(root).children;
        assert_eq!(
            tree.get(children[0]).value,
            Some(Value::Text("Author".into()))
        );
        assert_eq!(tree.get(children[1]).value, Some(Value::Text("Ada".into())));
    }

    #[test]
    fn flags_text_with_no_separator() {
        let chunk = fake_chunk(b"tEXt", b"no-null-here");
        let mut tree = ParseTree::new();
        let root = tree.add(
            None,
            "tEXt",
            ByteRange::new(0, 0),
            NodeKind::Container,
            None,
        );

        decode_text(&chunk, &mut tree, root);

        let child = tree.get(tree.get(root).children[0]);
        assert_eq!(child.kind, NodeKind::Warning);
        assert_eq!(child.label, "missing keyword separator");
    }

    #[test]
    fn decodes_palette_entries_as_hex_colors() {
        let data = [0xFF, 0x00, 0x00, 0x00, 0x80, 0xFF];
        let chunk = fake_chunk(b"PLTE", &data);
        let mut tree = ParseTree::new();
        let root = tree.add(
            None,
            "PLTE",
            ByteRange::new(0, 0),
            NodeKind::Container,
            None,
        );

        decode_plte(&chunk, &mut tree, root);

        let children = &tree.get(root).children;
        assert_eq!(tree.get(children[0]).value, Some(Value::U64(2)));
        assert_eq!(
            tree.get(children[1]).value,
            Some(Value::Text("#FF0000".into()))
        );
        assert_eq!(
            tree.get(children[2]).value,
            Some(Value::Text("#0080FF".into()))
        );
        // Second entry starts three bytes into the payload, which begins at 8.
        assert_eq!(tree.get(children[2]).range, ByteRange::new(11, 3));
    }

    #[test]
    fn warns_on_a_palette_length_that_is_not_a_multiple_of_three() {
        let chunk = fake_chunk(b"PLTE", &[1, 2, 3, 4]);
        let mut tree = ParseTree::new();
        let root = tree.add(
            None,
            "PLTE",
            ByteRange::new(0, 0),
            NodeKind::Container,
            None,
        );

        decode_plte(&chunk, &mut tree, root);

        assert_eq!(tree.get(tree.get(root).children[0]).kind, NodeKind::Warning);
    }

    #[test]
    fn decodes_physical_pixel_dimensions() {
        // 2835 pixels per metre is 72 dpi — what most editors write.
        let mut data = Vec::new();
        data.extend_from_slice(&2835u32.to_be_bytes());
        data.extend_from_slice(&2835u32.to_be_bytes());
        data.push(1);
        let chunk = fake_chunk(b"pHYs", &data);
        let mut tree = ParseTree::new();
        let root = tree.add(
            None,
            "pHYs",
            ByteRange::new(0, 0),
            NodeKind::Container,
            None,
        );

        decode_phys(&chunk, &mut tree, root);

        let children = &tree.get(root).children;
        assert_eq!(tree.get(children[0]).value, Some(Value::U64(2835)));
        assert_eq!(tree.get(children[1]).value, Some(Value::U64(2835)));
        assert_eq!(
            tree.get(children[2]).value,
            Some(Value::Enum {
                raw: 1,
                name: "metre"
            })
        );
        // The unit byte sits 8 bytes into a payload that starts at file offset 8.
        assert_eq!(tree.get(children[2]).range, ByteRange::new(16, 1));
    }

    #[test]
    fn marks_a_truncated_phys_without_panicking() {
        let chunk = fake_chunk(b"pHYs", &[0, 0]);
        let mut tree = ParseTree::new();
        let root = tree.add(
            None,
            "pHYs",
            ByteRange::new(0, 0),
            NodeKind::Container,
            None,
        );

        decode_phys(&chunk, &mut tree, root);

        let child = tree.get(tree.get(root).children[0]);
        assert_eq!(child.kind, NodeKind::Error);
        assert_eq!(child.label, "pHYs truncated");
    }

    #[test]
    fn decodes_gamma_as_raw_and_decimal() {
        // 45455 is the value nearly every PNG writes: gamma 1/2.2.
        let data = 45455u32.to_be_bytes();
        let chunk = fake_chunk(b"gAMA", &data);
        let mut tree = ParseTree::new();
        let root = tree.add(
            None,
            "gAMA",
            ByteRange::new(0, 0),
            NodeKind::Container,
            None,
        );

        decode_gama(&chunk, &mut tree, root);

        let children = &tree.get(root).children;
        assert_eq!(tree.get(children[0]).value, Some(Value::U64(45455)));
        assert_eq!(
            tree.get(children[1]).value,
            Some(Value::Text("0.45455".into()))
        );
    }

    #[test]
    fn marks_a_truncated_gama_without_panicking() {
        let chunk = fake_chunk(b"gAMA", &[0, 0]);
        let mut tree = ParseTree::new();
        let root = tree.add(
            None,
            "gAMA",
            ByteRange::new(0, 0),
            NodeKind::Container,
            None,
        );

        decode_gama(&chunk, &mut tree, root);

        let child = tree.get(tree.get(root).children[0]);
        assert_eq!(child.kind, NodeKind::Error);
        assert_eq!(child.label, "gAMA truncated");
    }

    #[test]
    fn reads_trns_according_to_color_type() {
        let chunk = fake_chunk(b"tRNS", &[0, 64, 128]);
        let mut tree = ParseTree::new();

        let palette_root = tree.add(
            None,
            "tRNS",
            ByteRange::new(0, 0),
            NodeKind::Container,
            None,
        );
        decode_trns(&chunk, &mut tree, palette_root, Some(3));
        let child = tree.get(tree.get(palette_root).children[0]);
        assert_eq!(child.label, "paletteAlphaCount");
        assert_eq!(child.value, Some(Value::U64(3)));

        let rgb_root = tree.add(
            None,
            "tRNS",
            ByteRange::new(0, 0),
            NodeKind::Container,
            None,
        );
        decode_trns(&chunk, &mut tree, rgb_root, Some(2));
        assert_eq!(
            tree.get(tree.get(rgb_root).children[0]).label,
            "transparentColor"
        );

        let orphan_root = tree.add(
            None,
            "tRNS",
            ByteRange::new(0, 0),
            NodeKind::Container,
            None,
        );
        decode_trns(&chunk, &mut tree, orphan_root, None);
        assert_eq!(
            tree.get(tree.get(orphan_root).children[0]).kind,
            NodeKind::Warning
        );

        // Colour types 4 and 6 already carry an alpha channel, so the PNG spec
        // forbids tRNS there — the realistic way this warning fires in the wild.
        for forbidden in [4u8, 6] {
            let root = tree.add(
                None,
                "tRNS",
                ByteRange::new(0, 0),
                NodeKind::Container,
                None,
            );
            decode_trns(&chunk, &mut tree, root, Some(forbidden));
            let child = tree.get(tree.get(root).children[0]);
            assert_eq!(child.kind, NodeKind::Warning, "colour type {forbidden}");
            assert_eq!(child.label, "tRNS without a usable colour type");
        }
    }
}

use crate::model::{ByteRange, NodeId, NodeKind, ParseTree, Value};
use crate::png::chunks::Chunk;
use crate::reader::Reader;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ihdr {
    pub width: u32,
    pub height: u32,
    pub bit_depth: u8,
    pub color_type: u8,
    pub interlace: u8,
}

impl Ihdr {
    /// Samples per pixel for this colour type.
    pub fn channels(&self) -> usize {
        match self.color_type {
            0 => 1, // greyscale
            2 => 3, // RGB
            3 => 1, // palette index
            4 => 2, // greyscale + alpha
            6 => 4, // RGBA
            _ => 1,
        }
    }

    /// How far back a filter looks for "the pixel to the left", in bytes.
    /// PNG defines this as `max(1, floor(channels * bit_depth / 8))`, so at
    /// sub-byte depths it clamps to one byte.
    ///
    /// This is NOT the scanline stride — see [`Ihdr::stride`]. Conflating the
    /// two silently breaks every image below 8-bit depth.
    pub fn filter_distance(&self) -> usize {
        ((self.channels() * self.bit_depth as usize) / 8).max(1)
    }

    /// Bytes in one unfiltered scanline: `ceil(width * channels * depth / 8)`.
    /// At 1, 2 and 4 bits per sample several pixels share a byte, so this is
    /// much smaller than `width * filter_distance`.
    ///
    /// Returns `None` if the dimensions overflow `usize`, which a crafted IHDR
    /// can do on a 32-bit target such as wasm32.
    pub fn stride(&self) -> Option<usize> {
        (self.width as usize)
            .checked_mul(self.channels())?
            .checked_mul(self.bit_depth as usize)
            .map(|bits| bits.div_ceil(8))
    }
}

/// The bit depths PNG permits for each colour type (RFC 2083 §4.1.1).
fn bit_depth_allowed(color_type: u8, bit_depth: u8) -> bool {
    match color_type {
        0 => matches!(bit_depth, 1 | 2 | 4 | 8 | 16),
        3 => matches!(bit_depth, 1 | 2 | 4 | 8),
        2 | 4 | 6 => matches!(bit_depth, 8 | 16),
        _ => false,
    }
}

fn color_type_name(raw: u8) -> &'static str {
    match raw {
        0 => "Greyscale",
        2 => "RGB",
        3 => "Palette",
        4 => "Greyscale+Alpha",
        6 => "RGBA",
        _ => "unknown",
    }
}

/// Adds one field node whose range is expressed in file coordinates.
fn field(
    tree: &mut ParseTree,
    parent: NodeId,
    label: &str,
    chunk: &Chunk,
    offset: u64,
    len: u64,
    value: Value,
) {
    let range = ByteRange::new(chunk.data_range.start + offset, len);
    tree.add(Some(parent), label, range, NodeKind::Field, Some(value));
}

pub fn decode_ihdr(chunk: &Chunk, tree: &mut ParseTree, parent: NodeId) -> Option<Ihdr> {
    let mut r = Reader::new(chunk.data);

    // All thirteen bytes are read up front; if any read fails the chunk is
    // damaged and we record one Error node rather than a half-filled tree.
    let (
        Ok(width),
        Ok(height),
        Ok(bit_depth),
        Ok(color_type),
        Ok(compression),
        Ok(filter),
        Ok(interlace),
    ) = (
        r.u32_be(),
        r.u32_be(),
        r.u8(),
        r.u8(),
        r.u8(),
        r.u8(),
        r.u8(),
    )
    else {
        tree.add(
            Some(parent),
            "IHDR truncated",
            chunk.data_range,
            NodeKind::Error,
            None,
        );
        return None;
    };

    field(tree, parent, "width", chunk, 0, 4, Value::U64(width as u64));
    field(
        tree,
        parent,
        "height",
        chunk,
        4,
        4,
        Value::U64(height as u64),
    );
    field(
        tree,
        parent,
        "bitDepth",
        chunk,
        8,
        1,
        Value::U64(bit_depth as u64),
    );
    field(
        tree,
        parent,
        "colorType",
        chunk,
        9,
        1,
        Value::Enum {
            raw: color_type as u64,
            name: color_type_name(color_type),
        },
    );
    field(
        tree,
        parent,
        "compression",
        chunk,
        10,
        1,
        Value::U64(compression as u64),
    );
    field(
        tree,
        parent,
        "filter",
        chunk,
        11,
        1,
        Value::U64(filter as u64),
    );
    field(
        tree,
        parent,
        "interlace",
        chunk,
        12,
        1,
        Value::U64(interlace as u64),
    );

    // A structurally fine IHDR can still describe an impossible image. Saying
    // so is the point of the tool, so these are warnings on the exact byte.
    if !matches!(color_type, 0 | 2 | 3 | 4 | 6) {
        tree.add(
            Some(parent),
            format!("colour type {color_type} is not one of 0, 2, 3, 4, 6"),
            ByteRange::new(chunk.data_range.start + 9, 1),
            NodeKind::Warning,
            None,
        );
    } else if !bit_depth_allowed(color_type, bit_depth) {
        tree.add(
            Some(parent),
            format!("bit depth {bit_depth} is not allowed for colour type {color_type}"),
            ByteRange::new(chunk.data_range.start + 8, 1),
            NodeKind::Warning,
            None,
        );
    }

    Some(Ihdr {
        width,
        height,
        bit_depth,
        color_type,
        interlace,
    })
}

/// PLTE is a flat array of RGB triples. Each entry becomes a field so hovering
/// a palette index in the hex view lights up the exact three bytes.
pub fn decode_plte(chunk: &Chunk, tree: &mut ParseTree, parent: NodeId) {
    if !chunk.data.len().is_multiple_of(3) {
        tree.add(
            Some(parent),
            "PLTE length is not a multiple of 3",
            chunk.data_range,
            NodeKind::Warning,
            None,
        );
        return;
    }

    field(
        tree,
        parent,
        "entries",
        chunk,
        0,
        chunk.data.len() as u64,
        Value::U64((chunk.data.len() / 3) as u64),
    );

    // `array::<3>` yields a fixed-size array, so indexing it is checked at
    // compile time rather than being a raw slice index.
    let mut r = Reader::new(chunk.data);
    let mut i = 0u64;
    while let Ok(rgb) = r.array::<3>() {
        field(
            tree,
            parent,
            &format!("entry {i}"),
            chunk,
            i * 3,
            3,
            Value::Text(format!("#{:02X}{:02X}{:02X}", rgb[0], rgb[1], rgb[2])),
        );
        i += 1;
    }
}

/// gAMA stores gamma × 100000 as a big-endian u32.
pub fn decode_gama(chunk: &Chunk, tree: &mut ParseTree, parent: NodeId) {
    let mut r = Reader::new(chunk.data);
    let Ok(raw) = r.u32_be() else {
        tree.add(
            Some(parent),
            "gAMA truncated",
            chunk.data_range,
            NodeKind::Error,
            None,
        );
        return;
    };

    field(tree, parent, "gamma", chunk, 0, 4, Value::U64(raw as u64));
    field(
        tree,
        parent,
        "gammaDecimal",
        chunk,
        0,
        4,
        Value::Text(format!("{:.5}", raw as f64 / 100_000.0)),
    );
}

/// tRNS means different things per colour type, so the label says which
/// reading applies instead of silently guessing.
pub fn decode_trns(chunk: &Chunk, tree: &mut ParseTree, parent: NodeId, color_type: Option<u8>) {
    match color_type {
        Some(3) => {
            field(
                tree,
                parent,
                "paletteAlphaCount",
                chunk,
                0,
                chunk.data.len() as u64,
                Value::U64(chunk.data.len() as u64),
            );
        }
        Some(0) | Some(2) => {
            field(
                tree,
                parent,
                "transparentColor",
                chunk,
                0,
                chunk.data.len() as u64,
                Value::Bytes(chunk.data.len() as u64),
            );
        }
        _ => {
            tree.add(
                Some(parent),
                "tRNS without a usable colour type",
                chunk.data_range,
                NodeKind::Warning,
                None,
            );
        }
    }
}

pub fn decode_text(chunk: &Chunk, tree: &mut ParseTree, parent: NodeId) {
    let mut r = Reader::new(chunk.data);

    let Some(keyword_bytes) = r.bytes_until(0) else {
        tree.add(
            Some(parent),
            "missing keyword separator",
            chunk.data_range,
            NodeKind::Warning,
            None,
        );
        return;
    };
    let text_bytes = r.rest();

    let sep = keyword_bytes.len() as u64;
    let keyword = String::from_utf8_lossy(keyword_bytes).into_owned();
    let text = String::from_utf8_lossy(text_bytes).into_owned();

    field(tree, parent, "keyword", chunk, 0, sep, Value::Text(keyword));
    field(
        tree,
        parent,
        "text",
        chunk,
        sep + 1,
        text_bytes.len() as u64,
        Value::Text(text),
    );
}

pub fn decode_phys(chunk: &Chunk, tree: &mut ParseTree, parent: NodeId) {
    let mut r = Reader::new(chunk.data);
    let (Ok(x), Ok(y), Ok(unit)) = (r.u32_be(), r.u32_be(), r.u8()) else {
        tree.add(
            Some(parent),
            "pHYs truncated",
            chunk.data_range,
            NodeKind::Error,
            None,
        );
        return;
    };

    field(
        tree,
        parent,
        "pixelsPerUnitX",
        chunk,
        0,
        4,
        Value::U64(x as u64),
    );
    field(
        tree,
        parent,
        "pixelsPerUnitY",
        chunk,
        4,
        4,
        Value::U64(y as u64),
    );
    field(
        tree,
        parent,
        "unit",
        chunk,
        8,
        1,
        Value::Enum {
            raw: unit as u64,
            name: if unit == 1 { "metre" } else { "unknown" },
        },
    );
}
