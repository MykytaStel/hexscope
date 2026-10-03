# Shared Clean-Copy Verification Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Verify the cleaned bytes a person is about to send, using one conservative Rust classifier through core, CLI, and WebAssembly/web.

**Architecture:** `hexscope-core` owns a value-free report and a multiset comparator over parser-backed snapshots. The WebAssembly parser retains the source snapshot it already derived; its existing `Parsed.verifyCopy(bytes)` bridge summarizes actual output bytes under a 10 MiB bound and invokes the same comparator. Web carries only a local worker request token with the parsed model, preserving PR #13's async report flow and browser-specific QR/PDF checks. The CLI snapshots before cleaning, atomically writes the copy, reads it back, and invokes the same comparator.

**Tech Stack:** Rust workspace, wasm-bindgen, TypeScript, Vitest, Playwright, pnpm.

**Spec:** `docs/superpowers/specs/2026-10-03-shared-clean-verification-design.md`

## Global Constraints

- No new runtime dependencies, network access, telemetry, OCR, or server processing.
- Reports use schema version `1` and never contain finding values, scopes, parser trees, or file bytes.
- The only initially complete coverage is JPEG file scope: `camera`, `serial`, `owner`, `location`.
- A warning or error node makes a parser summary incomplete.
- Web verification skips output parsing above `10 MiB`; copy creation remains available.
- `clean` without `--verify` keeps its current behavior and exit status.
- CLI `--verify` returns `0` only for a produced copy with no present or unchecked items; `1` for operation failure/no copy/residual/unchecked; `2` for usage or I/O errors.
- Do not raise JavaScript or WASM size budgets; compare gzip-9 artifacts to the captured baseline.

## Review Focus

- Duplicate same-kind facts can survive cleaning independently — core test must preserve multiplicity.
- A same-kind fact with a changed value is still personal information — core test must classify it `present`.
- A damaged JPEG can yield partial facts — both source and output incomplete parses must yield `unchecked`.
- A browser copy over 10 MiB must still be offered to the user while verification says `verification_skipped` — WASM and web tests cover it.
- `clean --verify --in-place` replaces the source path — CLI test must prove the source snapshot predates replacement and the written bytes are checked.

## File Structure

- `crates/hexscope-core/src/summary.rs`: records whether parser output has warning/error nodes.
- `crates/hexscope-core/src/verification.rs`: owned snapshots, capabilities, report types, multiset matching, value-free JSON.
- `crates/hexscope-core/src/lib.rs`: exports the new module.
- `crates/hexscope-wasm/src/lib.rs`: retains the existing parse's summary snapshot and exposes `Parsed.verifyCopy(bytes: Uint8Array)` for actual output bytes.
- `apps/web/src/worker.ts`: maps the open parse to its RPC token, invokes the shared bridge on copy bytes, and merges QR/PDF supplements into the existing verification response.
- `apps/web/src/verification.ts`: TypeScript report shape, defensive decode, and cautious status sentence.
- Existing PR #13 `apps/web/src/copyverification.ts`, `verification.ts`, `cleancard.ts`, `drawer.ts`, and `copyverification-loader.ts`: pass the local source token, decode the strict core report, and retain the lazy accessible result UI and copy actions.
- `apps/web/src/i18n.ts`: translates the new changed-value reason within the page JavaScript budget.
- `apps/web/src/styles/input-and-accessibility.css`: styles the verification result card.
- `apps/web/src/logic.test.ts` and a focused verification test: real WASM contract and value-free/decode behavior.
- `apps/web/e2e/site.e2e.ts`: real clean-copy UI path.
- `crates/hexscope-cli/src/main.rs`: `clean --verify`, disk read-back, human/JSON output and conservative exit.
- `crates/hexscope-cli/tests/cli.rs`: actual output, mixed batch, in-place, no-copy, JSON, and I/O cases.
- `apps/web/src/wasm*`: gitignored glue and binary artifacts, regenerated locally by `pnpm wasm` as inputs to Web tests/build; source changes stay in `crates/hexscope-wasm/src/lib.rs`.
- This plan's `Execution log` below: rulings, task test evidence, and commit IDs.

## Interfaces

Task 1 produces the following core contract for Tasks 2 and 4:

```rust
pub struct VerificationFinding { pub kind: String, pub scope: String, pub value: Option<String> }
pub struct VerificationSnapshot { pub format: String, pub complete: bool, pub findings: Vec<VerificationFinding> }
pub struct VerificationItem { pub kind: String, pub reason: VerificationReason, pub unexpected: bool }
pub struct VerificationReport { pub schema_version: u8, pub removed: Vec<VerificationItem>, pub present: Vec<VerificationItem>, pub unchecked: Vec<VerificationItem> }
pub fn snapshot(summary: &Summary) -> VerificationSnapshot;
pub fn compare(source: &VerificationSnapshot, output: &VerificationSnapshot) -> VerificationReport;
```

`VerificationReport::to_json()` emits only the version, groups, kind, reason, and unexpected marker. Task 2 exposes `Parsed.verifyCopy(bytes: Uint8Array): string`, returning that JSON; it uses the saved source snapshot and summarizes the copy's bytes once unless they exceed the cap. Task 3 decodes it into `VerificationReport`. Task 4 consumes the same core `snapshot` and `compare` functions.

---

### Task 1: Core Snapshot and Classifier

**Files:**
- Modify: `crates/hexscope-core/src/summary.rs`
- Modify: `crates/hexscope-core/src/lib.rs`
- Create: `crates/hexscope-core/src/verification.rs`

**Interfaces:** Produces the `VerificationSnapshot`, `VerificationReport`, `snapshot`, and `compare` contract above.

- [x] **Step 1: Write failing core tests**
  - `verification_compares_duplicate_findings_without_collapsing_occurrences`: two duplicate source findings and one exact output finding produce one `present` and one `removed` item.
  - `verification_treats_a_changed_value_of_the_same_kind_as_present`: JPEG camera source and output values differ; the source result is `present/value_changed_same_kind`, not `removed`.
  - `verification_marks_uncovered_and_incomplete_findings_unchecked`: non-whitelisted kind/scope and either incomplete snapshot produce `unchecked` with the matching reason.
  - `verification_reports_output_only_findings_as_unexpected`: output-only kind yields `present/unexpected_output/unexpected=true`.
  - `verification_does_not_call_an_empty_comparison_clear`: two empty snapshots yield `unchecked/no_comparable_findings`.
  - `verification_serialized_report_contains_no_finding_values_or_scopes`: sentinel values and scopes do not appear in `to_json()`.
  - `verification_summary_is_incomplete_when_it_has_a_warning_or_error`: damage/warning fixtures set `Summary.complete` false.
- [x] **Step 2: Run the focused tests and observe the missing-contract failure**

  Run: `cargo test -p hexscope-core verification`

  Expected: FAIL because the module/API and `Summary.complete` are not implemented.
- [x] **Step 3: Implement the smallest core contract**
  - Derive `Summary.complete` from absence of `Warning` and `Error` nodes.
  - `snapshot` keeps all facts, with scope `file` and stable values in memory.
  - `compare` consumes exact matches first, then same-kind/scope matches, preserving duplicate occurrences. It reports a source item as removed only for the explicit JPEG whitelist and complete snapshots, with reason `removed`.
  - `to_json()` JSON-escapes kind strings and emits only report fields.
- [x] **Step 4: Re-run the focused tests**

  Run: `cargo test -p hexscope-core verification`

  Expected: all verification tests pass.
- [x] **Step 5: Run the core crate suite and commit**

  Run: `cargo test -p hexscope-core`

  Expected: exit 0. Commit core code/tests as `feat(core): add clean-copy verification`.

### Task 2: WebAssembly Verification Bridge

**Files:**
- Modify: `crates/hexscope-wasm/src/lib.rs`
- Modify generated files under `apps/web/src/wasm/` and `apps/web/src/wasm-media/` using `pnpm wasm`.

**Interfaces:** Consumes Task 1. Produces `Parsed.verifyCopy(bytes: Uint8Array): string`.

- [x] **Step 1: Write failing bridge tests**
  - `verifies_the_clean_copy_using_the_source_parse`: parse the real JPEG fixture, make its real clean copy, verify it, and assert the whitelisted findings are removed while unwhitelisted `lens` remains unchecked.
  - `reports_an_unchanged_copy_as_still_present`: verify an unmodified `CleanCopy` and assert supported source facts remain present.
  - `skips_a_copy_over_ten_mib_without_reparsing_it`: verify a copy over 10 MiB and assert `verification_skipped`.
- [x] **Step 2: Run the focused bridge test and observe failure**

  Run: `cargo test -p hexscope-wasm verifies_the_clean_copy_using_the_source_parse`

  Expected: FAIL because `verify_copy` is not implemented.
- [x] **Step 3: Implement the bridge**
  - Retain `snapshot(summary_of(&doc))` while the source document is already parsed.
  - On `Parsed.verify_copy(bytes)`, return an unchecked skipped report if bytes exceed `10 * 1024 * 1024`; otherwise summarize the supplied bytes once and call `hexscope_core::verification::compare`.
  - Return the core's value-free JSON and keep source values inside Rust.
- [x] **Step 4: Run the bridge tests and crate suite**

  Run: `cargo test -p hexscope-wasm`

  Expected: exit 0.
- [x] **Step 5: Build both browser WASM variants and commit the bridge source**

  Run: `pnpm wasm`

  Expected: both builds succeed and gitignored generated artifacts are available for Web tests/build. Commit the bridge source as `feat(wasm): expose clean-copy verification`.

### Task 3: Web Worker and User Result

**Files:**
- Modify: `apps/web/src/worker.ts`
- Create: `apps/web/src/verification.ts`
- Modify: `apps/web/src/cleancard.ts`
- Modify: `apps/web/src/drawer.ts`
- Modify: `apps/web/src/i18n.ts`
- Modify: `apps/web/src/logic.test.ts`; add focused verification unit tests.
- Modify: `apps/web/e2e/site.e2e.ts`

**Interfaces:** Consumes `Parsed.verifyCopy(bytes)` JSON from Task 2. The existing PR #13 verification response keeps its `VerificationReport` shape and copy actions.

- [x] **Step 1: Write failing report decode/display tests**
  - `decodes_only_the_versioned_value_free_report`: valid report fields are accepted and the shape has no value/scope fields.
  - `turns_malformed_or_unknown_reports_into_unchecked`: malformed JSON/schema returns an explicit unchecked state.
  - Extend the existing real-WASM logic test so the fixture's clean copy produces removed JPEG findings and an unchanged copy produces present findings.
- [x] **Step 2: Run focused web tests and observe failure**

  Run: `pnpm --filter web exec vitest run src/verification.test.ts src/logic.test.ts`

  Expected: FAIL because report decoding and bridge response plumbing are missing.
- [x] **Step 3: Implement worker and result types**
  - After the actual output `Blob` bytes are read, look up the source parse by its worker token and synchronously call `Parsed.verifyCopy(bytes)`. If the source has already been replaced, report its parser findings as unchecked.
  - JSON-decode the core report defensively; retain the existing QR rescan and PDF selected-text supplements.
  - Keep every operation's actual returned bytes and saved/share flow unchanged.
- [x] **Step 4: Implement the localized, accessible status card**
  - Show a clear checked / findings remain / could not check state with grouped kinds and reason explanations; never render values or scopes.
  - Add English keys and Ukrainian translations. Preserve `prefers-reduced-motion`, keyboard access, and existing clean actions.
- [x] **Step 5: Run Web unit tests and build**

  Run: `pnpm --filter web test && pnpm --filter web build`

  Expected: exit 0.
- [x] **Step 6: Add and run the focused clean-copy E2E flow**

  Run: `pnpm --filter web e2e --grep "clean copy verification"`

  Expected: the real JPEG fixture shows verified removal before save/share, with no personal values in the result.
- [x] **Step 7: Commit the Web slice**

  Commit as `feat(web): show clean-copy verification`.

### Task 4: CLI `clean --verify`

**Files:**
- Modify: `crates/hexscope-cli/src/main.rs`
- Modify: `crates/hexscope-cli/tests/cli.rs`

**Interfaces:** Consumes Task 1 snapshots/comparator. Adds `Options.verify` and applies it only to `clean`.

- [x] **Step 1: Write failing CLI integration tests**
  - `clean_verify_checks_the_copy_written_to_disk_and_hides_values`: real JPEG copy JSON is version 1, reports removed kinds, excludes sentinel fact values, and opens the actual output bytes.
  - `clean_verify_handles_multiple_files_and_in_place`: mixed cleanable and unsupported inputs produce separate JSON lines and conservative exit status; in-place output is compared with the pre-clean snapshot.
  - `clean_verify_returns_one_when_no_copy_was_created`: an already-clean/non-cleanable input has no successful verification and exits 1.
  - `clean_verify_rejects_non_clean_commands`: `repair --verify` and `redact --verify` return usage status 2.
  - `clean_verify_reports_residual_or_unchecked_findings`: a fixture outside the whitelist yields unchecked and exit 1 while the copy remains present.
- [x] **Step 2: Run the focused CLI tests and observe failure**

  Run: `cargo test -p hexscope-cli clean_verify`

  Expected: FAIL because `--verify` and its result handling are absent.
- [x] **Step 3: Implement verification after atomic write**
  - Save the source snapshot before cleaning. Compute the output path before `finish`; after successful write, read it from disk and compare summaries.
  - Preserve default non-verify output. `--json` suppresses human copy output for verify mode and emits one valid object per input.
  - Aggregate exit code exactly as specified; never remove the copy when verification is present or unchecked.
- [x] **Step 4: Run CLI focused and crate tests**

  Run: `cargo test -p hexscope-cli`

  Expected: exit 0.
- [x] **Step 5: Commit CLI support**

  Commit as `feat(cli): verify clean copies`.

### Task 5: Repository Gate, Budgets, and Review

**Files:**
- Modify: `CONTRIBUTING.md` only if artifact measurements show a factual budget mismatch.
- Modify: this plan's `Execution log` with commands/results and commit IDs.

- [x] **Step 1: Run the Rust workspace tests**

  Run: `cargo test --all`

  Expected: exit 0.
- [x] **Step 2: Run Clippy**

  Run: `cargo clippy --workspace --all-targets -- -D warnings`

  Expected: exit 0.
- [x] **Step 3: Run the Web suite and focused E2E**

  Run: `pnpm --filter web test && pnpm --filter web e2e --grep "clean copy verification"`

  Expected: exit 0.
- [x] **Step 4: Build and measure production artifacts**

  Run: `pnpm build`, then gzip-9 measure main JS, media WASM, and full WASM.

  Expected: main JS ≤65,000 bytes, media WASM ≤140,000 bytes, full WASM ≤290,000 bytes as enforced by `.github/workflows/ci.yml`. Original captured baseline: 64,528; 126,889; 286,084 bytes respectively. Rebuilt `feff4f2` bridge baseline during this integration: full WASM 288,556 bytes.
- [x] **Step 5: Inspect the complete diff and commit final documentation/evidence**

  Run: `git diff --check` and `git status --short`.

  Expected: no whitespace errors, only plan-scoped changes. Commit any final correction with its own focused message.

## Execution log

- `Ruling: The earlier draft said the Web verifier already existed, but the checked-out source has no reparse/report path — replace that premise with a new end-to-end implementation — cost if wrong: the slice is broader by one Web status card.`
- Baseline build: `pnpm --filter web build` passed; warnings about `./theme.js` missing `type="module"` exist in 11 HTML files.
- Baseline gzip-9 artifact sizes: main JS `64,528/65,000`, media WASM `126,889/140,000`, full WASM `286,084/290,000` bytes.
- Task 1: the initial test run failed on the intentionally absent verification API; after implementation, `cargo test -p hexscope-core verification` passed 8/8 and `cargo test -p hexscope-core` passed 301 unit, 17 golden, and 14 property tests (6 timing tests remained ignored). `cargo fmt --all` and `git diff --check` passed.
- Task 1 commit: `2643b8d` (`feat(core): add clean-copy verification`).
- `Ruling: The real JPEG fixture also exposes lens/date/software/thumbnail facts outside the proven whitelist — keep them unchecked in the report and test the mixed result — cost if wrong: this milestone does not provide a whole-file clean verdict for that fixture.`
- Task 2: bridge RED failed because `Parsed.verify_copy` was absent; a real-photo first pass showed the expected uncovered lens/date/software/thumbnail items, so the test was corrected to require those unchecked results. `cargo test -p hexscope-wasm` passed 35/35; `pnpm wasm` built both variants; `pnpm --filter web build` passed with the 11 baseline `theme.js` warnings. Post-bridge gzip-9 sizes: main JS `64,529/65,000`, media WASM `129,573/140,000`, full WASM `288,565/290,000` bytes.
- Task 2 commit: `05cdef5` (`feat(wasm): expose clean-copy verification`).
- `Ruling: Eagerly loading the Web status card exceeded the main-JS limit by 128 gzip-9 bytes — lazy-load the card and its translations after the copy is made — cost if wrong: one small chunk loads on the result screen.`
- Task 3: decoder RED failed because the module was missing; `pnpm --filter web test` passed 64/64, `pnpm --filter web build` passed with the same 11 `theme.js` warnings, and the focused E2E passed on desktop and phone. The first E2E assertion used a new Ukrainian synonym for Location; it was corrected to the app's existing translation `Місце зйомки`. Final gzip-9 sizes after lazy-loading the status card: main JS `64,676/65,000`, media WASM `129,573/140,000`, full WASM `288,565/290,000` bytes.
- Task 3 commit: `f7b9087` (`feat(web): show clean-copy verification`).
- Task 4: `clean --verify` snapshots before cleaning, reads back the produced path (including in-place replacement), keeps copies with residual or unchecked facts, and emits value-free JSON or human output. Task 4 commit: `feff4f2` (`feat(cli): verify clean copies`).
- Final reviewer (fresh read-only review of the effective diff against `origin/main`) found three Important cases. Added red regressions for format changes, unknown-value matching, and empty directories; all three failed before their fixes. `cargo test -p hexscope-core verification_` then passed 14/14, and the focused empty-directory CLI test passed 1/1.
- Final fixes: format changes now yield `coverage_incomplete`; stable same-kind values pair before unknown values, and an unknown output prevents unsupported removal claims; `clean --verify` returns 1 when directory traversal produces no copy.
- `cargo test --all`: CLI 16, core 307, inflate 46, WASM 35, golden 17, and property 14 passed; 6 timing tests are ignored in debug by design. `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check`, `cargo check -p hexscope-core -p hexscope-wasm --target wasm32-unknown-unknown`, and `git diff --check` passed.
- Release large-file gate: `cargo test --release -p hexscope-core --test big -- --nocapture` passed 6/6, covering an 8 MB email, 134 MB movie, ZIP, XLSX, and two PDF workloads.
- Final Web verification: `pnpm --filter web test` passed 94/94; focused clean-copy E2E passed 8/8 on desktop and phone; full `pnpm --filter web e2e` passed 131 with 15 profile-specific skips.
- Final gzip-9 artifacts: main JS `64,994/65,000`, media WASM `130,799/140,000`, full WASM `289,921/290,000` bytes. The main JS has 6 bytes of headroom; this was kept under budget without raising any CI threshold. Vite still reports the 11 existing `theme.js` script-type warnings.
- RepoPilot's static review returned `REVIEW`, with zero semantic findings but two high-confidence heuristic signals: the optional worker token was labeled an access-control change, and freeing a temporary output parser was labeled removed error handling. Manual inspection confirmed the token is only a worker lookup key (not auth/permissions) and no output parser is allocated anymore; the outer error boundary and guarded PDF cleanup remain. Its potential nested-loop signal for the core comparator describes group-local linear sweeps, not an O(n²) scan.
- Final code review reported no Critical findings and no other concrete defect after these three fixes. Broader formats/OCR and existing file traversal behavior remain outside this bounded JPEG verification milestone.
