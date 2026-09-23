# Known issues

Findings from the final whole-branch review of the PNG core that were not
fixed before merge, with why. Nothing here breaks the crate's three
guarantees — the parser does not panic, always returns a tree, and has no
runtime dependencies — but several affect how useful that tree is.

## Open: byte ranges in the pixel pipeline are placeholders

`png/mod.rs` — three nodes report damage at `ByteRange::new(0, 0)`:
"IDAT decompression failed", "unfiltering failed", and the interlaced warning.

The first is the worst: it is the flagship node for the product's main
scenario — a file is broken and you need to know where — and it points at
nothing. The cause is that `decode_pixels` receives `idat: &[u8]`, a buffer
concatenated from possibly many IDAT chunks, which has already discarded every
file coordinate.

**Fix when the UI exists:** carry a `Vec<ByteRange>` of the IDAT segments
alongside the concatenated bytes, and map a failure offset back through it.
`inflate` would need to return the failing bit offset in its error. Deferred
because the right shape is clearer once there is a consumer that renders it.

The interlaced warning is cheaper and independent: emit it inside
`decode_ihdr`, where the interlace byte's file offset is in hand.

## Open: `Checkpoint.bit_pos` is a block position, not an event position

`inflate/trace.rs` — `CheckpointSink` only updates `last_bit_pos` on
`BlockStart`, so every checkpoint in a typical one- or two-block PNG carries
the same bit position while `event_index` and `out_pos` advance normally. The
three fields therefore describe different moments and cannot be used together
to resume decoding, which is the entire purpose of the type. `Checkpoint` also
does not record `BlockKind`, so a replayer cannot tell whether to read a
dynamic table.

`checkpoints_record_increasing_positions` looks like it guards this but asserts
only on `event_index` and `out_pos`.

**Fix:** decide what a checkpoint is — either pass the current bit position
with every event, or redefine it as an honest block-resume record — then assert
on `bit_pos` in that test. Needed before the scrubber in the next milestone.

## Open: one bad chunk length ends the walk

`png/chunks.rs`, `png/mod.rs` — `ChunkError::LengthTooLarge` seeks to
end-of-input, and the error node claims every remaining byte is damaged. For a
genuinely truncated file that is right. For a single corrupt length byte in the
middle of an intact file, every later chunk disappears from the tree.

The spec asks for the opposite at this severity: a red node with a range, then
skip to the next chunk. **Fix:** scan forward for the next plausible chunk
header and resume.

## Open: `inflate/zlib.rs` indexes untrusted bytes by hand

Five raw indexes into caller-supplied bytes rest on a single exact `len < 6`
guard. Correct today, but it is the one place where the README's claim that
`Reader` is the single bounds-checked chokepoint is untrue, and the next person
to extend the header parsing has no local signal that the guard is load-bearing.

**Fix:** route it through `Reader`.

## Smaller items

- **Warning/Error drift.** A malformed tEXt or PLTE is a `Warning`; a truncated
  IHDR, gAMA or pHYs is an `Error`. All five are the same class of damage.
- ~~**Three conventions for "about the whole file".**~~ Fixed for the harmful
  case: `no IDAT` was `(0, len)`, which would have made every byte hover into
  the warning. It is now zero-length. `no IEND` still sits at `(len, 0)`.
- **`gamma` and `gammaDecimal` share a byte range**, which makes a
  byte-to-node lookup arbitrary. The decimal form belongs in the value, not as
  a sibling node.
- **Public API panic paths.** `BitReader::bits` guards `n <= 32` with
  `debug_assert!` only; `CheckpointSink::new` and `ParseTree::get` assert.
  None is reachable from file bytes, all are reachable by a future caller.
- **IHDR validation is partial.** Compression and filter method bytes are
  decoded but not validated though both must be 0; `width == 0` surfaces later
  as an unfilter error rather than at the header.
- **IDAT has no children.** No node for the zlib header, the Adler-32 trailer
  or the block structure — for the chunk that is the reason PNG was chosen
  first.
- **`next_chunk` copies every payload to compute its CRC**, a 10 MB
  allocation and copy for a large IDAT, on the path the budget measures.
- **`Chunk::kind_str` maps bytes through `as char`.** Mitigated: the WASM
  bridge replaces every control character before labels reach the UI. The
  core's own labels are still raw.
- **`fuzz/Cargo.toml` has no `license` field.**
- **Performance headroom is thin.** 276 ms against a 300 ms budget. The
  bitwise CRC-32 and the one-bit-at-a-time `BitReader` are both deliberately
  unoptimised and are the first places to look.
- **IDAT has no children, so hovering any IDAT byte selects the whole chunk.**
  Same root cause as the item above about IDAT structure; visible now that the
  UI exists.
- **The structure tree is not virtualised.** 1,334 rows for the 10 MB test file
  render fine; a file with tens of thousands of chunks would not.
- **Deploying the web app.** GitHub Pages on a private repository needs a paid
  plan; the app is a static build, so any static host works.
