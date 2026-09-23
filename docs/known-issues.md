# Known issues

What is known to be missing or wrong, and why it has not been fixed yet.
None of it breaks the crate's guarantees — the parser does not panic, always
returns a tree, and has no runtime dependencies.

## Open

**IDAT has no children.** No node for the zlib header, the Adler-32 trailer or
the DEFLATE block structure, so hovering any IDAT byte selects the whole chunk.
The player now shows the stream step by step, which covers most of the need,
but the tree itself still says only "IDAT". Blocks would be the natural first
children: the decoder already knows where each one starts.

**One bad chunk length ends the walk.** `ChunkError::LengthTooLarge` stops at
the damaged chunk and marks everything after it unreadable. For a truncated
file that is right. For a single corrupt length byte in an otherwise intact
file, every later chunk disappears. The spec asks for a red node and a resync
to the next plausible chunk header.

**`inflate/zlib.rs` indexes untrusted bytes by hand.** Five raw indexes rest on
one exact `len < 6` guard. Correct today, but it is the only place where the
rule that `Reader` is the single bounds-checked accessor does not hold.

**The player shows positions, not codes.** It says how many bits a step read
and where, but not the Huffman code itself or the tables a dynamic block
builds. That is the natural next depth for the animation.

**Warning and error severity drift.** A malformed tEXt or PLTE is a warning; a
truncated IHDR, gAMA or pHYs is an error. All are the same class of damage.

**Smaller items.**
- `gamma` and `gammaDecimal` share a byte range; the decimal is a rendering
  of the same field and belongs in its value, not in a sibling node.
- `BitReader::bits` guards `n <= 32` with `debug_assert!` only;
  `CheckpointSink::new` and `ParseTree::get` assert. None is reachable from
  file bytes.
- IHDR does not validate the compression and filter method bytes, both of
  which must be 0.
- `Chunk::kind_str` maps bytes through `as char`. The WASM bridge sanitises
  labels before they reach the UI; the core's own labels are still raw.
- `fuzz/Cargo.toml` has no `license` field.
- The structure tree is not virtualised. Fine for the 1,334 rows of a 10 MB
  test file, not for tens of thousands of chunks.
- The layout is designed for windows at least 1,024 px wide.

## JPEG and EXIF

**Only EXIF is read.** XMP, IPTC and camera makers' MakerNotes are shown as
opaque byte ranges. MakerNotes in particular often hold more serial numbers.

**Only the first EXIF segment counts.** Some tools add a second one — Apple's
`sips` does — and its facts are not merged in.

**The embedded thumbnail is not opened.** It is a JPEG in its own right and
could be parsed recursively; thumbnails sometimes survive edits that removed
something from the main image.

**HEIC is not supported**, only recognised. It is the default on iPhones, so
many photos people have to hand will be refused with a hint to export as JPEG.

**An unexpected byte where a marker should be ends the JPEG walk**, the same
limitation PNG has with a corrupt chunk length.

**The WebAssembly grew from 36 KB to 59 KB gzipped** with EXIF support, mostly
number formatting and the new code itself. A size-optimised build saved under
2 KB and was not worth slower parsing; `wasm-opt` has not been tried.

## Fixed

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
