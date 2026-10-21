# Web category dependency boundary

**Status:** Proposed for review
**Date:** 2026-10-21

## Goal

Remove the `share.ts` ↔ `verdict.ts` import cycle without changing any user-visible category order, share copy, or verdict.

## Behavior and structure

- Move the category table and pure category-selection/counting helpers into a small `fact-categories.ts` module that depends only on the file model.
- Move the generic English list-joining helper into a small `list-format.ts` module.
- Make `share.ts` and `verdict.ts` import those helpers directly; neither helper module imports either consumer.
- Keep the current product hierarchy and defer broad `drawer.ts`/`analyzer.ts` decomposition until a concrete behavior boundary justifies it.

## Acceptance

- Existing verdict, category-order, share-sentence, and localization expectations remain unchanged.
- The dependency graph no longer contains the share/verdict cycle.
- No feature or runtime dependency is added.

## Out of scope

Renaming categories, changing translations, or splitting modules solely to reduce line count.
