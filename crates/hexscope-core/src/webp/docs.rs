//! What each part of a WebP is, cited to Google's container specification.

use crate::docs::{Concern, Doc, Table, lookup};

const SPEC: &str = "https://developers.google.com/speed/webp/docs/riff_container";

const fn webp(text: &'static str) -> Doc {
    Doc::new(text).cite("WebP container", SPEC)
}

const TABLE: Table = &[
    ("WebP", webp("A WebP image: a RIFF file of chunks, the picture in one of them and, around it, what the extended header lists.")),
    ("RIFF header", webp("RIFF, the file's size, and WEBP: the twelve bytes every WebP begins with.")),
    ("size", webp("How many bytes follow, not counting padding to an even length.")),
    ("VP8X", webp("The extended header: the canvas size, and which of a colour profile, transparency, EXIF, XMP and animation the file has.")),
    ("flags", webp("Which extras the file says it carries.")),
    ("canvas width", webp("How many pixels wide the picture is.")),
    ("canvas height", webp("How many pixels tall the picture is.")),
    ("VP8", webp("The picture, compressed with loss the way VP8 video frames are.")),
    ("VP8L", webp("The picture, compressed without loss.")),
    ("picture", webp("The compressed picture itself.")),
    ("ALPH", webp("Transparency for a lossy picture, kept apart from its colours.")),
    ("transparency", webp("The compressed transparency.")),
    ("ANIM", webp("Animation settings: the background colour and how many times it plays.")),
    ("loops", webp("How many times the animation plays; 0 is forever.")),
    ("ANMF", webp("One frame of the animation, with where it goes and how long it shows.")),
    ("frame", webp("The frame's own picture chunks.")),
    ("ICCP", webp("A colour profile: how the picture's colours should look on any screen.")),
    ("profile", webp("The colour profile's bytes.")),
    ("EXIF", Doc::new("EXIF metadata: the camera, the time, and often where the picture was taken.").cite("WebP container: EXIF", SPEC)),
    ("XMP", Doc::new("XMP metadata: an XML record of the picture's history, author and edits.").cite("WebP container: XMP", SPEC)),
    ("packet", Doc::new("The XMP record itself, as XML.")),
    ("chunk*", webp("A chunk the WebP format does not define.")),
    ("data", webp("The chunk's contents.")),
    ("the file is shorter than its header says", webp("The RIFF header counts more bytes than the file has: it was cut short.").concern(Concern::Damage)),
    ("chunk runs past the end of the file", webp("A chunk says it is longer than what is left of the file: the file was cut short.").concern(Concern::Damage)),
    ("data after the end of the image", webp("Bytes after the RIFF file ends: not part of the picture, and hidden from anyone viewing it.").concern(Concern::Hidden)),
];

pub(crate) fn describe(label: &str, problem: bool) -> Option<Doc> {
    // EXIF's tables, and a thumbnail inside it, a JPEG of its own.
    lookup(TABLE, label, problem)
        .or_else(|| crate::exif::docs::describe(label, problem))
        .or_else(|| crate::jpeg::docs::describe(label, problem))
}
