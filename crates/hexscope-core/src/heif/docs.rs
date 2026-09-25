//! What each part of a HEIF image is. HEIF itself (ISO/IEC 23008-12) is not
//! freely published, so its structure is cited to Nokia's public technical
//! description of it; the boxes it borrows, to ISO base media (ISO/IEC
//! 14496-12, which is free), and AVIF to its own open specification.

use crate::docs::{Concern, Doc, Table, lookup};
use crate::model::{Node, ParseTree};

const HEIF: &str = "https://nokiatech.github.io/heif/technical.html";
const BMFF: &str = "https://standards.iso.org/ittf/PubliclyAvailableStandards/";
const AVIF: &str = "https://aomediacodec.github.io/av1-avif/";

const fn heif(text: &'static str) -> Doc {
    Doc::new(text).cite("HEIF, ISO/IEC 23008-12", HEIF)
}
const fn bmff(text: &'static str) -> Doc {
    Doc::new(text).cite("ISO base media, ISO/IEC 14496-12", BMFF)
}
const fn damage(text: &'static str) -> Doc {
    Doc::new(text).concern(Concern::Damage)
}

const TABLE: Table = &[
    (
        "HEIF",
        heif(
            "A HEIF image, such as an iPhone's HEIC or an AVIF: a tree of boxes, with the picture and its metadata stored as items.",
        ),
    ),
    // Boxes.
    (
        "ftyp",
        bmff(
            "The file type: which brands of HEIF the file follows, and so which readers can open it.",
        ),
    ),
    (
        "meta",
        heif(
            "The metadata box: which items the file holds, what they are, and where their bytes are.",
        ),
    ),
    ("hdlr", bmff("The handler: says the items are pictures.")),
    (
        "dinf",
        bmff("Data information: where item data lives, which is always this same file."),
    ),
    ("dref", bmff("The list of places data can come from.")),
    ("url ", bmff("A data reference saying: in this file.")),
    (
        "pitm",
        heif("The primary item: which item is the picture a viewer shows."),
    ),
    (
        "iinf",
        heif("Item information: one entry per item, naming its type."),
    ),
    (
        "infe",
        heif(
            "One item's entry: its number and its type, such as a picture, a tile or EXIF metadata.",
        ),
    ),
    (
        "iref",
        heif(
            "Item references: which items are tiles of which picture, which is the thumbnail, which metadata describes which picture.",
        ),
    ),
    (
        "dimg",
        heif("Says a picture is built from these items, as a grid from its tiles."),
    ),
    ("thmb", heif("Says this item is a thumbnail of that one.")),
    (
        "cdsc",
        heif("Says this item, often EXIF, describes that picture."),
    ),
    (
        "auxl",
        heif("Says this item is auxiliary to that picture, such as a depth map or an alpha mask."),
    ),
    (
        "iprp",
        heif("Item properties: sizes, rotation, colour, and codec settings, shared among items."),
    ),
    (
        "ipco",
        heif("The properties themselves, listed once and referred to by number."),
    ),
    ("ipma", heif("Which properties belong to which item.")),
    ("ispe", heif("A picture's width and height in pixels.")),
    (
        "irot",
        heif("How far to turn the picture to show it upright."),
    ),
    (
        "imir",
        heif("Whether to mirror the picture before showing it."),
    ),
    (
        "colr",
        heif("The picture's colour space, or a colour profile."),
    ),
    ("pixi", heif("How many bits each colour channel has.")),
    (
        "clli",
        heif("How bright the brightest pixel is, for HDR screens."),
    ),
    (
        "hvcC",
        heif("Settings the HEVC decoder needs before it can read the picture."),
    ),
    (
        "av1C",
        Doc::new("Settings the AV1 decoder needs before it can read the picture.")
            .cite("AVIF", AVIF),
    ),
    (
        "auxC",
        heif("What kind of auxiliary picture this is, such as depth or alpha."),
    ),
    (
        "iloc",
        heif("Item locations: for every item, where its bytes are."),
    ),
    (
        "item location",
        heif("Where one item's bytes are: an offset and a length for each piece of it."),
    ),
    (
        "item * location",
        heif("Where one item's bytes are: an offset and a length for each piece of it."),
    ),
    (
        "idat",
        heif("Item data kept inside the metadata box, for small items such as a grid's layout."),
    ),
    (
        "mdat",
        bmff("The media data: the bytes of the pictures and the metadata items."),
    ),
    // Items.
    (
        "item * · hvc1*",
        heif("HEVC-compressed picture data: the photo itself, a tile of it, or its thumbnail."),
    ),
    (
        "item * · av01*",
        Doc::new("AV1-compressed picture data: the photo itself, a tile of it, or its thumbnail.")
            .cite("AVIF", AVIF),
    ),
    (
        "item * · grid*",
        heif(
            "The layout of a picture made of tiles: how many rows and columns, and the full size.",
        ),
    ),
    (
        "item * · Exif*",
        heif("EXIF metadata: the camera, the time, and often where the photo was taken."),
    ),
    (
        "item * · mime*",
        heif("Metadata in another format, usually XMP: editing history and author."),
    ),
    (
        "item * · *",
        heif("An item of a type this tool does not name."),
    ),
    // Fields.
    (
        "size",
        bmff("How many bytes this box takes, header included."),
    ),
    ("type", bmff("Four letters that say what this box is.")),
    (
        "largeSize",
        bmff("The box's size, when it is too large for four bytes."),
    ),
    (
        "version",
        bmff("Which version of this box's layout follows."),
    ),
    ("flags", bmff("Switches that change what this box holds.")),
    ("majorBrand", bmff("The main brand: heic, avif or mif1.")),
    ("minorVersion", bmff("A version number for the main brand.")),
    (
        "compatibleBrands",
        bmff("Every brand the file also follows."),
    ),
    ("preDefined", bmff("Always zero.")),
    (
        "handlerType",
        bmff("The kind of content: pict, for pictures."),
    ),
    (
        "reserved",
        bmff("Space reserved by the standard; always zero."),
    ),
    ("name", bmff("A name for the handler, for people.")),
    ("entryCount", bmff("How many entries follow.")),
    (
        "itemId",
        heif("The item's number, which other boxes refer to."),
    ),
    (
        "protectionIndex",
        heif("Whether the item is encrypted; 0 means not."),
    ),
    (
        "itemType",
        heif("What the item is: hvc1 or av01 for a picture, grid, Exif, mime."),
    ),
    ("itemName", heif("A name for the item, usually empty.")),
    (
        "contentType",
        heif("The kind of text a mime item holds; application/rdf+xml is XMP."),
    ),
    ("fromItemId", heif("The item the reference is from.")),
    ("referenceCount", heif("How many items it refers to.")),
    ("toItemId", heif("An item it refers to.")),
    ("width", heif("How many pixels wide.")),
    ("height", heif("How many pixels tall.")),
    (
        "angle",
        heif("The rotation, in quarter turns anticlockwise."),
    ),
    ("axis", heif("Which way to mirror: across or up and down.")),
    ("channels", heif("How many colour channels.")),
    ("bitsPerChannel", heif("Bits in one colour channel.")),
    (
        "colourType",
        heif("How the colour is given: nclx numbers, or an ICC profile."),
    ),
    (
        "colourData",
        heif("The colour space's numbers, or the profile."),
    ),
    (
        "associations",
        heif("For each item, the numbers of its properties."),
    ),
    (
        "offsetSize·lengthSize",
        heif("How many bytes each offset and each length takes below."),
    ),
    (
        "baseOffsetSize·indexSize",
        heif("How many bytes each base offset and each index takes below."),
    ),
    ("itemCount", heif("How many items have locations.")),
    (
        "constructionMethod",
        heif("Where the offsets count from: 0 the file, 1 the idat box."),
    ),
    (
        "dataReferenceIndex",
        heif("Which data reference holds the bytes; 0 means this file."),
    ),
    ("baseOffset", heif("Added to every offset of this item.")),
    (
        "extentCount",
        heif("How many pieces the item's bytes are in."),
    ),
    (
        "extentIndex",
        heif("Which item a piece comes from, for items built from others."),
    ),
    ("extentOffset", heif("Where a piece of the item starts.")),
    ("extentLength", heif("How long a piece of the item is.")),
    ("data", heif("The data held in the box.")),
    (
        "exifHeaderOffset",
        heif("How far past this number the TIFF header starts, skipping Exif\\0\\0."),
    ),
    (
        "identifier",
        heif("The letters Exif, then two zero bytes, as in a JPEG."),
    ),
    (
        "construction method *",
        heif("An item made from other items' bytes, which this tool does not assemble."),
    ),
    // Problems.
    (
        "a box header is cut short",
        damage("The file stops in the middle of a box's header: it was cut short."),
    ),
    (
        "a box's 64-bit size is cut short",
        damage("The file stops in the middle of a box's size."),
    ),
    (
        "* box size * is smaller than its header",
        damage("A box claims to be smaller than its own header, so these bytes are damaged."),
    ),
    (
        "* box runs past the end of its container",
        damage("A box claims more bytes than there are: the file was cut short or is damaged."),
    ),
    (
        "* box is shorter than its fields",
        damage("A box is too short to hold what its type says it holds."),
    ),
    (
        "* reference runs past the end of iref",
        damage("An item reference claims more bytes than the references box holds."),
    ),
    (
        "item *'s data lies past the end of the file",
        damage("An item is said to be where the file has no bytes, so it cannot be read."),
    ),
    (
        "the EXIF header offset points past the item",
        damage("The EXIF item's offset points outside it, so its metadata cannot be read."),
    ),
];

pub(crate) fn describe(tree: &ParseTree, node: &Node, problem: bool) -> Option<Doc> {
    let label = node.label.as_str();
    lookup(TABLE, label, problem).or_else(|| {
        // Inside the EXIF item, the EXIF tables explain the TIFF block.
        let in_exif = std::iter::successors(node.parent, |&p| tree.try_get(p)?.parent)
            .any(|p| tree.try_get(p).is_some_and(|n| n.label.contains("· Exif")));
        if in_exif {
            // A thumbnail inside it is a JPEG of its own.
            crate::exif::docs::describe(label, problem)
                .or_else(|| crate::jpeg::docs::describe(label, problem))
        } else if !problem
            && node
                .children
                .iter()
                .any(|&c| tree.try_get(c).is_some_and(|n| n.label == "type"))
        {
            Some(bmff(
                "A box this tool does not decode; readers skip boxes they do not know.",
            ))
        } else {
            None
        }
    })
}

#[cfg(test)]
pub(crate) const ALL: Table = TABLE;
