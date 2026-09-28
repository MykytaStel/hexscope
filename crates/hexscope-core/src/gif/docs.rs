//! What each part of a GIF is, cited to the GIF89a specification.

use crate::docs::{Concern, Doc, Table, lookup};

const SPEC: &str = "https://www.w3.org/Graphics/GIF/spec-gif89a.txt";

const fn gif(text: &'static str, cite: &'static str) -> Doc {
    Doc::new(text).cite(cite, SPEC)
}

const TABLE: Table = &[
    ("GIF", gif("A GIF image: a screen size and colours, then frames and the extensions between them.", "GIF89a §17")),
    ("header", gif("GIF87a or GIF89a: the six bytes every GIF begins with, and its version.", "GIF89a §17")),
    ("logical screen descriptor", gif("The size of the area the frames are drawn on, and whether a colour table follows.", "GIF89a §18")),
    ("width", gif("How many pixels wide the picture is.", "GIF89a §18")),
    ("height", gif("How many pixels tall the picture is.", "GIF89a §18")),
    ("global colour table", gif("The colours every frame without its own may use: up to 256.", "GIF89a §19")),
    ("local colour table", gif("This frame's own colours.", "GIF89a §21")),
    ("frame *", gif("One picture of the GIF: where it goes, its size, and its pixels.", "GIF89a §20")),
    ("size", gif("The frame's width and height.", "GIF89a §20")),
    ("pixels", gif("The frame's pixels, compressed with LZW, in blocks of up to 255 bytes.", "GIF89a §22")),
    ("graphic control", gif("How long the next frame shows, and how it is cleared.", "GIF89a §23")),
    ("delay", gif("How long the next frame stays on screen.", "GIF89a §23")),
    ("comment", gif("A comment: free text any program may have left, which no viewer shows.", "GIF89a §24")),
    ("text", gif("The comment's text.", "GIF89a §24")),
    ("plain text", gif("Text to draw over the picture, which viewers rarely do.", "GIF89a §25")),
    ("application · NETSCAPE*", gif("The animation's looping, as Netscape first wrote it.", "GIF89a §26")),
    ("application · ANIMEXTS*", gif("The animation's looping.", "GIF89a §26")),
    ("application · XMP*", Doc::new("XMP metadata: an XML record of the picture's history, author and edits.").cite("XMP, part 3", "https://github.com/adobe/XMP-Toolkit-SDK/blob/main/docs/XMPSpecificationPart3.pdf")),
    ("application*", gif("A program's own data, named by the program.", "GIF89a §26")),
    ("extension", gif("An extension this version of the format does not define.", "GIF89a §17")),
    ("loops", gif("How many times the animation plays; 0 is forever.", "GIF89a §26")),
    ("packet", Doc::new("The XMP record itself, as XML.")),
    ("trailer", gif("The one byte that ends the GIF.", "GIF89a §27")),
    ("the file ends before its screen size", gif("The file stops before the screen size: it was cut short.", "GIF89a §18").concern(Concern::Damage)),
    ("the file ends inside a block", gif("A block runs past the end of the file: it was cut short.", "GIF89a §15").concern(Concern::Damage)),
    ("no trailer*", gif("The file ends without the byte that closes a GIF: it was cut short, and the last frame may be missing.", "GIF89a §27").concern(Concern::Damage)),
    ("unknown block*", gif("A byte where a block should start that starts none: the file is damaged from here.", "GIF89a §17").concern(Concern::Damage)),
    ("data after the end of the image", gif("Bytes after the GIF's end: not part of the picture, and hidden from anyone viewing it.", "GIF89a §27").concern(Concern::Hidden)),
];

pub(crate) fn describe(label: &str, problem: bool) -> Option<Doc> {
    lookup(TABLE, label, problem)
}
