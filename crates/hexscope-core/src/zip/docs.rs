//! What each part of a ZIP archive is. Citations are to PKWARE's APPNOTE,
//! the ZIP specification, by section number.
//!
//! Entries and central directory records are labelled with file names, so
//! where a node sits decides what it is: a child of the root that is none of
//! the archive's own records is an entry, and a child of the central
//! directory is a record.

use crate::docs::{Concern, Doc, Table, lookup};
use crate::model::{Node, ParseTree};

const APPNOTE: &str = "https://pkware.cachefly.net/webdocs/casestudies/APPNOTE.TXT";

const fn zip(text: &'static str, cite: &'static str) -> Doc {
    Doc::new(text).cite(cite, APPNOTE)
}
const fn damage(text: &'static str) -> Doc {
    Doc::new(text).concern(Concern::Damage)
}
const fn hidden(text: &'static str) -> Doc {
    Doc::new(text).concern(Concern::Hidden)
}
const fn oddity(text: &'static str) -> Doc {
    Doc::new(text).concern(Concern::Oddity)
}

const ROOT: Doc = zip(
    "A ZIP archive: files stored one after another, then a directory at the end that lists them all. Word documents, spreadsheets, Android apps and e-books are ZIPs too.",
    "APPNOTE 4.3.6",
);
const ENTRY: Doc = zip(
    "One file in the archive: a local header with its name, then its data, compressed or as it is.",
    "APPNOTE 4.3.7",
);
const RECORD: Doc = zip(
    "This file's line in the directory: its name, sizes, checksum, and where its local header is.",
    "APPNOTE 4.3.12",
);

/// Direct children of the root other than entries.
const AT_ROOT: Table = &[
    (
        "central directory",
        zip(
            "The directory at the end that lists every file, and where to find it. Unzip tools trust this, not the order of the files.",
            "APPNOTE 4.3.12",
        ),
    ),
    (
        "end of central directory",
        zip(
            "The last record: how many files there are and where the directory starts. Readers look for it first.",
            "APPNOTE 4.3.16",
        ),
    ),
    (
        "ZIP64 end of central directory",
        zip(
            "The larger end record ZIP64 adds, for archives with more files or bytes than the original fields can count.",
            "APPNOTE 4.3.14",
        ),
    ),
    (
        "ZIP64 locator",
        zip("Where the ZIP64 end record is.", "APPNOTE 4.3.15"),
    ),
    (
        "spanned over several disks*",
        zip(
            "The archive was split across several disks or files; only this last part's records are here.",
            "APPNOTE 4.4.19",
        ),
    ),
    (
        "data before the archive*",
        hidden(
            "Bytes before the archive starts. A self-extracting program has them, but so does a file disguised as two formats at once.",
        ),
    ),
    (
        "* bytes nothing points at",
        hidden(
            "Bytes between the files that no record points at: nothing extracts them, so they can hide anything.",
        ),
    ),
    (
        "* bytes after the end of the archive",
        hidden("Bytes after the archive ends: no unzip tool will show them."),
    ),
    (
        "no end of central directory*",
        damage(
            "The archive has no end record, usually because it was cut short; the files are read one by one from the start instead.",
        ),
    ),
    (
        "its sizes are in a data descriptor*",
        damage(
            "Without the directory, where this file ends cannot be known, so reading stops here.",
        ),
    ),
];

/// Children of the central directory other than its records.
const IN_DIRECTORY: Table = &[
    (
        "expected a central directory record here",
        damage(
            "Where the next directory record should start there is something else: the directory is damaged.",
        ),
    ),
    (
        "central directory record truncated",
        damage("The directory stops in the middle of a record."),
    ),
    (
        "stopped after * entries",
        oddity("The archive lists more files than this tool reads, so the rest are not shown."),
    ),
    (
        "points at offset *, where there is no local header",
        damage(
            "This record points at a place in the file where no file starts, so it cannot be extracted.",
        ),
    ),
];

/// Everything else: parts of entries, records and end records.
const PARTS: Table = &[
    (
        "local header",
        zip(
            "The header in front of a file's data: its name, method, sizes and checksum, repeated from the directory.",
            "APPNOTE 4.3.7",
        ),
    ),
    (
        "data",
        zip(
            "The file's contents, compressed or as they are.",
            "APPNOTE 4.3.8",
        ),
    ),
    (
        "data descriptor",
        zip(
            "The checksum and sizes, written after the data by tools that did not know them in advance.",
            "APPNOTE 4.3.9",
        ),
    ),
    (
        "extra fields",
        zip(
            "Extra information tools attach to a file: precise times, owners, larger sizes.",
            "APPNOTE 4.5.1",
        ),
    ),
    (
        "ZIP64 sizes",
        zip(
            "Sizes and offsets too large for the original four-byte fields.",
            "APPNOTE 4.5.3",
        ),
    ),
    (
        "NTFS times",
        zip(
            "Windows file times, to a tenth of a microsecond.",
            "APPNOTE 4.5.5",
        ),
    ),
    (
        "extended timestamp",
        zip(
            "Unix file times: when it was changed, accessed, created.",
            "APPNOTE 4.6.1",
        ),
    ),
    (
        "Unix owner",
        zip(
            "The Unix user and group that owned the file: they can name who made the archive.",
            "APPNOTE 4.6.1",
        ),
    ),
    (
        "Unicode path",
        zip(
            "The file's name in UTF-8, for archivers that stored it another way too.",
            "APPNOTE 4.6.9",
        ),
    ),
    (
        "Unicode comment",
        zip("The file's comment in UTF-8.", "APPNOTE 4.6.8"),
    ),
    (
        "AES encryption",
        zip(
            "WinZip's AES encryption settings for this file.",
            "APPNOTE 4.6.1",
        ),
    ),
    (
        "JAR marker",
        zip(
            "A marker Java adds to show the archive is a JAR.",
            "APPNOTE 4.6.1",
        ),
    ),
    (
        "Android alignment",
        zip(
            "Padding Android's tools add so the app's files line up in memory.",
            "APPNOTE 4.6.1",
        ),
    ),
    (
        "extra 0x*",
        zip("An extra field this tool does not name.", "APPNOTE 4.5.2"),
    ),
    (
        "signature",
        zip(
            "Four bytes, starting PK, that mark what kind of record starts here.",
            "APPNOTE 4.3.6",
        ),
    ),
    (
        "versionMadeBy",
        zip(
            "Which system and ZIP version made the archive: it tells Windows from macOS or Unix.",
            "APPNOTE 4.4.2",
        ),
    ),
    (
        "versionNeeded",
        zip(
            "The ZIP version a tool must support to extract this file.",
            "APPNOTE 4.4.3",
        ),
    ),
    (
        "flags",
        zip(
            "Switches: encrypted, sizes after the data, names in UTF-8, and others.",
            "APPNOTE 4.4.4",
        ),
    ),
    (
        "method",
        zip(
            "How the file is compressed; 8 is DEFLATE, 0 is stored as it is.",
            "APPNOTE 4.4.5",
        ),
    ),
    (
        "modified",
        zip(
            "When the file was last changed, in local time, to two seconds.",
            "APPNOTE 4.4.6",
        ),
    ),
    (
        "crc32",
        zip(
            "A checksum of the uncompressed file, to catch damage on extraction.",
            "APPNOTE 4.4.7",
        ),
    ),
    (
        "compressedSize",
        zip(
            "How many bytes the file takes in the archive.",
            "APPNOTE 4.4.8",
        ),
    ),
    (
        "uncompressedSize",
        zip(
            "How many bytes the file takes once extracted.",
            "APPNOTE 4.4.9",
        ),
    ),
    (
        "nameLength",
        zip("How long the file's name is.", "APPNOTE 4.4.10"),
    ),
    (
        "extraLength",
        zip("How long the extra fields are.", "APPNOTE 4.4.11"),
    ),
    (
        "commentLength",
        zip("How long the comment is.", "APPNOTE 4.4.12"),
    ),
    (
        "diskStart",
        zip(
            "Which disk the file starts on, for archives split across disks.",
            "APPNOTE 4.4.13",
        ),
    ),
    (
        "internalAttributes",
        zip(
            "Whether the file is text or binary, as the archiver guessed.",
            "APPNOTE 4.4.14",
        ),
    ),
    (
        "externalAttributes",
        zip(
            "The file's permissions on the system that made it.",
            "APPNOTE 4.4.15",
        ),
    ),
    (
        "localHeaderOffset",
        zip(
            "Where this file's local header is in the archive.",
            "APPNOTE 4.4.16",
        ),
    ),
    (
        "name",
        zip("The file's path inside the archive.", "APPNOTE 4.4.17"),
    ),
    (
        "comment",
        zip("A comment, which archivers rarely show.", "APPNOTE 4.4.18"),
    ),
    (
        "disk",
        zip(
            "The number of this disk, for archives split across disks.",
            "APPNOTE 4.4.19",
        ),
    ),
    (
        "centralDirectoryDisk",
        zip("Which disk the directory starts on.", "APPNOTE 4.4.20"),
    ),
    (
        "entriesOnDisk",
        zip(
            "How many files this disk's directory lists.",
            "APPNOTE 4.4.21",
        ),
    ),
    (
        "entries",
        zip("How many files the archive lists.", "APPNOTE 4.4.22"),
    ),
    (
        "centralDirectorySize",
        zip("How many bytes the directory takes.", "APPNOTE 4.4.23"),
    ),
    (
        "centralDirectoryOffset",
        zip("Where the directory starts.", "APPNOTE 4.4.24"),
    ),
    (
        "recordSize",
        zip(
            "How long the rest of this ZIP64 record is.",
            "APPNOTE 4.3.14",
        ),
    ),
    (
        "recordDisk",
        zip("Which disk the ZIP64 end record is on.", "APPNOTE 4.3.15"),
    ),
    (
        "recordOffset",
        zip("Where the ZIP64 end record is.", "APPNOTE 4.3.15"),
    ),
    (
        "disks",
        zip("How many disks the archive spans.", "APPNOTE 4.3.15"),
    ),
    // Problems.
    (
        "name differs from the central directory's*",
        hidden(
            "The file's header and the directory give different names, so different tools extract different files. Android malware has used exactly this.",
        ),
    ),
    (
        "method differs from the central directory's*",
        hidden(
            "The header and the directory disagree on how the file is compressed, so tools extract it differently.",
        ),
    ),
    (
        "CRC-32 differs from the central directory's",
        hidden("The header and the directory give different checksums for the same file."),
    ),
    (
        "sizes differ from the central directory's",
        hidden(
            "The header and the directory give different sizes, so tools may read different bytes.",
        ),
    ),
    (
        "overlaps *",
        hidden(
            "Two files share the same bytes. Zip bombs use this to unpack to far more than the archive holds.",
        ),
    ),
    (
        "counts * entries; the central directory holds *",
        oddity(
            "The end record's count of files does not match the directory; the directory is what is shown.",
        ),
    ),
    (
        "central directory offset * is past*",
        damage("The end record says the directory starts later than it does."),
    ),
    (
        "central directory size is larger*",
        damage("The end record says the directory is larger than the space in front of it."),
    ),
    (
        "points at offset *, where there is no ZIP64 end record",
        damage("The ZIP64 locator points where no ZIP64 end record is."),
    ),
    (
        "local header truncated",
        damage("The file's header is cut short by the end of the archive."),
    ),
    (
        "data runs past the end of the file",
        damage(
            "The file's data is cut short by the end of the archive, so it cannot be extracted whole.",
        ),
    ),
    (
        "extra field runs past the extra block",
        oddity("An extra field claims more bytes than the extra block holds; the rest is skipped."),
    ),
];

#[cfg(test)]
pub(crate) const ALL: Table = PARTS;

pub(crate) fn describe(tree: &ParseTree, node: &Node, problem: bool) -> Option<Doc> {
    let label = node.label.as_str();
    let Some(parent) = node.parent.and_then(|p| tree.try_get(p)) else {
        return (!problem).then_some(ROOT);
    };
    if parent.parent.is_none() {
        return lookup(AT_ROOT, label, problem).or((!problem).then_some(ENTRY));
    }
    if parent.label == "central directory"
        && parent
            .parent
            .and_then(|p| tree.try_get(p))
            .is_some_and(|p| p.parent.is_none())
    {
        return lookup(IN_DIRECTORY, label, problem).or((!problem).then_some(RECORD));
    }
    lookup(PARTS, label, problem).or_else(|| lookup(IN_DIRECTORY, label, problem))
}
