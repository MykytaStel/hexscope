# hexscope

A microscope for binary files — drop one in and watch it take itself apart,
entirely in your browser.

hexscope shows you what is actually inside a file: the structure tree, the
bytes each field occupies, where the file is damaged, what a photo gives away
about whoever took it, and — the part no other tool does — a PNG's compression
running step by step, with an arc from each back-reference to the bytes it
copies. Nothing is uploaded anywhere. The file never leaves the tab.

> **Live at [hexscope.pages.dev](https://hexscope.pages.dev).** PNG, JPEG and
> ZIP (with `.docx`, `.xlsx`, `.apk`, `.jar`, `.epub`) are supported.

## What it does

**Opens any PNG, JPEG or ZIP, broken or not.** A damaged file is the normal
case, not the error case: hexscope always shows the full structure, with every
problem marked on the exact bytes. A broken file opens on its first problem;
**N** steps through the rest. Anything else is named when its signature is
familiar — a PDF, a gzip, an iPhone's HEIC photo — rather than just refused.

**Takes archives apart, and what is inside them.** A ZIP — and so a Word
document, a spreadsheet, an Android app, a Java archive, an e-book — is read
the way `unzip` reads it, in file order. The tree names what an archive can
hide: data before it (a self-extractor, or a file that is two formats at
once), local headers that disagree with the central directory (the trick
behind APK signature bypasses), entries whose bytes overlap (a zip-bomb
technique), bytes nothing points at. A truncated download still lists every
entry it can. Any deflated entry plays in the DEFLATE player; *Open* shows an
entry as a file of its own — a photo inside a Word document gets its own EXIF
card — with breadcrumbs back (**Backspace** goes up one). An Office document
shows who wrote it, who saved it last, when, with what, for which company.

**Shows what a photo reveals.** Drop a JPEG and hexscope reads its EXIF:
where it was taken, the camera and lens, their serial numbers, the owner's
name if the camera recorded one, the time, the software, an embedded
thumbnail. Each fact links to the bytes that spell it out, so you can see
exactly which part of the file gives you away. A photo with a location opens
on it. The map link sends the coordinates nowhere unless you click it.

**Links every view.** Hover a byte and its field lights up in the tree, the
details panel says what it is, and the status bar shows its offset and path.
Hover a tree row and its bytes light up. Click to pin.

**Plays the decompression.** *Watch it decompress* (or **P**) turns the bottom
panel into a player for a PNG's DEFLATE stream, or a ZIP entry's:

- each step in plain words — a literal written as is, N bytes copied from M
  bytes back, a new block and its kind — and how many bits it read;
- those bits split into what they meant: the Huffman code as the decoder
  assembled it, its extra bits, and for a block header BFINAL, BTYPE, HLIT,
  HDIST and HCLEN;
- the block's code tables grouped by code length, with the path the current
  code took: past every shorter range, into its own;
- the output as it grows, with an arc from each back-reference's source to
  where the copy lands; a copy that overlaps itself is drawn as the repeating
  pattern it is;
- a reading head on the hex view outlining the exact file bytes whose bits the
  current step consumed;
- a damaged stream plays up to the step where it breaks, and says so.

| Key | Action |
|---|---|
| **Space** | play / pause |
| **←** **→** | one step (with **Shift**, ten) |
| **Home** **End** | first / last step |
| **P** | open the player (for a ZIP, on the selected entry) |
| **N** | next problem |
| **Esc** | close the player, or clear the selection |
| **Backspace** | back out of a file opened inside an archive |

## Why

To see inside a file today you either read the spec with `xxd` open in another
window, or install a desktop hex editor. Both are fine for people who already
know what they are looking for. Neither helps when you just want to know why a
PNG will not open, where your parser reads the wrong offset, or how DEFLATE
actually works.

It is built for:

1. **A file is broken and you need to know where.**
2. **You are writing a parser** and need to see which bytes hold a field.
3. **You are learning** a format or a compression algorithm and want to look at
   one rather than read about one.
4. **Reverse engineering, CTF, security triage** — inspect a suspicious file
   without executing it. Everything runs locally, which is the point.

## How fast

Measured in the browser on a 10.9 MB PNG of incompressible noise — the worst
case, since every byte becomes its own decoding step:

| | Measured | Budget |
|---|---|---|
| Parse (WebAssembly, in a worker) | 160–190 ms | 300 ms |
| Drop to first frame | 180–280 ms | 1 s |
| Scrolling the byte view | 8.3 ms per frame, none dropped at 120 Hz | 16 ms |
| Hover to highlight | ~0.09 ms | 16 ms |
| Seek to step 7.9 million of 10.8 million | 12 ms | — |
| The whole app, gzipped | ~76 KB | — |

The byte view draws only the rows on screen, so file size does not affect
scrolling. The player never holds the stream's steps in memory: it asks for
them in small batches, and the decoder resumes from the nearest checkpoint.

## How it is built

- **`crates/hexscope-core`** — the parsers, in Rust with no runtime
  dependencies: PNG with its own DEFLATE decoder, written to be paused and
  resumed one step at a time; JPEG segments; EXIF in either byte order; and a
  dispatcher that recognises the format. A reference DEFLATE implementation is
  used only in tests, to check ours.
- **`crates/hexscope-wasm`** — the bridge to the browser. The parse tree
  crosses as a handful of typed arrays rather than one object per node.
- **`apps/web`** — the interface: TypeScript, Vite, Canvas 2D, no UI
  framework.

Three rules hold across the parser:

- **It never panics and never loops forever**, on any input. Every read goes
  through one bounds-checked accessor; parsing code never indexes a slice by
  hand. EXIF is the hostile case — every offset in it comes from the file — so
  IFD chains remember where they have been and counts are capped by what fits.
  Fuzzed across both formats with no crash.
- **Every input returns a tree.** There is no error return: damage becomes a
  node with its own byte range, so a broken file is still fully explorable.
- **It is checked against real files.** All 176 files of the
  [PngSuite](http://www.schaik.com/pngsuite/) conformance corpus: every one of
  the 14 intentionally corrupt files is flagged, and none of the 162 valid ones
  is. The sample photo's metadata is checked against what Apple's ImageIO
  reads from the same file.

Open problems are listed in [`docs/known-issues.md`](docs/known-issues.md).
The original design and plan are in [`docs/superpowers/`](docs/superpowers/).

## Developing

You need rustup, Node 24+, pnpm, and the wasm-bindgen CLI at the exact version
the bridge pins. The Rust version, its components and the WebAssembly target
are pinned in `rust-toolchain.toml`; one command installs them.

```bash
rustup toolchain install          # reads rust-toolchain.toml
cargo install wasm-bindgen-cli --version 0.2.128 --locked
pnpm install
pnpm dev            # builds the WebAssembly, then serves the app
```

Checks, as CI runs them:

```bash
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --all
cargo bench -p hexscope-core                                  # 10 MB parse time
cargo +nightly fuzz run parse_png -- -max_total_time=120      # needs cargo-fuzz
```

## Deploying

The app is a static site: no server, no backend, nothing to configure.

```bash
pnpm build          # output: apps/web/dist
```

The sample photo is generated by `scripts/make-sample-photo.py` (macOS, for
`sips`); its location is the Eiffel Tower and every other value is invented.

Upload `apps/web/dist` to any static host — Cloudflare Pages, Netlify, Vercel,
GitHub Pages, or any web server. Asset paths are relative, so it also works
from a subpath. It must be served over HTTP(S), not opened from disk, because
the parser runs in a module worker.

It is deployed on Cloudflare Pages as the project `hexscope`. CI builds the
site on every push, keeps it as the `hexscope-site` artifact, and the `deploy`
job uploads it: `main` to production, every other branch to a preview URL.
The job needs a `CLOUDFLARE_API_TOKEN` repository secret — an API token with
*Account → Cloudflare Pages → Edit* — and deploys nothing until it exists.

`apps/web/public/_headers` sets the response headers: a CSP that lets the page
run only its own code and send nothing anywhere, and long caching for hashed
assets. To try them locally:

```bash
pnpm build && npx wrangler pages dev apps/web/dist
```

A manual deploy, after `npx wrangler login`:

```bash
npx wrangler pages deploy apps/web/dist --project-name hexscope --branch main
```

## Roadmap

1. **PNG** — structure, damage, pixels, the DEFLATE player. *Done.*
2. **EXIF from JPEG** — what a photo reveals, on its bytes. *Done.*
3. **ZIP** — which also opens `.docx`, `.apk`, `.jar` and `.epub`, with
   recursion into nested files. *Done.*
4. **WebAssembly binaries.**
5. **Full JPEG decoding.**

No analytics, ever. A tool people use to look at suspicious files has no
business watching them.

## License

Licensed under either of [MIT](LICENSE) or [Apache License 2.0](LICENSE), at
your option.
