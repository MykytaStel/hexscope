//! What each part of an RTF document is, cited to the RTF 1.5 specification.

use crate::docs::{Concern, Doc, Table, lookup};

const RTF: &str = "https://www.biblioscape.com/rtf15_spec.htm";

const fn spec(text: &'static str, cite: &'static str) -> Doc {
    Doc::new(text).cite(cite, RTF)
}

const TABLE: Table = &[
    ("RTF", spec("A Rich Text Format document: groups in braces, control words after backslashes, and the text between them.", "RTF 1.5, Basic Format")),
    ("text", spec("The document's text, with the control words that format it.", "RTF 1.5, Document Area")),
    ("font table", spec("The fonts the document uses, each with a number the text refers to.", "RTF 1.5, Font Table")),
    ("colour table", spec("The colours the document uses, each with a number the text refers to.", "RTF 1.5, Color Table")),
    ("stylesheet", spec("The document's named styles: headings, body text and the like.", "RTF 1.5, Style Sheet")),
    ("info", spec("The document's information: its title, author, who saved it last, company, manager, dates and editing time.", "RTF 1.5, Information Group")),
    ("title", spec("The document's title, as set in its properties.", "RTF 1.5, Information Group")),
    ("subject", spec("The document's subject, as set in its properties.", "RTF 1.5, Information Group")),
    ("author", spec("Who made the document: the name the program had for its user.", "RTF 1.5, Information Group")),
    ("operator", spec("Who made the last changes to the document.", "RTF 1.5, Information Group")),
    ("manager", spec("The author's manager, as set in the document's properties.", "RTF 1.5, Information Group")),
    ("company", spec("The author's company, as set in the document's properties.", "RTF 1.5, Information Group")),
    ("category", spec("The document's category, as set in its properties.", "RTF 1.5, Information Group")),
    ("keywords", spec("Keywords set in the document's properties.", "RTF 1.5, Information Group")),
    ("note", spec("A note set in the document's properties, not shown in its text.", "RTF 1.5, Information Group")),
    ("document comment", spec("A note set in the document's properties, not shown in its text.", "RTF 1.5, Information Group")),
    ("hyperlink base", spec("The address relative links in the document start from.", "RTF 1.5, Information Group")),
    ("created", spec("When the document was created.", "RTF 1.5, Information Group")),
    ("revised", spec("When the document was last saved.", "RTF 1.5, Information Group")),
    ("printed", spec("When the document was last printed.", "RTF 1.5, Information Group")),
    ("backed up", spec("When the document was last backed up.", "RTF 1.5, Information Group")),
    ("version", spec("How many times the document has been saved.", "RTF 1.5, Information Group")),
    ("editing minutes", spec("How many minutes the document has been open for editing in all.", "RTF 1.5, Information Group")),
    ("pages", spec("How many pages the document had when it was saved.", "RTF 1.5, Information Group")),
    ("words", spec("How many words the document had when it was saved.", "RTF 1.5, Information Group")),
    ("characters", spec("How many characters the document had when it was saved.", "RTF 1.5, Information Group")),
    ("characters with spaces", spec("How many characters, spaces included, the document had when it was saved.", "RTF 1.5, Information Group")),
    ("program version", spec("The version of the program that saved the document.", "RTF 1.5, Information Group")),
    ("id", spec("A number the program gave the document.", "RTF 1.5, Information Group")),
    ("info item", spec("Another item of the document's information.", "RTF 1.5, Information Group")),
    ("generator", spec("The program that wrote the document, and its version.", "RTF 1.5, Generator")),
    ("template", spec("The template the document came from, with where it was on the author's computer.", "RTF 1.5, Document Formatting Properties")),
    ("user properties", spec("Properties the author added to the document, by name and value.", "RTF 1.5, User Properties")),
    ("revision table", spec("The names of everyone whose tracked changes are in the document.", "RTF 1.5, Revision Marks")),
    ("rsid table", Doc::new("Numbers Word gives each editing session: copies of one document share them, which ties the copies together.")),
    ("data store", Doc::new("Data the program that wrote the document keeps beside it, such as its own properties.")),
    ("comment", spec("A comment on the text, with who wrote it and when.", "RTF 1.5, Annotations")),
    ("comment author", spec("Who wrote a comment.", "RTF 1.5, Annotations")),
    ("comment initials", spec("The initials of who wrote a comment.", "RTF 1.5, Annotations")),
    ("the braces do not balance*", Doc::new("A group opened with a brace is never closed, or one is closed that was never opened: the document is cut short or damaged.").concern(Concern::Damage)),
];

#[cfg(test)]
pub(crate) const ALL: Table = TABLE;

pub(crate) fn describe(label: &str, problem: bool) -> Option<Doc> {
    lookup(TABLE, label, problem)
}
