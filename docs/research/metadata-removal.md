# Removing metadata: hexscope, ExifTool and MAT2 on the same files

*October 2026. Reproduce with `python3 -I scripts/compare-tools.py`; the
numbers below are in [metadata-removal-tables.md](metadata-removal-tables.md)
and [metadata-removal-results.json](metadata-removal-results.json).*

hexscope says a clean copy is "confirmed" when its own parser no longer finds
what it found before. That is a check of the copy against hexscope's idea of
the file: anything the parser does not know about is neither found nor
missed. This study asks a reader written by someone else.

## Method

- **Files.** The 45 distinct files in hexscope's test fixtures, the site's
  samples and the browser tests' fixtures: photos (JPEG, HEIC, AVIF, PNG,
  WebP, GIF, TIFF), videos and a recording (MP4, MOV, M4A), PDFs, Word,
  Excel, PowerPoint and LibreOffice documents, RTF, a Word 97 document,
  emails, a ZIP and a WebAssembly module. Damaged and encrypted test files
  are left out.
- **Tools**, each in its default mode: `hexscope clean` (0.5.0),
  `exiftool -all=` (13.55) and `mat2` (0.15.0).
- **Two readers for every copy.** ExifTool reads each copy, hexscope's
  included; a tag counts when its name says who, where, when, with what
  device or program, or carries words, a thumbnail, an edit history or a
  provenance record, and it still holds a value — a tag left in place but
  emptied counts as removed. hexscope's own check reads each copy too,
  because ExifTool reads only a PDF's latest version and no text hidden on a
  page.
- **Pixels.** For pictures Pillow can decode, the mean difference between
  the original's pixels and the copy's, on a 0–255 scale.

## Results

| | hexscope | ExifTool | MAT2 |
|---|---|---|---|
| Files with something to remove | 37 | 37 | 37 |
| …of which a copy was made | 33 | 21 | 22 |
| Copies where ExifTool still reads identifying tags | 0 | 4 | 0 |
| Copies where hexscope still finds something | 3 | 8 | 10 |
| Pictures whose pixels changed | 0 | 0 | 5 |

**What each tool could not copy.** hexscope makes no copy of an email (4
files): it explains a message but does not rewrite one. ExifTool cannot write
ZIP-based documents (Word, Excel, PowerPoint, LibreOffice), RTF, Word 97,
email or WebAssembly (16 files). MAT2 refused HEIC, MOV, M4A, RTF, Word 97,
email and WebAssembly, failed on three WebP files, and refused three Office
documents for a part it did not expect (15 files) — refusing is MAT2's
deliberate choice when it is not sure.

**ExifTool's copies.**

- *PDF:* ExifTool reads its own PDF copies as clean, and hexscope finds
  everything still in them — the author, the dates, the title, the
  programs, the earlier versions, and text under black boxes. ExifTool
  writes a PDF's changes as an update after the original, which stays in
  the file; it warns about this itself: "ExifTool PDF edits are reversible.
  Deleted tags may be recovered!" Text hidden on a page is not metadata,
  and no metadata tool looks for it.
- *Video:* the creation and modification dates in a movie's own headers
  (`mvhd`, `tkhd`, `mdhd`) stay; ExifTool can change them but not remove
  them. hexscope blanks them in place.
- *TIFF:* the camera's make and model, the program and the date in the
  first image directory stay.

**MAT2's copies.**

- *JPEG:* MAT2 draws the picture again and saves it anew, so its pixels
  change (a mean difference of 0.5–3 on 0–255 in four of four JPEGs). hexscope and
  ExifTool copy the picture's bytes as they are.
- *PDF:* MAT2 turns each page into a picture: the copy has no text to
  select, search or read aloud. Its copies also point to a cross-reference
  section at an offset where none starts (`/Prev 12432` lands inside the
  table), which hexscope reports as breaking the format; PDF readers usually
  recover from this.
- *Office and LibreOffice:* comments stayed in the LibreOffice files' copies, the
  thumbnail of the first page in a Word document from Windows, and the folder Excel saved
  a workbook in (`x15ac:absPath`, which holds the account's name on
  Windows). The Mac's `__MACOSX` notes — where files were downloaded from,
  their tags — stayed in the ZIP.

**hexscope's copies.** ExifTool reads no identifying tag in any of them.
hexscope's own check still finds, by design, what is part of the content:
hidden sheets and rows in a workbook, speaker notes and hidden slides in a
presentation, and a WebAssembly module's imports and the paths in its data.
The clean copy says so before it is made.

## What this found in hexscope

ExifTool read three things in a Word 97 document (`.doc`) that hexscope
neither showed nor removed: the creation and modification dates and the
revision number in the document's own properties (the DOP in its table
stream), beside the property streams hexscope did blank. The same stream can
hold the list of who saved the document and the folders it was saved in, and
the names and initials of its comments' authors. hexscope now reads all of
these, shows them, and blanks them in the clean copy; a test reads the copy
back.

## Limits

- **The files are few and mostly made for tests.** Many were written by
  hexscope's own scripts, which describe formats as hexscope understands
  them; others come from real programs (an iPhone, Android, Keynote,
  Chrome, ffmpeg, cwebp) on one machine. A file from a program no one here
  has seen can hold what none of these tools reads.
- **Which tags count is decided by their names**, listed in the script. A
  tag with an unusual name that identifies someone can be missed, and a
  harmless one counted.
- **hexscope as a reader is not neutral**: it looks for what hexscope knows
  to look for. That is why ExifTool reads hexscope's copies, and why
  hexscope's reading of the other tools' copies is reported separately.
- **Each tool in its default mode.** ExifTool can remove more from a PDF by
  rewriting it with another tool (qpdf, for example), and MAT2 has a
  lightweight mode that keeps PDF text; neither is tried here.
- **Nothing visible is judged.** A face, a street sign or a name on the page
  is not metadata, and none of these tools removes it.
