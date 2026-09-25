//! What each part of a PNG is. Citations are to the W3C PNG specification
//! (third edition); the DEFLATE stream inside IDAT, to RFC 1950 and 1951.

use crate::docs::{Concern, Doc, Table, lookup};

const fn png(text: &'static str, cite: &'static str, url: &'static str) -> Doc {
    Doc::new(text).cite(cite, url)
}

const fn damage(text: &'static str) -> Doc {
    Doc::new(text).concern(Concern::Damage)
}

const fn oddity(text: &'static str) -> Doc {
    Doc::new(text).concern(Concern::Oddity)
}

const TABLE: Table = &[
    // The file and its framing.
    (
        "PNG",
        png(
            "A PNG image: an 8-byte signature, then a run of chunks, each with a length, a type, data and a checksum.",
            "PNG §5.3",
            "https://www.w3.org/TR/png-3/#5Chunk-layout",
        ),
    ),
    (
        "signature",
        png(
            "Eight fixed bytes every PNG starts with, chosen so that a damaged transfer or a wrong file type shows at once.",
            "PNG §5.2",
            "https://www.w3.org/TR/png-3/#5PNG-file-signature",
        ),
    ),
    // Critical chunks.
    (
        "IHDR",
        png(
            "The image header: its width and height, and how each pixel's colour is stored.",
            "PNG §11.2.1",
            "https://www.w3.org/TR/png-3/#11IHDR",
        ),
    ),
    (
        "PLTE",
        png(
            "The palette: the list of colours a palette image's pixels pick from by number.",
            "PNG §11.2.2",
            "https://www.w3.org/TR/png-3/#11PLTE",
        ),
    ),
    (
        "IDAT",
        png(
            "Image data: the pixels, filtered row by row and then compressed with DEFLATE.",
            "PNG §11.2.3",
            "https://www.w3.org/TR/png-3/#11IDAT",
        ),
    ),
    (
        "IEND",
        png(
            "The end marker: an empty chunk that says the image is complete.",
            "PNG §11.2.4",
            "https://www.w3.org/TR/png-3/#11IEND",
        ),
    ),
    // Ancillary chunks.
    (
        "tRNS",
        png(
            "Transparency: which colours, or which palette entries, are see-through and by how much.",
            "PNG §11.3.1.1",
            "https://www.w3.org/TR/png-3/#11tRNS",
        ),
    ),
    (
        "cHRM",
        png(
            "The exact red, green, blue and white the image's colours were made for.",
            "PNG §11.3.2.1",
            "https://www.w3.org/TR/png-3/#11cHRM",
        ),
    ),
    (
        "gAMA",
        png(
            "Gamma: how the stored brightness values map to the light a screen should give off.",
            "PNG §11.3.2.2",
            "https://www.w3.org/TR/png-3/#11gAMA",
        ),
    ),
    (
        "sBIT",
        png(
            "How many bits of each colour were significant in the original, before it was stored in more.",
            "PNG §11.3.2.4",
            "https://www.w3.org/TR/png-3/#11sBIT",
        ),
    ),
    (
        "tEXt",
        png(
            "A text note: a keyword such as Author or Software, and its value, stored as plain text.",
            "PNG §11.3.3.2",
            "https://www.w3.org/TR/png-3/#11tEXt",
        ),
    ),
    (
        "zTXt",
        png(
            "A text note like tEXt, compressed to save space.",
            "PNG §11.3.3.3",
            "https://www.w3.org/TR/png-3/#11zTXt",
        ),
    ),
    (
        "iTXt",
        png(
            "A text note in any language, in UTF-8, optionally compressed.",
            "PNG §11.3.3.4",
            "https://www.w3.org/TR/png-3/#11iTXt",
        ),
    ),
    (
        "bKGD",
        png(
            "A suggested background colour to show behind transparent parts.",
            "PNG §11.3.4.1",
            "https://www.w3.org/TR/png-3/#11bKGD",
        ),
    ),
    (
        "hIST",
        png(
            "How often each palette colour is used, to help a viewer that has fewer colours to choose.",
            "PNG §11.3.4.2",
            "https://www.w3.org/TR/png-3/#11hIST",
        ),
    ),
    (
        "pHYs",
        png(
            "Physical size: how many pixels make a metre, or just the pixels' shape.",
            "PNG §11.3.4.3",
            "https://www.w3.org/TR/png-3/#11pHYs",
        ),
    ),
    (
        "sPLT",
        png(
            "A suggested reduced palette, for screens that cannot show every colour.",
            "PNG §11.3.4.4",
            "https://www.w3.org/TR/png-3/#11sPLT",
        ),
    ),
    (
        "eXIf",
        png(
            "Camera metadata in the EXIF format, the same kind a JPEG photo carries.",
            "PNG §11.3.4.5",
            "https://www.w3.org/TR/png-3/#eXIf",
        ),
    ),
    (
        "tIME",
        png(
            "When the image was last changed.",
            "PNG §11.3.5.1",
            "https://www.w3.org/TR/png-3/#11tIME",
        ),
    ),
    // IHDR fields.
    (
        "width",
        png(
            "How many pixels wide the image is.",
            "PNG §11.2.1",
            "https://www.w3.org/TR/png-3/#11IHDR",
        ),
    ),
    (
        "height",
        png(
            "How many pixels tall the image is.",
            "PNG §11.2.1",
            "https://www.w3.org/TR/png-3/#11IHDR",
        ),
    ),
    (
        "bitDepth",
        png(
            "How many bits each sample, or each palette index, takes.",
            "PNG §11.2.1",
            "https://www.w3.org/TR/png-3/#11IHDR",
        ),
    ),
    (
        "colorType",
        png(
            "How pixels store colour: grey, red-green-blue, palette, each with or without transparency.",
            "PNG §11.2.1",
            "https://www.w3.org/TR/png-3/#11IHDR",
        ),
    ),
    (
        "compression",
        png(
            "The compression method; 0, DEFLATE, is the only one defined.",
            "PNG §10.1",
            "https://www.w3.org/TR/png-3/#10Compression",
        ),
    ),
    (
        "filter",
        png(
            "The filter method: how each row is predicted from its neighbours before compression; only 0 exists.",
            "PNG §9.1",
            "https://www.w3.org/TR/png-3/#9Filters",
        ),
    ),
    (
        "interlace",
        png(
            "0 stores rows top to bottom; 1 stores the image in seven passes so it can appear gradually.",
            "PNG §8.1",
            "https://www.w3.org/TR/png-3/#8Interlace",
        ),
    ),
    // PLTE, tRNS, gAMA, pHYs, tEXt fields.
    (
        "entries",
        png(
            "How many colours the palette holds.",
            "PNG §11.2.2",
            "https://www.w3.org/TR/png-3/#11PLTE",
        ),
    ),
    (
        "entry *",
        png(
            "One palette colour, as red, green and blue.",
            "PNG §11.2.2",
            "https://www.w3.org/TR/png-3/#11PLTE",
        ),
    ),
    (
        "paletteAlphaCount",
        png(
            "How many palette colours have a transparency value; the rest are opaque.",
            "PNG §11.3.1.1",
            "https://www.w3.org/TR/png-3/#11tRNS",
        ),
    ),
    (
        "transparentColor",
        png(
            "The one colour that should be shown as fully transparent.",
            "PNG §11.3.1.1",
            "https://www.w3.org/TR/png-3/#11tRNS",
        ),
    ),
    (
        "gamma",
        png(
            "The gamma value times 100,000, as the format stores it.",
            "PNG §11.3.2.2",
            "https://www.w3.org/TR/png-3/#11gAMA",
        ),
    ),
    (
        "pixelsPerUnitX",
        png(
            "Pixels per unit, across.",
            "PNG §11.3.4.3",
            "https://www.w3.org/TR/png-3/#11pHYs",
        ),
    ),
    (
        "pixelsPerUnitY",
        png(
            "Pixels per unit, down.",
            "PNG §11.3.4.3",
            "https://www.w3.org/TR/png-3/#11pHYs",
        ),
    ),
    (
        "unit",
        png(
            "The unit: the metre, or none, when only the pixels' shape is given.",
            "PNG §11.3.4.3",
            "https://www.w3.org/TR/png-3/#11pHYs",
        ),
    ),
    (
        "keyword",
        png(
            "What the text is about, such as Title, Author or Software.",
            "PNG §11.3.3.2",
            "https://www.w3.org/TR/png-3/#11tEXt",
        ),
    ),
    (
        "text",
        png(
            "The text itself, inflated first when it is compressed.",
            "PNG §11.3.3.2",
            "https://www.w3.org/TR/png-3/#11tEXt",
        ),
    ),
    (
        "compression method",
        png(
            "How the text is compressed; 0, the only method, is zlib.",
            "PNG §11.3.3.3",
            "https://www.w3.org/TR/png-3/#11zTXt",
        ),
    ),
    (
        "compression flag",
        png(
            "Whether the text is compressed: 1 for yes, 0 for plain UTF-8.",
            "PNG §11.3.3.4",
            "https://www.w3.org/TR/png-3/#11iTXt",
        ),
    ),
    (
        "language tag",
        png(
            "The language the text is in, such as en or uk.",
            "PNG §11.3.3.4",
            "https://www.w3.org/TR/png-3/#11iTXt",
        ),
    ),
    (
        "translated keyword",
        png(
            "The keyword in that language.",
            "PNG §11.3.3.4",
            "https://www.w3.org/TR/png-3/#11iTXt",
        ),
    ),
    (
        "time",
        png(
            "When the image was last changed, in UTC.",
            "PNG §11.3.5.1",
            "https://www.w3.org/TR/png-3/#11tIME",
        ),
    ),
    (
        "* truncated",
        damage("The chunk is too short for the fields it must hold."),
    ),
    (
        "unknown text compression method",
        oddity("The text is compressed with a method PNG does not define, so it cannot be read."),
    ),
    (
        "data after the end of the image",
        Doc::new("Bytes after IEND. Viewers ignore them, so files made to be two formats at once, or to carry something unseen, keep it here.")
            .cite("PNG §5.6", "https://www.w3.org/TR/png-3/#5ChunkOrdering")
            .concern(Concern::Hidden),
    ),
    (
        "compressed text cannot be inflated",
        damage("The compressed text is damaged: it does not inflate."),
    ),
    // The compressed stream inside IDAT.
    (
        "zlib header*",
        png(
            "Two bytes that open the compressed stream: which method, and how far back it may copy from.",
            "RFC 1950 §2.2",
            "https://www.rfc-editor.org/rfc/rfc1950#section-2",
        ),
    ),
    (
        "block * · stored*",
        png(
            "A DEFLATE block kept uncompressed, for data that would not get smaller.",
            "RFC 1951 §3.2.4",
            "https://www.rfc-editor.org/rfc/rfc1951#section-3",
        ),
    ),
    (
        "block * · fixed Huffman*",
        png(
            "A DEFLATE block compressed with the fixed codes the format defines.",
            "RFC 1951 §3.2.6",
            "https://www.rfc-editor.org/rfc/rfc1951#section-3",
        ),
    ),
    (
        "block * · dynamic Huffman*",
        png(
            "A DEFLATE block that brings its own codes, tuned to its data.",
            "RFC 1951 §3.2.7",
            "https://www.rfc-editor.org/rfc/rfc1951#section-3",
        ),
    ),
    (
        "Adler-32*",
        png(
            "A checksum of the decompressed data, to catch damage the compression itself would not.",
            "RFC 1950 §2.2",
            "https://www.rfc-editor.org/rfc/rfc1950#section-2",
        ),
    ),
    // Problems.
    (
        "not a PNG signature",
        damage(
            "The file does not start the way every PNG must: it may be another format under a .png name.",
        ),
    ),
    (
        "damaged PNG signature",
        damage(
            "The signature is damaged in the way a text-mode transfer damages it, so the rest may be damaged too.",
        ),
    ),
    (
        "truncated chunk*",
        damage(
            "The file ends in the middle of a chunk: it was cut short, as by an interrupted download.",
        ),
    ),
    (
        "chunk length out of range*",
        damage(
            "A chunk's length is impossible, so its bytes cannot be trusted; reading resumes at the next intact chunk.",
        ),
    ),
    (
        "chunk type is not four ASCII letters",
        damage("A chunk's type is not a readable name, a sign that these bytes are damaged."),
    ),
    (
        "CRC mismatch*",
        damage(
            "The chunk's checksum does not match its bytes: they changed after the file was written.",
        ),
    ),
    (
        "IHDR truncated",
        damage(
            "The image header is shorter than it must be, so the image's size and colours are unknown.",
        ),
    ),
    (
        "gAMA truncated",
        damage("The gamma chunk is too short to hold its value."),
    ),
    (
        "pHYs truncated",
        damage("The physical-size chunk is too short to hold its values."),
    ),
    (
        "PLTE length is not a multiple of 3",
        oddity(
            "The palette's size is not a whole number of colours; the leftover bytes are ignored.",
        ),
    ),
    (
        "missing keyword separator",
        oddity(
            "A text note has no zero byte between its keyword and its text, so the two cannot be told apart.",
        ),
    ),
    (
        "tRNS without a usable colour type",
        oddity("Transparency is given for an image whose colour type cannot use it."),
    ),
    (
        "bit depth * is not allowed for colour type *",
        damage(
            "This combination of bit depth and colour type is not allowed, so viewers refuse the image.",
        ),
    ),
    (
        "colour type * is not one of *",
        damage("The colour type is not one the format defines, so viewers refuse the image."),
    ),
    (
        "compression method *",
        damage("An undefined compression method: no viewer knows how to read the image data."),
    ),
    (
        "filter method *",
        damage("An undefined filter method: no viewer knows how to rebuild the rows."),
    ),
    (
        "interlace method *",
        damage("An undefined interlace method: no viewer knows the order the rows are stored in."),
    ),
    (
        "no IDAT chunk*",
        damage("There is no image data at all: the file describes a picture it does not contain."),
    ),
    (
        "no IEND chunk*",
        damage("The file never says it is finished, which usually means it was cut short."),
    ),
    (
        "image dimensions are too large*",
        damage("The width and height are too large for any real image to hold."),
    ),
    (
        "IDAT decompression failed*",
        damage("The compressed image data breaks here: everything after this point is lost."),
    ),
    (
        "unfiltering failed*",
        damage(
            "The decompressed rows do not fit the image's size or name an undefined filter, so the pixels cannot be rebuilt.",
        ),
    ),
];

/// A chunk this tool does not decode: a child of the root with any other name.
const UNKNOWN_CHUNK: Doc = png(
    "A chunk this tool does not decode. A lower-case first letter tells readers they may safely skip it.",
    "PNG §5.4",
    "https://www.w3.org/TR/png-3/#5Chunk-naming-conventions",
);

pub(crate) fn describe(label: &str, parent: Option<&str>, problem: bool) -> Option<Doc> {
    lookup(TABLE, label, problem)
        .or_else(|| (!problem && parent == Some("PNG")).then_some(UNKNOWN_CHUNK))
}

#[cfg(test)]
pub(crate) const ALL: Table = TABLE;
