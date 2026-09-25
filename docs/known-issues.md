# Known issues

What is known to be missing or wrong, and why it has not been fixed yet.
None of it breaks the crate's guarantees — the parser does not panic, always
returns a tree, and has no runtime dependencies.

## Not yet built

These are features in their own right rather than defects, each worth its own
milestone.

**Pixels and bytes for other images.** The picture view maps pixels to
bytes for PNGs that are not interlaced, and blocks to bytes for sequential
JPEGs. An interlaced PNG stores its pixels in seven passes, and a progressive
JPEG spreads each block over several scans; neither is followed yet. For a
JPEG, the map follows its first scan: in the rare file with one scan per
colour component, that scan is the brightness alone.

**Vendor MakerNotes.** Camera makers store extra metadata — often more serial
numbers — in a MakerNote whose format differs per vendor. It is shown as an
opaque byte range.

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

**WebAssembly size.** About 197 KB gzipped, grown from 58 KB as ZIP, the
explanations, the file map, the clean copy, HEIF (17 KB), PDF (29 KB), PNG
metadata (14 KB), PDF decryption (11 KB) and video arrived. Measured with
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
