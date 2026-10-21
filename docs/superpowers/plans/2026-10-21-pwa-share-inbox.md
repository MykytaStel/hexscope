# PWA shared-file inbox implementation plan

> For agentic workers: use the `superpowers:executing-plans` skill to implement this plan task by task. Keep the approved design spec open while working.

**Goal:** Receive every file shared to the installed app, keep submissions isolated, and expire unconsumed local files after 24 hours.

**Architecture:** The service worker will validate a submission before caching it, write each file under a random submission id with preserved metadata and a receive timestamp, and roll back partial writes on any failure. The page will consume only the id in `?shared=...` through the existing `openFiles` path. A one-time migration will preserve the legacy single entry for 24 hours.

**Tech Stack:** Classic service worker JavaScript, CacheStorage, TypeScript, Vitest, Playwright, Web Manifest.

**Spec:** `docs/superpowers/specs/2026-10-21-pwa-share-inbox-design.md`

## Global constraints

- Limit each submission to 50 files and 400 MiB total; never analyze a partial submission.
- Keep all file contents local; keep file names and MIME types intact.
- Purge expired inbox entries on worker activation, new receive, and inbox consumption. Do not claim a dormant worker runs a timer.
- Accept legacy `?shared=1` and `shared-file` entries through the one-time 24-hour migration grace.
- Keep the single-file analyzer path and use `openFiles` for multi-file batches.

## Review focus

- Are ids validated before they are used to form cache request URLs?
- Do independent submissions stay isolated, including repeated filenames?
- Are all write failures atomic from the user's point of view, with actionable localized feedback?
- Do worker and page agree on expiry, metadata, and legacy migration semantics?

---

### Task 1: Add deterministic inbox and service-worker regression coverage

**Files:** `apps/web/public/sw.js`, new `apps/web/src/sw-share.test.ts`, `apps/web/e2e/share-target.e2e.ts`, `apps/web/playwright.config.ts` only if needed for service-worker routing.

**Interfaces:** Produces deterministic service-worker fixtures and browser coverage; Task 2 consumes the worker harness and Task 3 consumes the share-target browser path.

- [x] Add a focused worker harness that can dispatch install/activate/fetch events against an in-memory CacheStorage implementation, including a cache that throws a quota error.
- [x] Add failing cases for multiple files, no files, duplicate names, limits, partial-write rollback, separate submissions, expiry, and legacy migration. Invalid-id and page error cases belong to Task 3 because the page consumes those ids.
- [x] Add browser-level cases for a one-file share opening the analyzer and a multi-file share opening the existing batch flow with preserved names/types.
- [x] Run `cd apps/web && npm test -- src/sw-share.test.ts` and `cd apps/web && npm run e2e -- e2e/share-target.e2e.ts`; **Observed RED:** worker tests exposed the old `?shared=1` single-key behavior and missing expiry/migration; browser single-file and legacy behavior worked, while the multi-file case failed because only one item was stored.

### Task 2: Implement bounded, isolated share reception and cleanup

**Files:** `apps/web/public/sw.js`, worker harness tests.

**Interfaces:** Consumes Task 1's service-worker harness; produces the grouped id/metadata/expiry/legacy-cache contract consumed by Task 3.

- [x] Read every `file` value with `form.getAll("file")`; validate the count and aggregate size before storing anything.
- [x] Allocate a random submission id; store each file under a distinct same-origin cache key grouped by that id, preserving name, MIME type, receive time, and ordering.
- [x] Add expiry cleanup and run it during activation, before accepting a new share, and before a page consumes inbox contents.
- [x] If any cache write fails, remove all entries for that submission and redirect with a localized actionable failure state; do not redirect as if a partial share succeeded.
- [x] Redirect successful receives to `?shared=<submission-id>` and strictly validate ids before constructing cache keys.
- [x] Migrate an existing `shared-file` entry once, assign its first migration timestamp, and preserve it for a 24-hour grace period.
- [x] Run `cd apps/web && npm test -- src/sw-share.test.ts`. **Observed:** all 6 worker inbox cases passed after the expected pre-implementation failures.

### Task 3: Consume the inbox through the existing open-files flow

**Files:** `apps/web/src/analyzer.ts`, `apps/web/src/i18n-uk.ts`, English source strings in `analyzer.ts`, new/updated share-target browser tests.

**Interfaces:** Consumes Task 2's id and metadata contract; produces localized analyzer opening/error behavior. Task 4 is independent and changes only shell theme metadata.

- [x] Add failing page-level cases for one file, a multi-file batch, legacy `?shared=1`, missing/expired ids, invalid ids, incomplete submissions, and localized errors.
- [x] Run `cd apps/web && npm run e2e -- e2e/share-target.e2e.ts`; **Observed RED:** multi-file submission opened only its first item; missing/expired IDs and localized failure queries were silent; an invalid ID could consume the legacy key. The single-file and legacy flows already worked.
- [x] Read only the requested submission, restore File metadata, consume it atomically, and pass its files to the existing `openFiles` function.
- [x] Show a localized missing/expired or receive-failure message with a clear retry instruction; remove the query string after handling it.
- [x] Run `cd apps/web && npm test -- src/sw-share.test.ts` and `cd apps/web && npm run e2e -- e2e/share-target.e2e.ts`. **Observed:** 7 inbox tests passed; share-target E2E passed for both computer and phone (then the added incomplete-submission case passed on computer).

### Task 4: Align installed-app shell color and check regressions

**Files:** `apps/web/public/manifest.webmanifest`, `apps/web/index.html`, relevant PWA tests.

**Interfaces:** Independent shell metadata correction; consumes the existing page theme-color as the value to match.

- [x] Add a manifest expectation that its `theme_color` matches the page's existing `theme-color` metadata.
- [x] Set the manifest to the existing graphite value `#090b0d` and keep both sources aligned.
- [x] Run focused PWA checks and the Web build: `cd apps/web && npm test -- src/sw-share.test.ts` and `cd apps/web && npm run build`. **Observed:** PWA and manifest tests passed (96 focused tests), production Web build passed, full Web unit suite passed 354 tests, and Share Target Playwright passed 14 cases across computer and phone. Existing Vite config-loader and inline classic-script warnings remain.

**Acceptance:** The full submitted set opens exactly once in one analyzer/batch; each independent inbox remains separate; stale or partial entries never open; legacy entries get one grace period; the app makes no server upload and uses matching shell colors.

## Final review follow-up — 2026-10-21

- Independent review found an expired file could open when cache cleanup rejected. `openShared()` now checks the recorded timestamp and consume marker itself, so cleanup failure cannot authorize opening stale bytes. Purge removes data before its marker and keeps the marker if a data deletion fails.
- The regression passed on computer and phone Playwright profiles. The full E2E run passed 338 cases and skipped 32 platform-gated cases; two guide-classifier cases initially failed because the technical-token allowlist was incomplete, then both passed on computer and phone in a focused rerun.
- The final independent review found no remaining actionable findings. Physical OS share-sheet behavior, real browser storage failures, simultaneous consumption in multiple tabs, and cleanup timing while a service worker is dormant remain unverified. Expiry is enforced on activation, share receipt, and open; it is not a background timer.
