# hexscope

A microscope for binary files — drop one in and watch it take itself apart,
entirely in your browser.

hexscope shows you what is actually inside a file: the structure tree, the
bytes each field occupies, and — the part no other tool does — the compression
algorithm running step by step, with arrows from each back-reference to the
bytes it copies. Nothing is uploaded anywhere. The file never leaves the tab.

> **Status: early. Not usable yet.**
> The Rust parsing core is under construction and there is no user interface.
> See [Where this actually is](#where-this-actually-is) before trying it.

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
| DEFLATE / inflate with a step trace | not started |
| Scanline unfiltering, pixel output | not started |
| `parse_png` end-to-end + PngSuite golden tests | not started |
| Fuzzing, benchmarks, CI | not started |
| WASM bridge and web interface | not started |

28 tests pass; `cargo clippy --all-targets -- -D warnings` is clean.

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
