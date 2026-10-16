//! What each node is, sent once per kind of part; and what a ZIP entry needs
//! to be played or opened after parsing.

use super::*;

/// What each node is, deduplicated: thousands of nodes share a few hundred
/// explanations, so each is sent once with a per-node index into it.
#[derive(Default)]
pub(crate) struct DocTables {
    pub(crate) ids: Vec<i32>,
    pub(crate) texts: Vec<&'static str>,
    pub(crate) cites: Vec<&'static str>,
    pub(crate) urls: Vec<&'static str>,
    pub(crate) concerns: Vec<u8>,
}

impl DocTables {
    pub(crate) fn build(tree: &ParseTree, format: Format) -> Self {
        let mut t = DocTables::default();
        // Docs already sent, by where their text and citation live: every
        // doc is a constant, so its strings' addresses name it. Sorted, for
        // binary search; a file has a few hundred kinds of part at most. A
        // hash map here cost 3 KB of the build.
        let mut seen: Vec<((usize, usize, u8), i32)> = Vec::new();
        for n in tree.nodes() {
            let id = match describe(tree, n.id, format) {
                None => -1,
                Some(doc) => {
                    let key = (
                        doc.text.as_ptr() as usize,
                        doc.spec.map_or(0, |s| s.cite.as_ptr() as usize),
                        doc.concern.map_or(0, |c| c as u8),
                    );
                    match seen.binary_search_by(|(k, _)| k.cmp(&key)) {
                        Ok(i) => seen[i].1,
                        Err(i) => {
                            t.texts.push(doc.text);
                            t.cites.push(doc.spec.map_or("", |s| s.cite));
                            t.urls.push(doc.spec.map_or("", |s| s.url));
                            t.concerns.push(doc.concern.map_or(0, |c| c as u8));
                            let id = t.texts.len() as i32 - 1;
                            seen.insert(i, (key, id));
                            id
                        }
                    }
                }
            };
            t.ids.push(id);
        }
        t
    }
}

/// What playing or opening a ZIP entry needs after parsing is done.
pub(crate) struct ZipState {
    pub(crate) source: Vec<u8>,
    pub(crate) entries: Vec<ZipEntry>,
}

/// Whether the player can decode this entry: deflated, readable, whole.
pub(crate) fn playable(e: &ZipEntry) -> bool {
    e.method == 8 && !e.is_encrypted() && e.compressed > 0 && e.data.len == e.compressed
}

/// Whether the entry can be opened as a file of its own: a file, not a
/// folder, with something in it, in a method this tool reads.
pub(crate) fn openable(e: &ZipEntry) -> bool {
    matches!(e.method, 0 | 8)
        && !e.is_encrypted()
        && !e.is_dir()
        && e.uncompressed > 0
        && e.data.len == e.compressed
}

pub(crate) fn extract_message(err: ExtractError) -> String {
    match err {
        ExtractError::Encrypted => "it is encrypted".into(),
        ExtractError::Unsupported(m) => {
            format!("its compression method ({m}) is not one this tool reads")
        }
        ExtractError::OutOfRange => "its data runs past the end of the file".into(),
        ExtractError::Inflate(e) => format!("its DEFLATE data is damaged ({e:?})"),
        ExtractError::Crc { stored, actual } => {
            format!("its CRC-32 does not match: stored {stored:08X}, computed {actual:08X}")
        }
        ExtractError::TooLarge => "it would decompress to more than 512 MB".into(),
    }
}
