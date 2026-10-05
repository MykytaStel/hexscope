# Photo Platform Implementation Plan

> For agentic workers: REQUIRED SUB-SKILL: Use superpowers:executing-plans for native execution in the current checkout.

**Goal:** Deliver verified photo copies, batch receipts, shared film processing, real-scan evaluation and local photo mosaic across Rust, CLI and web.
**Architecture:** Conservative parser-backed verification; codec-free raster/mosaic core; native CLI codecs and a lazy TIFF/roll WASM module; sequential batch jobs.
**Tech Stack:** Rust, wasm-bindgen, TypeScript, Vite, browser worker, image/TIFF codecs.
**Spec:** `docs/superpowers/specs/2026-10-05-photo-platform.md`.

## Global Constraints

Use current checkout on a feature branch. Preserve originals and existing JSON contracts. Bound allocations before decode. No private values in exported receipts. Continue all tasks without approval pauses; user authorized merge and deploy. Each implementation task starts with failing regression tests, ends with passing focused checks and a commit.

## Review Focus

1. Malformed/truncated metadata must never produce false removed or safe results.
2. Duplicate paths, existing outputs, symlinks and output directories inside inputs must not overwrite originals or each other.
3. TIFF bomb dimensions, unsupported colors, bit depths, multiple pages and profiles must fail or have an explicit limitation before allocation.
4. Cancellation and stale worker results must not mix two rolls or batches; errors must retain successful outputs.
5. Mosaic must handle poles/date-line, invalid dates, missing serials and export without personal metadata; matching stored data is not identity evidence.

### Task 1: PNG/WebP verification

**Files:** `crates/hexscope-core/src/verification.rs`, core integration tests, `docs/verification-coverage.md`.
**Interfaces:** Produces canonical four-kind file-scope capability matrix consumed unchanged by CLI/WASM/web.

- [ ] Add valid EXIF PNG/WebP regressions for removal, payload preservation and incomplete parses; run failing tests.
- [ ] Add exact capabilities and fix any parser completeness hole demonstrated by a regression.
- [ ] Run core verification/clean regressions; document matrix; commit.

### Task 2: Batch receipts and folder safety

**Files:** CLI `main.rs`/`batch_report.rs`, web `batchclean.ts`, `batch.ts`, `batch-controller.ts`, regression tests.
**Interfaces:** Consumes Task 1 core report; produces version-1 value-free receipt with file indexes, output states and aggregate counts.

- [ ] Reproduce duplicate-name writes and unknown-file reassurance; implement preflight collision/output exclusion guards.
- [ ] Add optional CLI aggregate report and per-file failure continuation; verify actual written copies.
- [ ] Web batch clean reads every bounded copy back, stores per-file status and adds report to ZIP; cancellation guards cover every await.
- [ ] Run CLI/web batch regressions; commit.

### Task 3: Shared renderer and roll/TIFF processing

**Files:** core `film.rs`, CLI `film.rs`, optional raster crate/WASM bridge, web film renderer and lazy roll UI, build scripts.
**Interfaces:** Validated `srgb-density-v1` recipe; RGBA8/RGBA16 core rendering, decoder limits, per-roll value-free receipts.

- [ ] Add identity, negative, crop, precision, invalid-input and shared-recipe tests; observe failures.
- [ ] Port density algorithm to bounded Rust core; bind existing web rendering to it and verify parity.
- [ ] Native decode, orientation, bounded processing and CLI recipe batch command; optional TIFF WASM decode/encode with 16-bit output.
- [ ] Add web roll UI with recipe import, file selection, format selection, progress, explicit sRGB/profile assumptions and ZIP receipt.
- [ ] Run raster/recipe/CLI/web tests and build; measure optional download and normal bundle budgets; commit.

### Task 4: Real-scan evaluation

**Files:** `scripts/evaluate-film.*`, `docs/film-evaluation.md`, licensed fixture manifest.
**Interfaces:** Real-source manifest plus deterministic machine-readable evaluation; consumes roll renderer and frame heuristic.

- [ ] Verify source rights and fetch bounded real scan samples; record provenance, hashes and expected frame behavior.
- [ ] Evaluate quality bounds, frame decisions and positive-scan false positives separately from synthetic tests.
- [ ] Record measurements, limitations and explicit OCR gate; commit.

### Task 5: Photo mosaic

**Files:** core `mosaic.rs`, CLI mosaic command, WASM bridge, lazy web batch panel and tests.
**Interfaces:** Bounded file-index signals, repeated-serial groups, 100m stored-location groups, dated index order; value-free JSON.

- [ ] Test repeated/missing serials, poles/date-line, invalid coordinates/dates and value-free export.
- [ ] Implement deterministic shared core and CLI folder command.
- [ ] Add local-only web batch analysis panel with evidence wording and report export; avoid retaining raster bytes.
- [ ] Run focused and complete gates; commit.

### Task 6: Review and ship

- [ ] Independent whole-branch review, RepoPilot integrity/security reconciliation, fix important findings with regressions.
- [ ] Full Rust/web gates, lazy-module budgets and desktop/phone browser proof.
- [ ] Create/attach PR, inspect hosted CI, merge and deploy, verify live new flows.
- [ ] Report shipped functionality, evidence and precise remaining research limitations in Ukrainian.
