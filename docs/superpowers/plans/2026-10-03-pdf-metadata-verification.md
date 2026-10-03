# PDF Metadata Verification Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Verify removal of the approved PDF Info/XMP metadata kinds through the shared core classifier and CLI.

**Architecture:** Keep PDF coverage in `hexscope-core::verification::covered`, so CLI and the existing WASM/Web path inherit the same decisions. Track whether declared Info/XMP sources were readable and include that in summary completeness. Exercise real input and cleaned output bytes with `compact.pdf`, `report.pdf`, and a PDF containing Subject/Keywords; leave all other PDF finding kinds unchecked.

**Tech Stack:** Rust workspace, core/CLI integration tests, existing PDF fixtures.

**Spec:** `docs/superpowers/specs/2026-10-03-pdf-metadata-verification-design.md`

## Global Constraints

- Only complete PDF parses, file scope, and these kinds may be confirmed removed: `author`, `title`, `subject`, `keywords`, `application`, `producer`, `created`, `modified`, `history`.
- `updates`, `earlier`, hidden or covered text, comments, forms, attachments, links, and embedded-photo metadata remain outside the whitelist.
- Verification reports stay value-free; existing Web QR and selected-PDF-text checks stay supplemental.
- Do not change clean-copy creation, CLI defaults, dependencies, or production deployment.

## Review Focus

- PDF metadata in compressed object streams must still be read and verified; the `compact.pdf` real-fixture test covers it.
- Unsupported revision findings must stay unchecked after cleaning; the `report.pdf` real-fixture test asserts this.
- Unsupported kinds, wrong scope, and non-PDF formats must not be promoted; the core whitelist boundary test covers these cases.
- Incomplete PDF summaries must not be called removed; the core classifier test exercises PDF coverage with an incomplete source snapshot.
- An unreadable declared XMP stream must make the PDF summary incomplete, keep extracted claims unchecked, and still allow cleaner output.
- Subject and Keywords must be extracted from a real Info dictionary and classified as removed after cleaning/reparsing.
- CLI output must not leak source metadata values; both real-fixture CLI tests assert every source value is absent from its report.

---

### Task 1: Extend the shared core whitelist

**Files:**
- Modify: `crates/hexscope-core/src/verification.rs`

**Interfaces:**
- Consumes: `crate::summary::summarize`, `crate::clean::clean`, existing `snapshot` and `compare` functions.
- Produces: PDF file-scope coverage for the nine metadata kinds listed in Global Constraints.

- [x] **Step 1: Write real-fixture and boundary regressions**

In the existing verification tests, clean and reparse `compact.pdf` and `report.pdf`; assert the approved metadata kinds are removed, `updates` and `earlier` are unchecked, and unsupported kinds, wrong scopes, other formats, and incomplete PDF snapshots remain unchecked.

Also cover Subject and Keywords in a real PDF Info dictionary. Add a malformed/unsupported XMP filter case that proves incomplete metadata coverage stays unchecked while the cleaner still writes a copy.

- [x] **Step 2: Run the tests to confirm the PDF coverage regression**

Run: `cargo test -p hexscope-core pdf_metadata_verification`

Expected: the real PDF metadata is currently `coverage_incomplete`, and the tests fail before production changes.

- [x] **Step 3: Add the narrow PDF file-scope whitelist**

In `covered`, add a PDF branch for exactly the nine approved kinds while retaining the existing JPEG branch and requiring `scope == "file"` for both.

- [x] **Step 4: Run the focused core tests**

Run: `cargo test -p hexscope-core pdf_metadata_verification`

Expected: all real-fixture and whitelist boundary tests pass.

- [x] **Step 5: Track unreadable PDF metadata sources**

Have PDF fact extraction report when a declared Info/XMP source could not be read. Fold that state into `Summary.complete` so shared verification will not call related findings removed. Keep the cleaner path available.

- [x] **Step 6: Verify real Info dictionary Subject and Keywords**

Build a small valid PDF with those fields, clean it, reparse the actual output, and assert both are reported removed.

### Task 2: Prove CLI behavior against the written copy

**Files:**
- Modify: `crates/hexscope-cli/tests/cli.rs`

**Interfaces:**
- Consumes: shared PDF verification behavior from Task 1 and existing `clean --verify --json` interface.
- Produces: CLI regression coverage for a fully checked compact PDF and a report PDF with unsupported revision findings.

- [x] **Step 1: Add fixture-backed CLI regressions**

Copy each fixture into a unique temporary directory, run `clean --verify --json --out OUTPUT INPUT`, and assert `compact.pdf` exits 0 with metadata in `removed` and empty `present`/`unchecked`; assert `report.pdf` exits 1 with `updates` and `earlier` in `unchecked`. In both reports assert every source fact value is absent.

- [x] **Step 2: Run the focused CLI tests**

Run: `cargo test -p hexscope-cli clean_verify_pdf_metadata`

Expected: the compact fixture fails until the core whitelist is active; the report fixture keeps unsupported findings unchecked.

- [x] **Step 3: Run the full CLI test target**

Run: `cargo test -p hexscope-cli`

Expected: all CLI integration tests pass.

### Task 3: Update the roadmap and run the repository gate

**Files:**
- Modify: `docs/roadmap.md`

- [x] **Step 1: Mark the completed PDF metadata slice and preserve remaining work**

Update the P0 PDF verification item to name shared Info/XMP metadata coverage and retain Web-only QR/selected-text integration and unsupported PDF findings as follow-up work.

Clarify that residual findings observed in output are `present`, and that PR #14's original JPEG-only coverage is historical.

- [x] **Step 2: Run the Rust repository gate**

Run: `cargo fmt --all --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo test --all`.

Expected: all commands pass; report ignored tests separately if any.

- [x] **Step 3: Run RepoPilot and request an independent diff review**

Run the current-tree RepoPilot review and inspect every signal. The final review reported no in-diff findings; two function-growth signals were manually checked against the small metadata-completeness changes, and no extra implementation was needed. Then review the full diff against `origin/main` before opening a draft PR.

- [ ] **Step 4: Commit the focused implementation**

Commit the spec, plan, core change, fixture tests, and roadmap note after verification and review.
