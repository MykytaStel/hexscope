# PWA shared-file inbox

**Status:** Proposed for review
**Date:** 2026-10-21

## Goal

Make the installed app's share target handle every file the user selects and limit how long unconsumed files remain in local browser storage.

## Behavior

- Read every `file` value from the share-target `FormData`, preserve its filename and MIME type, and store each file under a unique cache key grouped by a random id for that submission. A second share submission must not merge with an earlier unconsumed batch.
- Redirect to `?shared=<submission-id>` and let the page read only that submission's entries; validate the id before constructing same-origin cache keys.
- Continue to recognize the old `?shared=1` URL during migration, reading only its legacy entry.
- Reuse the existing `openFiles` flow: one file opens in the analyzer; multiple files enter the existing batch analyzer.
- Enforce a per-submission cap of 50 files and 400 MiB total, matching the existing batch share-output bounds. On a count, size, or storage failure, remove any entries from that partial receive and show an actionable message; never silently analyze only part of the submitted set.
- Store a receive timestamp and expire unconsumed entries after 24 hours. Purge expired entries when the service worker activates, receives a new share, or the app consumes the inbox. Service workers cannot guarantee a timed background deletion while fully dormant, so the limit is enforced at the next event.
- Show a localized expired/missing-file message when `?shared` has no usable entries. The processing remains local; no shared bytes are sent to a server.
- Migrate the old single `shared-file` cache entry on activation and give it one 24-hour grace period so an upgrade does not immediately discard a pending share.
- Set the installed-app manifest theme color to the same graphite value as the page's existing theme-color metadata.

## Acceptance

- A share POST containing two or more files opens all files as one batch, with stable names and types.
- Two separate share submissions remain separate; opening one cannot consume the other.
- One file keeps the current single-file experience.
- Expired entries are removed and never opened; fresh entries remain until consumed.
- A legacy single-file cache entry survives migration for its grace period, then expires if left unconsumed.
- A page launched with the old `?shared=1` link can consume that migrated legacy entry during the grace period.
- Over-limit or partially stored requests leave no partial inbox and explain how to retry.
- The manifest and page metadata use the same shell theme color.
- Tests cover duplicate names, zero files, stale entries, quota failure, and a fresh single-file share.

## Out of scope

Changing the browser's share-sheet UI, retaining files after they are opened, or adding server-side upload/storage.
