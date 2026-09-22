# hexscope

A microscope for binary files — drop one in and watch it take itself apart,
entirely in your browser.

hexscope shows you what is actually inside a file: the structure tree, the
bytes each field occupies, and — the part no other tool does — the compression
algorithm running step by step, with arrows from each back-reference to the
bytes it copies. Nothing is uploaded anywhere. The file never leaves the tab.

> **Status: the parsing core is complete. There is no user interface yet.**
> `hexscope-core` parses PNG end to end — structure, damage, decoded pixels and
> a step-by-step DEFLATE trace — but nothing renders it. See
> [Where this actually is](#where-this-actually-is).

## Why

To see inside a file today you either read the spec with `xxd` open in another
window, or install a desktop hex editor. Both are fine for people who already
know what they are looking for. Neither helps when you just want to know why
this PNG will not open, where your parser is reading the wrong offset, or what
your phone wrote into that photo's metadata.

Five things hexscope is meant to be good at, roughly in order:

1. **A file is broken and you need to know where.** A damaged file is the
   normal case, not the error case — hexscope always renders a tree, with the
   damage marked in place.
2. **You are writing a parser** and need to see which bytes hold the field you
   are decoding.
3. **You are learning** how a format works and want to look at one rather than
   read about one.
4. **Reverse engineering, CTF, security triage** — inspect a suspicious file
   without executing it. Everything runs locally, which is the point.
5. **Photo privacy** — see the GPS coordinates your camera wrote into a JPEG.

## Where this actually is

Implemented and tested:

| Piece | State |
|---|---|
| Parse tree model (nodes, byte ranges, values) | done |
| `Reader` — the crate's single bounds-checked byte accessor | done |
| CRC-32 and the PNG chunk walker | done |
| Chunk decoders: IHDR, PLTE, tEXt, pHYs, gAMA, tRNS | done |
| IHDR validation and required-chunk checks | done |
| DEFLATE / inflate written from scratch, with a step trace | done |
| Checkpointing so a long trace can be scrubbed | done |
| zlib wrapper, scanline unfiltering, pixel output | done |
| `parse_png` end-to-end + PngSuite golden tests | done |
| Fuzzing, property tests, benchmark, CI | done |
| WASM bridge and web interface | not started |

**77 tests** (68 unit, 6 golden, 3 property). Validated against the full
[PngSuite](http://www.schaik.com/pngsuite/) conformance corpus — 176 files,
including all 14 intentionally corrupt ones, every one of which is flagged
rather than silently accepted.

Fuzzing: 1.3 million executions, zero crashes.
Benchmark: a 10.3 MB PNG parses in **~276 ms** against the 300 ms budget — met,
but with little headroom. The bitwise CRC-32 and the one-bit-at-a-time
`BitReader` are both deliberately unoptimised and are the obvious first targets
if that margin needs to grow.
`cargo clippy --all-targets -- -D warnings`, `cargo fmt --check` and
`cargo check --target wasm32-unknown-unknown` are all clean.

Open findings from the final review are tracked in
[`docs/known-issues.md`](docs/known-issues.md).

The full design is in
[`docs/superpowers/specs/`](docs/superpowers/specs/) and the implementation
plan in [`docs/superpowers/plans/`](docs/superpowers/plans/).

## Design rules

Three constraints shape the whole crate:

- **The parser never panics and never loops forever.** Every byte read goes
  through `Reader`, which returns a `Result`. Parsing code never indexes a
  slice by hand — `Reader` is the single bounds-checked chokepoint.
- **Every input returns a tree.** `parse_png` has no error return. Damage
  becomes `Warning` and `Error` nodes with their own byte ranges, so a broken
  file is still fully explorable.
- **No runtime dependencies for parsing.** `flate2` is a dev-dependency only,
  used as an oracle to check our own inflate against a reference.

## Build

```bash
cargo test --all
cargo clippy --all-targets -- -D warnings
cargo bench -p hexscope-core          # reports the 10 MB parse time
```

Fuzzing needs a nightly toolchain and `cargo-fuzz`:

```bash
cargo +nightly fuzz run parse_png -- -max_total_time=120
```

## Roadmap

1. **v1** — PNG: full structure, CRC validation, decoded pixels, and the
   DEFLATE step animation.
2. **v1.1** — EXIF from JPEG (metadata only, no image decoding). This is where
   the "what does my photo know about me" hook lands.
3. **v2** — ZIP, which also opens `.docx`, `.apk`, `.jar` and `.epub`, with
   recursion into nested files.
4. **v3** — WebAssembly binaries.
5. **v4** — full JPEG decoding.

No analytics, ever. A tool people use to look at suspicious files has no
business watching them.

## License

Licensed under either of [MIT](LICENSE) or [Apache License 2.0](LICENSE), at
your option.
