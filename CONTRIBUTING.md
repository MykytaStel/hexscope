# Contributing to hexscope

Thank you for looking. hexscope is two things on one core: a way for anyone
to see what a file gives away before they send it, and a way for the curious
to see how every byte of a file works. A change is welcome when it makes one
of those better without making the other worse.

The quickest useful contribution is **a file hexscope gets wrong**: a false
alarm, something it missed, a file it cannot open. Use the *Report a problem
with this file* link in the app: it fills in the file's layout, without its
content, for an issue. Every real bug becomes a test fixture.

## Principles that do not bend

- **Nothing leaves the device.** No uploads, accounts, trackers or server
  processing of files. A feature that needs a server does not ship. The one
  exception is the website's visit counter (`apps/web/server/count.ts`): empty
  requests at most once a day, totals only, no cookie, id or address, nothing
  about files, and the totals read only by the maintainer (`pnpm stats`). Keep it that small.
- **The parser never panics and never loops forever**, on any input. Every
  read goes through one bounds-checked accessor (`reader.rs`); parsing code
  never indexes a slice by hand. `#![forbid(unsafe_code)]` everywhere.
- **Every input returns a tree.** There is no error return from a parser:
  damage becomes a node with its own byte range, so a broken file can still
  be explored.
- **Keep runtime dependencies bounded and reviewed.** Core/inflate remain dependency-light; optional BZIP2/Zstandard decoders and CLI serialization are explicit dependencies. The Web OCR engine and language data are opt-in, same-origin assets, excluded from first-visit precaching.
- **Every part is explained.** A node the parser can name without a plain
  sentence and, where there is one, the section of its specification fails
  the coverage test.
- **Say what is true, in plain words.** A finding is what reading the
  structure shows; hexscope is not a virus scanner and does not say it is.
  When something cannot be done — text that is part of a picture under a black
  box — the app says so rather than implying it.

## Finding your way around

```
crates/
  hexscope-inflate/   DEFLATE and zlib, decoded one explainable step at a time
  hexscope-core/      every parser, the explanations, clean, repair, summary
    src/reader.rs       the one bounds-checked way to read bytes
    src/model.rs        ParseTree: nodes with a byte range, a kind, a value
    src/document.rs     which parser a file goes to, by its first bytes
    src/docs/           how explanations are looked up; each format keeps its
                        own table next to its parser (png/docs.rs, pdf/docs.rs…)
    src/png, jpeg, heif, video, pdf, zip, wasm, exif/
                        one module per format
    src/clean.rs        the clean copy, per format
    src/repair.rs       "save what survived"
    src/summary.rs      the short verdict the CLI prints
    tests/              golden snapshots, property tests, fixtures
    fuzz/               cargo-fuzz targets
  hexscope-wasm/      the bridge: the tree crosses as a few typed arrays
  hexscope-cli/       `hexscope check | clean | repair`, and action.yml
apps/web/
  src/main.ts         the page: loading, selection, keys
  src/worker.ts       parsing off the main thread
  src/model.ts        FileModel over the typed arrays
  src/drawer.ts       the verdict, what a file reveals, the clean copy
  src/verdict.ts      the short answer at the top
  src/tree.ts, hexview.ts, minimap.ts
                      the structure, the bytes, the whole-file map
  src/player.ts       the DEFLATE player
  public/samples/     the sample files; made by scripts/make-sample-*.py
```

## Adding to it

**A new finding** (something a file gives away): read it in the format's
module and push a `DocumentFact { kind, text, node }` — or a photo fact for
EXIF and XMP. Add its code and the words for its detail, category and verdict
to `crates/hexscope-core/src/finding-kinds.tsv`; the page and CLI read that
catalogue. Add advice if there is something to do (`advice.ts`), and make the
clean copy remove it — or list it in `KEPT` with the reason it stays.

**A new part of a format**: name it in the parser, and add its explanation to
that format's `docs.rs` table. The coverage test tells you if you forgot.

**A new format** — only when real files ask for it: a module under
`hexscope-core/src/`, a signature check in `document.rs`, a `Document`
variant, its docs table, a sample and fixtures, and a case in the bridge's
`parse` (`crates/hexscope-wasm/src/lib.rs`), behind the `documents` feature
unless it is a picture or a movie. Keep it within the budgets (below).

**The web app** has no framework: plain TypeScript and DOM, Canvas 2D for the
bytes. Colours are tokens at the top of `style.css`, in both themes; text must
keep 4.5:1 contrast. Check it at 375 px wide, and with the keyboard alone.

## Budgets

- Four format-aware WASM families are lazy-loaded: media (pictures/movies/SVG),
  PDF, docs (Office/mail/modules), and audio. Raster is a separate image pipeline.
  `apps/web/src/family.ts` routes input signatures; `parser-builds.ts` owns the
  loaders. Core's Cargo features and the root `wasm:*` scripts define what each
  family can inspect. Update routing, bridge and corpus gates together.
- Actual compressed artifact budgets live in `.github/workflows/ci.yml`; check
  its per-family WASM, landing, analyzer and article CSS limits after rebuilding.
- The tree arena retains at most 500,000 ordinary nodes plus one limit error.
  This bounds tree storage, not all parser allocations. Existing per-format
  decoding limits still apply. A limit must remain an incomplete result.
- Whole-file admission limits are 128 MiB in Web and 512 MiB in CLI. Supported
  large movies use the existing sliced path. Web parsing has a 60-second worker
  deadline; an overdue worker is discarded and the user reopens the file.
- Read-back verification stays capped at 10 MiB in Web. Do not raise that cap
  without physical-device memory measurements.
- Optional OCR assets are prepared from locked npm packages by
  `scripts/prepare-ocr.mjs`; never use a runtime CDN. OCR is a user-confirmed
  suggestion tool, not an inspection-completeness or removal guarantee.
- A 10 MB PNG parses within 300 ms in the browser (`cargo bench -p hexscope-core`).

## Checks

As CI runs them:

```bash
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --all
pnpm build
pnpm --filter web test
```

Setting up is in the README, under *Developing*. Commits are small, say why in
their message, and go through a branch and CI before `main`, which deploys.

## Security

A way to make the parser crash, hang or read out of bounds is a security bug:
see [`SECURITY.md`](SECURITY.md) rather than opening a public issue.
