# Clean Copy Verification Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** After Hexscope creates a clean or redacted copy, show which parser findings were removed, remain, or could not be checked by parsing the actual output locally.

**Architecture:** A pure verifier classifies file-level finding occurrences and selected PDF text. The existing worker reparses eligible output and returns only a compact report. Copy delivery happens before the asynchronous check finishes, so a slow or failed check cannot delay save/share/open actions.

**Tech Stack:** TypeScript 7, Vite 8, Vitest 5, Playwright, the existing Rust-to-WebAssembly parser.

**Spec:** `docs/superpowers/specs/2026-10-02-clean-copy-verification-design.md`

## Global Constraints

- Keep file bytes and finding values in the tab and worker only; do not upload them, persist them, or add analytics.
- Do not add a score, OCR, password collection, server processing, or new format parsers.
- Never claim that a file is universally safe or free of every possible disclosure.
- Report removal only for explicit format/kind/scope capabilities with complete parser coverage. Everything else is **Not checked**.
- Select and record the maximum verification input before production-code changes. Use the repository's 10 MB browser parse target and a resource profile in the existing Playwright phone project as evidence; treat Pixel 7 emulation as browser-profile evidence, not a physical-device claim. Start with a 10 MiB ceiling and lower it if the measured run needs more headroom.
- Start the verification request only after the clean copy exists; return the copy and make its existing save/share/open actions available immediately. Bound the check with a timeout and map every failure to **Not checked**.
- Keep existing clean-copy behavior, PDF fail-closed refusal, compare flow, English/Ukrainian UI, keyboard behavior, and assistive-technology access.

## File Map

- Create `apps/web/src/verification.ts` and `apps/web/src/verification.test.ts` for matching and report classification.
- Modify `apps/web/src/worker.ts`, `apps/web/src/copies.ts`, and `apps/web/src/rpc.ts` to inspect the output asynchronously, enforce the size/timeout bounds, and preserve copy delivery.
- Modify `apps/web/src/cleancard.ts`, `apps/web/src/drawer.ts`, and `apps/web/src/i18n.ts` for the report UI and PDF selection handoff.
- Modify `apps/web/src/i18n.test.ts`, create `apps/web/src/rpc.test.ts` for timeout behavior, and extend `apps/web/e2e/site.e2e.ts` and `apps/web/e2e/a11y.e2e.ts` for real flows and accessible status.

## Review Focus

1. **Duplicate metadata values:** two equal source findings and one output match yield one present and one removed. Cover this in `verification.test.ts`.
2. **Unsupported or partial metadata coverage:** missing output facts are unchecked without a matching complete capability. Cover supported, unsupported, and incomplete cases in `verification.test.ts` and verify the worker passes only whitelisted capabilities.
3. **Repeated PDF text:** a match on another page cannot keep a selection present; a same-page match is conservatively present. Cover both in `verification.test.ts` and the PDF end-to-end case.
4. **Picture-area and incomplete-page checks:** neither can be called removed. Cover area-only, source-incomplete, output-incomplete, and missing-page cases in `verification.test.ts`; retain the existing incomplete-PDF refusal test.
5. **Resource or worker failure:** the 10 MiB boundary, timeout, parser error, and worker death produce an unchecked report while the already-created copy remains actionable. Cover the boundary and error mapping in unit tests, the timeout cleanup in RPC tests, and action availability in the end-to-end flow.

---

### Task 0: Confirm the resource ceiling before production implementation

**Files:** none; use a temporary diagnostic script and remove it after the measurement.

- [x] Read the existing 10 MB browser parse target in `CONTRIBUTING.md` and the large-input workloads in `crates/hexscope-core/tests/big.rs`.
- [x] Build and serve the current app. In the Playwright `phone` project, exercise the current parse-clean-reparse path with a deterministic PNG close to 10 MiB. Record elapsed time and the Chromium renderer memory metric if available; otherwise record that the emulated profile could not provide a reliable memory measurement.
- [x] Set the implementation ceiling at or below 10 MiB, lowering it if the phone profile needs headroom. Record the actual measurement method and chosen bound in a code comment during Task 2. Treat Playwright's Pixel 7 profile as emulation evidence, never as physical-device proof.

### Task 1: Pure report algorithm (TDD)

**Files:**
- Create: `apps/web/src/verification.ts`
- Create: `apps/web/src/verification.test.ts`

**Interfaces:**
- `VerificationFinding`: `{ format: ParsedFile["format"]; kind: string; label: string; value: string | null; scope: string | null }`.
- `VerificationCoverage`: `{ format: ParsedFile["format"]; kind: string; scope: string; complete: boolean; reason?: string }`.
- `VerificationItem`: `{ kind: string; label: string; reason?: string; unexpected?: boolean }`.
- `VerificationReport`: `{ removed: VerificationItem[]; present: VerificationItem[]; unchecked: VerificationItem[] }`. Never include finding `value` or `scope` in a report.
- Export `fileFindings(format, facts, location, labels)` to collect comparable file-level facts and a stable location identity; export `capabilitiesFor(format, kinds)` to return complete coverage only for the explicit tested whitelist. Also export `verifyFindings(source, output, coverage, retainedReasons: Readonly<Record<string, string>>)`, `verifyPdfSelections(selections, pages)`, `uncheckedReport(source, selections, reason)`, and `verificationEligible(sizeBytes, maxBytes = MAX_VERIFY_BYTES)`.
- PDF selection input: `{ page: number; text: string; sourceComplete: boolean; areaOnly: boolean }`; output-page input: `{ page: number; text: string; complete: boolean }`.
- File-finding identity is `(kind, scope, value.normalize("NFC").trim())`; preserve case and multiplicity. PDF matches are page-scoped and use the redactor's case/whitespace-tolerant search semantics. If a matching string remains on the selected page, classify conservatively as present.
- Capability lookup is exact on format, kind, and scope. Start with JPEG file facts and location, adding another pair only when a fixture proves complete output coverage. A finding with no stable value/scope, unsupported capability, incomplete source/output page, or no corresponding page is unchecked.

- [x] **Step 1: Write failing tests** for `fileFindings` (facts, location, duplicate facts, and no stable value), `capabilitiesFor` (only explicit JPEG fact kinds represented by complete parser fixtures and location are complete initially), equal duplicate output values, intentional-retention reasons, output-only residual facts, unsupported and incomplete coverage, and absence of private values/scopes from serialized reports. Add PDF cases for a match on another page, a same-page repeated match, incomplete source/output pages, a missing page, and a picture-only area. Cover `MAX_VERIFY_BYTES - 1`, the exact limit, and the limit plus one.
- [x] **Step 2: Run the focused tests and confirm the expected failures.**

  Run: `pnpm --filter web exec vitest run src/verification.test.ts`

  Expected: FAIL because the verifier module and exports do not exist.
- [x] **Step 3: Implement the pure classifier.** Use an occurrence multiset; report unmatched output occurrences as unexpected residuals; attach an intentional-retention explanation only to matching present findings. Keep the initial capability whitelist limited to JPEG fact kinds present in regression fixtures and location; leave every unlisted format/kind unchecked. For PDF, do not search another page and never infer a removal for picture-only text or incomplete coverage. Keep raw values inside the function inputs only.
- [x] **Step 4: Run the focused tests and confirm they pass.**

  Run: `pnpm --filter web exec vitest run src/verification.test.ts`

  Expected: PASS for matching, incomplete-coverage, privacy, and boundary cases.
- [x] **Step 5: Commit the algorithm.**

  ```bash
  git add apps/web/src/verification.ts apps/web/src/verification.test.ts
  git commit -m "feat: add clean copy verification model"
  ```

### Task 2: Bounded worker verification and immediate copy delivery (TDD)

**Files:**
- Modify: `apps/web/src/worker.ts`
- Modify: `apps/web/src/copies.ts`
- Modify: `apps/web/src/rpc.ts`
- Create or modify: `apps/web/src/rpc.test.ts`
- Modify: `apps/web/src/verification.ts` and `apps/web/src/verification.test.ts`

**Interfaces:**
- Add a `verifyCopy` worker request with the generated `Blob`, source file-level findings, source format, retained-reason mapping, and optional PDF selections.
- Add a `verification` response with only `VerificationReport`; no parsed values, source bytes, output bytes, or parser tree cross back to the page.
- On a successfully created copy, `CleanResult.verification` is a promise that always resolves to a report. It is absent when copy creation itself failed. `cleanCopy`/`redactCopy` save or return the copy before awaiting that promise; the report must never reject into the copy flow.
- Add an optional timeout to `rpc.call` for this request only. Expiry removes the pending ID and resolves it with an error response so a stopped worker cannot leave a pending promise or a hanging UI.
- Set `MAX_VERIFY_BYTES` to the bound selected in Task 0 (no more than `10 * 1024 * 1024`); check `Blob.size` before calling `arrayBuffer()` or the parser.

- [ ] **Step 1: Add failing tests** for source-finding extraction (including a location and duplicate facts), format/kind capability lookup, output observation mapping, over-limit skip, and conversion of an error/timeout response into an unchecked report. Assert report serialization contains no source values.
- [ ] **Step 2: Run the focused tests and confirm they fail.**

  Run: `pnpm --filter web exec vitest run src/verification.test.ts src/rpc.test.ts`

  Expected: FAIL on the missing worker mapping and timeout behavior.
- [ ] **Step 3: Add the worker branch.** Below the byte limit, parse the copy with the existing WASM module without replacing or freeing the user's open-file stack. For file findings, use `describe(parsed).facts` and `.location`; use `fileFindings` and pass `capabilitiesFor` output to `verifyFindings`. For PDF redactions, use the existing `pageTexts` parser result and its `complete` flags. Free every temporary WASM value in `finally`. Post only the compact report. Catch parser/verification errors inside this branch and post an unchecked report.
- [ ] **Step 4: Connect the source findings and PDF selections.** Build file findings from `FileModel.file.facts` and `.location`, label facts with existing `FACT_LABELS`, label location explicitly, and use the existing `KEPT`/`KEPT_NOTE` explanation for intentionally retained kinds. Extend `CleanActions.redact` so the drawer passes each selected `{page, text, sourceComplete, areaOnly}` alongside the existing areas. An empty-text box is unchecked; do not infer anything about picture pixels.
- [ ] **Step 5: Deliver the copy before verification finishes.** In both `cleanCopy` and `redactCopy`, call the existing save/share handoff as soon as the cleaner returns a valid copy, then start `verifyCopy` and return its non-rejecting promise. Map an over-limit, worker error, unexpected response, timeout, or parser error to `uncheckedReport`; preserve the same copy Blob and filename.
- [ ] **Step 6: Add and test the RPC timeout cleanup.** Use a focused fake-worker test to prove a timed-out request resolves once, leaves no pending ID, and does not affect later requests. Do not add a timeout to unrelated parse/player calls.
- [ ] **Step 7: Run focused tests and TypeScript checks.**

  Run: `pnpm --filter web exec vitest run src/verification.test.ts src/rpc.test.ts`

  Run: `pnpm --filter web exec tsc --noEmit`

  Expected: PASS; the worker response is compact, size is checked before reading bytes, and the RPC timeout resolves without a pending request.
- [ ] **Step 8: Commit the worker path.**

  ```bash
  git add apps/web/src/worker.ts apps/web/src/copies.ts apps/web/src/rpc.ts apps/web/src/rpc.test.ts apps/web/src/verification.ts apps/web/src/verification.test.ts
  git commit -m "feat: verify generated copies in worker"
  ```

### Task 3: Show the three results in the existing copy flow (TDD)

**Files:**
- Modify: `apps/web/src/cleancard.ts`
- Modify: `apps/web/src/drawer.ts`
- Modify: `apps/web/src/i18n.ts`
- Modify: `apps/web/src/i18n.test.ts`
- Modify: `apps/web/e2e/site.e2e.ts`
- Modify: `apps/web/e2e/a11y.e2e.ts`
- Modify: the existing report styles in `apps/web/src/styles/` only if needed for readable narrow-screen layout.

**Interfaces:**
- Render a pending status immediately, then update the same report region when `CleanResult.verification` resolves.
- Show **Removed**, **Still present**, and **Not checked**, with plain reasons for retained, residual, incomplete, picture-only, over-limit, and failed checks. For a picture-area redaction, say the requested area was redacted by the operation while text inside its pixels remains unchecked because Hexscope has no OCR.
- Keep open, compare, save, share, and the existing cleaner summary available while the report is pending. Do not render finding values or create an all-clear state.

- [ ] **Step 1: Write failing localization and browser assertions** for a generated photo copy, one retained explanation, an unchecked report, and a PDF selection whose text is removed from searchable output. Assert no selected text value is copied into the report. Add a controlled verification-worker response containing a retained reason and unchecked item to exercise report rendering, plus a separate worker-failure case that asserts the copy actions remain available. Add an accessibility assertion for the polite status region and keyboard-reachable copy actions. Keep the existing incomplete-PDF refusal scenario unchanged.
- [ ] **Step 2: Run the focused tests and confirm the expected failures.**

  Run: `pnpm --filter web exec vitest run src/i18n.test.ts`

  Run: `pnpm build`

  Run: `pnpm --filter web exec playwright test e2e/site.e2e.ts --grep "clean copy verification|incomplete PDF form"`

  Run: `pnpm --filter web exec playwright test e2e/a11y.e2e.ts`

  Expected: the new translation and browser assertions fail because the report UI is absent; the existing refusal case continues to pass.
- [ ] **Step 3: Render an accessible pending/report region** in the existing clean-copy result for both ordinary cleaning and redaction. Update the region in place with `role="status"`/polite announcement; use lists under the three headings, and show a useful unchecked reason when no supported source finding exists.
- [ ] **Step 4: Add English strings and Ukrainian translations** in `i18n.ts`, with focused exact assertions in `i18n.test.ts`. Keep labels short and reasons specific; never translate the report into a safety verdict.
- [ ] **Step 5: Exercise the real output parser in E2E** for a sample photo and the searchable-text PDF redaction path. Exercise a controlled compact report and the returned error path. Cover area-only and incomplete-page classification in `verification.test.ts`; keep the current real incomplete-form refusal E2E as a regression check.
- [ ] **Step 6: Run focused and complete web checks.**

  Run: `pnpm --filter web exec vitest run src/verification.test.ts src/rpc.test.ts src/i18n.test.ts`

  Run: `pnpm --filter web test`

  Run: `pnpm build`

  Run: `pnpm --filter web exec playwright test e2e/site.e2e.ts --grep "clean copy verification|incomplete PDF form"`

  Run: `pnpm --filter web exec playwright test e2e/a11y.e2e.ts`

  Expected: all selected unit and browser checks pass in English and Ukrainian; on the phone project, copy actions remain usable while verification is pending and once it is unchecked, and assistive technology can read the updated report.
- [ ] **Step 7: Commit the user-facing report.**

  ```bash
  git add apps/web/src/cleancard.ts apps/web/src/drawer.ts apps/web/src/i18n.ts apps/web/src/i18n.test.ts apps/web/src/styles apps/web/e2e/site.e2e.ts apps/web/e2e/a11y.e2e.ts
  git commit -m "feat: show clean copy verification results"
  ```

### Task 4: Stabilization and final evidence

**Files:**
- Modify `docs/known-issues.md` only if the implementation exposes a new user-visible limitation not already described.
- Review `CONTRIBUTING.md` and `.github/workflows/ci.yml` for the canonical checks.

- [ ] **Step 1: Run Rust correctness and large-input gates.**

  Run: `cargo fmt --all --check`

  Run: `cargo clippy --all-targets -- -D warnings`

  Run: `cargo test --all`

  Run: `cargo test --release -p hexscope-core --test big -- --nocapture`

  Expected: all commands exit zero.
- [ ] **Step 2: Run the full web, production-build, and browser gates.**

  Run: `pnpm build`

  Run: `pnpm --filter web test`

  Run: `pnpm --filter web e2e`

  Expected: all commands exit zero; the phone project completes the generated-copy path within the existing browser budget and the configured bundle/WASM budgets remain green.
- [ ] **Step 3: Review the final diff against every acceptance criterion** in the approved spec, inspect the format/kind capability table and report serialization for false removal or PII leakage, run `git diff --check`, and update known-issues only for a real new limitation.
- [ ] **Step 4: Commit any documentation correction separately** and report local unit/build/E2E/Rust results separately; do not describe these as hosted CI, physical-device, or production proof.
