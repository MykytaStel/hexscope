# PDF: structure and what a document reveals

## Why

People share PDFs more than any other document, and a PDF says who wrote it,
with what, and when. Worse, a PDF edited after it was first saved usually
keeps every earlier version: an incremental update appends the changed
objects and leaves the old ones in place. "I removed that line" is often not
true of the file. hexscope should show both.

## Scope

In: reading the file's structure in file order, the document information
dictionary, the XMP metadata stream, incremental updates, compressed object
streams and cross-reference streams (Flate only), and three things that run
or hide: JavaScript, launch actions and embedded files.

Out, for now: the clean copy (its own step), decrypting encrypted PDFs (their
strings are not read, and the file says so), filters other than Flate,
rendering, text extraction.

## Reading

A tolerant scanner, in file order, never a resolver that follows the
cross-reference table: a damaged table must not hide objects, and the
order of bytes is what the tree shows.

- Header `%PDF-x.y` and the optional binary-marker comment.
- `N G obj … endobj`: the value is read by a small lexer (names with `#xx`,
  literal strings with nesting and escapes, hex strings, numbers, arrays,
  dictionaries, `N G R`). A stream's data runs for `/Length` bytes when that
  is a direct number that lands on `endstream`; otherwise to the next
  `endstream`, with a warning when `/Length` disagreed.
- `xref` tables with their subsections and entries, `trailer`, `startxref`
  (checked: it must point at a cross-reference section), `%%EOF`.
- Anything else is skipped to the next line that starts one of the above,
  as a problem over the skipped bytes.
- Every top-level item's range runs to the next item, so whitespace and
  comments are not "hidden".

Revisions: every `startxref N %%EOF` ends one. With more than one, the top
level groups into `revision 1`, `revision 2`, …; found by a pre-scan so
parents are always created before children.

Limits: nesting depth 32, dictionary and array entries read in full but
cross-reference entries get nodes only for the first 4,096, decompression
capped at 16 MB per stream.

## What it reveals

Facts, each linked to the bytes that hold it:

- From the latest information dictionary: title, author, subject, keywords,
  application (`/Creator`), producer, created, modified. Strings decoded from
  UTF-16BE (with BOM) or PDFDocEncoding; dates as `2026-03-05 17:42 +02:00`.
- When the dictionary sits in a compressed object stream, it is read from
  the decompressed bytes, and the fact links to that stream.
- From XMP, only what the dictionary did not say, plus the number of steps in
  its editing history.
- `updates`: "changed N times after it was first saved; the earlier versions
  are still inside", linked to revision 2.

Problems of concern Hidden, which the verdict lists: a document that runs
JavaScript (`/JS`, `/JavaScript`), launches a program (`/Launch`), or carries
files (`/EmbeddedFile`).

An encrypted PDF (`/Encrypt` in the trailer) yields no string facts.

## Map, docs, web

Roles: the information dictionary and XMP stream are metadata; other streams
(pages, fonts, images) content; object and cross-reference streams and
everything else structure. Docs cite ISO 32000-1:2008 via Adobe's free copy
by section. The web gets `pdf` as a format: chips, tints (header sig, objects
text/idat by kind, xref and trailer ihdr, `%%EOF` iend), the reveals card
titled "What this document reveals", and a sample.

## Testing

Fixtures from `scripts/make-sample-pdf.py`: `report.pdf` (two revisions,
UTF-16 title, XMP) and `compact.pdf` (object stream, cross-reference stream,
compressed XMP). Facts checked against those; macOS PDFKit reads the same
author. Every truncation of each fixture parses without an Error-free lie;
random bytes after `%PDF-` never panic; the docs coverage test includes
both.
