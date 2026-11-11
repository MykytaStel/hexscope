# Hexscope roadmap

## Audit follow-up: publication tools and reliability

The 2026-11-11 implementation adds bounded input/tree admission, worker
recovery deadlines, stronger cleaner fuzz invariants, typed inspection status,
shared Core publication profiles/plans, semantic comparison, copy packages,
local OCR suggestions, evidence origins and a synthetic damage laboratory.
See [publication tools](publication-tools.md) and the
[implementation record](audits/2026-11-11-audit-implementation.md).

The next acceptance gate is physical iPhone/Android behavior, HDR display,
VoiceOver/TalkBack and measured OCR recall/false positives on a representative
corpus. Automated browser profiles and a synthetic OCR example do not close
that gate. No release version is published by this change.


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
| 0.11.0 | Hardened clean copies, one wording for every interface, one set of styles, Ogg clean copies | shipped 2026-11-03 |
| 0.12.0 | Make batch photo privacy mosaic relationships easier to understand and navigate | shipped 2026-11-03 |
| 0.13.0 | Per-format coverage reports for local inputs, with safer handling of unsupported files | shipped 2026-11-08 |
| 0.14.0 | Batch coverage in the Web, format-backed quality gates, and verified-copy confidence on phones | shipped 2026-11-08 |
| 0.15.0 | Portable batch reports and reliability fixes | shipped 2026-11-08; Homebrew tap updated 2026-11-09 |
| 0.16.0 | Private reports, file maps, and safer photo/PDF inspection | shipped 2026-11-09 |
| 0.17.0 | Source-linked inspectors across file families, plus bounded WAV inspection | shipped 2026-11-09 |
| 0.18.0 | Verified WAV cleaning and deeper format inspection | shipped 2026-11-10 |
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

### 0.11.0 — shipped 2026-11-03

Mostly holding what 0.8–0.10 added, so that 1.0 has little left, and one
new kind of clean copy. In order:

1. **Clean copies hold under damage.** A fuzz target makes a clean copy of
   any input and requires it to read back as the same format, clean again
   and compare against its source; property tests damage a cleanable HDR
   photo, MP3 and FLAC. Done 2026-11-02; fuzzing also found and fixed a
   copy that would open as another format.
2. **One wording for every interface.** The core reports what it finds as
   codes with values, not English sentences; the page, CLI, JSON and SARIF
   word them from the same codes, so they cannot drift apart.
   Done 2026-11-03: file-level fact wording comes from the core catalogue in
   the page and CLI; structured JSON keeps the code and value.
   Verification reason groups and default messages also come from one core
   table. Parser problems now carry the page's stable explanation code, and
   CLI, JSON, GitHub annotations and SARIF use its wording.
3. **One set of styles.** Done 2026-11-03: light and dark palette values
   are defined once in the foundation tokens, and shared chrome, dialogs,
   the story player and photo tools use those tokens. The style diff now
   checks rendered pixels as well as computed styles. Its full matrix
   compared 1,014 states and 996 screenshots with no pixel differences;
   `sample.png` computed styles also match across its views, screen sizes
   and themes.
4. **Keyboard and screen reader on the main paths**: done 2026-11-03. An
   automated browser flow opens a file from the keyboard, checks the live
   answer, makes and verifies a copy, then reads and saves a batch. Axe passes
   on the answer, copy report and batch. Physical screen-reader checks remain
   open.
5. **Ogg clean copies**: done 2026-11-03. Core replaces complete Vorbis and
   Opus comments, rebuilds page checksums, reads the copy back and keeps audio
   packets byte for byte. CLI `clean --verify`, Web and batch support it;
   multiplexed, incomplete, unsupported and invalid header layouts stay
   read-only.
6. **Small debts**: done 2026-11-03. A heap probe of a flat archive with
   100,000 filenames retained about 0.94 MB for the index and 8.6 KB for 50
   visible row objects in local Node 26.11; this synthetic local measurement
   is not a browser or phone measurement.

Every item above is merged and the automated release gates pass. The site is
deployed, the `v0.11.0` GitHub Release has all ten platform archives and
checksums, all four crates are published, and the Homebrew formula is updated
and passes `brew test`. At the author's request, physical iPhone and Android
checks from 0.9.0 and 0.10.0 remain explicitly deferred and unverified. The
batch privacy mosaic is 0.12's lead.

### 0.12.0 — release candidate

The core already finds repeated stored serials, nearby GPS points, and date
orders; the CLI already exports the bounded, value-free report. This release
makes those relationships actionable in the browser batch:

- Each file in a mosaic group links back to its row in the batch list. The
  list returns to the all-files view, reveals rows beyond the first page,
  scrolls to the selected file and gives it keyboard focus.
- The summary reports checked and unchecked files, the distinct files that
  appear in relationships, and the number of relationship groups.
- Selecting a relationship group in the browser chooses only its cleanable
  photos for the existing verified-copy flow. Other cleanable files are
  excluded, and each selection can still be changed in the batch list.
- Core exposes typed relationship groups; the CLI text view names and numbers
  each evidence group and can make verified copies of the unique photos in
  those groups with `mosaic --clean-related`. Its receipts map original input
  indexes to the batch receipt without paths or metadata values; the existing
  `hexscope.photo-mosaic` JSON version 1 stays unchanged.
- Matching remains evidence-only and local. The report exports no file names,
  coordinates, dates or serial values; files that could not be checked remain
  explicit. Physical-device checks stay deferred.

The implementation is merged and the post-merge CI and site deploy pass. The
0.12.0 release, its packages and Homebrew update are complete; physical iPhone
and Android checks remain explicitly deferred.

### 0.13.0 — shipped 2026-11-08

`hexscope coverage [--json] PATH...` reports complete, partial and unsupported
reads by format, with input bytes and elapsed time. The versioned, value-free
JSON schema contains no file names or finding values; `hexscope-core` exposes
the typed status and scorecard. A file skipped because its format is
unsupported now fails the default incomplete check; `--fail-on none` keeps a
report-only run available.

This 0.13 scorecard describes the supplied local files, not parser accuracy
or a safety certification. Version 0.14.0 adds a reusable per-format corpus
with expected findings, clean-copy readback and quality regression gates.

### 0.14.0 — shipped 2026-11-08

Batch coverage now groups complete, partial and unsupported reads by format
and lets people focus on files that need attention, without sending file
names or finding values. The clean-copy evidence gate checks all 14 advertised
formats against deterministic fixtures, including source-to-copy read-back;
MP3's verification contract only claims fields the ID3 reader inspects.
Feedback structure reports are opt-in, dependency advisory checks run in CI,
and mobile downloads preserve their file type. The iPhone save/share flow was
confirmed by the user. GitHub Release assets cover all five supported
platform targets, four crates are published, and the Homebrew formula was
updated and tested.

Android Chrome could not be tested because no Android phone was available.
The wider physical-device matrix and read-back for Web copies over 10 MiB
remain open; those outcomes stay explicitly **Not checked** until measured.

The full audit, evidence, secondary maintenance candidates and release gates
are in [the 0.14 audit](audits/2026-11-08-hexscope-0.14-audit.md).

### 0.15.0 — shipped 2026-11-08

The Web batch workflow now has a versioned coverage report and a separate,
value-free diagnostics report for rows that need attention. Both can be saved
or shared from phones that support file sharing. Diagnostics contain only a
one-based row index, detected format and status; they omit raw errors, names,
paths, contents, metadata, sizes and timestamps. Failed batch rows can be
retried without reading completed files again.

Core now reads and cleans bounded `.svgz` files, validates gzip integrity and
checks the cleaned copy by reading it back. Input and expanded XML are each
limited to 64 MiB, and only one gzip member is accepted. Physical iPhone
checks for report sharing and Files downloads remain open.

The Web sample picker and SVG metadata guide include a compressed Inkscape
example, generated alongside the SVG source, so `.svgz` can be tried without
finding or preparing a local file.

The 0.14 maintenance follow-ups are now addressed: shared batch types live
outside the photo mosaic, same-repository pull requests reuse their branch-push
CI run, and advisory scans also cover manifest changes. The advisory workflow
still runs weekly and on demand; fork pull requests run checks in the base
repository.
The v0.15.0 site and GitHub Release shipped with five CLI archives and
checksums; all four crates are published. The Homebrew tap was updated on
2026-11-09, and upgrading from 0.14.0 plus `brew test` passed.

Android Chrome and Web read-back for clean copies over 10 MiB remain
evidence-gated; browser emulation cannot close those physical-device
questions.

### 0.16.0 — shipped 2026-11-09

The first slice adds spreadsheet-friendly CSV saves for batch coverage and
diagnostic reports, alongside the existing versioned JSON. Compatible phones
can also share those CSV files directly to another app or Files. CSV output
keeps the same privacy boundary: aggregate counts, or row index, format and
status; no file names, paths, contents or raw errors. Spreadsheet formula
cells are escaped, and downloads carry the `text/csv` type.

0.16 completes the private batch-report path: coverage and diagnostics export as
JSON or CSV, reports remain available when every read fails, CSV includes an
all-files total row, and a local numbered list links diagnostics back to the
current batch. Format coverage can focus the matching rows while batch-wide
totals remain visible. Exports keep their value-free privacy boundary, and
formula-like CSV cells are escaped.

The file workspace gains a first-class byte-layout map, a HEIF item graph and
an AVIS timing timeline. The UI labels parser coverage as complete, partial or
unsupported. Signature search is asynchronous, bounded and cancellable; a
signature is only a possible match. Photo blackout regions can be edited one
by one, with clear warnings about re-encoding. PDF pages report text and image
read status, and the reconstructed preview explains what may be missing.
Image comparison gains an interactive wipe between two bounded previews.

The pair comparison remains a tool for the two files selected. A repeatable
multi-pair sharing-path lab is not part of 0.16 and must not be described as
measuring or certifying a messaging service. Treat it as a separate follow-up
with explicit pairing, privacy and evidence requirements.

Keep the 10 MiB Web read-back gates and report affected findings as **Not
checked** when the generated copy cannot be reparsed. JSON/CSV sharing from
emulated phone profiles has automated coverage; physical iPhone Files
handoff and Android Chrome remain unverified. Do not raise the limit based on
emulation alone, and require fixture-backed read, clean and read-back evidence
for any parser expansion.

The release is live on hexscope.org; the live-site check passed. The published
GitHub Release has all ten platform archives and checksums, and all four
crates.io packages are available. The Homebrew tap now points to 0.16.0; an
upgrade from 0.15.0, `brew test`, and a fresh `cargo install` all report 0.16.0.

### Formats and visual evidence selected for 0.16

- **File anatomy map.** Promote the existing byte composition into a first-class
  Map view. It remains bounded for large files, follows byte order, exposes a
  keyboard-operable role legend, and jumps to the source part in Bytes. A merged
  bar segment points to its largest source slice; the map does not claim that
  every compressed archive entry is individually visible.
- **HEIF item graph and AVIF sequence timing.** Show primary, derived, tile,
  thumbnail, auxiliary and metadata items with their `iref` relationships.
  Construction method 2 stays explicitly **not assembled** and cannot be
  mistaken for source bytes. For `avis`, read the `pict` track's `mdhd` timebase
  and `stts` sample runs into a bounded timeline; this reports stored timing,
  not decoded or played frames. Apple Live Photo pairs can be selected as a
  still and a motion clip for side-by-side inspection, but the pairing
  identifier is not checked.
- **Image wipe comparison.** When both images fit the existing 16 MiB input
  bound and a 12-megapixel-per-image preview budget, a keyboard-operable slider
  reveals one browser-rendered image over the other. JPEG orientation is
  applied. The display is explicitly approximate; colour and HDR rendering
  can differ, and parser-backed metadata/structure comparison stays separate.

These views are local and value-free in exported reports. Keep the map's
segment count bounded, the HEIF item/timing tables bounded, and decode failures
visible. The image wipe is available only when both previews are within budget;
manually paired motion previews use the browser's native local video player.

### 0.17.0 — understand the whole file

**Theme:** make the existing format coverage feel like one coherent inspector,
then deepen each family and add one carefully bounded format. The version starts
after 0.16.0 ships; this is a scope, not a release date.

The shared inspection path should answer, in order: what this file is; what
content and metadata were read; what the parser could not check; where each
finding came from in the structure or bytes; and what a clean copy changes and
confirms. Reuse the core's typed findings, parse tree, byte ranges, coverage
state and clean-copy read-back. Do not invent a second confidence model in the
Web UI. Preserve the detailed byte view as the evidence layer beneath a short
summary.

#### User-facing scope

1. **Shared evidence overview.** Give each format the same way to reach its
   summary, content, metadata/privacy, unchecked areas, structure and copy
   status. Findings link to their source node or byte range when the parser can
   establish that link. Use the states *found*, *not checked*, *removed and
   confirmed*, and *still present* only when backed by parser and read-back
   evidence. Add local search and filters for privacy fields, unchecked items,
   attachments and metadata. Keep English and Ukrainian wording aligned.
2. **Photo inspector.** Group visible image properties separately from metadata
   and privacy findings. Explain GPS, camera identifiers, thumbnails, profiles,
   animation and auxiliary HEIF/AVIF items when present. Link fields to their
   bytes or structural item, compare original and clean-copy findings, and
   provide a bounded visual before/after view. State when image re-encoding can
   change pixels, HDR, profiles or animation.
3. **Document map.** For ZIP-based Office, LibreOffice and EPUB, group entries
   by purpose and surface comments, hidden/deleted content, attachments,
   compression, encryption, read status and clean-copy outcome. Keep encrypted,
   unsupported and unread entries explicitly unchecked; link findings to their
   package entry. For legacy Office and RTF, make the difference between
   property inspection and document-content inspection clear.
4. **PDF navigator.** Connect page, text/image read status, finding, object and
   available action. Surface forms, attachments and document actions in their
   own groups. Keep OCR absence and reconstructed-preview limits visible; never
   make a scanned page look text-checked when it was not.
5. **Media and remaining family polish.** Present video tracks and stored
   timing separately from decoded media. For MP3/FLAC/Ogg, distinguish tags,
   cover art and streams, and list unsupported blocks. For SVG/SVGZ, group
   external references, scripts, embedded images and metadata in a safe local
   preview. For EML/MSG, separate claimed sender, transport route, links and
   attachments. For WASM, show imports, exports and dependencies; for unknown
   files, make the searched byte range and limits explicit.
6. **New format: WAV.** Add bounded, read-only RIFF/WAVE inspection: chunk map,
   audio format, channel count, sample rate, bit depth and duration where the
   parsed fields support it; inspect supported broadcast and INFO metadata with
   exact source ranges. Do not decode or play audio, and do not offer cleaning
   until a writer can preserve audio data and prove metadata removal by reading
   the copy back. Include malformed, truncated, oversized and padded-chunk
   behavior in the format contract.
7. **Mobile and learning paths.** Keep the overview, filters, page/entry lists
   and evidence links usable with keyboard, screen readers, zoom and narrow
   screens. Add local samples for the main inspectors and connect the existing
   byte-to-meaning lessons to the selected file where practical.

#### Delivery slices and gates

- **A — shared evidence vocabulary:** map current core facts, coverage states,
  source ranges and clean-copy receipts into one presentation contract. Begin
  with photos and documents; avoid a generic abstraction until it is used by
  more than one family.
- **B — photo inspector:** ship the grouped view, source links and before/after
  evidence for supported fixtures.
- **C — document and PDF maps:** complete package-entry and page navigation,
  including negative states for encrypted, unsupported and unread content.
- **D — WAV:** finish core parsing and coverage first, then CLI/Web display and
  English/Ukrainian guidance. Keep WAV read-only in this release unless its
  clean-copy evidence is independently complete.
- **E — family polish and mobile/accessibility pass:** finish the selected
  audio, video, SVG, email and WASM improvements; add samples and verify the
  common interaction path across the families touched.

**Implementation started 2026-11-09:** a separate `feat/0-17-format-inspectors`
branch now contains the first WAV slice. Core and Web recognize RIFF/WAVE,
read bounded chunk structure plus supported INFO/Broadcast Wave fields, and
show their source ranges and privacy categories. The file stays read-only and
audio is never decoded or played. CLI format reporting, Ukrainian copy and a
synthetic browser sample are wired in. The audio WebAssembly module and its
explanation catalogue compile.

**Progress 2026-11-09:** the shared evidence panel now filters local findings
by privacy, metadata, attachments and unchecked parts. Fact rows keep their
parser-node links; unreadable nodes and deferred media bytes link to their
source. Its incomplete state comes from the parser's coverage result and parse
tree, without a second UI confidence score. The complete Web production build
passes after generating the repository's WASM bindings. The photo card now
separates visible pixel properties from metadata, links properties to parser
nodes where available, and names stored frame/auxiliary image data without
implying that it was decoded. Clean-copy status and visual comparison continue
to use the Core read-back report and existing bounded browser preview.

The document map groups supported ZIP-based Office, OpenDocument and EPUB
entries, exposes local name filtering, compression sizes/methods and explicit
encrypted, incomplete, unsupported and unopened states. Legacy Office and RTF
explain their different content-read scope. The PDF navigator links pages to
page objects, groups findings, forms, attachments and document actions, and
keeps OCR absence and image-read status visible; password-protected PDFs show
their structural findings while page reading stays disabled. These maps do not
claim entry-level clean-copy outcomes because Core read-back findings are
file-scoped. The clean-copy card reports supported removals and its read-back
result; the map lets someone inspect the output's package parts. PDF facts are
grouped by page when the parser identifies one, while document-level forms and
actions remain in their own groups.

This family pass adds bounded source-linked summaries from the existing
parse tree: video tracks and stored timing, audio tags/artwork/container
blocks, SVG scripts/references/images, email sender/route claims and links,
and WebAssembly imports/exports/debug references. These panels do not decode
media, execute SVG/WebAssembly, verify email authentication claims or fetch
external references. Unknown files now say that metadata and cleaning were not
structurally checked, and point to the existing bounded signature search for
its actual searched range. No per-entry clean-copy removal status is claimed;
adding that requires a Core verification scope for package parts. The family
panel implementation is complete, while screen-reader, zoom and physical
device review remain open.
Uncompressed SVG up to 2 MiB now has an opt-in static preview;
the sanitizer strips active content, styles and external references before
the browser renders it. SVGZ and larger files keep their structural view only.
The sample picker now includes tiny synthetic MP3, FLAC, Ogg Vorbis and Ogg
Opus structures alongside the existing WAV sample; these contain no encoded
audio or personal metadata.

The full Web production build passed after regenerating the repository's WASM
bindings. Vite still reports existing warnings for static pages that load
`theme.js` without `type="module"`; the build completes successfully.

The inspector's source links, filters, search fields and disclosure rows now
keep 44-pixel touch targets on coarse-pointer devices, including wrapped PDF
findings and package-part names. Native controls retain keyboard focus
indication and labels.
This is a code-level accessibility pass; screen-reader, zoom and physical-phone
checks remain open and are not inferred from the CSS or production build.
Evidence links select the actual node or byte range in the currently open
file, where the existing byte-to-meaning explanations are shown. PDF page
findings are indexed once when page text loads, so filtering pages does not
rescan the full findings list for each displayed page.

Each slice needs representative fixtures and explicit incomplete/malformed
behavior. Parser or cleaner changes must retain Core + Web coverage and the
existing value-free report boundary. Keep physical phone evidence separate from
browser automation; Android remains open until a device is available. Do not
raise the 10 MiB Web read-back limit without bounded-memory design and device
measurements. Web previews never execute file scripts, follow links or decode
media solely to imply that its contents are safe.

**Release is done when** every included format has an honest read/unchecked
state; visible findings can reach their evidence when available; clean-copy
claims agree with read-back; English and Ukrainian cover the new path; and the
new WAV format is recognized consistently in Core, CLI, Web, coverage and
documentation. Any family polish that does not meet those gates moves forward
as a complete slice rather than weakening the evidence contract.

### 0.18.0 — verified audio cleaning and deeper format inspection

**Theme:** extend the existing format readers into one new evidence-backed
action, then choose further format work by user value and bounded parser scope.
RIFF/WAVE clean copies and read-only RIFX/RF64/BW64 structural inspection are
now part of the 0.18.0 accumulation branch; additional formats remain gated by
their own bounded-read and evidence requirements.

1. **Make a verified clean WAV copy.** For a complete, supported RIFF/WAVE
   file, remove complete `LIST/INFO` and whole `bext` chunks while preserving
   the format description and audio sample chunks byte for byte. Explain that
   removing `bext` also removes broadcast timing, loudness values and coding
   history. Keep malformed files, incomplete metadata and unknown chunks
   read-only. RIFX has bounded structural inspection, but stays read-only until
   a separate cleaner has its own copy evidence. RF64 and BW64 have bounded
   structural readers for `ds64` sizes and chunk traversal, but remain read-only
   until a separate cleaner has its own copy evidence.
2. **Use one read-back contract.** Core owns the WAV removal capabilities and
   verifies the actual output. Web single-file and batch flows, CLI output,
   JSON, the coverage table, and English/Ukrainian explanations must agree on
   which findings were removed, remain present, or were not checked. Audio is
   not decoded or re-encoded.
3. **Select the next format slices from explicit limits.** Candidate work
   after the WAV path includes HEIF items stored by construction method 2 and
   additional ZIP compression methods. Each
   stays a candidate until its representative inputs, malformed cases,
   parser limits, size cost, and user benefit are clear.

**Implementation update 2026-11-10:** Core, CLI and Web now identify and inspect
RIFX/WAVE using big-endian reads for form, chunk, format, INFO-size and selected
Broadcast Wave fields. RIFX has its own machine format code and remains
read-only, outside the verified RIFF/WAVE clean-copy contract.

**Implementation update 2026-11-10:** Core, CLI and Web now identify RF64 and
BW64 separately and resolve the mandatory first `ds64` chunk's 64-bit form,
data and table sizes with checked bounds and a 4,096-entry limit. Supported
WAVE metadata chunks are inspected, while BW64 XML and channel metadata remain
uninterpreted; both formats are read-only and outside the WAV clean-copy
contract.

**Implementation update 2026-11-10:** HEIF construction method 2 now maps an
item extent through its 1-based `iloc` reference when the requested bytes lie
inside one explicit, in-file extent of a local source item. The source item may
have multiple extents, including extents in `idat`; a request crossing an
extent boundary, chained locations, external references and implicit lengths
remain visibly unread. This does not decode HEIF/AVIF image payloads.

**Implementation update 2026-11-10:** ZIP method 12 (BZIP2) now decompresses
entry bytes on demand in native Core/CLI builds with the optional `zip-bzip2`
feature, applying the output limit, uncompressed-size check and CRC-32 check.
The browser leaves these entries named and unread until a separately loaded
decoder fits its WebAssembly budget; other unsupported methods remain unread.

**Implementation update 2026-11-10:** ZIP method 93 (Zstandard) now decompresses
sequential frames on demand in native Core/CLI builds with the optional
`zip-zstd` feature. Output is bounded and still checked against the recorded
size and CRC-32; each frame's window is capped at 128 MiB, and dictionary
frames are unread. Deprecated method 20 remains unsupported, as does browser
decompression pending a separately loaded module.

4. **Make encrypted ZIP entries more informative.** Read WinZip AES `0x9901`
   fields in both the local header and central directory, show AE version,
   key strength and the underlying compression method, compare recognized
   fields, and call out malformed or conflicting settings. Keep unknown
   extension bytes and encrypted file data unread without a password.

**Release is done when** a clean WAV is offered only after a complete read;
the resulting file retains the same `fmt ` and `data` bytes; every removal
claim is backed by a complete read-back under the shared Core contract; CLI,
Web, batch and docs agree; and incomplete or unrecognized metadata remains
explicitly unchecked. Recognized AES ZIP fields in the local header and
central directory must be reported and compared without implying that
encrypted bytes or unknown extension bytes were read. Each user-facing change
gets its own `changes/` fragment.
Screen-reader, zoom, and physical-device gaps recorded during 0.17 remain
open for the 1.0 acceptance checklist.

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
   `apps/web/scripts/style-diff.mjs` compares computed styles and rendered
   pixels between two builds, so the rest can be merged one file at a time.
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

- **Local by default:** no file uploads, accounts, user-level tracking, or server-side file inspection. The site stores aggregate visit counts by country, page, device class, language and referring host; it stores no cookie, user ID or IP address in those totals. A feedback message stores the chosen kind and text, optional contact and structure report, page, language, version and browser/system; no file contents or IP address are stored with the message. The maintainer reads stored counts and feedback (`pnpm stats`, `pnpm feedback`).
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
