# Known issues

What is known to be missing or wrong, and why it has not been fixed yet.
None of it breaks the crate's guarantees — the parser does not panic, always
returns a tree, and has no runtime dependencies.

## Not yet built

These are features in their own right rather than defects, each worth its own
milestone.

**WebAssembly components and element segments.** A component (the
component model's format) is listed section by section but not decoded, and
a module's element segments are shown as one range. Types in the form the
garbage-collection proposal adds are named, not decoded.

**Arithmetic-coded and lossless JPEGs.** The picture view maps pixels to
bytes for every PNG, interlaced or not, and blocks to bytes for Huffman-coded
JPEGs, sequential or progressive. Arithmetic-coded and lossless JPEGs, both
rare, are not mapped.

**Other makers' notes.** Apple's, Canon's, Nikon's and Fujifilm's MakerNotes
are read; other makers' are shown as one range. Nikon encrypts parts of its
note (ShotInfo, LensData) with the camera's serial number and shutter count;
those parts are named, not decrypted.

**IPTC.** Photoshop's APP13 segment is named but not decoded.

**PDFs with a password.** A PDF that opens without a password (RC4 40 or
128, AES-128, AES-256) is decrypted to read what it says, as any viewer
would. One that needs a password is only named as such; hexscope does not
ask for passwords. No encrypted PDF gets a clean copy: rewriting it would
drop the protection its author chose.

**PDF filters other than Flate.** Object streams and XMP compressed any
other way are not read. Neither are predictors, so the entries of a
cross-reference stream are not shown one by one.

**HEIF items built from other items.** An item stored by construction
method 2 is assembled from other items' bytes; it is named, not assembled.
HEIF sequences (Live Photo videos, bursts) are read as boxes only.

**Other ZIP compression methods.** Stored and deflated entries open and play;
Deflate64, bzip2, LZMA, zstd and xz are named but not decompressed, and
encrypted entries are marked but not read. Multi-disk archives are named and
read no further.

## Limits by design

**Large archives.** Up to 100,000 entries are read. Past the first 2,000,
entries get no field-level nodes, only their entry, data and central record:
every field of every entry would be millions of nodes, which a browser tab
cannot hold comfortably. Opened files nest at most four deep.

**Touch is second to pointing.** Below 900 px a file opens on its summary —
the verdict, what it reveals, the clean copy — and *Bytes* shows the tree
and the bytes, 8 to a row, the text shrinking to fit. Tapping a pixel works
as hovering does; elsewhere, hovering over a byte to see what it is has no
touch equivalent yet beyond tapping to select it.

**Text under black boxes is found by estimate.** A PDF page is walked as a
viewer paints it, but a string's width is taken from its length and the font
size, not from the font's own widths, and content drawn by form XObjects is
not followed. So a box that covers only part of a word may be missed or may
take the whole word, and a box inside a form is not seen. Text in a font
whose codes are not letters (most CID fonts) is counted, not shown. The
clean copy does not remove covered text: it keeps pages as they are.

**WebAssembly size.** About 241 KB gzipped against a CI budget of 256,000
bytes, grown from 58 KB as ZIP, the explanations, the file map, the clean
copy, HEIF (17 KB), PDF (29 KB), PNG metadata (14 KB), PDF decryption
(11 KB), video, WebAssembly, MakerNotes and the PDF redaction check (10 KB,
after a hand-written reader for PDF numbers saved 15 KB over the standard
float parser) arrived. Writing numbers with a fixed count of decimals by
hand, in place of the standard float formatting, saved 9.6 KB. Measured with
`twiggy`: the biggest single part is the explanations' text; dropping the
function-name section from the build saved 9 KB gzipped, and a small
decimal parser in place of the standard one about 6 KB before compression. The explanations'
text is under 10 KB of that; where the rest went has not been measured. A size-optimised Rust build saved
under 2 KB. `wasm-opt -O3` saved 17% raw and 7% gzipped — and made parsing a
10 MB PNG five to six times slower in the browser (950–1,500 ms against
165–266 ms, measured side by side). Neither was kept.

## Fixed

- **One corrupt chunk length or JPEG segment length ended the walk.** Both
  formats now resume at the next intact chunk or plausible segment, so one bad
  byte costs one chunk. PNG requires a matching CRC before resuming.
- **IDAT had no children.** It now shows the zlib header, every DEFLATE block
  and its kind, and the Adler-32 trailer, split at chunk boundaries.
- **Severity drifted between warning and error.** Every site was audited
  against the rule on `NodeKind`; two were wrong. A third put a phantom
  "problem" on every valid interlaced PNG.
- **Only the first EXIF segment counted**; a second now fills the gaps.
- **The EXIF thumbnail was an opaque blob**; it is parsed as a JPEG in place.
- **XMP was an opaque blob**; it is shown as text.
- **The structure tree put every row in the DOM**; it is virtualised.
- **The player lost long repeating copies.** A copy that repeats a pattern
  further back than the strip is wide showed neither where it landed nor
  the arc; it now uses the source | gap | destination layout.
- **`zlib.rs` indexed untrusted bytes by hand**; it reads through `Reader`.
- **`gamma` and `gammaDecimal` shared a byte range**; they are one field.
- **IHDR did not check its method bytes**; undefined values are warnings.
- **Corrupt chunk types could put control characters in labels**; they are
  shown in hex.
- **Public functions could panic**: `BitReader::bits` above 32 bits and
  `CheckpointSink::new(0)` no longer do; `ParseTree::try_get` exists for ids
  from outside.
- **The fuzz crate had no license field.**
- **Decompression failures pointed at nothing, then at everything.** They
  first used range `(0, 0)`, then a range across every IDAT chunk that
  straddled its sibling chunks and split hover between them. The failing byte
  is now located exactly and the node nested under the chunk that holds it.
- **Checkpoints could not be resumed.** `bit_pos` was the enclosing block's,
  so every checkpoint in a one-block stream carried the same value. The
  decoder is now a resumable step machine and checkpoints are proven to
  reproduce the same steps.
- **Scanline stride was confused with filter distance**, reporting 44 of 162
  valid PngSuite files as truncated.
- **Every chunk payload was copied to compute its CRC**, and CRC-32 and
  Adler-32 were both computed the slow way. 10 MB parse: 288 ms to 188 ms.
- **A whole-file range on "no IDAT chunk"** would have captured every hover.
