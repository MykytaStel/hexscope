# Clean-copy panel extraction design

**Status:** Approved for planning on 2026-10-05

## Goal

Make the clean-copy flow easier to understand and change by giving it a
focused module, while preserving what a person sees and can do when preparing
a file to send.

Success means `Drawer` delegates clean-copy UI creation to one module and the
existing copy, readback verification, and follow-up actions behave the same.

## Current structure

`apps/web/src/drawer.ts` owns the file detail panels and a `cleaner()` method.
That method builds the clean button and optional comments/notes checkbox,
calls the existing cleaning action, and renders the result. It also wires the
before/after summary to the readback report and marks revealed facts removed
only when the report confirms it.

`apps/web/src/cleancard.ts` already owns reusable clean-copy parts, including
the comparison card, limits, copy actions, and verification status. The
cleaner is attached in two branches of `Drawer.reveals()`: files with
removable image data but no listed facts, and the regular revealed-facts
branch.

## Design

Add `apps/web/src/cleaner.ts` with an exported `createCleaner` function. It
will receive the `FileModel`, the existing `CleanActions` implementation, and
the reveals group that contains the cleaner. It will return the complete
cleaner element and own its click handling, optional notes choice, success or
error rendering, copy actions, verification callback, and fact-state updates.

`Drawer.reveals()` will call `createCleaner(m, this.cleaning, group)` at both
existing attachment points. `Drawer` will continue to own the surrounding
reveal panel and its facts; the cleaner module will use the passed group to
find and update only facts affected by this clean-copy flow. Existing helpers
in `cleancard.ts`, including `beforeAfter`, `renderBeforeAfter`, `cleanLimits`,
`copyVerification`, `shareButton`, and `copyPictureButton`, remain the source
of those shared UI pieces.

The action sequence remains unchanged: disable the button, optionally clear
the kept markers for requested comments and notes, call `cleaning.clean`,
show the existing error state or build the result, then let asynchronous
readback update the comparison and confirmed removed facts. Save, share, open,
compare, and picture-copy actions remain available under their current
conditions.

## Failure handling and privacy

Preserve current behavior for cleaner errors, verification unavailable states,
and action availability. The extraction adds no network calls, persistence,
telemetry, or new handling of file bytes. Verification continues to use the
existing value-free report and only its confirmed results may update the
summary or fact styling.

## Non-goals

- Change user-facing text, layout, localization, or keyboard behavior.
- Change which formats or findings the cleaner supports, or how copy bytes
  are produced.
- Restructure the other `Drawer` panels or redesign `CleanActions`.
- Add new product behavior or dependencies.

## Acceptance and validation

1. `Drawer` delegates both clean-copy attachment points to `createCleaner`,
   and the old `Drawer.cleaner()` implementation is removed.
2. The optional comments/notes choice, error state, before/after summary,
   verification result, kept-fact behavior, and all copy actions retain their
   current behavior.
3. Run the web TypeScript/build check and unit suite, then the focused
   clean-copy Playwright coverage in `apps/web/e2e/site.e2e.ts` and
   `apps/web/e2e/a11y.e2e.ts`.
4. Review the final diff to confirm this is a module extraction without
   unrelated product changes.
