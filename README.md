# hexscope

A microscope for binary files — drop one in and watch it take itself apart,
entirely in your browser.

hexscope shows you what is actually inside a file: the structure tree, the
bytes each field occupies, and — the part no other tool does — the compression
algorithm running step by step, with arrows from each back-reference to the
bytes it copies. Nothing is uploaded anywhere. The file never leaves the tab.

> **Status: PNG support is feature-complete; not yet deployed.** Drop a PNG into
> the browser, explore its structure, bytes and damage, and watch its DEFLATE
> stream decompress step by step. See [Where this actually is](#where-this-actually-is).

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
| WASM bridge (parsing in a Web Worker) | done |
| Web interface: tree, hex canvas, details, hover linking, problem navigation | done |
| DEFLATE player: step, play, seek, reading head over the input | done |

**101 Rust tests** across the parser, the WASM bridge and the golden corpus. Validated against the full
[PngSuite](http://www.schaik.com/pngsuite/) conformance corpus — 176 files,
including all 14 intentionally corrupt ones, every one of which is flagged
rather than silently accepted.

Fuzzing: 1.3 million executions, zero crashes.
Benchmark: a 10.3 MB PNG parses natively in **~188 ms** against the 300 ms
budget.
`cargo clippy --all-targets -- -D warnings`, `cargo fmt --check` and
`cargo check --target wasm32-unknown-unknown` are all clean.

In the browser, on a 10.9 MB PNG of incompressible noise — the worst case,
since every byte is its own step:

| | Measured | Budget |
|---|---|---|
| Parse (WASM, in a worker) | 160–190 ms | 300 ms |
| Drop to first frame | 180–280 ms | 1 s |
| Scrolling the hex view | 8.3 ms per frame, none dropped at 120 Hz | 16 ms |
| Hover to highlight | ~0.09 ms | 16 ms |
| Seek to step 7.9 million of 10.8 million | 12 ms | — |
| Whole app, gzipped | ~56 KB | — |

Open findings are tracked in
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

## Run it

Needs Rust with the `wasm32-unknown-unknown` target, Node, pnpm, and the
wasm-bindgen CLI at the exact version the crate pins:

```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.128 --locked
pnpm install
pnpm dev
```

## Build and test

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
