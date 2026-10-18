//! What each part of an SVG drawing is, cited to the SVG 2 specification and
//! to Inkscape's description of its own additions.

use crate::docs::{Concern, Doc, Table, lookup};

const SVG: &str = "https://www.w3.org/TR/SVG2/";
const STRUCT: &str = "https://www.w3.org/TR/SVG2/struct.html";
const INTERACT: &str = "https://www.w3.org/TR/SVG2/interact.html";
const EMBEDDED: &str = "https://www.w3.org/TR/SVG2/embedded.html";
const XML: &str = "https://www.w3.org/TR/xml/#sec-prolog-dtd";
const INKSCAPE: &str = "https://wiki.inkscape.org/wiki/Inkscape_SVG_vs._plain_SVG";
const XMP: &str = "https://developer.adobe.com/xmp/docs/XMPNamespaces/xmp/";

const TABLE: Table = &[
    ("SVG", Doc::new("An SVG drawing: shapes, text and pictures written as XML, which a browser draws at any size.").cite("SVG 2", SVG)),
    ("XML declaration", Doc::new("The line that says the file is XML, and in which version and character encoding.").cite("XML 1.0, Prolog", XML)),
    ("processing instruction", Doc::new("An instruction for a program that reads the file, such as which stylesheet to use.").cite("XML 1.0, Prolog", XML)),
    ("DOCTYPE", Doc::new("The document type: which kind of XML this is, sometimes with names it defines for itself, as Illustrator writes them.").cite("XML 1.0, Prolog", XML)),
    ("comment", Doc::new("A comment: text a browser does not show. Illustrator and Sketch write one naming themselves and their version.").cite("XML 1.0, Comments", "https://www.w3.org/TR/xml/#sec-comments")),
    ("svg element", Doc::new("The drawing itself: everything a browser draws, with the attributes of the drawing as a whole.").cite("SVG 2, The svg element", STRUCT)),
    ("metadata", Doc::new("Facts about the drawing that are not drawn: who made it, its title, dates and keywords, written as RDF or XMP.").cite("SVG 2, The metadata element", STRUCT)),
    ("sodipodi:namedview", Doc::new("Inkscape's settings for the drawing: its pages, guides, grid, zoom, and the size and place of the window it was last edited in.").cite("Inkscape SVG and plain SVG", INKSCAPE)),
    ("Illustrator data", Doc::new("Illustrator's own copy of the drawing, kept so Illustrator can open it again with everything it had: it can hold layers, hidden objects and artboards the drawing does not show.")),
    ("script", Doc::new("A script, which runs when the drawing is opened on its own in a browser or put in a page as an object — not when a page shows it as a picture.").cite("SVG 2, The script element", INTERACT)),
    ("event handler", Doc::new("A script in an attribute, run when something happens to the drawing: when it loads, or someone clicks it.").cite("SVG 2, Event attributes", INTERACT)),
    ("image", Doc::new("A picture placed in the drawing: linked by its address or path, or carried in the drawing itself as base64.").cite("SVG 2, The image element", EMBEDDED)),
    ("embedded picture", Doc::new("A photo carried in the drawing as base64. It keeps its own metadata — where it was taken, the camera — as it had in its own file.").cite("SVG 2, The image element", EMBEDDED)),
    ("sodipodi:docname", Doc::new("The name of the file the drawing was saved as in Inkscape.").cite("Inkscape SVG and plain SVG", INKSCAPE)),
    ("sodipodi:absref", Doc::new("Where a linked picture was on the author's computer, with the whole path.").cite("Inkscape SVG and plain SVG", INKSCAPE)),
    ("inkscape:export-filename", Doc::new("Where Inkscape last exported the drawing as a picture: older versions keep the whole path, with the user's folder.").cite("Inkscape SVG and plain SVG", INKSCAPE)),
    ("inkscape:version", Doc::new("The version of Inkscape that saved the drawing.").cite("Inkscape SVG and plain SVG", INKSCAPE)),
    ("CreatorTool", Doc::new("The program the drawing was made with, as XMP names it.").cite("XMP namespace", XMP)),
    ("CreateDate", Doc::new("When the drawing was made, as XMP records it.").cite("XMP namespace", XMP)),
    ("ModifyDate", Doc::new("When the drawing was last changed, as XMP records it.").cite("XMP namespace", XMP)),
    ("the tags do not close*", Doc::new("An element is opened and never closed, or closed out of order: the drawing is cut short or damaged.").concern(Concern::Damage)),
];

pub(crate) fn describe(label: &str, problem: bool) -> Option<Doc> {
    lookup(TABLE, label, problem)
}

/// Every explanation here, for the page's copy of the sentences ([`crate::docs::every`]).
pub(crate) const EVERY: &[Table] = &[TABLE];
