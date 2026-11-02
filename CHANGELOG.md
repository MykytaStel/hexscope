# Changelog

What changed in each version of hexscope — the website, the command-line
tool and the crates share one version number. Newest first. The format is
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/): until 1.0, a minor version may
change what the CLI's JSON says. How a version is made: [docs/releasing.md](docs/releasing.md).

## [Unreleased]

What is merged but not released yet is in [`changes/`](changes/), a file a
change; `node scripts/changelog.mjs` shows it as it will read here.

## [0.10.0] — 2026-11-01

### Added

- **Ogg track chains are read together.** Hexscope now finds Vorbis and Opus tags in each sequential audio stream in a chained file.
- **Select every file that can be cleaned in one step.** Batch results can
  select or clear all eligible clean copies at once, including files hidden by
  the current filter. The control is available in English and Ukrainian.
- **Read and safely clean stacked MP3 ID3v2 tags.** The core inspects each
  consecutive leading tag, keeps byte ranges tied to the correct tag, and
  removes all inspected tags while copying compressed audio unchanged.
  Inspection is capped at 16 tags and 65,536 frames across the chain; files
  beyond the tag limit stay read-only, and frame-limited reads remain incomplete.
- **Make a safe FLAC copy without Vorbis comments or embedded cover art.** Entire picture blocks are removed; other supported metadata blocks and compressed frames stay byte for byte. Files with metadata Hexscope cannot inspect remain read-only. The CLI can verify the copy after writing it; the Web confirms findings on copies within its read-back limit.
- **Clean FLAC files with supported metadata blocks in a batch.** Ogg and FLAC files with uninspected blocks remain read-only.
- **Firefox and WebKit browser checks** cover the analyzer's privacy findings and keyboard-operated clean-copy verification alongside the existing Chrome suites.
- **Ultra HDR clean copies** keep one bounded MPF-listed gain map. The core
  reads the map's photo metadata into the parent file's evidence, preserves
  MPF offsets and both pictures, and removes personal metadata from both JPEGs
  while retaining supported Ultra HDR or ISO 21496-1 settings. The Web explains
  this behavior in English and Ukrainian before the copy is made.
- **Make an MP3 copy without its edge metadata tags.** Leading ID3v2 and trailing APE, ID3v1 Plus and ID3v1 blocks are removed; compressed MPEG audio bytes stay unchanged. The copy is read back, and findings are confirmed removed only when both metadata reads are complete.
- **Clean tagged MP3 files in a batch.** Only formats with a safe clean-copy path are selectable.
- **See text from APE audio tags in MP3 files.** Titles, artists and other bounded text values are shown with their source bytes. Binary items and other tag locations are not inspected, so the result remains partial.

### Fixed

- **Malformed video timestamps no longer panic the core.** Unsigned MP4 version 1 dates are converted without integer wraparound; values outside the calendar range are omitted from findings but are still scrubbed from clean copies.
- **Validate Ogg pages beyond the tags.** Supported sequential streams are checked for page boundaries, order and checksums through the final page. Damage after metadata, missing end pages and incomplete chains keep metadata marked incomplete; interleaved streams are left explicitly incomplete.
- **Keep keyboard focus in batch controls.** Changing a filter or clean-copy
  selection no longer drops focus when the results list redraws, so keyboard
  users can keep moving through the batch.
- **Homebrew installs the published 0.9.0 release** with checksums from its four platform archives; the tap no longer stays on 0.8.0.
- **Release instructions and roadmap match the published state** and require a fresh package dry run for each release tag.
- **Unknown or incomplete gain-map metadata** now makes cleaning refuse the
  copy; an unrecognized MPF-listed picture is never replaced with blank bytes.
  Core tests verify safe metadata retention and read-back. The synthetic
  iPhone-format fixture does not establish visual behavior on a physical
  iPhone.
- **Batch clean copies no longer include read-only formats.** Audio files, emails and unknown files can still show their findings, but the batch does not offer a copy it cannot make; email attachments can be opened separately.
- **MP3 files with APE tags are marked as partially inspected.** The tag footer is detected even when ID3v1 follows it; unsupported APE content remains unchecked.

## [0.9.0] — 2026-10-29

### Added

- **Browse the files in an archive.** A ZIP gets a Files view: its folders, a filter by name, each entry's size and, beside the list (in a sheet on phones), its method, sizes and Archive Trace with Open and Watch it decompress. Entries are flagged when they are encrypted, compressed in a way hexscope does not unpack, shorter or longer than the directory says, or named to unpack outside their folder (`../`, `/`, `C:\`). Names are shown exactly as the archive writes them, and nothing is unpacked until you ask. A large archive costs the page a few dozen rows, not one per entry.
- **Share the film lab's JPEG straight from a phone.** Once the copy is made and read back, Share JPEG opens the phone's share sheet with that exact file; Save JPEG stays beside it. A dismissed sheet keeps the copy ready, and where a browser cannot share files, or the sheet does not open, the page says to save the copy and share it from Files or Downloads.
- **Pause and resume multi-file reading** on the batch screen; an interrupted file returns to the queue, while finished results stay available.

### Changed

- **Compare and adjust film scans more easily on a phone.** Switch between the full-width original and sharing preview, use a compact crop layout, and find the copy actions together. Picking a film base brings the original back into view; switching previews keeps the adjustments.
- **Understand and act on an archive entry sooner.** Files now explains how to choose an entry, opens its details on phones, and puts Open and decompression actions before the byte trace.
- **Open Film Lab from the overview** when a picture has film-scan clues, without hunting through the rest of the file details.
- **Clean-copy summaries name what was confirmed removed** without implying that everything in the file was checked. The phone overview brings the visual evidence closer to the answer.
- **The DEFLATE player loads when playback is opened.** It remains available offline through the app's existing cache. Closing while it loads cancels playback; if the player download fails, the file stays open and playback can be retried.
- **The file analyzer loads when someone chooses a file.** Browsing the landing page no longer parses the analyzer on an idle timer. Opening a file picker starts loading it, while sample links and files shared to the app still open directly. The full offline cache remains unchanged; this reduces analyzer work before file intent, not total first-visit transfer.
- **PDF and report tools are imported on demand.** Opening a non-PDF does not parse or run the PDF redaction UI, and report/export modules are imported only after the person asks for them. The offline service worker still caches the full app on the first visit so these tools remain available without a connection.
- **Very large movie files use less memory while being inspected or cleaned**, with media bytes kept out of the initial read and clean copies preserved byte for byte.
- On phones, hold a byte briefly to read its details without pinning a selection. A short tap still selects the byte, and moving the finger cancels the peek so scrolling remains natural.
- **SVG privacy checks now cover more embedded photos:** WebP, TIFF, HEIF and AVIF metadata is reported and removed in clean copies, alongside JPEG and PNG.
- **PDFs with repeated page and image references require less redundant work** while preserving reading order and incomplete-document warnings.
- **Expired files shared to the installed app are cleaned up after a normal visit**, even when no new share or app update occurs.

### Fixed

- **Batch results keep partially inspected and unreadable files visible.** The attention filter includes unsupported and incomplete files, and the summary and spoken update say when checks could not finish instead of suggesting every file was read.
- **Office files with conflicting ZIP entry headers no longer produce a misleading clean copy.** Hexscope refuses to rewrite an entry when its local and central records disagree on the name, flags or compression method.
- **Feedback requests are bounded while they arrive.** Oversized bodies are stopped before they can be fully buffered, and simultaneous submissions cannot bypass the daily message limit.
- **Archives with deeply nested paths can open their Files view** without overflowing the browser's call stack while building the folder list.
- **The "What it's made of" bar shows the whole of a large file.** An archive with thousands of entries drew one piece per header and per entry, so the bar showed only the start of the file and the page held two elements per entry. Past 400 pieces, the bar is now drawn at a lower resolution: about 200 equal stretches, each with its colours in proportion, so every byte still counts. Opening an archive of 100,000 entries leaves about 200,000 fewer elements on the page.
- **Byte Tools no longer redraws its entropy strip** when the byte canvas repaints without moving the visible range.
- **Cancelling a large-file read restores the open workspace:** the previous filename and view remain in place, and assistive technology hears that the read was cancelled.
- **SVG drawings with a namespace-prefixed root are recognized** and can be inspected and cleaned like other SVG files.
- **Pausing a batch now stops QR scans and releases temporary PDF resources.**
- **Clipboard image reading handles permission failures** before opening the batch and shows the existing choose-or-paste guidance.

## [0.8.0] — 2026-10-25

### Added

- **Follow a ZIP entry down to its bytes.** Trace it through the central-directory record, local header and compressed data; each step opens the actual range in Bytes, and the existing DEFLATE view opens the decoded content. Missing and truncated ranges are shown as such.
- **Audio tag inspection:** see titles, artists, albums, dates, track details and other tags in MP3, FLAC, Ogg Vorbis and Opus files. The read-only parser keeps each finding linked to source bytes and says when metadata could not be fully inspected; it does not decode or play audio.
- **A clean-copy receipt you can check offline.** After the copy has been read back, save an optional record of what its verification found. It contains no file name, path or metadata values; the CLI can check whether the copy's bytes still match its SHA-256 fingerprint. A match confirms the bytes only, not who made the receipt or whether the file is safe. Batch fingerprints are opt-in; the existing batch receipt stays unchanged by default.

### Changed

- **Email authentication results stay claims from the message.** Hexscope shows SPF, DKIM and DMARC statuses recorded in `Authentication-Results` as unverified claims, including when the header is present but contains no recognized results.
- **Large-video progress works with screen readers.** Spoken updates skip rapid byte-by-byte steps while the visible indicator stays current.
- **Large movie analysis uses fewer temporary byte copies.** The parser keeps offsets tied to the original file while scanning metadata in bounded ranges.
- **Shared files expire after 24 hours.** The PWA keeps a shared group together and accepts up to 50 files and 400 MiB per group.
- **Large video files show reading progress.** You can cancel the read while Hexscope is loading file ranges; the final metadata-parsing step cannot be interrupted.
- **Large files use less memory while they are inspected.** The parser now reuses its owned input buffer for JPEG block analysis, ZIP navigation, and attachment extraction instead of making another full-size copy inside WebAssembly.
- **Large pictures inside ZIP archives and email attachments can be downsampled before QR scanning.** PNG, JPEG, GIF, WebP and BMP dimensions are read from their headers; an unsupported or malformed header keeps the existing browser decode fallback.
- **Large batches do less repeated work while they update.** File counts, filters and visible rows are now prepared together, so large folders cost less CPU on phones as files finish reading.
- **SHA-256 can be calculated with fixed memory.** The CLI hashes large copies in bounded chunks instead of holding a second copy of the file in memory. The browser creates a single-copy fingerprint only when a receipt is requested and the copy is within its existing 10 MiB verification limit.
- **Optional comparisons load on demand.** The comparison tool is downloaded only when a second file is selected, reducing the analyzer's initial compressed bundle by about 2.5 KiB.
- **Unavailable batch fingerprints are explicit.** If a clean copy was written but its requested fingerprint could not be computed, the receipt keeps the copy and records why the fingerprint is absent.

### Fixed

- **Film Scan samples large rasters without repeating a fixed pixel stride.** Sampling is bounded and deterministic, including on 32-bit WebAssembly.
- **Overly complex videos stop safely.** Hexscope explains why instead of falling back to reading the entire file.
- **Byte views stay responsive on fragmented media.** Files with many separated media ranges load only the bytes being viewed.
- **Large-photo QR scans use less temporary memory where the browser supports it.** Images with known dimensions are decoded at the scanner's size instead of first allocating a full-resolution bitmap; the browser keeps the photo's proportions, and the bitmap is released on every exit path.
- **Share a cleaned or edited photo directly from a phone** when its browser supports it; otherwise save a copy with clear instructions for sharing from Files or Downloads.
- **Keep batch sharing available when the share sheet fails or is unavailable**, with controls and results that remain readable on narrow screens.
- **Keep edited-photo actions visible on small screens** and preserve sentence spacing in translated results.
- **A timed-out copy check no longer leaves the reader blocked.** If copy verification exceeds its time limit, Hexscope stops the parser worker and reopens the current file from its local source so the reader can continue. Requests from the stopped worker cannot affect its replacement.
- **Partial ID3 reads are explicit:** unsupported frames and reserved flags no longer make an MP3 look completely inspected, and later supported tags remain visible.
- **ZIP extraction now checks the archive's declared output size.** An entry is rejected when its actual decompressed length disagrees with the central directory, even if its CRC happens to match.
- **Web copy verification recognizes changed values of the same kind.** It no longer reports a replaced metadata value as both removed and unexpectedly present; exact matches are reserved before changed values are paired.
- **Batch copies keep their original file indexes when items repeat.** Naming collisions and receipt rows no longer rely on repeatedly searching the input list.

## [0.7.2] — 2026-10-19

### Fixed

- **`cargo install hexscope-cli` failed on a compiler a few months old**:
  0.7.1 needed a feature only the newest Rust accepts, so Rust 1.94 (March
  2026), for one, stopped with "`if let` guards are experimental". It builds
  again with Rust 1.88 and later, and the crates now say they need 1.88, so
  an older compiler gets a clear message instead. The prebuilt commands and
  Homebrew were not affected.

## [0.7.1] — 2026-10-19

### Added

- **"Clean with hexscope" in the file manager**: `hexscope menu` adds it to
  Finder's Quick Actions on a Mac, to Send to in Windows' File Explorer and
  to the Scripts menu of GNOME Files. It runs `clean --verify` on the files
  picked — each clean copy is saved beside its file — and shows what was
  removed. `hexscope menu --remove` takes it away.
- **Homebrew**: `brew install mykytastel/hexscope/hexscope` installs the
  command built for each release, on macOS and Linux.

## [0.7.0] — 2026-10-18

### Added

- **Tools under the bytes**, as a hex editor has them, in a panel that loads
  only when opened: what the bytes at the cursor read as — 8- to 64-bit
  whole numbers, unsigned and signed, both byte orders, 32- and 64-bit
  floats, a Unix time when it could be one, the bits, the UTF-8 character;
  the SHA-256 of the file and of the part chosen, to copy; and the entropy
  along the file, low where bytes repeat and high where data is compressed
  or encrypted, which goes there on a click. And `@0x1F0` (or `@496`) in
  the find box goes to that offset.
- **What a file of no known format still names**: hexscope reads its text —
  as ASCII and as UTF-16, as Windows programs write it — for home folders
  (`/Users/olena/…`, `/home/olena/…`, `C:\Users\Olena\…`), which name the
  account of whoever made or built it, and for email addresses. A Photoshop
  or Blender file, a compiled program or a database now says so instead of
  only "not read"; on the command line it is listed and can fail a check
  (`account`, `emails`). Shared and build accounts (`Shared`, `runner`) and
  names like `icon@2x.png` are not counted. The small WebAssembly build
  grew by about 2.4 KB for it.
- **A table of what was found on the run's page in GitHub Actions**: GitHub
  shows at most ten annotations of a level for each step, so a folder with
  more findings showed only some of them. `hexscope check` now also writes
  the step summary — a row per file with findings: its format, the kinds it
  gives away and its problems, never their values — with the files it did
  not read counted.
- **A guide to what an SVG keeps**, in English and Ukrainian: Inkscape's
  names and paths, the metadata block, Illustrator's own copy of the
  drawing, embedded photos and scripts, and how to take them out — with
  hexscope, in Inkscape and Illustrator themselves, or with SVGO.
- **SVG drawings**, on the page, on the command line and in the Action —
  what they say about whoever made them, and a copy without it:
  - Inkscape's name for the file, the whole path it was last exported to,
    pictures linked by their path on the author's computer, its version and
    settings;
  - the creator, publisher, title, date, description and keywords of the
    drawing's metadata (RDF or XMP);
  - a comment naming the program, as Illustrator and Sketch leave;
  - Illustrator's own copy of the drawing (`i:pgf`), which can hold layers
    and objects the drawing does not show;
  - photos embedded as base64 with their GPS, cleaned as photos of their own;
  - scripts and event handlers, named and left in place.

  Confirmed removal covers it. A sample is on the home page.
- **A file no other build reads opens with the small one**: a drawing, a
  text file or an unknown file no longer downloads the parser for documents
  (about 238 KB); that one is asked for only by ZIP, Office 97, RTF,
  WebAssembly and email. The small build grew by about 9 KB for SVG.
- **What the command line keeps from version to version**, in one page
  (`docs/cli.md`): its commands and options, the exit status of each, the
  fields of its JSON, SARIF and receipts, every format and kind of finding it
  reports, and the reasons a clean copy's check gives. A test runs the
  command over every test file in the repository and fails when the program
  and the page disagree. `hexscope --help` links to it.

### Changed

- **A file's answer arrives sooner**: the parser every visitor downloads
  first is 15 KB smaller for photos and movies (148 → 133 KB gzipped), 17 KB
  for PDFs and 21 KB for Office files and email. The sentence that says what
  each part of a file is, and where its format defines it, now comes in a
  file of its own once the file is open, for the structure tree, the
  inspector and the structure JSON.
- **`hexscope check` counts the files it does not read**: a folder with
  photos and recordings no longer reads "read 3 files" as if that were all
  of it — the last line says how many were not read and of which extensions
  (`3 not read … (2 .mp3, 1 .html)`), `--all` lists them as "not read"
  instead of "nothing found", and in GitHub Actions they get one notice.

### Fixed

- **The structure tree's tooltips were in English on the Ukrainian page**:
  what each part is now reads as the inspector says it.
- **What a text file or an unknown one is, in Ukrainian**: "Plain text…",
  "This looks like an MP3 recording…" and the list of what hexscope reads
  were shown in English.
- **In GitHub Actions, a saved `--json` or `--sarif` report was broken**:
  the annotations for the files that failed were written into the same
  output, so `hexscope check --sarif > report.sarif` saved a file code
  scanning could not read. With either option they now go to standard error.

## [0.6.0] — 2026-10-16

### Added

- **The command, built for each system, on every release**: Linux (x86-64
  and ARM, static), macOS (Apple silicon and Intel) and Windows, each with a
  checksum and GitHub's signed record of the workflow that built it. The
  GitHub Action at a released version downloads the one for its runner and
  checks both, instead of compiling for a minute or more.
- **What metadata removers leave, measured**: hexscope, ExifTool and MAT2
  cleaned the same 45 files and every copy was read back by both ExifTool and
  hexscope (`scripts/compare-tools.py`, `docs/research/metadata-removal.md`).
  And a kit to measure what messengers take out of a file
  (`docs/research/messenger-study.md`).
- **Two guides**, in English and Ukrainian: what ExifTool, MAT2 and hexscope
  each leave in a file, from that measurement, and what gives away a photo
  from a sensitive place — in its file, in the picture and in how it is sent.

### Changed

- **The analyzer in Ukrainian, all of it**: the parts of a file in the bytes'
  colours, what a file is made of, the details pane and its tools, the way an
  email came, blacking out text and pictures, the keys, finding in a file,
  several files at once, the DEFLATE player and the pictures' captions —
  about 250 lines that stayed in English. Sizes read "105,2 КБ", a verdict's
  list is Ukrainian to its end ("… і ще 4"), and a list's heading names a
  finding in its own case. A new end-to-end test opens every sample and test
  file in every view, makes a clean copy, compares, lists several files, and
  fails on any English word of hexscope's own.
- The photo's facts are on the home page from its first paint; the clean
  copy strikes them once it is checked.
- The home page's top bar and sections share one gutter at every width.
- The film-scan clues show only where a picture has some: a camera's photo
  no longer carries a section saying it has none.
- The row under "Location" is called "Coordinates".
- The home page says "nothing is uploaded" once, and in its other places what
  else is true: it works offline, and the developer tools show nothing sent.
- The README lists every guide, and the crates by the site's version.
- CI's actions are pinned to commits, and Dependabot proposes their updates.

### Fixed

- **A Word 97–2003 document (.doc)** showed and cleaned its property
  streams, but not Word's own record beside them: the dates, the revision
  number and the editing time, the list of who saved it and in which
  folders, and who wrote its comments, with their initials. ExifTool read
  them in a clean copy; hexscope now shows them and blanks them.
- Ukrainian guides close every quote they open and keep the space after a
  name in italics ("EXIF — блоку…"); a test reads each built page for it.
- What a document is made of no longer says "the the document".
- The visitor counter does not count a browser a program drives.

## [0.5.0] — 2026-10-15

### Added

- **Four guides**, in English and Ukrainian: removing EXIF and metadata from
  a photo, removing metadata and the location from a video, removing
  metadata from LibreOffice and RTF files, and telling what type a file
  really is when its name says another. Two new samples go with them: a
  LibreOffice lease and a HEIC photo named `.jpg`.
- **What Office on Windows leaves** is found and taken out of a clean copy:
  the folder Excel last saved a workbook in (`x15ac:absPath`, a path with the
  account's name), the folder of a Word document's template, the printer a
  printer-settings part names, SharePoint's properties kept as custom XML
  (with the names and addresses in them), a picture of the first page, and
  Word's editing-session numbers (`w:rsid`), which tie documents written in
  the same sittings to one another. The test files `windows.docx` and
  `windows.xlsx` are written as Office writes these parts
  (`scripts/make-sample-windows-office.py`).

### Fixed

- **A link to a site under `.com.ua`, `.org.ua`, `.co.uk` and other shared
  suffixes** was compared by its last two labels, so `rozetka.com.ua` and
  `rozetka-sale.com.ua` counted as one site and a lying link went unsaid.
  The registered name now counts the suffixes anyone registers under (106
  of them, Ukraine's regional ones among them) — in
  emails, Word documents and PDFs, and for replies and bounces sent to
  another domain.
- **A clean copy with any hidden finding left in it** — an attachment, text
  under a black box — was called not completely read, and every finding
  went unchecked. Only bytes that were not read make a copy incomplete now:
  damage, and data nothing in the file accounts for.
- **A PDF's clean copy kept its notes, who wrote every comment, and the files
  it carried.** Now notes and their pop-ups go, every other annotation loses
  its author and dates, a highlight the words of its note, and attached files
  go — as in Word's copy. What a person typed or signed on the page stays,
  and so do the files an e-invoice or a PDF/A-3 file names as part of itself
  (`/AF`). Comments, attachments, text under boxes and hidden text join the
  PDF row of the removal contract, with `redacted.pdf` as evidence.
- **An email's "SPF passed"** was believed from any `Authentication-Results`
  header. A result naming a server the message never came through — as
  anyone can type into a message — is now said to be one, as a sign of
  forgery.
- A ZIP with bytes past its end, or in entries nothing points at, was taken
  for completely read; and a PNG's caption, which every copy loses, was said
  to be unchecked.
- A Ukrainian support page drew its ways to give in English: the words it
  draws for itself were only in the articles' dictionary, which a Ukrainian
  page no longer loads.
- The clean copy of an iPhone photo said it looks the same without its HDR
  gain map; on an HDR screen its highlights are less bright, and it says so.

### Changed

- **An article loads a fifth of the stylesheet:** guides, the support page,
  the privacy page and the 404 page wear the tokens, the top bar, buttons
  and links, the dialog "Write to the developer" opens and their own type —
  3.8 KB gzipped in one file instead of the app's 21 KB in three. What every
  page shares is in one place (`chrome.css`, `dialog.css`); about 25 rules
  that never won — the redesign's story type under the story's own, a
  narrow top bar's spacing under the wide one's — are gone. A script
  (`apps/web/scripts/style-diff.mjs`) reads the computed style of every
  element in 940 states of two builds and names what differs: only the
  feedback field's corners changed, which keep their own rounding in
  keyboard focus now. CI holds an article's stylesheet to 6 KB.
- **On a phone the workspace's bar keeps the views of the answer** —
  Overview, Metadata, Content — and the structure and the bytes wait under
  More, which says when one of them is open. On the landing the header's
  Open shows only once the landing's own button has scrolled away.
- **The Structure and Bytes views explain each part in Ukrainian:** the 512
  explanations a reader meets on real files — what a field, a box, a chunk
  or an object is — have their Ukrainian, loaded with the analyzer for a
  reader in Ukrainian only (27 KB gzipped). The corpus test now holds every
  explanation shown for a test file to its translation.
- The WebAssembly builds are 1.4–2.8 KB smaller: three stable sorts of
  their own item types gave way to the one key sort the builds share.
- **What the core says about a file reads in Ukrainian:** about 30 kinds of
  finding written as sentences — tracked changes, attachments, links, signs
  of forgery, hidden text, hidden rows, editing history, what a module asks
  for — and 13 lines of what a clean copy took out were shown in English.
  A test now reads every test file and sample and fails on any sentence a
  reader meets without its Ukrainian; two more hold the page's table of
  file names to the core's, and every public page's head to its own address.
- **An article opens in the reader's language as its own page:** a guide,
  the support or the privacy page in the other language than the reader's
  opens its `/uk/` or English page before anything is drawn, with where it
  was on the page, and the language button opens the other page instead of
  translating this one. Nothing flashes in the wrong language, and no
  article loads the articles' dictionary any more. The app itself still
  switches in place, so an open file stays open.
- The landing says by its button that nothing is uploaded, where it is seen
  first, not only below the demo.
- **Pages load less for a reader in English:** the Ukrainian words are a file
  of their own, loaded only when a page is read in Ukrainian or was written
  in it. A guide now loads about 7 KB of script instead of about 78 KB, and
  the home page about 25 KB less. A Ukrainian guide read in Ukrainian, which
  is translated at build, no longer loads the articles' dictionary either.
- **The home page starts with 18 KB of script instead of about 85 KB:** the
  analyzer that shows an open file is a module of its own, loaded once the
  page is idle or as soon as a file or a sample is asked for. The size budget
  in CI now counts the home page's script and what it preloads, and the
  analyzer on its own.
- `hexscope-raster`, the film lab's codecs, is published with the other
  crates, so `cargo install hexscope-cli` builds the `film` command too.
- Every guide lists all the others, made from one list at build, so a new
  guide is linked from each page at once.
- Search engines get each page's real dates — from its commits, not the day
  of the build — in the sitemap and in the page's structured data, with a
  breadcrumb back to the home page; after a deploy, IndexNow hears only of
  the pages that changed.

## [0.4.0] — 2026-10-13

### Added

- **A file named as another format** — a Word document called `.pdf`, a HEIC
  photo called `.jpg` — is named by the core, so the CLI says it too: what
  the file is and the name that fits, in its output, JSON (`misnamed`) and
  SARIF, and `--fail-on misnamed` fails on it. A document in a ZIP is named
  as what it is: a Word file called `.xlsx`, or a LibreOffice spreadsheet
  called `.xlsx`, is misnamed, and the name that fits is `.docx` or `.ods`,
  not `.zip` — on the site as well, where the line is now in Ukrainian too.

- **RTF documents**: their title, author, who saved them last, company,
  manager, dates, printing date, versions and editing time, the program and
  the template with its path, custom properties, who made the tracked changes
  and wrote the comments, and the text a deletion still holds — Ukrainian
  names in Windows code page 1251 read right. The clean copy leaves out the
  information and the rest, accepts the tracked changes and deletes the
  comments, and is confirmed to have lost them.
- A document's custom properties count among the things it gives away.

- **LibreOffice documents** (.odt, .ods, .odp): who made it and who saved it
  last, when, how long it was edited, the program and the system it ran on,
  who printed it and the printer's name, the template's path, its comments
  and tracked changes — with the text a deletion still holds — the picture of
  its first page kept inside, and the photos in it. The clean copy empties
  its properties and settings, accepts the changes, deletes the comments,
  blanks the thumbnail and cleans the photos, and is confirmed to have lost
  them. Word's tracked changes and the photos in a document are now
  confirmed removed too.

- **Install the app**, from the menu or the footer: where the browser offers
  it (Chrome, Edge, Android) the button asks it; an iPhone or iPad, which has
  no such offer, is shown the three taps to Add to Home Screen. Nothing shows
  where hexscope is installed already.

- **Write to the developer, without an account.** A problem, an idea or
  thanks goes from the page itself — from the footer, the end of every guide,
  or a file's “Report a problem”, which can attach the file's structure
  report (its layout, none of its content) when ticked. What is sent is what
  the form shows; never the file, never an address. Read by the developer
  with `pnpm feedback`.

- **A way to thank the developer**: a support page with ways to help — Ko-fi,
  from anywhere by card or PayPal, once or every month, and a Monobank jar,
  which takes a card from any Ukrainian bank, Apple Pay or Google Pay, shown
  first to Ukrainian readers — and, after the third clean copy, one quiet line
  saying how, closed for good with ×. The repository's Sponsor button leads
  there too.

- Installed as an app, hexscope also opens TIFF files from the computer or a
  phone's share sheet.

- **TIFF.** Scans and camera TIFFs say where they were taken, the camera and
  its serial number, the owner, the time and the software, read by the same
  code as a JPEG's EXIF. The clean copy empties those tags and the EXIF and
  GPS tables where they lie, so every strip of the picture stays where it
  was, and confirmed removal covers it.

- **Guides for what people search.** An EXIF viewer page, how to remove
  metadata from a PDF and from Word, Excel and PowerPoint files, and whether
  a picture says AI made it — each in English and Ukrainian, linked from every
  guide, the home page and the sitemap.

- **An AI-made picture among the samples**: a PNG with the prompt, settings
  and model a Stable Diffusion web UI writes.

- A PNG's clean copy is confirmed to have lost what made it and the prompt
  (`ai`, `prompt` in the removal contract), with a real file as evidence.

- **hexscope.org.** The site moves to its own address; hexscope.pages.dev
  forwards to it, and the canonical links, sitemap and link preview name it.

- **The site in Ukrainian for search engines.** Every page has a Ukrainian
  copy of its own at `/uk/…`, written at build time, so a search in Ukrainian
  finds the guides; the two are linked to each other (hreflang), described as
  structured data (a free web application, articles) and listed in a sitemap
  of both. The site's address is set once (`SITE` in `apps/web/seo.ts`).

- **A page for a wrong address.** A mistyped or old address now gets a real
  404 with a way back to the app and the guides, in English or Ukrainian,
  instead of the app's home page answering for every address.

- Search engines that take IndexNow (Bing, Yandex and others) hear of the
  site's pages after each deploy.

### Changed

- The visit numbers are read only by the site's author, straight from the
  database with `pnpm stats`; the `/stats` page and its key are gone.

- Pages link to each other by the address the host serves them at, the home
  page and the DEFLATE story name their own address, and link previews say
  which language they are in.

- The link preview is a JPEG half the size, so apps that skip large images
  (WhatsApp) show it.

- Browsers that have seen the site come back to it over HTTPS only.

- The home page's title and description say what it is in the words people
  search with — metadata, EXIF — and its structured data lists what it does.

- **A document loads a quarter less.** The parser is built per family of
  files: a PDF fetches 221 KB of WebAssembly instead of 295 KB, a Word file
  or an email 212 KB, a photo 131 KB as before. A new format now grows only
  its own family.

- The home page loads faster on phones: the demo's photo and its facts are in
  the page from the start, so nothing below it jumps when the demo begins
  (layout shift 0.23 → under 0.1), and the archive frames are shown from small
  copies (≈190 KB instead of ≈950 KB) while the originals stay samples.

- In Ukrainian, a list of what a file reveals ends with “і”, and a button's
  name in quotes has no space inside them.

### Fixed

- **Several photos on a phone**: one full-width button makes the clean
  copies and the next shares them all; making them again steps back once
  they are ready. The photo tools fold under “More tools”, the names option
  is only offered for a ZIP, and the receipt is only promised when there is a
  ZIP. In Ukrainian, the whole batch view is translated, numbers agreeing.
- After a clean copy, its check shows only what has something under it: no
  empty “Still present” or “Not checked”.

- Every dialog's × sits in the corner beside its heading, not on a line of
  its own at the left.
- **A photo from an iPhone is no longer said to hide something.** After the
  picture, an iPhone keeps a second one — the HDR gain map — and lists it in
  the file's MPF index. hexscope now reads that index and shows the picture as
  what it is, so the verdict no longer says “Hidden in it”, and the clean
  copy, which still leaves it out, is confirmed clean instead of “not fully
  checked”. Bytes after the picture that no index lists are still flagged.
- A page left open while a new version came out checks its clean copy again:
  the previous version's files stay until the one after, instead of going the
  moment the new one arrives.
- On a phone: no tour over the first result; the clean copy's actions stand
  one under another, all the same size, sharing first; “Find in file” reads
  as a link, leaving one button on the card; the location badge shows the
  place in the summary rather than the bytes; the photo is drawn as sharp as
  the screen; and the keys and tour links leave the footer.
- On a computer, the tour rings the part it is about with a rounded, softer
  outline that its text never touches, and its note stands clear of it.
- In Ukrainian: what a clean copy removed, the files found inside a file, “Save
  it”, the kept orientation, the bytes view's colours and how fast a file was
  read are translated.

- On an English page, a full stop after a link or a bold word no longer
  turns into “it is.”.

- After a clean copy, its card says what its number means: “8 removed —
  nothing is left in the copy”, not a bare 8 over “nothing found”.

- What a file reveals adds up to the headline: four named, “and 4 more”.

- In Ukrainian, the clean copy has one name, and its button fits a phone:
  «Зберегти копію без цих даних».

- Visits to hexscope.org are counted: the counter still waited for
  hexscope.pages.dev, which now forwards there.

- The share card and its sentence name hexscope.org.

- The guides' section numbers are readable in the dark theme.

## [0.3.0] — 2026-10-12

### Added

- **Confirmed clean copies for more files.** Word, Excel and PowerPoint, old
  and new, HEIC photos and videos now say *Removed* for each property they
  lose instead of *Not checked*; photos also confirm the lens, the date, the
  software and the thumbnail. The CLI's `clean --verify` exits 0 when every
  finding is confirmed.
- The coverage table in `docs/verification-coverage.md` is written from the
  core's contract, and a test fails when they differ.
- **Links that open a sample where it matters.** `?sample=photo.jpg&view=metadata`
  opens a view, `&part=GPS IFD` selects a part; the guides' samples use them.

### Changed

- **Metadata is a view of its own:** every field the file has, grouped, the
  extra ones open — the answer and the actions stay on the overview.
- **A status bar under the bytes:** the byte under the pointer on the left,
  the part chosen and the file's size on the right.
- A new link preview in the site's look, and a demo at the top of the README
  (`scripts/make-demo.mjs` records it again).

### Fixed

- The note under the short answer no longer sits on the findings below it.

## [0.2.0] — 2026-10-11

### Added

- **Clean copies are checked.** The copy is read back and compared with the
  original: what was removed, what remains, and what could not be checked,
  in the Web, the CLI (`clean --verify`) and batch receipts. Confirmed for
  JPEG, PNG and WebP photo data and for PDF Info and XMP.
- **Film photo lab.** Scans of film: frame borders and perforations spotted,
  negatives converted with a calibrated film base, rolls processed alike,
  16-bit TIFF output, and a JPEG for sharing that is checked like any copy.
- **Photo privacy mosaic.** Across several photos: the same serial number,
  places near each other, dates in order — what a set gives away that one
  photo does not.
- **Zips made on a Mac** say where each file was downloaded from, with which
  app and when, and its Finder tags (the hidden `__MACOSX` notes). The clean
  copy leaves the notes out.
- **More of what a document says.** Word, Excel and PowerPoint, old and new:
  subject, keywords, description, category, status, manager, custom
  properties, and Microsoft sensitivity labels, which name the organisation.
- **Comments in pictures** (JPEG, GIF, PNG) are shown: a program's stamp as
  the software, anything else as a caption.
- **Ukrainian interface**, with every verdict, piece of advice and clean-copy
  note translated; tests fail when one is missing.
- **Visit counter without tracking.** One empty request a day at most, no
  cookies, ids or addresses, nothing about files; Do Not Track and Global
  Privacy Control are honoured. Daily totals only, read only by the site's
  author with a key.
- The version is shown at the foot of the site and in structure reports.

### Changed

- **New look.** Graphite design, an analyzer with its own navigation on
  computers, tablets and phones, grouped metadata, a clearer comparison, and
  contents beside each guide.
- The landing page shows a real analyzer and real Apollo 11 film scans.

### Fixed

- A Mac's screen recording had its camera, software and date shifted by one
  field.
- Drawings were reported as hidden text: white labels on rounded or gradient
  panels, boxes over a drawing program's shadows, see-through boxes, and
  letters of a word cut off by the page's edge.
- A PDF from macOS with references to impossible object numbers was called
  damaged.
- Clean-copy results no longer claim more than was checked.

## [0.1.0] — 2026-10-02

First published version: the website at hexscope.pages.dev, and
`hexscope-cli`, `hexscope-core` and `hexscope-inflate` on crates.io. Reads
photos, videos, PDFs, Office documents, emails, archives and WebAssembly in
the browser; says what each gives away; makes clean, redacted and repaired
copies; and a CLI and GitHub Action for the same checks in CI.

[Unreleased]: https://github.com/MykytaStel/hexscope/compare/v0.10.0...HEAD
[0.10.0]: https://github.com/MykytaStel/hexscope/compare/v0.9.0...v0.10.0
[0.9.0]: https://github.com/MykytaStel/hexscope/compare/v0.8.0...v0.9.0
[0.8.0]: https://github.com/MykytaStel/hexscope/compare/v0.7.2...v0.8.0
[0.7.2]: https://github.com/MykytaStel/hexscope/compare/v0.7.1...v0.7.2
[0.7.1]: https://github.com/MykytaStel/hexscope/compare/v0.7.0...v0.7.1
[0.7.0]: https://github.com/MykytaStel/hexscope/compare/v0.6.0...v0.7.0
[0.6.0]: https://github.com/MykytaStel/hexscope/compare/v0.5.0...v0.6.0
[0.5.0]: https://github.com/MykytaStel/hexscope/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/MykytaStel/hexscope/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/MykytaStel/hexscope/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/MykytaStel/hexscope/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/MykytaStel/hexscope/releases/tag/v0.1.0
