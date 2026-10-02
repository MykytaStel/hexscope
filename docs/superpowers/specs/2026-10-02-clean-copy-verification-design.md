# Clean Copy Verification Design

## Context

Hexscope helps an individual understand what a file reveals before sending it.
The existing clean-copy flow reports what the writer removed, shows a before
and after count, and lets the user open or compare the result. The user must
open or compare it themselves to learn what the parser finds in the actual
output. The landing-page photo demonstration already reparses its sample copy,
but the everyday clean-copy flow does not.

This change adds an on-device check of the generated copy. It is a bounded first
slice of a broader request to stabilize the product and add useful engagement.
The primary audience is a person checking a file before sending it. The measure
of success is a clear, evidence-backed answer about the specific findings
Hexscope detected and whether they were found again in the produced copy.

## Goals

- Check the actual output bytes after a clean or redacted copy is produced.
- Report each comparable finding as removed, still present, or not checked.
- Explain intentional retention and format limits where they apply.
- Preserve the privacy guarantee: all parsing and comparison stay in the tab;
  no file bytes or finding values are uploaded or stored for analytics.
- Keep the copy available when verification is skipped or fails, while never
  describing an unchecked result as verified.

## Non-goals

- Claim that a file is universally safe, private, or free of every possible
  disclosure. The result only describes parser checks that ran on this output.
- Add a numeric safety score, user accounts, telemetry, or server processing.
- Add OCR, password collection, or new format parsers in this slice.
- Change which content the cleaner removes or keeps.
- Replace the existing compare dialog or educational stories.

## User flow

1. The user makes a clean copy or applies PDF redactions as they do today.
2. Hexscope parses the generated copy locally when the copy is within the
   verification resource budget.
3. The result appears with three groups: **Removed**, **Still present**, and
   **Not checked**. A still-present item explains whether the cleaner keeps it
   by design or whether it remains unexpectedly.
4. The user can still open, compare, save, or share the copy using the current
   actions. The verification result does not silently replace those actions.

## Verification algorithm

### Ordinary clean copies

- Parse the output with the existing file parser in a worker.
- Build an in-memory multiset of comparable findings for the original and
  output. A finding identity uses its kind, a conservative normalized value,
  and stable scope such as a PDF page or embedded-file path when the parser
  provides one. Duplicate findings remain separate occurrences; node numbers
  and byte offsets are not identities because rewriting can move them.
- Maintain an explicit verifier-capability table by format and finding kind.
  Absence is evidence of removal only for a kind the output parser can
  completely inspect. If completeness cannot be established, report
  **Not checked** instead of inferring removal from an empty result.
- A source finding is **Removed** only when its matching occurrence is absent
  from the parsed output.
- A matching output occurrence is **Still present**. Existing cleaner
  knowledge identifies findings intentionally retained and supplies the reason;
  other matches are reported as unexpected residual findings.
- A finding without a stable comparable value, an incomplete parse, or a
  verification skipped for resource limits is **Not checked**.
- In the first release, enable only format/kind pairs covered by regression
  tests and with a reliable completeness signal. Every other pair is
  **Not checked** with the format or coverage reason.
- Compare finding values only in memory. The visible result uses the finding's
  existing plain-language label and does not add values to the share card.
  The worker returns the compact verification result, not the parsed output's
  private values, solely for this feature.

### PDF redactions

- For selected searchable text, search the parsed output for each selected
  occurrence, scoped to its page when possible. A match found again is **Still
  present**; an occurrence not found is reported as removed from searchable
  output text only when that page is complete. Repeated words on other pages
  do not make the selected occurrence ambiguous.
- If the source or output page is marked incomplete, the affected check is
  **Not checked**.
- For a box over picture pixels, report that the requested area was rewritten
  by the redaction operation, but mark text inside those pixels **Not checked**:
  Hexscope does not perform OCR. Do not turn the absence of searchable text into
  a claim that pictured text was removed.
- Existing fail-closed behavior remains: if PDF cleaning refuses to create a
  copy because it cannot safely rewrite a page, verification must not offer a
  clean-copy success state.

### Resource and worker failures

- Verification runs off the main thread.
- A failed worker, parser error, incomplete result, or resource-budget skip
  yields **Not checked** with a plain explanation; it must not leave the UI
  waiting indefinitely.
- Verification is bounded by a documented maximum input size and the existing
  large-file performance evidence. Above that bound, the copy remains
  available and the report says it was not checked because of its size.
- Select the threshold from the repository's large-file fixtures and target
  browser memory behavior before implementation; cover below, at, and above
  the boundary. Verification failure does not prevent the existing save/share
  action.

## Product language

Use local, specific statements such as “This copy no longer contains the
location Hexscope found in the original” or “The selected words were not found
in searchable page text.” Avoid “safe,” “clean of private data,” or a green
all-clear when any relevant check is not checked. Show retained content with its
reason, using the existing per-format cleaning explanations.

## Likely implementation areas

- `apps/web/src/copies.ts`: run verification on the returned copy without
  blocking save/share on verification errors.
- `apps/web/src/compare.ts` or a focused verification module: parse output and
  compare finding multisets without exposing finding values.
- `apps/web/src/cleancard.ts` and `apps/web/src/drawer.ts`: present the three
  groups and retain the existing open, compare, save, and share actions.
- `apps/web/src/worker.ts`: keep parsing off the main thread and handle worker
  failures as an unchecked result rather than a hanging promise.
- `apps/web/src/i18n.ts`, focused unit tests, and `apps/web/e2e/site.e2e.ts`:
  localize and exercise the user-visible result.

## Acceptance criteria

1. A successful clean-copy flow reparses the actual generated output when
   eligible and reports source findings removed, still present, or not checked.
   No finding is described as removed unless its format/kind has a verifier
   that confirms complete coverage of the relevant output scope.
2. Intentionally retained findings include their reason; unexpected residual
   findings remain visible and are not described as removed.
3. PDF text selected for redaction is checked against searchable output text.
   Incomplete pages and text inside picture pixels are never reported as
   verified absent.
4. A worker or parser failure and an over-budget copy produce an explicit
   unchecked state, do not hang the page, and do not block saving or sharing
   the copy.
5. The result does not claim that the file is universally safe, send data over
   the network, or add persistent analytics.
6. English and Ukrainian screens remain understandable with keyboard and
   assistive technology; existing clean-copy, PDF refusal, and compare flows
   continue to work.

## Later engagement candidates

These are deliberately outside this first slice:

- An explainable local “data connections” view that links findings such as
  location, capture time, and camera identifiers to the evidence and explains
  what they reveal, without reducing them to a risk score.
- Short challenges over synthetic sample files that teach one format concept
  and let the user inspect the exact bytes, building on (rather than repeating)
  the existing demo and DEFLATE story.
