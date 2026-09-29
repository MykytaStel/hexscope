//! What each part of a compound file is. Citations are to Microsoft's
//! MS-CFB, the compound file format, and MS-OXMSG for Outlook's messages.
//!
//! Streams are labelled with their names, so where a node sits decides
//! what it is: a child of the root that is none of the file's own tables is
//! a stream, and a child of the directory is one of its entries.

use crate::docs::{Concern, Doc, Table, lookup};
use crate::model::{Node, ParseTree};

const CFB: &str = "https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-cfb/53989ce4-7b05-4f8d-829b-d08d6148375b";
const OXMSG: &str = "https://learn.microsoft.com/en-us/openspecs/exchange_server_protocols/ms-oxmsg/b046868c-9fbf-41ae-9ffb-8de2bd4eec82";

const fn cfb(text: &'static str, cite: &'static str) -> Doc {
    Doc::new(text).cite(cite, CFB)
}

const ROOT: Doc = cfb(
    "A compound file: a small file system inside one file, its parts kept in sectors that a table chains together. Word, Excel and PowerPoint 97–2003 files and Outlook's saved messages are compound files.",
    "MS-CFB 2",
);
const STREAM: Doc = cfb(
    "One stream, a file inside the file, in the sectors its chain in the table runs through.",
    "MS-CFB 2.6",
);
const ENTRY: Doc = cfb(
    "One entry in the directory: a stream's or a storage's name, its size, and its first sector.",
    "MS-CFB 2.6",
);

const AT_ROOT: Table = &[
    (
        "header",
        cfb(
            "The header: the format's signature and version, the sector size, and where the table, the directory and the mini stream's table start.",
            "MS-CFB 2.2",
        ),
    ),
    (
        "FAT",
        cfb(
            "The table that chains sectors: for each sector, the next one in its stream, or the end.",
            "MS-CFB 2.3",
        ),
    ),
    (
        "DIFAT",
        cfb(
            "Where the table's own sectors are, past the 109 the header lists.",
            "MS-CFB 2.5",
        ),
    ),
    (
        "directory",
        cfb(
            "The directory: every stream and storage by name, as a tree.",
            "MS-CFB 2.6",
        ),
    ),
    (
        "mini FAT",
        cfb(
            "The table that chains the mini stream's 64-byte sectors.",
            "MS-CFB 2.4",
        ),
    ),
    (
        "mini stream",
        cfb(
            "The stream that holds the small streams, those under 4 KB, in sectors of 64 bytes.",
            "MS-CFB 2.4",
        ),
    ),
    (
        "free sectors",
        cfb(
            "Sectors the table marks as free. Deleted parts of a document can still be in them.",
            "MS-CFB 2.3",
        ),
    ),
    (
        "unclaimed sectors",
        Doc::new("Sectors in use by the table's account but in no stream: nothing reads them, so they can hide anything.")
            .concern(Concern::Hidden),
    ),
    (
        "not readable: *",
        Doc::new("The compound file's header or tables could not be read, so its streams cannot be found.")
            .concern(Concern::Damage),
    ),
];

/// Streams and entries known by name.
const NAMED: Table = &[
    (
        "SummaryInformation",
        cfb(
            "The document's summary: title, author, who saved it last, the template, when it was made and saved, how long it was edited.",
            "MS-OLEPS 3.1",
        ),
    ),
    (
        "DocumentSummaryInformation",
        cfb(
            "More about the document: the company and manager, and any properties someone added.",
            "MS-OLEPS 3.2",
        ),
    ),
    (
        "WordDocument",
        cfb("The Word document itself: its text and formatting.", "MS-DOC 2.1"),
    ),
    (
        "Workbook",
        cfb("The Excel workbook itself: its sheets and cells.", "MS-XLS 2.1"),
    ),
    (
        "PowerPoint Document",
        cfb("The PowerPoint presentation itself: its slides.", "MS-PPT 2.1"),
    ),
    (
        "internet headers",
        Doc::new("The headers the message arrived with, as a mail server wrote them: who sent it, from where, and through which servers.")
            .cite("MS-OXMSG 2.1", OXMSG),
    ),
    (
        "attachment *",
        Doc::new("One attached file: its name, its type and its bytes.").cite("MS-OXMSG 2.2.2", OXMSG),
    ),
    (
        "recipient *",
        Doc::new("One recipient: their name and address.").cite("MS-OXMSG 2.2.1", OXMSG),
    ),
    (
        "properties",
        Doc::new("The message's fixed-size properties: dates, flags, sizes.").cite("MS-OXMSG 2.4", OXMSG),
    ),
];

const FIELDS: Table = &[
    (
        "signature",
        cfb(
            "The signature every compound file starts with: D0 CF 11 E0 A1 B1 1A E1.",
            "MS-CFB 2.2",
        ),
    ),
    (
        "CLSID",
        cfb("A class ID, unused: always zero.", "MS-CFB 2.2"),
    ),
    (
        "minorVersion",
        cfb("The minor version, 0x3E.", "MS-CFB 2.2"),
    ),
    (
        "majorVersion",
        cfb("3 for sectors of 512 bytes, 4 for 4096.", "MS-CFB 2.2"),
    ),
    (
        "byteOrder",
        cfb(
            "FE FF: every number in the file is little-endian.",
            "MS-CFB 2.2",
        ),
    ),
    (
        "sectorShift",
        cfb("The sector size, as a power of two: 9 or 12.", "MS-CFB 2.2"),
    ),
    (
        "miniSectorShift",
        cfb(
            "The mini stream's sector size, as a power of two: 6, for 64 bytes.",
            "MS-CFB 2.2",
        ),
    ),
    ("reserved", cfb("Reserved: zero.", "MS-CFB 2.2")),
    (
        "directorySectors",
        cfb(
            "How many sectors the directory takes; zero in version 3, which does not count them.",
            "MS-CFB 2.2",
        ),
    ),
    (
        "fatSectors",
        cfb("How many sectors the table takes.", "MS-CFB 2.2"),
    ),
    (
        "firstDirectorySector",
        cfb("Where the directory starts.", "MS-CFB 2.2"),
    ),
    (
        "transactionSignature",
        cfb("For transactions, which nothing uses: zero.", "MS-CFB 2.2"),
    ),
    (
        "miniStreamCutoff",
        cfb(
            "Streams shorter than this, 4096 bytes, are kept in the mini stream.",
            "MS-CFB 2.2",
        ),
    ),
    (
        "firstMiniFatSector",
        cfb("Where the mini stream's table starts.", "MS-CFB 2.2"),
    ),
    (
        "miniFatSectors",
        cfb(
            "How many sectors the mini stream's table takes.",
            "MS-CFB 2.2",
        ),
    ),
    (
        "firstDifatSector",
        cfb(
            "Where the list of the table's sectors goes on, when 109 are not enough.",
            "MS-CFB 2.2",
        ),
    ),
    (
        "difatSectors",
        cfb("How many sectors that list goes on for.", "MS-CFB 2.2"),
    ),
    (
        "DIFAT",
        cfb("The first 109 of the table's sectors.", "MS-CFB 2.2"),
    ),
    ("name", cfb("The entry's name, in UTF-16.", "MS-CFB 2.6.1")),
    (
        "type",
        cfb(
            "1 for a storage, 2 for a stream, 5 for the root.",
            "MS-CFB 2.6.1",
        ),
    ),
    (
        "startSector",
        cfb(
            "The stream's first sector; the table gives the next.",
            "MS-CFB 2.6.3",
        ),
    ),
    ("size", cfb("The stream's length in bytes.", "MS-CFB 2.6.3")),
];

pub(crate) fn describe(tree: &ParseTree, node: &Node, problem: bool) -> Option<Doc> {
    let label = node.label.as_str();
    let Some(parent) = node.parent.and_then(|p| tree.try_get(p)) else {
        return (!problem).then_some(ROOT);
    };
    let own = label
        .rfind('›')
        .map_or(label, |i| label[i + '›'.len_utf8()..].trim_start());
    if parent.parent.is_none() {
        return lookup(AT_ROOT, label, problem)
            .or_else(|| lookup(NAMED, own, problem))
            .or((!problem).then_some(STREAM));
    }
    if parent.label == "directory" {
        return lookup(NAMED, own, problem).or((!problem).then_some(ENTRY));
    }
    lookup(FIELDS, label, problem)
        .or_else(|| lookup(NAMED, own, problem))
        .or_else(|| (parent.label == "mini stream" && !problem).then_some(STREAM))
}
