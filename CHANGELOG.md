# Changelog

What changed in each version of hexscope — the website, the command-line
tool and the crates share one version number. Newest first. The format is
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/): until 1.0, a minor version may
change what the CLI's JSON says. How a version is made: [docs/releasing.md](docs/releasing.md).

## [Unreleased]

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

[Unreleased]: https://github.com/MykytaStel/hexscope/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/MykytaStel/hexscope/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/MykytaStel/hexscope/releases/tag/v0.1.0
