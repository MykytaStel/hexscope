# ZIP, Part 1: Structure — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** hexscope opens ZIP archives (and `.docx`, `.apk`, …) and shows their structure in file order, naming what an archive can hide. Entries can be extracted by the core.

**Architecture:** A new `zip` module in the core reads the EOCD, optional ZIP64 records and the central directory, then each local header. The central directory and end records are built in separate trees and grafted at the end, so the tree runs in file order. A small field emitter reads a value and adds its node in one call. `extract` decompresses one entry on demand, through the existing `inflate`. The dispatcher, bridge and page learn a `zip` format.

**Tech Stack:** Rust 1.98 (edition 2024, no runtime deps), flate2 (dev only), wasm-bindgen 0.2.128, TypeScript 7.

Spec: `docs/superpowers/specs/2026-09-23-zip-design.md` (Part 1 plus the limits and no-EOCD sections).

## Global Constraints

- `#![forbid(unsafe_code)]`; no new runtime dependencies. Nothing panics and every loop terminates, whatever the bytes.
- Every offset and size comes from the file and is checked before use; reads go through `Reader`.
- Error: bytes that cannot be read. Warning: bytes read that break the format. A limit of the tool adds no node.
- Valid archives, including the `textutil` fixture, produce no Warning or Error.
- Caps: 100,000 entries; field nodes for the first 2,000; names shown up to 1,024 bytes; EOCD searched over the last 65,557 bytes.
- Before each commit: `cargo fmt --all`, `cargo clippy --all-targets --all -- -D warnings`, `cargo test --all`, and `pnpm --filter web build` when the web changes. Check each command's exit code, not a filtered pipe.

## File Structure

| File | Responsibility |
|---|---|
| `crates/hexscope-core/src/reader.rs` | `u64_le` |
| `crates/hexscope-core/src/zip/mod.rs` | `ZipDocument`, `ZipEntry`, `parse_zip`, `is_zip`; EOCD/ZIP64/central/local reading; overlap and gap checks |
| `crates/hexscope-core/src/zip/fields.rs` | `Fields`: reads little-endian fields and adds their nodes; DOS time, method names, extra fields |
| `crates/hexscope-core/src/zip/extract.rs` | `extract`, `ExtractError` |
| `crates/hexscope-core/src/zip/testing.rs` | Byte-exact archive builder for tests |
| `crates/hexscope-core/src/document.rs`, `lib.rs` | `Format::Zip`, `Document::Zip`, dispatch; `identify` becomes `pub(crate)` |
| `crates/hexscope-core/tests/golden.rs`, `tests/properties.rs`, `tests/fixtures/report.docx` | Real-file and property tests |
| `scripts/make-sample-docx.py` | Writes the fixture and the site's sample |
| `crates/hexscope-wasm/src/lib.rs` | `format = "zip"` |
| `apps/web/src/{model,main}.ts`, `index.html` | `zip` format, chips, tints, *Try a document* |

## Tasks

### Task 1: Field emitter, test builder, EOCD and central directory
- [ ] `Reader::u64_le`, with a test.
- [ ] `zip/testing.rs`: `Entry { name, data, method, descriptor, zip64 }`, `Archive { entries, comment, prefix, zip64_eocd }`, and `build(&Archive) -> Built { bytes, local: Vec<u64>, central: Vec<u64>, eocd: u64 }`. Offsets are written relative to the archive's start, not the file's: the unadjusted self-extractor case.
- [ ] Failing tests: a two-entry archive (stored + deflate) gives root value "2 entries"; children in order are the two entries, then "central directory" and "end of central directory"; `entries[i]` carry name, method, sizes, CRC and the data range.
- [ ] `fields.rs` and `mod.rs`: find the EOCD, read it into its own tree, read the central directory into its own tree while collecting `Central` values, read local headers into the main tree in offset order, then graft. Make the tests pass.
- [ ] Tests for ZIP64 (sizes from the 0x0001 extra field; EOCD from the ZIP64 record), a data descriptor (with and without its signature), a comment, and an empty archive.
- [ ] Commit: `feat(core): read ZIP structure`.

### Task 2: What an archive hides, damage, limits
- [ ] Failing tests, one per row of the spec's problem table: prepended data (node label starts "data before the archive", offsets shifted correctly); local vs central name mismatch (Warning under the local header, on the name field); overlapping entries (Warning under the later entry); unreferenced gap (Warning on the root); EOCD count disagreeing (Warning on the count field); a central offset past the end (Error on that record's offset field); no EOCD with sequential local-header recovery; truncation at every length of a small archive never panics and always yields a root.
- [ ] Implement until green; add the entry cap and the 2,000-entry field-detail cap, with a test that 3,000 tiny entries parse, list and stay under the node budget (fewer than 20 nodes per entry on average).
- [ ] Commit: `feat(core): name what a ZIP hides, and survive damage`.

### Task 3: Extract
- [ ] Failing tests: every entry of a built archive extracts to its input; an encrypted flag gives `Encrypted`; method 12 gives `Unsupported(12)`; a flipped data byte gives `Crc` or `Inflate`; a limit smaller than the output gives `TooLarge`; a data range clipped by truncation gives `OutOfRange`.
- [ ] `extract.rs`. Commit: `feat(core): extract a ZIP entry`.

### Task 4: Dispatch, fixture, bridge, page
- [ ] `document.rs`: `Format::Zip`, `Document::Zip(ZipDocument)`. `parse` routes to ZIP after PNG and JPEG when `zip::is_zip(data)`, meaning it starts with `PK\x03\x04` or `PK\x05\x06`, or an EOCD sits in the tail. Update the dispatch test. `identify` becomes `pub(crate)` so the prefix node can say what the prefix looks like.
- [ ] `scripts/make-sample-docx.py` (pattern: `make-sample-photo.py`): `textutil` → `tests/fixtures/report.docx` (title "Quarterly report", author "Olena Koval", editor "o.koval", company "Hexscope Test Co"); copy + `word/media/photo.jpg` stored → `apps/web/public/samples/report.docx`.
- [ ] Golden test: the fixture parses with no Warning/Error, lists 8 entries, and every entry extracts with a matching CRC; `docProps/core.xml` contains "Olena Koval". Property test: `PK\x03\x04` + arbitrary bytes never panics.
- [ ] Bridge: `Document::Zip(doc) => flatten(&doc.tree)` with `format = "zip"`. Page: `format` union gains `"zip"`; file chips "ZIP · N entries" (the count from the root's value); top-level tints for ZIP (entries `text`, central directory `ihdr`, end records `iend`); a *Try a document* button that opens `samples/report.docx`.
- [ ] Browser check: the sample opens, entries in order, hovering an entry's data highlights its bytes, no problems listed; 375 px width.
- [ ] Commit: `feat: open ZIP archives`.
