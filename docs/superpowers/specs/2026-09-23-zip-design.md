# ZIP archives

## Goal

Open ZIP archives, and with them `.docx`, `.xlsx`, `.pptx`, `.apk`, `.jar`
and `.epub`:

1. **Structure**, read the way `unzip` reads it: end of central directory,
   central directory, local headers, data. The tree names what an archive can
   hide.
2. **The DEFLATE player for any entry.** It works as it does for a PNG's IDAT
   stream.
3. **Nested files open as documents of their own**, with breadcrumbs back:
   `report.docx › word/media/image1.png`.
4. **What an Office document reveals**: author, last saved by, company,
   dates, revisions, editing time and the application, on the card the photo
   facts already use.

These are built and committed in that order, and each leaves a working app.

## Non-goals

- Compression methods other than stored (0) and deflate (8). Others are
  named (Deflate64, bzip2, LZMA, zstd, xz) but not decompressed. That is a
  limit of the tool, so the tree gets no warning for it.
- Decryption. Encrypted entries are marked, and their bytes are shown as
  they are.
- Multi-disk archives. A spanned archive is named and read no further.
- Grafting nested trees into the archive's tree. A compressed entry's
  decompressed bytes have no place in the file, so they get a document of
  their own.

## Approach: parse the structure eagerly, decompress on demand

Opening an archive reads its structure and decompresses nothing, apart from
the two small Office property entries (part 4). An entry is decompressed only
when it is played or opened. A 200 MB `.apk` and a zip bomb therefore cost
the same to open as a small archive.

## Core: `crates/hexscope-core/src/zip.rs`

```rust
pub struct ZipDocument {
    pub tree: ParseTree,
    pub entries: Vec<ZipEntry>,
    pub facts: DocumentFacts,        // part 4
}

pub struct ZipEntry {
    pub name: String,                // sanitised for display
    pub method: u16,
    pub flags: u16,
    pub crc32: u32,
    pub compressed: u64,
    pub uncompressed: u64,
    /// File range of the entry's data, as the local header places it.
    pub data: ByteRange,
    /// The entry's node in the tree: its local header and data.
    pub node: NodeId,
}

pub fn parse_zip(data: &[u8]) -> ZipDocument;
pub fn extract(data: &[u8], entry: &ZipEntry, limit: u64) -> Result<Vec<u8>, ExtractError>;
```

`ExtractError` covers an unsupported method, encryption, data out of range,
an inflate error, a CRC mismatch and too much output.

**Reading order.** Search backwards for the EOCD signature, over at most
65,557 bytes, which is the maximum comment plus the record. Then read the
ZIP64 locator and record if present, then the central directory, then each
local header its records point to. The tree is in file order:

```
ZIP
├ data before the archive            (only if present; Warning)
├ word/document.xml                  (local header + data, per entry)
│  ├ local header  → signature, version, flags, method, time, crc, sizes,
│  │                 name, extra fields (ZIP64 decoded, others named by id)
│  ├ data           deflate, 8.1 KB → 41 KB
│  └ data descriptor                  (flag bit 3 only)
├ …
├ central directory  (one record per entry, fields as above plus offsets)
├ ZIP64 end of central directory     (if present)
└ end of central directory           (counts, offset, comment)
```

**Problems.** These follow the existing rule: Error means the bytes cannot be
read, Warning means they were read but break the format.

| Finding | Kind | Where |
|---|---|---|
| No EOCD found | Error | root, zero-length at end |
| Data before the first local header | Warning | its own node |
| Central record points past the end or at no local header | Error | the record's offset field |
| Local and central disagree on name, method, CRC or sizes (sizes skipped when bit 3 is set) | Warning | the local field |
| Two entries' data overlap | Warning | the later entry |
| Bytes between entries that nothing references | Warning | their own node |
| EOCD counts disagree with the records read | Warning | the count field |
| Encrypted entry (flag bit 0) | none; the entry's value says "encrypted" | |

**Limits.** At most 65,535 entries are read, or 1,000,000 with ZIP64. Past
that, a Warning says where reading stopped. Names are capped at 1 KB for
display. `extract` stops at the caller's limit: 512 MB for the player and
for opening, the same limit PNG pixels use.

## Bridge and worker

**The player's stream becomes general.** It is file segments plus whether a
zlib wrapper surrounds them. PNG is segments with a wrapper; a ZIP entry is
one segment of raw DEFLATE. `body()` strips the wrapper only when there is
one.

- `Parsed.select_entry(i) -> bool` decodes entry `i` into the player's
  stream, output and checkpoints, and returns whether it can be played
  (deflate, not encrypted, in range). `trace` then describes that entry.
- `Parsed.open_entry(i) -> Option<Parsed>` extracts entry `i` and parses it
  as a new document. `Parsed.bytes_of_entry(i)` hands the bytes to the page,
  since the hex view needs them.
- `Parsed.entries` is a flat array per entry: `[node, method, flags,
  compressed, uncompressed, playable]`.

The worker keeps a stack of `Parsed`. `open` pushes and `back(n)` pops, with
a depth limit of 4. Past it, "Open" is disabled with a reason.

## Web

- **Entry panel.** Selecting an entry, or a node inside one, shows the name,
  method, sizes and ratio in the drawer, with *Watch it decompress* (also
  **P**) and *Open*. Each button is disabled with a reason when it cannot
  work: an unsupported method, encryption, or damage.
- **Breadcrumbs** in the top bar: the archive's name, then each opened
  entry. Clicking one returns to that document, and **Backspace** goes up
  one. Returning restores the selection that was there.
- **Facts card.** Its title follows the format: "What this photo reveals",
  "What this document reveals".

## Part 4: Office facts

If the archive holds `docProps/core.xml` or `docProps/app.xml` (each at
most 1 MB), `parse_zip` extracts them and reads these elements with a small
tag reader: no DTDs, no entity expansion beyond the five predefined and
numeric ones, and text capped at 512 bytes.

| Fact | Source |
|---|---|
| Title | `dc:title` |
| Author | `dc:creator` |
| Last saved by | `cp:lastModifiedBy` |
| Created, Modified | `dcterms:created`, `dcterms:modified` |
| Revisions | `cp:revision` |
| Editing time | `TotalTime` (minutes) |
| Company | `Company` |
| Application | `Application` + `AppVersion` |
| Template | `Template` |

Each fact points at its entry's node. The bytes are compressed, so the
entry is as exact as it can be.

## Testing

- **Built archives:** tests build ZIPs byte by byte, as the EXIF tests build
  TIFF blocks: stored and deflated entries, a data descriptor, ZIP64
  records, a comment, an empty archive.
- **Damage:** truncation at every length of a small archive; mismatched
  local and central names; overlapping entries; data prepended; a central
  offset past the end; a CRC mismatch on extract. Each gives the node the
  table above names, and none panics.
- **Round trip:** every entry of a built archive extracts to what was put
  in, and the player's steps rebuild it byte for byte.
- **Real file:** `tests/fixtures/report.docx`, written by macOS `textutil`
  (Apple's Cocoa document writer) with a known title, author, last editor,
  company and dates. It parses with no Warning or Error, and its facts match.
  `scripts/make-sample-docx.py` regenerates it.
- **Properties:** `parse` never panics on `PK\x03\x04` plus arbitrary bytes.
  The existing `parse` fuzz target covers ZIP through the dispatcher.
- **Sample for the site:** the same script writes
  `apps/web/public/samples/report.docx`: the `textutil` document plus
  `word/media/photo.jpg`, the existing sample photo, stored the way Word
  stores images. Opening that image inside the document shows its GPS
  location, which is the point of the demo. A *Try a document* button opens
  it.
- **Browser:** open the `.docx` sample; play `word/document.xml`; open an
  embedded image, check its EXIF card, go back; check the facts card; check
  at 375 px.
