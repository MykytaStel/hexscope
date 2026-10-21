# Film renderer sampling and parity implementation plan

> For agentic workers: use the `superpowers:executing-plans` skill to implement this plan task by task. Keep the approved design spec open while working.

**Goal:** Make the bounded film renderer's estimates resistant to fixed-stride aliasing and keep one authoritative implementation.

**Architecture:** Rust `hexscope-core::film` remains authoritative for native and Web/WASM paths. A deterministic stratified sampler will visit the whole raster with at most 131,072 positions, reused for base estimation, tone bounds, and clipping. Supplying a base skips base-histogram work. The duplicate TypeScript pixel renderer will be deleted after its behavioral cases move to Rust.

**Tech Stack:** Rust, `hexscope-core` unit tests, existing TypeScript/Vitest UI-helper tests, WASM build path.

**Spec:** `docs/superpowers/specs/2026-10-21-film-render-sampling-design.md`

## Global constraints

- Preserve the 12,000,000-pixel render limit and 131,072-sample limit.
- For rasters below the sample limit, use every pixel so estimates match the full raster.
- Keep sample selection deterministic, in bounds, spread over the entire raster, and identical across histogram passes.
- Keep crop, UI base-patch sampling, queue, and settings helpers that the interface still uses.
- Do not add film-recognition or archival color-recovery claims.

## Review focus

- Can periodic patterns still line up with a fixed interval and skew the estimate?
- Are the exact sampled positions reused for every estimate, and is a supplied base bypassing base estimation?
- Did all behavior formerly tested only in TypeScript move to Rust before deleting the duplicate renderer?
- Is the four-image Apollo corpus still described as a narrow smoke corpus?

---

### Task 1: Cover sampler behavior and renderer semantics in Rust

**Files:** `crates/hexscope-core/src/film.rs` and its Rust test module.

**Interfaces:** Produces Rust regression coverage for the renderer and sampled estimates; Task 2 implements those assertions, while Task 3 uses this coverage as the prerequisite for deleting TypeScript rendering.

- [x] Add failing tests comparing sampled estimates with full-raster estimates for periodic stripes, flat fields, gradients, very wide/tall rasters, and dimensions below the cap.
- [x] Add test coverage for sampler determinism, raster-wide strata coverage, in-bounds positions, and the hard cap.
- [x] Move renderer behavior cases from `apps/web/src/film-lab.test.ts` into Rust: neutral positive output, mono-negative output, calibrated color base, clipping bounds, 8/16-bit pixels, and invalid dimensions/settings.
- [x] Run `cargo test -p hexscope-core film`; **Expected:** focused tests fail on the current fixed-step estimates or expose missing Rust behavior coverage.

### Task 2: Implement deterministic stratified estimates

**Files:** `crates/hexscope-core/src/film.rs`, Rust film tests.

**Interfaces:** Consumes Task 1's tests; produces a bounded deterministic sampler used by the existing renderer and relied upon by Task 3's single-implementation cleanup.

- [x] Add one deterministic stratified index generator with no more than `MAX_SAMPLES` positions and no repeated positions.
- [x] Generate the positions once and reuse them for every histogram/clipping pass; visit the whole raster rather than only the same fixed offset in each interval.
- [x] Skip base histogram construction when `settings.base` is already supplied.
- [x] Keep tone conversion and output pixel generation unchanged unless a failing regression proves the sampling boundary requires a correction.
- [x] Run `cargo test -p hexscope-core film` and inspect edge cases at one pixel, exactly the cap, and just above the cap. **Expected:** all cases pass; sample count never exceeds the cap and degenerate inputs stay in bounds.

### Task 3: Remove the duplicate TypeScript pixel renderer

**Files:** `apps/web/src/film-lab.ts`, `apps/web/src/film-lab.test.ts`, any imports of `renderFilmPixels`.

**Interfaces:** Consumes the Rust coverage from Tasks 1–2; preserves only TypeScript helpers used by the UI and queue.

- [x] Confirm renderer behavior coverage now exists in Rust before changing the TypeScript test suite.
- [x] Remove `renderFilmPixels` and its pixel-only helpers/tests; retain UI helpers still used for crop geometry, base-patch sampling, recipes, and queue behavior.
- [x] Search for stale imports and remove only those that reference the duplicate renderer.
- [x] Run `cd apps/web && npm test -- src/film-lab.test.ts src/film-profile.test.ts` and `cargo test -p hexscope-core film`. **Expected:** remaining UI-helper tests and Rust renderer tests pass with no TypeScript renderer imports.

### Task 4: Check production integration and claims

**Files:** Web film integration and `docs/film-evaluation.md` if implementation evidence changes its wording.

**Interfaces:** Consumes the authoritative Rust renderer and remaining UI helpers; confirms production still calls the WASM path and documentation preserves the narrow smoke-corpus limits.

- [x] Confirm the Web UI still invokes the Rust/WASM renderer and that crop/base-selection UI behavior remains covered.
- [x] Keep the four Apollo positives labeled as a narrow smoke corpus; do not infer accuracy, recall, or negative-color calibration from it.
- [x] Run `cargo test -p hexscope-core film`, the focused Web tests above, and the relevant WASM build command recorded by the workspace CI. **Expected:** Rust and Web checks pass and the WASM build still exposes the production renderer.

**Acceptance:** The renderer is deterministic and capped, avoids fixed-stride aliasing on the regression corpus, skips an unnecessary base histogram when calibrated input is supplied, and has one production implementation.

## Final review follow-up — 2026-10-21

- Independent review found `stratum * size` could overflow `usize` in WASM32 for ordinary images. The sampler now computes equal-width boundaries using quotient/remainder arithmetic with bounded `u64` remainder products, avoiding overflow while spreading wider strata across the raster. Full core tests initially exposed that grouping the remainder only at the start also biased long-raster gradients; the evenly distributed boundary formula now passes all 9 focused film tests and all 388 core tests.
- `pnpm wasm:raster`, the Web test suite (354 tests, including the 512×512 WASM regression), and the production Web build passed. The four-image Apollo corpus remains a narrow smoke corpus and does not establish film-processing accuracy.
