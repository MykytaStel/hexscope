# Hexscope roadmap

This is the project's shared product and engineering roadmap. It describes priorities and decision gates, not release dates. Work moves forward in small, reviewable slices; a feature is not considered complete until the user-facing behavior, CLI/core parity where applicable, tests, and hosted state agree.

## Versions — what we do and when

Each version is a short list with a target date and a plain test of done.
Dates are targets, not promises: a version ships when its list is done, and
what slips moves to the next. What shipped is in [CHANGELOG.md](../CHANGELOG.md);
how a version is made is in [releasing.md](releasing.md).

| Version | Theme | Target |
|---|---|---|
| 0.1.0 | First published version | shipped 2026-10-02 |
| 0.2.0 | Checked clean copies, film lab, redesign, Ukrainian, private counter | shipped 2026-10-11 |
| 0.3.0 | Trust in every clean copy, a clearer analyzer, a preview and a demo | shipped 2026-10-12 |
| 0.4.0 | Scans and the formats people send: TIFF, LibreOffice, RTF, misnamed files, one build per family | shipped 2026-10-13 |
| Launch | The author's device test, then the posts | when the author has tested |
| 0.5.0 | Faster, smaller, easier to change; what Office on Windows leaves; all of it in Ukrainian | shipped 2026-10-15 |
| 0.6.0 | All of it in Ukrainian, release builds for the Action, metadata removal measured against ExifTool and MAT2 | shipped 2026-10-16 |
| 0.7.0 | SVG drawings, tools under the bytes, names in files no format reads, a smaller first download, the CLI's contract | shipped 2026-10-18 |
| 0.7.1 | "Clean with hexscope" in Finder, Send to and GNOME Files; Homebrew | shipped 2026-10-19 |
| 0.7.2 | `cargo install` with Rust 1.88 and later | shipped 2026-10-19 |
| 0.8.0 | Archive Trace, safer sharing, evidence-bound email claims, bounded large-file analysis, and read-only MP3/FLAC/Ogg tags | shipped 2026-10-25 |
| 0.9.0 | A lighter page until a file is chosen, large files on phones, sharing from the film lab, browsing large archives | GitHub Release published 2026-10-30; Homebrew tap updated 2026-11-01 |
| 0.10.0 | Chained Ogg metadata, safer audio clean copies, Ultra HDR clean copies, batch usability and core reliability | shipped 2026-11-02 |
| 0.11.0 and later | Substantial collections of features, reliability fixes and usability improvements | collect merged post-tag work; ship each collection when ready |
| 1.0.0 | Stable | when its criteria hold, not before 2026-11-30 |

### 0.9.0 — shipped 2026-10-30

The file-intent loading, Film Lab JPEG handoff and archive Files view are in
the code, alongside large-file memory and mobile interaction improvements.
The archive view has a recorded 100,000-entry browser benchmark; an emulated
phone is not a measurement of a physical device's memory limit.

The GitHub Release is published with all ten platform archives and checksums;
the site, crates and `v0.9.0` tag were published together. The Homebrew formula
now points at `0.9.0`; upgrading from `0.8.0` verified the archive checksum and
the formula's installed-version test passed.

The physical iOS Safari and Android Chrome checks recorded for this release
remain open, including large-file time and memory behavior, the Film Lab share
sheet, and screen-reader navigation. The HDR and audio work merged after this
tag shipped in `0.10.0`; published tags remain immutable.

### 0.10.0 — shipped 2026-11-02

The work merged after `v0.9.0` shipped together: the core reads chained
Vorbis and Opus metadata and checks Ogg page order and checksums; MP3 and FLAC
clean copies remove supported metadata and are read back for verification;
Ultra HDR copies preserve the bounded gain map while removing personal photo
metadata; batch selection is easier to operate by keyboard and across filters;
and the video parser safely handles malformed dates. The site, five-platform
CLI release, four crates and Homebrew formula are published as `0.10.0`.

Physical iPhone Safari checks remain open for the Film Lab share sheet and
large-file time and memory behavior. Browser emulation and synthetic fixtures
do not establish those device behaviors.

### 0.3.0 — shipped 2026-10-12

What was planned as 0.3.0 and 0.4.0, released together: confirmed removal
for documents, HEIC photos and videos, the coverage table written from the
core, a corpus of real kinds of files as a snapshot test, Metadata as its own
view, a status bar under the bytes, links that open a sample at a view and a
part, a new link preview and a demo. Details in the changelog.

### Launch (when the author has tested)

1. Real devices: a photo picked in iPhone Safari keeps or loses its GPS — the
   answer goes into the guide and the verdict.
2. Posts: Show HN, r/privacy, r/rust, DOU — written together, posted by the
   author.
3. Done when: two weeks of the counter's numbers are read, and what people
   open most decides 0.4.0's order.

### 0.4.0 — shipped 2026-10-13

Scans and the formats people send, two weeks early: every item below is done.

1. ~~TIFF in the core: scanners' and cameras' metadata, location, and a copy
   blanked in place.~~ Done 2026-10-13: read by the EXIF reader, its copy
   empties the tags and the EXIF and GPS tables where they lie; confirmed
   removal covers it.
2. ~~LibreOffice documents (ODT, ODS, ODP): author, dates, editing time,
   comments, tracked changes.~~ Done 2026-10-13: meta.xml (who made it,
   who saved and printed it, the program and system), the printer in
   settings.xml, comments, tracked changes with the deleted text, the
   first page's thumbnail and the photos inside; its copy empties those
   parts, accepts the changes and blanks the thumbnail; confirmed removal
   covers it.
3. ~~RTF: author, company, operator.~~ Done 2026-10-13: the information
   group, generator, template path, custom properties, the names in the
   revision table and comments, and the text tracked deletions hold; its
   copy leaves those out, accepts the changes and deletes the comments;
   confirmed removal covers it.
4. ~~A file whose name says one format and whose bytes another — a Word file
   named `.pdf` — is named as such by the core, for the CLI too.~~ Done
   2026-10-13: `hexscope_core::naming`, down to the kind of document a ZIP
   holds; the CLI prints it, puts it in JSON and SARIF, and fails on it with
   `--fail-on misnamed`.
5. ~~The full parser split per format, so a new format does not grow what
   every visitor downloads.~~ Done 2026-10-13: one build for pictures and
   movies, one with PDF, one with Office, email and WebAssembly. Done when: the corpus snapshot shows TIFF, ODT and
   RTF read, and confirmed removal covers them.

### 0.5.0 — shipped 2026-10-15

Shipped three weeks early. Items 1 and 5 are done and 4 in part; 2 is
begun and 3 waits for the author's phone. What is left moves to 0.6.0.

1. The analyzer's script apart from the landing page's: the first page loads
   less, and the page budget has room again. Begun 2026-10-15: the Ukrainian
   dictionaries load only for a page read in Ukrainian — a guide's script
   went from about 78 KB to 7 KB. Done 2026-10-15: the analyzer is a module
   loaded when the page is idle or a file is asked for — the home page
   starts with 18 KB of script instead of about 85 KB.
2. The stylesheets merged into one set of tokens and layers, without
   overrides fighting each other. Begun 2026-10-15: what every page shares
   (the top bar, buttons, links, focus, dialogs) is one file of its own, and
   an article loads it with its own type — 3.8 KB gzipped instead of 21 KB.
   `apps/web/scripts/style-diff.mjs` compares the computed style of every
   element in two builds, so the rest can be merged one file at a time.
3. Large files on phones within a time and memory budget. Waits for the
   author's phone: where a tab gives out decides the budget.
4. An accessibility pass over the new design: screen readers, keyboard,
   zoom, both themes. Done in part 2026-10-15: axe clean in both themes, the
   phone bar says which view is open under More; the screen-reader pass waits
   for a device.
5. Done when: the first page is lighter than in 0.2.0 and axe is clean.

Also done for 0.5.0, from two audits (2026-10-15): what Office on Windows
leaves (the folder, the template's path, the printer, SharePoint's
properties, editing-session numbers) found and taken out; a PDF's notes,
authors and carried files out of its copy; links under shared suffixes
(`.com.ua`, `.co.uk`) told apart; a typed-in SPF pass named as forgery;
copies no longer called incomplete for a finding read whole; articles in
the reader's language as their own pages; every sentence and explanation a
reader meets in Ukrainian, held by a test over every test file; one key
sort in place of three, 1.4–2.8 KB off each WebAssembly build.

### 0.6.0 — shipped 2026-10-16

From the third audit (2026-10-15): about 250 lines of the analyzer that
stayed in English, now Ukrainian, held by an end-to-end test over every
sample and test file; the command built for five systems on each release,
which the Action downloads and checks instead of compiling; hexscope,
ExifTool and MAT2 on the same 45 files, each copy read back by both
(docs/research), which found Word 97's own record of a document and had it
blanked; two guides; the browser bridge in modules; 134 overridden CSS
declarations out, with a test. What was planned as 0.6.0 is now 0.7.0.

### 0.7.0 — shipped 2026-10-18

From the fourth audit and what other metadata tools are asked for: SVG
drawings read and cleaned in the small build; files of no known format
searched for home folders and email addresses; values, SHA-256 and entropy
under the bytes, and `@offset` in find; a table of findings on an Actions
run's page; the CLI's options, exit codes and outputs written down and held
by a test. The sentence explaining each part left the WebAssembly for files
fetched once a file is open: the first download is 15–21 KB smaller. CI
runs its browser tests in four parts (9 minutes to under 5), and each
change's notes are a file of their own. From this release the Action can be
listed on GitHub Marketplace. What was planned as 0.7.0 is now 0.8.0.

### Next, after the launch

- An HDR photo's clean copy now keeps its MPF gain map and supported
  Ultra HDR/ISO 21496-1 settings, removes personal metadata from both JPEGs,
  and verifies the copy against the source findings. The checked-in fixture
  covers this path; opening and comparing the clean copy on a physical iPhone
  remains unverified.
- The core says what it finds as codes with values, not English sentences,
  and the page words them in each language.
- The stylesheets in one set of tokens, checked against screenshots of
  every page and state before and after.
- Research to publish: what messengers and social networks take out of a
  file sent through them, and what still ties a cleaned file to its author.

### 0.10.0 — safer media metadata, release parity and browser reliability (in progress)

The Rust core now reads metadata in a bounded MPF gain map, includes its
findings in the photo's evidence, and makes a fixed-length clean copy that
keeps supported HDR settings and both JPEG pictures. Unknown or incomplete
required map metadata fails closed. The Web explains the retained gain map
before cleaning, in English and Ukrainian.

The synthetic iPhone-format fixture, read-back verification and automated
browser flow cover the HDR code path. CI also runs a focused analyzer and
keyboard smoke in Firefox and WebKit, alongside the full Desktop Chrome and
Pixel 7 Chromium suites. FLAC clean copies remove Vorbis comments and entire
embedded picture blocks while retaining other supported metadata and audio
frame bytes; unknown or incomplete metadata keeps the file read-only. MP3
clean copies remove up to 16 consecutive leading ID3v2 tags and
preserve the compressed audio frames byte for byte. Ogg remains read-only.
Vorbis and Opus tags are collected across sequential chained streams. Ogg page
sequence and checksums are validated through the end of supported streams,
while interleaved streams remain incomplete and audio packets are not decoded.
Before calling HDR device-verified, open and compare the clean copy on a
physical iPhone.

Batch results now include a one-step control to select or clear every file
that can make a clean copy, even when a filter hides some eligible files.
The bulk selection control is available in English and Ukrainian. Batch
filters, bulk selection, and individual copy checkboxes retain keyboard focus
when the list redraws.

### 1.0.0 — Stable (when these hold)

- The CLI's flags, exit codes, JSON and SARIF are documented and kept.
  Documented in [cli.md](cli.md) on 2026-10-17, and held to the program by
  `crates/hexscope-cli/tests/contract.rs`.
- Every format with a clean copy has confirmed removal.
- Four weeks with no reported false alarm left unfixed.
- Tested on iOS Safari, Android Chrome, desktop Chrome, Firefox and WebKit.
- crates.io carries the same version as the site.

## Product direction

Hexscope should help a person understand what a file reveals before they share it, then let them act safely. It should also make file formats understandable to curious people, down to the bytes. Analysis stays on the user's device, conclusions stay tied to parser evidence, and unsupported cases are described as unchecked rather than clear.

## Principles

- **Local by default:** no file uploads, accounts, telemetry, or server-side inspection. The website counts visitors as daily totals only — no cookie, id or address, nothing about files — read only by the maintainer, from the database (`pnpm stats`).
- **Evidence before verdict:** explain what was read, what was changed, and what could not be checked.
- **One core, several interfaces:** keep format facts and conservative decisions in Rust core; expose consistent results to the Web, CLI, and GitHub Action.
- **Copies stay under user control:** never silently alter the source; keep a successfully produced copy available when post-write verification is incomplete, and refuse to produce one when source content cannot be inspected safely.
- **Measure before expanding:** protect parser safety, browser memory, gzip size budgets, accessibility, and real-device usability.
- **Teach without getting in the way:** give the plain-language answer first; let curious users open the structure, algorithms, and bytes.

## Shipped foundation

### Shared clean-copy verification — shipped in PR #14

- Rust core compares parser-backed facts before and after cleaning and emits a value-free report.
- Web verifies the actual produced bytes in the worker and retains its QR and selected-PDF-text checks.
- CLI `clean --verify` reads the written output back and returns a conservative status.
- PR #14 first shipped confirmed removal for complete JPEG file-scope findings: camera, serial number, owner, and location. The PDF Info/XMP extension below adds a second evidence-backed slice; other formats and findings remain unchecked until parser and cleaner behavior have proof and fixtures.
- Web output verification skips the additional parse above 10 MiB; copy creation remains available.

## Near-term priorities

### P0 — Expand verification without creating false reassurance

1. **Publish a format/finding/cleaning coverage matrix.** Done: what each format's clean copy is confirmed to remove is in [`verification-coverage.md`](verification-coverage.md), written from the core's contract and held by a core test; every format and kind of finding hexscope reports is listed in [`cli.md`](cli.md), held by a test that runs the command over every test file. These replace the hand-written matrix drafted on 2026-10-03, which described the earlier JPEG-only state.
2. **Expand PDF verification in narrow slices.** Shared core verification now confirms removal only for complete, file-scope Info/XMP findings: author, title, subject, keywords, application, producer, created, modified, and history. An unsupported fact observed in the cleaned output is reported as present; claims that unsupported revision findings, hidden or covered text, or other PDF content were removed remain unchecked until parser and cleaner evidence proves coverage. Keep Web QR rescanning and selected-PDF-text checks supplemental.
3. **Add real regression cases for incomplete documents.** The PDF slice now covers surviving hidden text, hidden and revision text absent from copies but left unchecked, unreadable page streams, and findings present only in the compared output. Extend equivalent cases to other formats as their verification coverage grows; every uncertain path must stay unchecked.
4. **Make coverage understandable in the result UI and CLI.** The shared clean-copy result now names what was removed, remains, or was not checked, with a plain-language reason. Keep any successfully produced copy available when post-write verification is incomplete.

### P1 — Make the core and CLI a dependable platform

1. **Keep a single capability contract.** Derive coverage labels and reason codes from the shared core contract so Web, CLI, JSON, and Action output cannot silently disagree.
2. **Improve batch workflows.** Make per-file results, aggregate exit status, and machine-readable output useful for folders and CI without hiding which files were unchecked.
3. **Harden parser and cleaner boundaries.** Extend property tests, fuzz targets, malformed-file fixtures, and read-after-write verification as new format coverage is added.
4. **Protect performance and package budgets.** Benchmark parsing and cleaning on representative large files; lazy-load optional work and keep existing JavaScript/WASM limits enforced in CI.
5. **Finish user-facing accessibility and localization as features grow.** Preserve keyboard operation, screen-reader announcements, reduced-motion behavior, mobile layouts, and matching Ukrainian/English explanations.

## Film Scan Inspector and local film photo lab

The feature is for a person preparing a scan of a film photograph to send. Hexscope receives a digital scan, so it can inspect both the digital file and evidence visible from the physical film.

### Two layers to inspect

- **Scan-file data:** camera/scanner and software fields, dates, location, embedded previews, and other metadata that the scanning or editing workflow may have written.
- **Visible film evidence:** if the scan includes the film border, identify frame boundaries, perforations, and readable edge markings. These may expose film type, emulsion/frame codes, or roll indexing. Such markings are clues, not necessarily personal identifiers.

Scanning creates a digital image file, so the scan can also carry metadata from the digitizing or editing workflow; embedded metadata is not guaranteed to be present or consistent across scanners. Fujifilm's guide documents edge markings, DX codes, and frame numbers on specific roll films. Kodak KEYKODE describes a motion-picture workflow for linking frames and rolls, so it must not be generalized to every still-photo negative. Realistic film grain can also be rendered onto digital images, which makes grain a weak clue rather than proof of origin. See the [Library of Congress scanning guide](https://blogs.loc.gov/thesignal/2014/03/personal-digital-archiving-the-basics-of-scanning/), [CIPA Exif standard](https://www.cipa.jp/e/std/std-sec.html), [Fujifilm Professional Data Guide](https://asset.fujifilm.com/www/us/files/2020-03/85d928f44b0df3b2a95913e46608881d/ProfessionalFilmDataGuide.pdf), [Kodak KEYKODE reference](https://www.kodak.com/en/motion/page/keykode-numbers/), and [film-grain rendering research](https://onlinelibrary.wiley.com/doi/abs/10.1111/cgf.13159).

### Current implementation

- The web app locally inspects JPEG, PNG and WebP pixels in its worker for repeated high-contrast openings and frame-like edge boundaries. The result lists the observed edges and signal agreement; it does not infer film origin from grain.
- The [film photo lab](film-lab.md) provides oriented before/after previews, explicit border cropping, film-base-calibrated color/monochrome negative conversion, exposure and contrast, and validated local recipes. It creates a bounded JPEG sharing rendition and re-reads the actual output with the shared core plus supplemental QR/border analysis. The source remains intact.
- The [real-scan evaluation](film-evaluation.md) records four published positive NASA flight-film scans, with 0/4 complete-frame and perforation false positives. This small, single-source set does not measure recall or raw-negative color recovery; the detector remains a visual clue rather than a validated origin classifier.
- Before making provenance claims or adding edge-marking recognition, validate precision and false-positive rates on consented or public examples of full-border scans, cropped scans, digital frames, and low-contrast scans. RAW/TIFF, 16-bit processing, roll-wide calibration and photographic evaluation remain future work.

### Stages and decision gates

1. **Discovery:** audit current JPEG facts and cleaners; gather lawful, consented or synthetic examples of flatbed scans, camera scans, negatives, positives, cropped scans, full borders, and digitally simulated film looks. Do not add private user photographs to the repository.
2. **MVP:** start with the existing JPEG path. Report embedded metadata separately from visible image content. Detect only evidence with measured accuracy; label uncertain origin as likely/uncertain rather than verified.
3. **User action:** offer an explicit new copy to remove supported metadata. If visible edge markings should be hidden, offer a separate crop/redaction preview; never imply that metadata cleaning removes text baked into pixels or change the master scan silently.
4. **Recognition expansion:** evaluate local OCR for edge markings only after a labeled sample set, precision/recall targets, language coverage, worker isolation, and bundle/runtime cost are known. Add TIFF or other scan formats only after parser and cleaner coverage is proven.
5. **Acceptance:** test genuine film scans, digital photographs with simulated grain, partial/cropped borders, unreadable markings, and damaged metadata. Grain alone must never be used as proof of film origin.

## Research and differentiation candidates

These are ideas to evaluate, not committed milestones. Select them by user value, evidence quality, and fit with local-first constraints.

### Local privacy mosaic for a batch of photos

Show when a group of photos reveals a pattern that a single file does not: repeated device identifiers, a location cluster, or a sequence of places. Keep analysis on-device, show the exact evidence and its limits, and let the user decide whether to clean or exclude files. Avoid inferring a home or identity from weak signals.

### Reproducible clean-copy receipt

Create an optional, value-free receipt that records the verifier/schema version, capabilities applied, and outcome for a specific produced copy. Design the privacy model before making receipts shareable; a receipt must not look like a safety certification or leak file contents.

### File-format lab

Extend Hexscope's byte-to-meaning experiences with more step-through views: how a JPEG scan is assembled, where a PDF page draws hidden text, or how an archive entry maps to compressed bytes. Keep each lesson connected to the actual file under inspection and its specification.

### Trustworthy format coverage scorecard

Build a continuously tested corpus from synthetic fixtures and redistributable samples. Track coverage, false reassurance, false positives, parse completeness, performance, and size per format. Use it to decide what can move from “unchecked” to “verified.”

## Longer-term areas

- **Architecture:** keep parsers, facts, cleaning, and verification separated behind small stable interfaces; add abstraction only where multiple formats need it.
- **Science and algorithms:** measure detection quality and failure modes; use deterministic methods first. Keep OCR or machine learning local and optional, with published evaluation data and explicit uncertainty.
- **Web product:** keep the main path short—summary, evidence, safe action—while preserving the detailed byte inspector as an optional depth layer.
- **CLI and CI:** maintain predictable exit codes, stable JSON/SARIF, batch support, and parity with Web for the same supported evidence.
- **Security and reliability:** continue fuzzing hostile inputs, testing corrupt and oversized files, and verifying real output bytes rather than trusting the requested operation.

## Out of scope unless the product direction changes

- Uploading user files or reports to a service for analysis.
- Accounts, per-person analytics or tracking, or background storage of inspected files.
- Calling Hexscope a malware scanner or treating a heuristic as proof of authenticity.
- Silently cropping, redacting, or overwriting a source image.

## Photo platform implemented in the current slice

- PNG/WebP camera, serial, owner and location clean-copy verification uses the shared core contract.
- CLI folder batches and browser ZIP copies include per-file, value-free receipts after bounded readback; duplicate output names cannot silently overwrite one another.
- Rust owns RGBA8/RGBA16 film rendering. CLI and browser roll tools reuse version-1 recipes and one estimated/calibrated film base, including TIFF16 output. Optional browser codecs load on demand.
- Photo privacy mosaic compares stored serial values, nearby GPS points and dated-file order locally across a batch. Saved reports contain indexes rather than raw metadata.
- [Feature guide and limits](photo-platform.md) describes source bounds, unsupported TIFF layouts and color assumptions.

Next research work: scanner color management, labeled raw color-negative/reference pairs, frame recall across diverse scanners, and OCR character-error evaluation. Existing receipts do not certify visible content safe.

## How to use this roadmap

Start with the highest-priority unfinished user problem. Before each substantial feature, verify the parser and cleaner can support the claim, define fixtures and failure behavior, then implement one bounded end-to-end slice across the necessary interfaces. Update this file when evidence changes priority or when work ships; keep speculative candidates clearly marked until selected.
