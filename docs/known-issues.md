# Known issues

What is known to be missing or wrong, and why it has not been fixed yet.
None of it breaks the crate's guarantees — the parser does not panic, always
returns a tree, and has no runtime dependencies.

## Not yet built

These are features in their own right rather than defects, each worth its own
milestone.

**HEIC.** The default photo format on iPhones is recognised and named, with a
hint to export as JPEG, but not read. Its EXIF lives in an ISO base media
file; reading it means parsing that container.

**Vendor MakerNotes.** Camera makers store extra metadata — often more serial
numbers — in a MakerNote whose format differs per vendor. It is shown as an
opaque byte range.

**IPTC.** Photoshop's APP13 segment is named but not decoded.

## Limits by design

**The interface is built for pointing devices.** Below 900 px the panes stack
and the byte view drops to 8 bytes per row, so it works on a phone, but hover
is the main way to explore and touch has none.

**WebAssembly size.** About 58 KB gzipped. A size-optimised Rust build saved
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
