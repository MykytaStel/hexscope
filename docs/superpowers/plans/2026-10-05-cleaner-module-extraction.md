# Clean-copy panel extraction Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move the clean-copy panel and its interaction flow out of `Drawer` into a focused module without changing the experience or output.

**Architecture:** `cleaner.ts` will export `createCleaner(m, cleaning, revealGroup)` and own the complete clean-copy panel. `Drawer.reveals()` will call it in both existing branches and keep responsibility for the surrounding facts and layout.

**Tech Stack:** TypeScript, Vite, Vitest, Playwright, Rust/WASM clean-copy action.

**Spec:** `docs/superpowers/specs/2026-10-05-cleaner-module-extraction-design.md`.

## Global Constraints

- Preserve current clean-copy text, layout, localization, keyboard behavior, copy actions, verification semantics, and output bytes.
- Keep `CleanActions` and the cleaning API unchanged.
- Add no network calls, persistence, telemetry, dependencies, or new handling of file bytes.
- Keep save, share, open, compare, and copy-picture actions available under their current conditions, including when verification is unavailable.

## Review Focus

1. **Two insertion paths:** files with removable image data but no listed facts, and files with a reveal list, both receive the same clean-copy panel. Existing E2E coverage includes `wifi.png` and `photo.jpg`.
2. **Optional presentation notes:** notes stay unless the user selects the checkbox; the opted-in output reports the emptied notes. Add a focused E2E characterization using `crates/hexscope-core/tests/fixtures/keynote.pptx`.
3. **Incomplete or unavailable verification:** the summary never implies all clear, facts are not marked removed without confirmation, and copy actions stay usable. Existing clean-copy E2E cases cover partial reports and worker/module failures.
4. **Cleaner refusal:** errors remain in the panel without success-only actions. Existing accessibility E2E covers the broken image sample's result state.
5. **Module boundary:** `Drawer` retains only the two calls to `createCleaner`; no unrelated panel or parser behavior moves.

## File Map

- Create `apps/web/src/cleaner.ts`: the factory, optional notes selection, result view, action wiring, and verification-to-facts callback.
- Modify `apps/web/src/drawer.ts`: call the factory from both branches in `reveals()`, remove `private cleaner()`, and prune imports used only by that method.
- Modify `apps/web/e2e/site.e2e.ts`: characterize the existing opt-in flow for presentation notes before moving the code.

## Task 1: Extract the clean-copy panel

**Files:**
- Create: `apps/web/src/cleaner.ts`
- Modify: `apps/web/src/drawer.ts`
- Test: `apps/web/e2e/site.e2e.ts`

**Interfaces:**
- Consumes: `FileModel`, existing `CleanActions`, existing `cleancard.ts` helpers, and the reveal group element.
- Produces: `createCleaner(m: FileModel, cleaning: CleanActions, revealGroup: HTMLElement): HTMLElement`.

- [x] **Step 1: Add an E2E characterization for presentation notes.** Load the existing `keynote.pptx` fixture using `readFileSync` and `setInputFiles`; confirm the checkbox starts unchecked. On the default cleaning attempt, accept either a visible `.cleaner .problem` refusal for an unchanged file or a result whose expanded `.clean-removed` list does not include speaker notes. Reload the page, upload the same fixture, select the checkbox, make a copy, expand `.clean-removed`, and confirm it reports `the speaker's notes, emptied` while retaining the package path.
- [x] **Step 2: Run the characterization against the current implementation.** Run `pnpm --filter web exec playwright test e2e/site.e2e.ts --grep "presentation notes are removed only when asked"`. It should pass before the extraction; this is a behavior-preserving refactor, so the characterization establishes the baseline rather than introducing new product behavior.
- [x] **Step 3: Add `createCleaner` in `apps/web/src/cleaner.ts`.** Move the existing `Drawer.cleaner()` implementation into the factory. Pass the `FileModel`, `CleanActions`, and reveal group explicitly; keep its strings, DOM structure, event order, error branch, and verified fact updates unchanged.
- [x] **Step 4: Wire both reveal branches.** Replace both `this.cleaner(m)` calls in `Drawer.reveals()` with `createCleaner(m, this.cleaning, group)`. Remove the old method and imports no longer used by `drawer.ts`; retain helpers still used by redaction, repair, and the verdict CTA.
- [x] **Step 5: Run the web unit suite and production build.** Run `pnpm --filter web test` and `pnpm --filter web build`. Both must complete successfully.
- [x] **Step 6: Run focused browser regression coverage.** Run `pnpm --filter web exec playwright test e2e/site.e2e.ts --grep "evidence-based clean-copy result|clean copy verification reparses a generated photo|presentation notes are removed only when asked|spreadsheet's clean copy keeps what is part of it"` and `pnpm --filter web exec playwright test e2e/a11y.e2e.ts --grep "clean-copy report is announced"`. Both desktop and phone projects must pass.
- [x] **Step 7: Review and commit the isolated extraction.** Inspect `git diff --check` and the full diff; commit only the factory, drawer wiring, and characterization test as `refactor: extract clean-copy panel`.

## Follow-on direction

This implementation covers the agreed UI module boundary only. Continue the wider product work in separate reviewable slices: first assess visual hierarchy and task flow in the web UI, then review Rust core and CLI capability parity against the existing photo-platform contracts. Do not fold those independent changes into this extraction.
