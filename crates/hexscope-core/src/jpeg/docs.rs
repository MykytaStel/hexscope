//! What each part of a JPEG is. Markers are cited to ITU T.81, the JPEG
//! standard; JFIF to its own specification; EXIF to [`crate::exif::docs`].

use crate::docs::{Concern, Doc, Table, lookup};

const T81: &str = "https://www.w3.org/Graphics/JPEG/itu-t81.pdf";
const JFIF: &str = "https://www.w3.org/Graphics/JPEG/jfif3.pdf";

const fn t81(text: &'static str, cite: &'static str) -> Doc {
    Doc::new(text).cite(cite, T81)
}
const fn jfif(text: &'static str) -> Doc {
    Doc::new(text).cite("JFIF 1.02", JFIF)
}
const fn damage(text: &'static str) -> Doc {
    Doc::new(text).concern(Concern::Damage)
}

const TABLE: Table = &[
    ("JPEG", t81("A JPEG image: a run of segments, each opened by a two-byte marker, around the compressed picture.", "T.81 §B.2.1")),
    ("SOI", t81("Start of image: the two bytes every JPEG begins with.", "T.81 §B.1.1.3")),
    ("EOI", t81("End of image: the two bytes that close the picture. Anything after them is not part of it.", "T.81 §B.1.1.3")),
    ("SOF*", t81("The frame header: the picture's size, precision and colour components. Its number says how it was compressed.", "T.81 §B.2.2")),
    ("DHT", t81("Huffman tables: the codes the compressed picture is written in.", "T.81 §B.2.4.2")),
    ("DQT", t81("Quantization tables: how much detail was thrown away, which sets the JPEG's quality.", "T.81 §B.2.4.1")),
    ("DRI", t81("Restart interval: how often the compressed data has restart markers, so damage does not spread.", "T.81 §B.2.4.4")),
    ("DNL", t81("Number of lines: the picture's height, given after the data when it was not known before.", "T.81 §B.2.5")),
    ("DAC", t81("Arithmetic coding conditions, for the rare JPEGs not compressed with Huffman codes.", "T.81 §B.2.4.3")),
    ("SOS", t81("Start of scan: which components the compressed data that follows holds, and with which tables.", "T.81 §B.2.3")),
    ("scan data", t81("The compressed picture itself.", "T.81 §B.1.1.5")),
    ("RST*", t81("A restart marker inside the compressed data: decoding can pick up again here after damage.", "T.81 §B.1.1.3")),
    ("COM", t81("A comment: free text that any program may have left.", "T.81 §B.2.4.5")),
    ("TEM", t81("A marker reserved for temporary use; real files do not contain it.", "T.81 §B.1.1.3")),
    ("JPG", t81("A marker reserved for JPEG extensions.", "T.81 §B.1.1.3")),
    ("APP* · JFIF", jfif("The JFIF header: the version and the pixel density, for printing.")),
    ("APP* · JFXX", jfif("A JFIF extension, usually a thumbnail.")),
    ("APP* · EXIF", Doc::new("EXIF metadata: the camera, its settings, the time, and often where the picture was taken.").cite("EXIF 2.32, file structure", "https://www.cipa.jp/std/documents/e/DC-X008-Translation-2019-E.pdf")),
    ("APP* · XMP", Doc::new("XMP metadata: Adobe's XML record of the picture's history, author and edits.").cite("XMP, part 3", "https://www.adobe.com/devnet/xmp.html")),
    ("APP* · ICC", t81("A colour profile: how the picture's colours should look on any screen.", "T.81 §B.2.4.6")),
    ("APP* · MPF", t81("Multi-picture format: this file holds more pictures, such as a depth map or a second exposure.", "T.81 §B.2.4.6")),
    ("APP* · Photoshop", t81("Photoshop's data: captions, keywords and settings it saved.", "T.81 §B.2.4.6")),
    ("APP* · Adobe", t81("Adobe's note on how the colours were transformed.", "T.81 §B.2.4.6")),
    ("APP*", t81("Application data: a segment programs use for their own information.", "T.81 §B.2.4.6")),
    ("marker 0x*", t81("A marker the JPEG standard does not assign.", "T.81 §B.1.1.3")),
    ("marker", t81("The two bytes, FF and a code, that say what this segment is.", "T.81 §B.1.1.2")),
    ("length", t81("How long the segment is, counting these two bytes but not the marker.", "T.81 §B.1.1.4")),
    ("precision", t81("How many bits each sample has; 8 in almost every JPEG.", "T.81 §B.2.2")),
    ("height", t81("How many pixels tall the picture is.", "T.81 §B.2.2")),
    ("width", t81("How many pixels wide the picture is.", "T.81 §B.2.2")),
    ("components", t81("How many colour components: 1 for greyscale, 3 for colour.", "T.81 §B.2.2")),
    ("restartInterval", t81("How many blocks of the picture lie between restart markers.", "T.81 §B.2.4.4")),
    ("identifier", Doc::new("The name that says what kind of data this segment carries.")),
    ("version", jfif("The JFIF version.")),
    ("units", jfif("The unit of the density: none, dots per inch or dots per centimetre.")),
    ("xDensity", jfif("Pixels per unit across, for printing.")),
    ("yDensity", jfif("Pixels per unit down, for printing.")),
    ("packet", Doc::new("The XMP record itself, as XML.").cite("XMP, part 3", "https://www.adobe.com/devnet/xmp.html")),
    ("comment", t81("The comment's text.", "T.81 §B.2.4.5")),
    // Problems.
    ("not a JPEG*", damage("The file does not start with the two bytes every JPEG must: it may be another format under a .jpg name.")),
    ("no EOI marker*", damage("The picture never ends: the file was cut short, so the bottom of the picture may be missing.")),
    ("the file ends inside a marker", damage("The file stops in the middle of a marker: it was cut short.")),
    ("expected a marker, found*", damage("Where a segment should start there is something else: these bytes are damaged.")),
    ("* segment length * is less than 2", damage("A segment's length is impossible, since it must count its own two bytes.")),
    ("* segment length * is wrong*", damage("A segment's length does not lead to the next segment, so reading skips to the next one it can find.")),
    ("* segment runs past the end of the file", damage("A segment claims more bytes than the file has left: it was cut short.")),
    ("* segment truncated before its length", damage("The file stops before a segment's length.")),
    ("SOF segment truncated", damage("The frame header is too short to hold the picture's size.")),
    ("data after the end of the image", Doc::new("Bytes after the picture ends. Phones keep extra pictures here, and files made to be two formats at once hide data here too.").concern(Concern::Hidden)),
];

#[cfg(test)]
pub(crate) const ALL: Table = TABLE;

pub(crate) fn describe(label: &str, problem: bool) -> Option<Doc> {
    lookup(TABLE, label, problem).or_else(|| crate::exif::docs::describe(label, problem))
}
