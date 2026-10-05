# Hexscope Product-wide UX Refresh Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Hexscope's web experience feel like one calm, readable technical workbench across file entry, inspection, findings, clean copies, batch tools, documents, and Film Lab.

**Architecture:** Extend the existing CSS token and stylesheet system, then improve the home page, file-result surfaces, and tool dialogs in separate reviewable slices. Reuse existing TypeScript interactions and local processing; preserve core, CLI, and machine-readable contracts.

**Tech Stack:** HTML, TypeScript, CSS, Vite, Vitest, Playwright, axe-core.

**Spec:** `docs/superpowers/specs/2026-10-05-hexscope-product-wide-ux-design.md`.

## Global Constraints

- Preserve the scope across photo and metadata checks, PDF/Word and other supported documents, email, file structure and bytes, verified clean copies, PDF redaction, batch results and privacy mosaic, and Film Lab/Roll.
- Use the existing spacing scale: 4, 8, 12, 16, 24, 32, and 48 px.
- Keep the existing light and dark themes; validate the same hierarchy and control states in both.
- Keep the source file untouched; show the produced copy's changes and readback-verification limits.
- Do not describe a file as “safe” or “fully cleaned” without the corresponding evidence.
- Do not change Rust core behavior, algorithms, CLI/API/JSON/SARIF contracts, or exit codes; preserve the shared meanings of removed, present, unchecked, and error.
- Do not add dependencies, uploads, persistence, or telemetry.
- Keep film-specific visual motifs inside Film Lab/Roll.

## Review Focus

1. **Viewport and browser zoom:** home cards must remain inside their grid even when the document itself has no horizontal overflow; pin child-card bounds in `site.e2e.ts` at tablet, desktop, and wide sizes.
2. **Long Ukrainian and English labels:** file pickers, selects, actions, and helper copy must not overlap or clip in the Film Roll dialog; cover both locales and narrow widths in the dialog E2E.
3. **Incomplete verification and processing errors:** present, unchecked, or unavailable results must not take the success style, and existing copy actions must remain available; preserve the targeted clean-copy E2E cases.
4. **Keyboard, focus, and reduced motion:** dialogs, disclosures, and native controls remain operable and named with keyboard focus visible in both themes; extend the existing axe and keyboard checks.
5. **Specialist-tool regressions:** layout work must not change Film Lab pixels, recipe behavior, batch receipts, mosaic privacy, or roll output; run the existing `film-lab.e2e.ts` and `photo-platform.e2e.ts` cases after shared styles land.

## File Map

- Modify `apps/web/src/styles/foundations.css`: add shared control and surface tokens while retaining the existing theme variables and spacing scale.
- Modify `apps/web/src/styles/shell.css`, `media-views.css`, and `input-and-accessibility.css`: align buttons, native form controls, focus states, and shared disclosure markers.
- Modify `apps/web/index.html`, `apps/web/src/i18n.ts`, `apps/web/src/styles/findings-and-actions.css`, and `apps/web/src/styles/narrow-layout.css`: organize the main entry, everyday doors, specialist-tool links, and supporting details.
- Modify `apps/web/src/styles/wide-layout.css` only if the measured home or result layout needs a wide-screen correction; keep the file inspector's tree/bytes/details workspace intact.
- Modify `apps/web/src/styles/facts-and-redaction.css`, `file-details.css`, `findings-and-actions.css`, and `apps/web/src/drawer.ts` only for the hierarchy and grouping of existing findings, evidence, redaction, and clean-copy results; do not change their decisions or bytes.
- Modify `apps/web/src/photo-tools.css`, `film-roll-ui.ts`, `film-lab.css`, and `film-lab-ui.ts` for consistent controls and the Film Roll/Lab presentation. Reuse the existing native dialog, field labels, and local worker flow.
- Apply shared dialog tokens in `apps/web/src/styles/media-views.css` and `facts-and-redaction.css` to the existing `.report`, `.compare`, `.blackpicture`, `.shortcuts`, `.photo-tool`, and `.photo-mosaic` surfaces; do not alter their behavior or exported data.
- Add or extend browser coverage in `apps/web/e2e/site.e2e.ts`, `a11y.e2e.ts`, `film-lab.e2e.ts`, and `photo-platform.e2e.ts`. Use `apps/web/src/i18n.test.ts` only if a changed translation mapping needs unit coverage.

## Task 1: Establish shared controls and visual tokens

**Files:**
- Modify: `apps/web/src/styles/foundations.css`
- Modify: `apps/web/src/styles/shell.css`
- Modify: `apps/web/src/styles/media-views.css`
- Modify: `apps/web/src/styles/input-and-accessibility.css`
- Test: `apps/web/e2e/a11y.e2e.ts`

**Interfaces:**
- Consumes: existing CSS variables (`--text`, `--muted`, `--faint`, `--accent`, `--on-accent`, `--panel`, `--panel-2`, `--border`, `--space-1` through `--space-7`), `.btn`, `.btn-primary`, native `button`, `input`, `select`, `details`, and `summary`.
- Produces: shared control tokens and consistent visual states for primary, secondary, disabled, hover, and keyboard focus without changing control semantics.

- [x] **Step 1: Add an accessibility regression for shared controls.** Inside the existing light/dark theme loop in `a11y.e2e.ts`, add `shared controls keep clear focus and disabled states`. Verify the main file action has at least a 44px hit height, the disabled Film Roll process action is distinguishable from an enabled secondary button, and keyboard focus is visible on a native disclosure and select. Open Film Roll from the landing page to reach the select controls.
- [x] **Step 2: Run the new check against the current styles.** Run `pnpm --filter web build`, then `pnpm --filter web e2e -- e2e/a11y.e2e.ts --grep "shared controls keep clear focus and disabled states"`. Record which assertions fail before changing styles.
- [x] **Step 3: Define shared control tokens and apply them.** Add named control-height/radius tokens in `foundations.css`; keep primary and form controls at a 44px minimum while preserving the compact density of file-view toolbars. Make the disabled primary visibly disabled without relying on a washed-out accent alone.
- [x] **Step 4: Unify select and disclosure affordances.** Keep native `select` and `details/summary` semantics. Reserve space for the select indicator, make its direction visible, and use one summary chevron that rotates when open; ensure the full summary row is a target.
- [x] **Step 5: Re-run the focused accessibility check and build.** Repeat the Step 2 commands; both theme cases must pass, and the build must succeed.
- [x] **Step 6: Review and commit the control foundation.** Run `git diff --check`, inspect the focused stylesheet/test diff, and commit as `style: establish shared control system`.

## Task 2: Fix the home-page hierarchy and responsive card grid

**Files:**
- Modify: `apps/web/index.html`
- Modify: `apps/web/src/i18n.ts`
- Modify: `apps/web/src/styles/findings-and-actions.css`
- Modify: `apps/web/src/styles/narrow-layout.css`
- Modify if required by measured layout: `apps/web/src/styles/wide-layout.css`
- Test: `apps/web/e2e/site.e2e.ts`

**Interfaces:**
- Consumes: Task 1 control styles; existing `#picker-empty`, `#film-roll-open`, `.doors-main`, the photo/document/email sample actions, the synthetic-film sample, and the existing batch/photo-mosaic entry points.
- Produces: the same working entry points with clear spacing and hierarchy; three everyday doors remain aligned where there is room, and specialist tools are grouped apart from guides and technical samples.

- [x] **Step 1: Add a card-boundary regression.** In `site.e2e.ts`, add `landing cards stay inside the content grid across widths and locales`. Check each visible `.doors-main > .door` bounding box against both `.doors-main` and the viewport; verify `documentElement.scrollWidth <= innerWidth` at widths `[768, 901, 1024, 1034, 1180, 1280, 1440, 1920]` in English and Ukrainian. This catches a clipped child even when the document itself does not overflow.
- [x] **Step 2: Run the focused home checks.** Ran the production build and focused Playwright sweeps. At 1034 CSS px the supplied crop did not reproduce (visual viewport scale 1, desktop DPR 1); recorded this limit in the ledger. The measured old custom tablet card pattern was removed.
- [x] **Step 3: Recompose the home page around the existing primary action.** Preserve “Choose a file” and “Try a sample”; retain the photo, document, and email doors; give the content a shared aligned frame and more separation between hero, everyday doors, privacy promise, and supporting sections.
- [x] **Step 4: Group specialist and explanatory paths.** Move the existing Film Roll and synthetic-film entry points into a clearly named specialist-tools area; keep batch/photo-mosaic discovery available; keep supported formats, “look inside a file,” guides, and extra samples visually secondary. Do not add new processing behavior.
- [x] **Step 5: Reflow the doors without an orphan row.** Use a grid that preserves readable card widths: three columns only when they fit, then two, then one. Remove the special tablet row treatment that can make the third door appear detached; make card children shrink or wrap instead of clip.
- [x] **Step 6: Verify both locales and viewport sweeps.** Repeat the Step 2 commands and the existing `landing page has two clear actions, three everyday doors, and a remembered Ukrainian choice` test. All cards must remain within the content frame at the measured widths.
- [x] **Step 7: Review and commit the home slice.** Run `git diff --check`, inspect the HTML/i18n/CSS/E2E diff, and commit as `style: clarify home page entry points`.

## Task 3: Make file findings, evidence, and copy actions read as one result

**Files:**
- Modify: `apps/web/src/drawer.ts` only if semantic grouping is needed
- Modify: `apps/web/src/styles/findings-and-actions.css`
- Modify: `apps/web/src/styles/facts-and-redaction.css`
- Modify: `apps/web/src/styles/file-details.css`
- Modify: `apps/web/src/styles/input-and-accessibility.css` for the narrow clean-copy comparison layout
- Modify: `apps/web/src/i18n.ts` only for changed user-facing wording
- Test: `apps/web/e2e/site.e2e.ts`
- Test: `apps/web/e2e/a11y.e2e.ts`

**Interfaces:**
- Consumes: existing `.verdict`, `.reveals`, `.more-details`, `.redactor`, `.cleaner`, `.before-after`, and `.copy-verification` markup; `clean-summary.ts` and `verification.ts` states remain authoritative.
- Produces: consistent summary → evidence/coverage → next-action hierarchy for photos, documents, and email, while keeping advanced bytes and structure reachable in one clear step.

- [x] **Step 1: Add a result-order regression.** Add `file results show the summary before evidence and available actions` to `site.e2e.ts`. Exercise `photo.jpg`, `report.pdf`, and `phishing.eml`; assert the verdict is first, evidence follows it, relevant clean/redaction actions remain available, and optional technical details stay in their existing disclosure.
- [x] **Step 2: Pin incomplete-result semantics.** Keep the existing tests `clean copy verification announces a controlled result while copy actions stay available`, `clean copy verification keeps copy actions when the worker returns an error`, `clean-copy coverage explanations are translated in Ukrainian`, and `incomplete PDF content warns in Ukrainian and produces no redacted copy`. Run those focused cases before style edits; present/unchecked/error states must not be styled as confirmed success.
- [x] **Step 3: Apply one finding and evidence hierarchy.** Align summary, finding rows, explanation, evidence links, coverage notes, redaction controls, and clean-copy comparison using existing tokens. Use text and labels along with color; preserve the current meaning and order of all verification states.
- [x] **Step 4: Keep technical depth progressive.** Make `.more-details` and nested byte/structure views easy to discover and open, while leaving the main result readable without opening every detail. Keep the existing `details/summary` keyboard behavior.
- [x] **Step 5: Verify representative formats and both themes.** Run `pnpm --filter web build`, then `pnpm --filter web e2e -- e2e/site.e2e.ts --grep "file results show the summary|evidence-based clean-copy result|clean copy verification keeps copy actions|clean-copy coverage explanations|incomplete PDF content"` and `pnpm --filter web e2e -- e2e/a11y.e2e.ts --grep "a file:"`. The photo, PDF, email, and broken-image states must remain accessible and semantically unchanged.
- [x] **Step 6: Review and commit the result slice.** Run `git diff --check`, inspect the focused markup/CSS/test changes, and commit as `style: clarify file findings and actions`.

## Task 4: Bring Film Lab, batch tools, and dialogs into the shared system

**Files:**
- Modify: `apps/web/src/photo-tools.css`
- Modify: `apps/web/src/film-roll-ui.ts` only for semantic grouping or clearer selected-file state
- Modify: `apps/web/src/film-lab.css`
- Modify: `apps/web/src/film-lab-ui.ts` only for presentation semantics, not rendering logic
- Modify: `apps/web/src/styles/findings-and-actions.css` for existing batch-result and photo-mosaic entry styles
- Modify: `apps/web/src/styles/media-views.css`
- Modify: `apps/web/src/styles/facts-and-redaction.css`
- Modify: `apps/web/src/styles/file-details.css`
- Test: `apps/web/e2e/a11y.e2e.ts`
- Test: `apps/web/e2e/film-lab.e2e.ts`
- Test: `apps/web/e2e/photo-platform.e2e.ts`

**Interfaces:**
- Consumes: Task 1 controls; existing `.photo-tool`, `.film-roll`, `.film-lab`, `.photo-mosaic`, `.report`, `.compare`, `.blackpicture`, and `.shortcuts` roots; existing worker and result contracts.
- Produces: readable forms and dialog shells with grouped inputs, visible select indicators, predictable scrolling, clear disabled/processing/result states, and unchanged tool behavior.

- [x] **Step 1: Add a Film Roll geometry regression.** In `a11y.e2e.ts`, add `film roll keeps controls and actions inside the dialog at narrow widths`. Open Film Roll and sweep `[320, 360, 600, 768, 1024, 1440]`; verify the dialog stays within the viewport, the body has no horizontal overflow, both selects fit their labels and indicators, and the disabled process action remains understandable. Run it in the existing light/dark theme loop.
- [x] **Step 2: Add selected-file and keyboard checks.** Verify picker labels report the chosen-file count, advanced settings open from the full summary hit area, and Escape/Close leave the page usable. Keep the existing `photo tools follow the theme and expose accessible controls` coverage.
- [x] **Step 3: Rebuild the Film Roll hierarchy with its current workflow.** Keep scan selection first; group scan type and output format; make calibrated-recipe loading a clear optional section; keep advanced processing notes expandable; keep status and Process/Download actions anchored predictably. Limit the desktop dialog to a readable width (target max 680px) and let only its body scroll; at narrow widths, stack fields without clipping values.
- [x] **Step 4: Apply the dialog/control tokens to other tools.** Use the same surface, border, close, focus, and action hierarchy for the photo mosaic, structure report, compare, black-picture editor, keyboard shortcuts, and Film Lab controls. Keep wide comparison content wide only where its byte view needs it; do not change crop, raster, batch, mosaic, or receipt algorithms.
- [x] **Step 5: Verify unchanged specialist behavior.** Run `pnpm --filter web build`, then `pnpm --filter web e2e -- e2e/a11y.e2e.ts --grep "film roll keeps controls|photo tools follow the theme"`, `pnpm --filter web e2e -- e2e/film-lab.e2e.ts`, and `pnpm --filter web e2e -- e2e/photo-platform.e2e.ts`. All existing output dimensions, recipes, readback checks, receipts, and offline behavior must remain unchanged.
- [x] **Step 6: Review and commit the specialist-tool slice.** Run `git diff --check`, inspect that no worker/algorithm/output code changed, and commit as `style: unify tool dialogs and controls`.

## Task 5: Run the whole-product visual and accessibility acceptance pass

**Files:**
- Modify only files listed in Tasks 1–4 to resolve integration failures
- Extend if a missing regression is found: `apps/web/e2e/site.e2e.ts`, `a11y.e2e.ts`, `film-lab.e2e.ts`, `photo-platform.e2e.ts`

**Interfaces:**
- Consumes: completed Tasks 1–4 and existing page routes and interactions.
- Produces: a checked web experience across the approved product scope, with no horizontal clipping and with Web/CLI/CI result-state meanings preserved.

- [x] **Step 1: Run the full web unit suite and production build.** Run `pnpm --filter web test` and `pnpm --filter web build`; both must pass.
- [x] **Step 2: Run the complete browser and axe coverage.** Run `pnpm --filter web e2e -- e2e/site.e2e.ts e2e/a11y.e2e.ts e2e/film-lab.e2e.ts e2e/photo-platform.e2e.ts`; both `computer` and `phone` projects must pass.
- [x] **Step 3: Inspect the actual visual matrix.** Capture and review the home page, a photo result, a PDF redaction result, an email result, a clean-copy result, Film Roll, Film Lab, and photo mosaic at 320, 768, 1024, and 1440 CSS px in light/dark and English/Ukrainian. Record viewport and browser zoom for every capture; correct clipped children even if document-level overflow is zero.
- [x] **Step 4: Recheck human-readable state vocabulary.** Compare Web clean-copy labels to the existing CLI `Removed`, `Still present`, and `Not checked` output in `crates/hexscope-cli/src/main.rs`. Change only presentation wording if a real mismatch exists; do not change JSON/SARIF fields, exit codes, or verification semantics.
- [x] **Step 5: Review the complete branch and commit only final polish.** Run `git diff --check`, inspect all changes for accidental behavior/core/CLI changes, and commit only any final UI corrections as `style: finish responsive UX review`.

## Execution Order

Run Tasks 1 through 5 in order. Tasks 2–4 depend on the shared primitives from Task 1; Task 5 is the cross-product release check. Each task is independently reviewable and must keep the existing file-processing behavior intact.
