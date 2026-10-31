# Clean-copy verification coverage

This matrix describes when Hexscope may report a parser-backed finding as
**Removed** after it reads the clean copy back. It is deliberately narrower
than the set of facts Hexscope can display or the transformations a cleaner can
perform.

The canonical runtime contract is
[`VERIFIED_REMOVAL_CAPABILITIES`](../crates/hexscope-core/src/verification.rs).
The core reports removal only when the source finding matches one exact
format/kind/scope combination below, the source and output parses are complete,
and the finding is absent from the output. Hexscope captures the source facts
before cleaning, then reads back the written copy. A value-free result from
this same core contract is used by the Web worker and `hexscope clean --verify`.

The table below is written from that contract, and a core test fails when
the two differ. Every finding is file-scoped: a fact about one entry inside a
file, such as a photo in a document, is not covered.

<!-- coverage:start -->
| Format | Findings a clean copy is confirmed to have removed |
| --- | --- |
| `jpeg` | location, camera, lens, serial, owner, taken, software, thumbnail |
| `png` | location, camera, lens, serial, owner, taken, software, ai, prompt, caption |
| `webp` | location, camera, lens, serial, owner, taken, software, thumbnail |
| `heif` | location, camera, lens, serial, owner, taken, software |
| `tiff` | location, camera, lens, serial, owner, taken, software |
| `video` | location, camera, owner, taken, software |
| `pdf` | author, title, subject, keywords, application, producer, created, modified, history, covered, hiddentext, comments, attachments |
| `zip` | title, subject, author, editor, keywords, description, category, status, created, modified, revisions, company, manager, application, editing, template, label, custom, downloaded, tags, tracked, deleted, photoplace, photo, printed, printer, thumbnail, folder |
| `office97` | title, subject, author, editor, keywords, description, category, created, modified, revisions, company, manager, application, editing, template |
| `rtf` | title, subject, author, editor, manager, company, category, keywords, description, created, modified, printed, revisions, editing, application, template, custom, tracked, deleted, comments |
| `svg` | original, links, software, author, company, title, created, description, keywords, embedded, photoplace |
| `mp3` | title, artist, album, album-artist, year, track, disc, genre, composer, encoder, organization, location, comment, audio-tag |
<!-- coverage:end -->

Why each row can be trusted:

- **Photos** (JPEG, PNG, WebP, HEIC): the copy drops or blanks EXIF, XMP,
  IPTC, the maker's notes and comments as a whole, so no copy of these kinds
  can stay in another block.
- **Videos** (MP4, MOV, M4A): the copy blanks every metadata item where it
  lies and zeroes the creation and change times of every header.
- **PDF**: the copy is written anew without the Info dictionary and XMP, and
  without the text under black boxes, marked for redaction or hidden. Notes
  and their pop-ups go; every other annotation loses who made it and when,
  and a mark on the text the words of its note — what a person typed or
  signed on the page stays. The files it carries go, unless the document
  names them as part of itself (`/AF`), as an e-invoice or a PDF/A-3 file
  does.
- **Word, Excel, PowerPoint** (`zip`): the copy replaces the core, app and
  custom properties with empty parts and leaves out a Mac's `__MACOSX` notes.
  Word's tracked changes are accepted, with the text a deletion held, and each
  photo placed in a document is cleaned as a photo of its own. What Office on
  Windows leaves goes too: the folder Excel last saved a workbook in
  (`x15ac:absPath`), the folder of a Word document's template (the file name
  stays), the printer a printer-settings part names, SharePoint's properties
  kept as custom XML, a JPEG or PNG picture of the first page (now a blank
  pixel), and Word's editing-session numbers (`w:rsid`). Comments and
  embedded files are outside this row.
- **OpenDocument** (.odt, .ods, .odp, also `zip`): the copy empties
  `meta.xml` and `settings.xml` — who made it, the program and system, who
  printed it, the printer — turns the first page's thumbnail into a blank
  pixel, accepts tracked changes, deletes comments, and cleans the photos
  under `Pictures/`.
- **Office 97-2003**: the copy blanks both property streams where they lie.
- **RTF**: the copy leaves out the information group, the generator, the
  template, the custom properties and the editing-session numbers; tracked
  deletions in groups of their own go with their text, insertions lose their
  marks, and comments go with who wrote them. A deletion sharing its group
  with other text stays, and is reported **Still present**.
- **SVG**: the copy leaves out the `<metadata>` block whole (RDF and XMP),
  every attribute in Inkscape's and Sodipodi's namespaces and Inkscape's
  window settings, a comment naming the program, and Illustrator's own copy
  of the drawing (`i:pgf`); a picture linked by its path on the author's
  computer keeps only its file name, and a photo embedded as base64 is
  cleaned as a photo of its own. Scripts are the drawing's and stay; they
  are named, not removed.
- **MP3**: leading ID3v2 and trailing APE, ID3v1 Plus and ID3v1 tags are
  removed as whole blocks; compressed MPEG frames are copied byte for byte. Only listed
  metadata kinds can be confirmed, and only when both parses are complete.
- **Every other format, finding, or scope**: none. A finding still observed in
  the copy is **Still present**; absent but uncovered, it is **Not checked**,
  as is anything from an incomplete read.

The Web skips read-back verification for copies larger than 10 MiB; the CLI
reads back every copy it is asked to verify.

## User-facing result meanings

- **Removed** means the eligible finding was present in the source and absent
  from a complete parse of the written copy.
- **Still present** means the output parser still found the finding, including
  when a different value of the same kind remains.
- **Not checked** means Hexscope cannot make a removal claim from its current
  capability, parse completeness, comparison evidence, or verification limit.
  The reason is shown next to the finding.

The human-readable Web result is available in English and Ukrainian. CLI output
uses the same three statuses and plain-language reasons. `--json` keeps the
version 1 schema and stable machine reason codes; scripts do not need to change.

## Regression evidence

- [`verification.rs`](../crates/hexscope-core/src/verification.rs) owns the
  capability list and tests that its exact combinations match the decision
  gate, while unknown kinds, formats, and scopes remain uncovered.
- [`verification.test.ts`](../apps/web/src/verification.test.ts) checks that
  every stable core reason decodes into its intended user-facing status and
  explanation.
- [`cli.rs`](../crates/hexscope-cli/tests/cli.rs) checks readable output,
  value-free reporting, and unchanged JSON behavior against written copies.
