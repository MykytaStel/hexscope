# hexscope

A microscope for binary files — drop one in and watch it take itself apart,
entirely in your browser.

hexscope shows you what is actually inside a file: the structure tree, the
bytes each field occupies, where the file is damaged, what a photo gives away
about whoever took it, and — the part no other tool does — a PNG's compression
running step by step, with an arc from each back-reference to the bytes it
copies. Nothing is uploaded anywhere. The file never leaves the tab.

> **Live at [hexscope.pages.dev](https://hexscope.pages.dev).** PNG, JPEG,
> HEIC and AVIF, MP4 and MOV, PDF, ZIP (with `.docx`, `.xlsx`, `.apk`, `.jar`,
> `.epub`) and WebAssembly are supported. Guides: [removing a photo's
> location](https://hexscope.pages.dev/remove-location-from-photo), [what an
> edited PDF still holds](https://hexscope.pages.dev/pdf-hidden-versions),
> [why a PNG won't open](https://hexscope.pages.dev/png-wont-open).

## What it does

**Opens any PNG, JPEG, HEIC, AVIF, MP4, PDF, ZIP or WebAssembly module, broken or not.** A damaged
file is the normal case, not the error case: hexscope always shows the full
structure, with every problem marked on the exact bytes. A broken file opens
on its first problem; **N** steps through the rest. Anything else is named
when its signature is familiar — a gzip, an MP4 video, a GIF — rather than
just refused.

**Says what it found, then explains.** A file opens on a short verdict:
damaged, something hidden or disguised, what it reveals, or healthy — each
line a link to the bytes behind it. A file named for another format (a HEIC
photo saved as .jpg) is told so, with the extension that fits it; that is
often the whole reason a file "won't open". Every part of every file then explains
itself at two depths, both always shown: one plain sentence first ("The
location directory: where the picture was taken"), and below it the offset,
the raw value and the section of the specification that defines it (PNG,
RFC 1950/1951, ITU T.81, JFIF, EXIF 2.32, PKWARE APPNOTE), linked. Problems
say how worried to be. The verdict is what reading the structure finds; it
is not a virus scan, and says so.

**Shows the whole file at once.** "What it's made of" splits every byte into
picture (or files), metadata, thumbnail, structure, hidden and damaged, with
a bar in file order and the share of each. Beside the hex view, a minimap of
the whole file colours each stretch by entropy — zeros and padding, text and
structure, compressed or encrypted — marks the problems, outlines what is on
screen, and jumps where you click. An encrypted blob in a text file, or data
tacked onto the end of an image, stands out at a glance.

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
shows who wrote it, who saved it last, when, with what, for which company;
a Word document also its comments and tracked changes, by whom, with the
text a tracked deletion still holds though no page shows it; and every photo
placed in it says where it was taken and with what camera.

**Shows where a video was recorded.** MP4 and QuickTime movies are read box
by box. An iPhone's video keeps its location, make, model, software and date
in QuickTime metadata; an Android phone writes the place in the user data;
3GPP phones in a `loci` box. All three are shown, with the map link, and the
clean copy blanks them where they lie, with the movie's header times: the
file keeps its size, and the picture and sound are copied byte for byte.

**Shows what a PDF kept.** A PDF is read in file order, not through its
cross-reference table, so the tree holds every object — including the ones
an edit replaced. A PDF changed after it was first saved usually still
carries the earlier version, and hexscope says so, revision by revision, and
shows the text an update took off a page that is still in the file. It
shows who wrote it, with which programs, when, and its editing history, from
the document information and the XMP metadata, compressed or not, and what
each JPEG photo on its pages says: where, with which camera. It finds
"redactions" that hide nothing: text a black box was painted over, which is
still in the page to select and copy, and areas marked for redaction that
were never applied — and shows the text. It marks
what a PDF can run or hide: JavaScript, actions that start programs,
attached files, data before the header or after the end. An encrypted PDF
that opens without a password — most of them — is decrypted to read it, as a
viewer would; one that needs a password is only named as such.

**Shows what a photo reveals.** Drop a JPEG, an iPhone's HEIC or a PNG, and
hexscope reads its EXIF (and a PNG's text notes and XMP): where it was
taken, the camera and lens, their serial numbers, the owner's name if the
camera recorded one, the time, the software, an embedded thumbnail — and
from the maker's own notes, what EXIF leaves out: more serial numbers, how
many photos the camera has taken, and on an iPhone, the IDs that tie a photo
to its Live Photo video and its burst, and how long the phone had been on,
which ties together every photo taken between two restarts. When the
embedded thumbnail is not the picture — the photo was cropped or edited and
the camera's small copy was left as it was — the two are shown side by side:
the thumbnail still shows what was taken out. Each
fact links to the bytes that spell it out, so you can see exactly which part
of the file gives you away. A photo with a location opens on it. The map
link sends the coordinates nowhere unless you click it. *Share what it
revealed* makes a card and a sentence that name only the kinds of thing a
file gave away — never the place, the name or the number.

**Removes it, if you want.** Under what a photo or a document reveals, one
button saves a copy without it, made in the tab and never uploaded. A photo
loses its camera data, location, serial numbers, thumbnail and comments; its
picture is copied byte for byte, and a photo taken sideways keeps only its
orientation. A PNG loses its text notes, EXIF, XMP, the time it was changed
and anything after its end; its pixels are copied byte for byte. A HEIC or
AVIF keeps its size: its EXIF and XMP are blanked where they lie, because
everything else in it is found by offset. A PDF is written anew with only
what its pages use: no author, programs or dates, no XMP, and none of the
earlier versions an edit left behind; its photos' EXIF is zeroed where it
lies, leaving the pictures untouched. A Word, Excel or PowerPoint file loses
its document properties, and the photos in it their EXIF (comments and
tracked changes are part of its text and stay, and the page says so). The
page lists what went, and *Open the clean copy* shows it in hexscope, so you
can see the card is empty.

**Shows which bytes make which pixels.** Beside a PNG's structure, its
picture: point at a pixel, or tap it, and hexscope names the DEFLATE step
that wrote it — a literal byte, or a copy of so many bytes from so far back,
often exactly one row up — marks the file bytes that hold that step, and
shows the copy's source in the picture. Point at a byte of IDAT and the
pixels it became light up. One click opens that step in the DEFLATE player.
An interlaced PNG stores its pixels in seven passes, a coarse picture first;
hexscope follows each pixel to its pass, and a slider shows the picture after
each one, with how much of the file it took.

A JPEG has no bytes per pixel: its picture is cut into blocks, usually 16×16
pixels, each written as a run of Huffman-coded bits. hexscope reads the scan
to find where each block begins, so pointing at the photo marks the bytes
that draw that block and says how many bits it took; pointing at a byte of
the scan outlines its block. **Where the bits go** lights the photo by what
each block costs: detail and noise take bits, a clear sky almost none. A
progressive JPEG writes each block a little at a time, over several scans —
averages first, then bands of detail — and hexscope maps every scan: a pixel
lists what each scan spends on it, and a slider shows the picture after each
one, blurry at 7% of the file and sharp at the end.

**Shows who built a WebAssembly module.** Its custom sections say how it
was made — the language, the compiler and its version, every function's
name, a link to its source map, DWARF debug info — and its data often holds
paths from the computer that built it, with the user's name in them.
hexscope lists each section and entry, finds those, and saves a copy
without the custom sections, the code and data copied byte for byte.

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

[How DEFLATE works, visually](https://hexscope.pages.dev/deflate) tells the
story on a short text, with the same player beside it: each section moves
the player to the step it describes.

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
| The whole app, gzipped | ~227 KB (WebAssembly 197 KB) | — |

The byte view draws only the rows on screen, so file size does not affect
scrolling. The player never holds the stream's steps in memory: it asks for
them in small batches, and the decoder resumes from the nearest checkpoint.

## How it is built

- **`crates/hexscope-core`** — the parsers, in Rust with no runtime
  dependencies: PNG with its own DEFLATE decoder, written to be paused and
  resumed one step at a time; JPEG segments, and a Huffman scan reader that
  finds each block's bits in every scan, progressive ones included, without
  decoding pixels; EXIF in either byte order; and a
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

Every part a parser can name must have an explanation: the coverage test in
`crates/hexscope-core/src/docs/tests.rs` fails otherwise. Specification links
are checked by hand, since it needs the network:

```bash
python3 scripts/check-spec-links.py
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

It installs as an app and works offline after the first visit: a service
worker (`apps/web/public/sw.js`) caches the page and its hashed assets, and
fetches nothing the page does not ask for. On Android, the installed app
appears in the share sheet; a shared file waits in a cache for the page to
open it, and never touches the network. The link preview and icons are
rendered from `scripts/brand/*.html` by `sh scripts/make-brand-images.sh`.

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
4. **WebAssembly modules** — sections, imports and exports, who built it.
   *Done; components are listed, not decoded.*
5. **JPEG blocks** — which bytes draw which part of a photo, scan by scan,
   sequential or progressive. *Done.*

No analytics, ever. A tool people use to look at suspicious files has no
business watching them.

## License

Licensed under either of [MIT](LICENSE) or [Apache License 2.0](LICENSE), at
your option.
