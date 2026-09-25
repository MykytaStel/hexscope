//! What each part of a PDF is, cited to ISO 32000-1:2008 by section, in the
//! copy Adobe publishes for free.

use crate::docs::{Concern, Doc, Table, lookup};
use crate::model::{Node, ParseTree};

const ISO: &str = "https://opensource.adobe.com/dc-acrobat-sdk-docs/pdfstandards/PDF32000_2008.pdf";

const fn iso(cite: &'static str, text: &'static str) -> Doc {
    Doc::new(text).cite(cite, ISO)
}
const fn damage(text: &'static str) -> Doc {
    Doc::new(text).concern(Concern::Damage)
}
const fn hidden(cite: &'static str, text: &'static str) -> Doc {
    iso(cite, text).concern(Concern::Hidden)
}

const TABLE: Table = &[
    (
        "PDF",
        iso(
            "ISO 32000-1 §7.5",
            "A PDF document: numbered objects, a table saying where each one is, and a trailer that says where to start.",
        ),
    ),
    // The file's structure.
    (
        "header",
        iso("ISO 32000-1 §7.5.2", "Says this is a PDF, and which version of the format it follows."),
    ),
    (
        "binary marker",
        iso(
            "ISO 32000-1 §7.5.2",
            "A comment of high bytes, so programs that copy files treat this one as binary, not text.",
        ),
    ),
    (
        "revision 1",
        iso(
            "ISO 32000-1 §7.5.6",
            "The document as it was first saved. Later revisions add to it rather than replace it, so all of it is still here.",
        ),
    ),
    (
        "revision *",
        iso(
            "ISO 32000-1 §7.5.6",
            "An incremental update: the objects that changed, appended after the earlier version, which stays in the file.",
        ),
    ),
    (
        "object * *",
        iso(
            "ISO 32000-1 §7.3.10",
            "An indirect object: a numbered value other parts of the document refer to — a page, a font, an image, some text.",
        ),
    ),
    (
        "value",
        iso("ISO 32000-1 §7.3", "The object's value, when it is not a dictionary."),
    ),
    (
        "stream data",
        iso(
            "ISO 32000-1 §7.3.8",
            "The stream's bytes: page drawing instructions, a font, an image or metadata, often compressed.",
        ),
    ),
    (
        "cross-reference table",
        iso(
            "ISO 32000-1 §7.5.4",
            "Where each object starts, by number, so a reader can jump to any of them.",
        ),
    ),
    (
        "object *",
        iso("ISO 32000-1 §7.5.4", "A run of entries in the table, for consecutive object numbers."),
    ),
    (
        "objects *",
        iso("ISO 32000-1 §7.5.4", "A run of entries in the table, for consecutive object numbers."),
    ),
    (
        "entry *",
        iso(
            "ISO 32000-1 §7.5.4",
            "Where one object starts, as a byte offset, or that its number is free.",
        ),
    ),
    (
        "trailer",
        iso(
            "ISO 32000-1 §7.5.5",
            "Where reading starts: the catalog, the document information, and the earlier table if there is one.",
        ),
    ),
    (
        "startxref",
        iso(
            "ISO 32000-1 §7.5.5",
            "Where the last cross-reference section is. Readers start at the end of the file and jump here.",
        ),
    ),
    (
        "offset",
        iso("ISO 32000-1 §7.5.5", "The byte offset of the cross-reference section."),
    ),
    (
        "%%EOF",
        iso("ISO 32000-1 §7.5.5", "The end of the file, or of one revision of it."),
    ),
    (
        "item *",
        iso("ISO 32000-1 §7.3.6", "One element of the array: a dictionary of its own."),
    ),
    // Keys that say what a dictionary is and how it is stored.
    ("/Type", iso("ISO 32000-1 §7.3.7", "What this dictionary is: a page, a font, a catalog.")),
    ("/Subtype", iso("ISO 32000-1 §7.3.7", "Which kind of that type it is.")),
    ("/Length", iso("ISO 32000-1 §7.3.8.2", "How many bytes of data the stream holds.")),
    (
        "/Filter",
        iso(
            "ISO 32000-1 §7.4",
            "How the stream is compressed or encoded; FlateDecode is the same DEFLATE as ZIP and PNG.",
        ),
    ),
    (
        "/DecodeParms",
        iso("ISO 32000-1 §7.4", "Settings for undoing the filter, such as a PNG predictor."),
    ),
    // The trailer.
    ("/Size", iso("ISO 32000-1 §7.5.5", "One more than the highest object number.")),
    ("/Root", iso("ISO 32000-1 §7.5.5", "The catalog: the object everything else hangs from.")),
    (
        "/Info",
        iso(
            "ISO 32000-1 §7.5.5",
            "The document information: title, author, the program that made it, dates.",
        ),
    ),
    (
        "/Prev",
        iso(
            "ISO 32000-1 §7.5.6",
            "Where the previous cross-reference section is: this file was updated after it was first saved.",
        ),
    ),
    (
        "/ID",
        iso(
            "ISO 32000-1 §14.4",
            "Two identifiers: one fixed when the file was first made, one changed on every save. The same first ID in two files means one came from the other.",
        ),
    ),
    (
        "/Encrypt",
        iso("ISO 32000-1 §7.6", "How the document is encrypted; its strings and streams cannot be read without it."),
    ),
    (
        "/XRefStm",
        iso("ISO 32000-1 §7.5.8.4", "Where a cross-reference stream is, for readers that know them."),
    ),
    // Cross-reference and object streams.
    (
        "/W",
        iso("ISO 32000-1 §7.5.8.2", "How many bytes each field of an entry takes in the stream."),
    ),
    (
        "/Index",
        iso("ISO 32000-1 §7.5.8.2", "Which object numbers the stream's entries are for."),
    ),
    ("/N", iso("ISO 32000-1 §7.5.7", "How many objects the object stream holds.")),
    (
        "/First",
        iso("ISO 32000-1 §7.5.7", "Where, in the decompressed stream, the first object starts."),
    ),
    (
        "/Linearized",
        iso(
            "ISO 32000-1 Annex F",
            "The file is arranged so the first page can show before the rest arrives.",
        ),
    ),
    // The catalog and pages.
    ("/Pages", iso("ISO 32000-1 §7.7.2", "The root of the page tree.")),
    (
        "/Metadata",
        iso("ISO 32000-1 §14.3.2", "The XMP metadata stream: author, programs and editing history."),
    ),
    ("/Lang", iso("ISO 32000-1 §14.9.2", "The language the text is in.")),
    ("/Outlines", iso("ISO 32000-1 §12.3.3", "The bookmarks.")),
    (
        "/Names",
        iso("ISO 32000-1 §7.7.4", "Named things: destinations, attached files, scripts."),
    ),
    ("/AcroForm", iso("ISO 32000-1 §12.7.2", "The form fields someone can fill in.")),
    (
        "/OpenAction",
        iso("ISO 32000-1 §7.7.2", "What happens when the document opens: a page to show, or an action to run."),
    ),
    (
        "/AA",
        iso("ISO 32000-1 §12.6.3", "Actions that run on events: opening, closing, printing, clicking."),
    ),
    ("/ViewerPreferences", iso("ISO 32000-1 §12.2", "How a viewer should show the document.")),
    ("/PageLayout", iso("ISO 32000-1 §7.7.2", "How pages are laid out when the document opens.")),
    ("/PageMode", iso("ISO 32000-1 §7.7.2", "Which panel is open when the document opens.")),
    ("/MarkInfo", iso("ISO 32000-1 §14.7", "Whether the document is tagged for accessibility.")),
    (
        "/StructTreeRoot",
        iso("ISO 32000-1 §14.7.2", "The document's logical structure, for screen readers."),
    ),
    ("/Kids", iso("ISO 32000-1 §7.7.3.2", "The pages, or groups of pages, below this one.")),
    ("/Count", iso("ISO 32000-1 §7.7.3.2", "How many pages are below this node.")),
    ("/Parent", iso("ISO 32000-1 §7.7.3", "The node above this one in the page tree.")),
    ("/MediaBox", iso("ISO 32000-1 §14.11.2", "The page size, in points: 72 to the inch.")),
    ("/CropBox", iso("ISO 32000-1 §14.11.2", "The part of the page that is shown.")),
    ("/Rotate", iso("ISO 32000-1 §7.7.3.3", "How far to turn the page to show it upright.")),
    (
        "/Resources",
        iso("ISO 32000-1 §7.8.3", "The fonts, images and colours the page's drawing refers to."),
    ),
    ("/Contents", iso("ISO 32000-1 §7.7.3.3", "The page's drawing instructions.")),
    ("/Annots", iso("ISO 32000-1 §12.5", "Comments, links and form fields on the page.")),
    ("/Font", iso("ISO 32000-1 §7.8.3", "The fonts the drawing refers to, by name.")),
    ("/XObject", iso("ISO 32000-1 §7.8.3", "Images and reusable drawings, by name.")),
    ("/ExtGState", iso("ISO 32000-1 §7.8.3", "Drawing settings such as transparency, by name.")),
    ("/ColorSpace", iso("ISO 32000-1 §8.6", "Which colour space the colours are in.")),
    ("/ProcSet", iso("ISO 32000-1 §14.2", "An obsolete list of what the drawing needs; ignored.")),
    // Fonts and images.
    ("/BaseFont", iso("ISO 32000-1 §9.6", "The font's name, such as Helvetica.")),
    ("/Encoding", iso("ISO 32000-1 §9.6", "How character codes map to the font's glyphs.")),
    (
        "/FontDescriptor",
        iso("ISO 32000-1 §9.8", "The font's measurements, and the font program if it is embedded."),
    ),
    ("/Width", iso("ISO 32000-1 §8.9.5", "How many pixels wide the image is.")),
    ("/Height", iso("ISO 32000-1 §8.9.5", "How many pixels tall the image is.")),
    ("/BitsPerComponent", iso("ISO 32000-1 §8.9.5", "Bits in one colour channel of a pixel.")),
    // The document information dictionary.
    ("/Title", iso("ISO 32000-1 §14.3.3", "The document's title.")),
    ("/Author", iso("ISO 32000-1 §14.3.3", "Who wrote the document, as the program recorded it.")),
    ("/Subject", iso("ISO 32000-1 §14.3.3", "What the document is about.")),
    ("/Keywords", iso("ISO 32000-1 §14.3.3", "Keywords someone gave the document.")),
    (
        "/Creator",
        iso("ISO 32000-1 §14.3.3", "The program the document was written in, before it became a PDF."),
    ),
    ("/Producer", iso("ISO 32000-1 §14.3.3", "The program that made the PDF.")),
    ("/CreationDate", iso("ISO 32000-1 §14.3.3", "When the document was made, with its time zone.")),
    ("/ModDate", iso("ISO 32000-1 §14.3.3", "When the document was last changed.")),
    ("/Trapped", iso("ISO 32000-1 §14.3.3", "Whether it was prepared for commercial printing.")),
    // What runs or hides.
    ("/JS", iso("ISO 32000-1 §12.6.4.16", "JavaScript for a viewer to run.")),
    ("/JavaScript", iso("ISO 32000-1 §7.7.4", "The document's scripts, by name.")),
    ("/S", iso("ISO 32000-1 §12.6.2", "What kind of action this is.")),
    ("/URI", iso("ISO 32000-1 §12.6.4.7", "A web address a link opens.")),
    (
        "/EmbeddedFiles",
        iso("ISO 32000-1 §7.11.4", "Files attached to the document, by name."),
    ),
    ("/EF", iso("ISO 32000-1 §7.11.3", "The attached file's data.")),
    ("/F", iso("ISO 32000-1 §7.11.3", "A file's name.")),
    ("/UF", iso("ISO 32000-1 §7.11.3", "A file's name, in Unicode.")),
    (
        "/*",
        iso("ISO 32000-1 §7.3.7", "A key of this dictionary; its name says what it sets."),
    ),
    // Problems.
    (
        "data before the PDF header",
        hidden(
            "ISO 32000-1 §7.5.2",
            "Bytes before the header: readers skip them, so the file can be another format as well as a PDF.",
        ),
    ),
    (
        "data after the end of the document",
        hidden(
            "ISO 32000-1 §7.5.5",
            "Bytes after the last %%EOF that are not an update: readers ignore them, so anything can be kept there.",
        ),
    ),
    (
        "bytes that are not part of any object",
        damage("Bytes this reader could not make into an object: damage, or a part of the file overwritten."),
    ),
    (
        "the file ends without %%EOF: it was cut short",
        damage("The file stops before its last revision ends: it was cut short, and the end is missing."),
    ),
    (
        "object * * cannot be read",
        damage("The object's value is broken: a bracket, string or dictionary that never closes."),
    ),
    (
        "object * * has no endobj",
        Doc::new("The object is not closed with endobj. Readers usually cope.")
            .cite("ISO 32000-1 §7.3.10", ISO)
            .concern(Concern::Oddity),
    ),
    (
        "the stream has no endstream: the file was cut short",
        damage("The stream never ends: the file was cut short in the middle of it."),
    ),
    (
        "the stream's /Length says * bytes, but it holds *",
        Doc::new("The stream is not as long as its dictionary says. Readers usually find the real end.")
            .cite("ISO 32000-1 §7.3.8.2", ISO)
            .concern(Concern::Oddity),
    ),
    (
        "cross-reference entry * cannot be read",
        damage("An entry of the cross-reference table is broken; readers must rebuild the table."),
    ),
    (
        "the trailer cannot be read",
        damage("The trailer is broken, so readers must guess where the document starts."),
    ),
    (
        "startxref has no offset",
        damage("startxref is not followed by a number, so readers cannot find the table."),
    ),
    (
        "startxref points to *, where no cross-reference section starts",
        Doc::new("startxref points to the wrong place. Readers usually rebuild the table by scanning the file.")
            .cite("ISO 32000-1 §7.5.5", ISO)
            .concern(Concern::Oddity),
    ),
    (
        "the document runs JavaScript",
        hidden(
            "ISO 32000-1 §12.6.4.16",
            "A script a viewer runs, when the document opens or on some event. Forms use it; so does malware.",
        ),
    ),
    (
        "an action that starts a program",
        hidden(
            "ISO 32000-1 §12.6.4.5",
            "A launch action: it asks the viewer to open another file or start a program.",
        ),
    ),
    (
        "a file carried inside the document",
        hidden(
            "ISO 32000-1 §7.11.4",
            "An attached file. It can be anything, and a viewer may offer to open it.",
        ),
    ),
];

pub(crate) fn describe(_tree: &ParseTree, node: &Node, problem: bool) -> Option<Doc> {
    lookup(TABLE, &node.label, problem)
}

#[cfg(test)]
pub(crate) const ALL: Table = TABLE;
