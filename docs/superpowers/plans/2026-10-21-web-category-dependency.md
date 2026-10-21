# Web category dependency boundary implementation plan

> For agentic workers: use the `superpowers:executing-plans` skill to implement this plan task by task. Keep the approved design spec open while working.

**Goal:** Remove the `share.ts` ↔ `verdict.ts` import cycle without changing any category, share sentence, verdict, or locale output.

**Architecture:** Move the fact-category table and pure category selection/counting helpers into `fact-categories.ts`, which depends only on the file model. Move English list joining into `list-format.ts`. Both `share.ts` and `verdict.ts` import the helpers directly; helper modules import neither consumer.

**Tech Stack:** TypeScript, Vitest, existing Web application module graph.

**Spec:** `docs/superpowers/specs/2026-10-21-web-category-dependency-design.md`

## Global constraints

- Preserve category ordering and all existing category/share/verdict strings.
- Add no runtime dependencies and change no user-facing behavior.
- Keep the intentionally lazy localization registration boundary intact.
- Do not split `drawer.ts` or `analyzer.ts` just to reduce line count.

## Review focus

- Are the moved helpers pure and dependent only on the file model / primitive values?
- Are there any remaining paths from `fact-categories.ts` or `list-format.ts` back into `share.ts` or `verdict.ts`?
- Do the existing category ordering, sentence, verdict, and localization expectations remain exact?

---

### Task 1: Pin category and copy behavior before moving code

**Files:** `apps/web/src/logic.test.ts`, `apps/web/src/i18n.test.ts`, any focused share test file.

**Interfaces:** Produces baseline assertions for category order/copy and verdict behavior; Task 2 must preserve those outputs.

- [x] Add/confirm assertions for category order, category count/selection, generated share sentence, and a representative verdict.
- [x] Run `cd apps/web && npm test -- src/logic.test.ts src/i18n.test.ts`; **Expected:** the baseline expectations pass against the current behavior.

### Task 2: Extract dependency-free helpers

**Files:** new `apps/web/src/fact-categories.ts`, new `apps/web/src/list-format.ts`, `apps/web/src/share.ts`, `apps/web/src/verdict.ts`.

**Interfaces:** Consumes Task 1's pinned outputs; produces dependency-free helpers imported directly by both consumer modules and verified by Task 3.

- [x] Move the category table and pure category-selection/counting helpers to `fact-categories.ts`, importing only `FileModel` as a type.
- [x] Move the generic list-joining helper to `list-format.ts`.
- [x] Update both consumers to import these helpers directly; preserve any existing internal exports needed by tests or other modules with narrow re-exports.
- [x] Run `cd apps/web && npm test -- src/logic.test.ts src/i18n.test.ts`. **Expected:** category order, generated sentences, verdicts, and locale tests remain unchanged.

### Task 3: Verify the dependency boundary and product output

**Files:** relevant Web source/tests, no broad module decomposition.

**Interfaces:** Consumes Task 2's new helper modules; produces the checked import boundary and an unchanged Web build.

- [x] Inspect imports with `rg -n 'from "\./(share|verdict)"' apps/web/src` and confirm the helper modules point only to the file model/primitives.
- [x] Add a focused assertion or dependency check if the existing suite does not detect a reintroduced cycle. **Expected:** a regression check fails if either helper imports a consumer or `share.ts` and `verdict.ts` import one another.
- [x] Run `cd apps/web && npm test -- src/logic.test.ts src/i18n.test.ts` and `cd apps/web && npm run build`. **Expected:** focused behavior checks and type-check/build pass.
- [x] Review snapshots/text assertions for unchanged category order, share copy, verdict, and translations.

**Acceptance:** `share.ts` and `verdict.ts` share pure helpers without importing each other; the Web build succeeds and all covered outputs stay unchanged.

## Final review follow-up — 2026-10-21

- The final independent static review reported no remaining actionable findings in the dependency extraction or the wider change set. The Web unit suite passed 354 tests and the production Web build passed. The full browser run passed 338 cases and skipped 32 platform-gated cases; two guide-classifier cases initially failed because the technical-token allowlist was incomplete, then both passed in a focused rerun on computer and phone profiles.
