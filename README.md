# hexscope

**See what your file reveals about you — before you send it.** Inspect metadata, hidden content,
unexpected structure and damage — then make a clean copy when hexscope can do so
safely. Your files stay on your device.

[Open hexscope](https://hexscope.org/?sample=photo.jpg) ·
[Install the CLI](https://crates.io/crates/hexscope-cli) ·
[Privacy](https://hexscope.org/your-files-stay-private) ·
[Guides](https://hexscope.org/remove-location-from-photo)

![A sample photo opened in hexscope: it gives away eight things, its location on a map, and a clean copy confirmed to have none of them](docs/demo.gif)

## Try it

Open the [web app](https://hexscope.org/) and choose a file, or
[load the sample photo](https://hexscope.org/?sample=photo.jpg). hexscope
shows a plain-language summary first. Select a finding to see the exact bytes
behind it; where supported, save a clean copy from the same page.

It also works offline after the first visit. File parsing happens in your
browser: hexscope does not upload your files or send telemetry. The website
counts how many people visit — totals per day, without cookies, ids or
addresses, never anything about a file — read only by its author.

## What you can do

- **Check before sharing.** Find photo location and camera details, PDF content
  hidden under black boxes, document history and comments, email indicators,
  embedded files, and more.
- **Inspect the evidence.** Explore a file's structure and bytes, search within
  it, compare files, and follow image pixels to the bytes that produced them.
- **Make a copy.** Remove supported metadata, redact PDF text or selected areas,
  and recover supported damaged PNG, JPEG and ZIP files. hexscope explains what
  it changed so you can inspect the result.
- **Learn how formats work.** Step through DEFLATE decoding and explore how PNG
  and JPEG data maps to the image.

### Formats

hexscope reads PNG, JPEG, HEIC/HEIF, AVIF, WebP, GIF, TIFF, SVG, MP4/QuickTime, PDF, ZIP
archives and common files inside them (`.docx`, `.xlsx`, `.pptx`, LibreOffice's
`.odt`, `.ods`, `.odp`, `.apk`, `.jar`, `.epub`), RTF, legacy Office files (`.doc`, `.xls`, `.ppt`), `.eml`, Outlook `.msg`,
and WebAssembly modules. What it can inspect or change varies by format. For
known limits — including password-protected PDFs and legacy Office document
text — see [known issues](docs/known-issues.md).

hexscope is a file-structure inspector, **not a virus scanner**. It does not
open links or follow redirects. A finding is an explanation of what the parser
can establish from the file, not a guarantee that a file is safe.

## Use the CLI

Install the published `hexscope` command:

```sh
cargo install --locked hexscope-cli
```

```sh
hexscope check photos/                              # inspect files in a folder
hexscope check --fail-on location,serial photos/    # choose what fails the check
hexscope check --fail-on misnamed uploads/          # a Word file named .pdf, a HEIC named .jpg
hexscope clean --out clean/ photos/                 # write clean copies separately
hexscope repair --out recovered/ broken.png         # recover supported damaged files
hexscope redact --text "Private Name" report.pdf    # redact matching PDF text
```

`check` reads files and folders recursively. By default, it exits with `1` when
it finds disclosures, hidden content or damage; `--fail-on` can select finding
types, and `--fail-on none` reports without failing. Exit `2` means a usage or
file error. Symlinks and unreadable folders stop the scan with an error rather
than being silently skipped. Use `--json` for newline-delimited JSON or `--sarif`
for SARIF 2.1.0.
Run `hexscope --help` for all options; what stays the same from one version
to the next — options, exit status, JSON and SARIF — is in
[docs/cli.md](docs/cli.md). Clean and repaired files are written as
copies by default; `clean --in-place` is available when you explicitly want to
replace the originals.

### GitHub Actions

The published Action checks paths and annotates selected findings on their files:

```yaml
- uses: actions/checkout@v7
- uses: MykytaStel/hexscope@v0.6.0
  with:
    paths: photos docs
    fail-on: location,serial,covered,hiddentext
```

At a released version, the Action downloads that release's build of
`hexscope` for the runner (Linux, macOS or Windows), checks it against its
checksum and GitHub's signed record of where it was built, and so starts in
seconds; at a branch or commit it builds the command from source.

Annotations name the file and finding kind, but leave actual values out of CI
logs. When `check` runs in GitHub Actions, its default human output is suppressed
for the same reason; explicitly requested `--json` and `--sarif` remain detailed.

For other CI systems, save a SARIF report with
`hexscope check --sarif --fail-on none PATH...` and upload it to your code
scanning service.

## Guides

Photos and videos:

- [EXIF viewer: what a photo's metadata says](https://hexscope.org/exif-viewer)
- [How to remove EXIF and metadata from a photo](https://hexscope.org/remove-metadata-from-photo)
- [How to remove the location from a photo](https://hexscope.org/remove-location-from-photo)
- [How to remove metadata from a video](https://hexscope.org/remove-metadata-from-video)
- [How to remove metadata from an SVG](https://hexscope.org/remove-metadata-from-svg)
- [Was this picture made by AI?](https://hexscope.org/ai-image-metadata)
- [What a screenshot gives away](https://hexscope.org/what-a-screenshot-gives-away)

Documents and email:

- [How to remove metadata from a PDF](https://hexscope.org/remove-metadata-from-pdf)
- [How to remove metadata from a Word document](https://hexscope.org/remove-metadata-from-word)
- [How to remove metadata from LibreOffice and RTF files](https://hexscope.org/remove-metadata-from-libreoffice)
- [Check a document before you send it](https://hexscope.org/check-document-before-sending)
- [How to black out a PDF properly](https://hexscope.org/black-out-a-pdf)
- [Text no one can see in a PDF](https://hexscope.org/hidden-text-in-pdf)
- [Deleted from a PDF, but still inside it](https://hexscope.org/pdf-hidden-versions)
- [Is this email real?](https://hexscope.org/is-this-email-real)

Files and formats:

- [What type of file is this?](https://hexscope.org/what-file-type-is-this)
- [Why won't my PNG open?](https://hexscope.org/png-wont-open)
- [Explore DEFLATE step by step](https://hexscope.org/deflate)
- [Your files stay on your device](https://hexscope.org/your-files-stay-private)

Every guide is also in Ukrainian, under [hexscope.org/uk/](https://hexscope.org/uk/).

Film scans and research notes:

- [Film photo lab: negative conversion, border crop and output inspection](docs/film-lab.md)
- [Verified photo batches, CLI/web film rolls, TIFF16 and local privacy mosaic](docs/photo-platform.md)
- [Reproducible real-scan measurements and research limits](docs/film-evaluation.md)

## For Rust users and contributors

Published on crates.io, at the same version as the site:

- [`hexscope-cli`](https://crates.io/crates/hexscope-cli) — command-line tool.
- [`hexscope-core`](https://crates.io/crates/hexscope-core) — parsing library.
- [`hexscope-inflate`](https://crates.io/crates/hexscope-inflate) — explainable
  DEFLATE and zlib decoding.

To run the web app locally, install the pinned Rust toolchain, Node.js 24+,
pnpm and the matching `wasm-bindgen` CLI, then run:

```sh
rustup toolchain install
cargo install wasm-bindgen-cli --version 0.2.128 --locked
pnpm install
pnpm dev
```

See [CONTRIBUTING.md](CONTRIBUTING.md) for project structure, checks and
contribution guidance. Known product and format limits are tracked in
[`docs/known-issues.md`](docs/known-issues.md). See the
[project roadmap](docs/roadmap.md) for planned work and research candidates.

## License

Licensed under either [MIT](LICENSE) or [Apache License 2.0](LICENSE), at your
option.
