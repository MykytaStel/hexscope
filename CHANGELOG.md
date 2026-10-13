# Changelog

What changed in each version of hexscope — the website, the command-line
tool and the crates share one version number. Newest first. The format is
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/): until 1.0, a minor version may
change what the CLI's JSON says. How a version is made: [docs/releasing.md](docs/releasing.md).

## [Unreleased]

### Added

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

[Unreleased]: https://github.com/MykytaStel/hexscope/compare/v0.3.0...HEAD
[0.3.0]: https://github.com/MykytaStel/hexscope/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/MykytaStel/hexscope/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/MykytaStel/hexscope/releases/tag/v0.1.0
