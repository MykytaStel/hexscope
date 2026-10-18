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

- **`hexscope check` counts the files it does not read**: a folder with
  photos and recordings no longer reads "read 3 files" as if that were all
  of it — the last line says how many were not read and of which extensions
  (`3 not read … (2 .mp3, 1 .html)`), `--all` lists them as "not read"
  instead of "nothing found", and in GitHub Actions they get one notice.

### Fixed

- **What a text file or an unknown one is, in Ukrainian**: "Plain text…",
  "This looks like an MP3 recording…" and the list of what hexscope reads
  were shown in English.

- **In GitHub Actions, a saved `--json` or `--sarif` report was broken**:
  the annotations for the files that failed were written into the same
  output, so `hexscope check --sarif > report.sarif` saved a file code
  scanning could not read. With either option they now go to standard error.
