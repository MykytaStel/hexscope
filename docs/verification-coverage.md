# Clean-copy verification coverage

This matrix describes when Hexscope may report a parser-backed finding as
**Removed** after it reads the clean copy back. It is deliberately narrower
than the set of facts Hexscope can display or the transformations a cleaner can
perform.

The canonical runtime contract is
[`VERIFIED_REMOVAL_CAPABILITIES`](../crates/hexscope-core/src/verification.rs).
The core reports removal only when the source finding matches one exact
format/kind/scope combination below, the source and output parses are complete,
and the finding is absent from the output. Hexscope captures the source facts
before cleaning, then reads back the written copy. A value-free result from
this same core contract is used by the Web worker and `hexscope clean --verify`.

| Format | Findings eligible for **Removed** | Required scope | Evidence and limits | Interfaces |
| --- | --- | --- | --- | --- |
| JPEG, PNG, WebP | camera, serial, owner, location | whole file | The complete source snapshot and read-back parse are compared for these four file-level kinds. A region-scoped or incomplete parse does not qualify. | Web worker and CLI. The Web skips read-back verification for copies larger than 10 MiB. |
| PDF | author, title, subject, keywords, application, producer, created, modified, history | whole file | The complete source snapshot and read-back parse are compared for these currently supported file-level Info/XMP kinds. Page content, revisions, annotations, hidden text, and other findings are outside this removal claim. | Web worker and CLI. The Web skips read-back verification for copies larger than 10 MiB. |
| Every other format, finding, or scope | none | — | A finding still observed in the copy is **Still present**. If it is absent but its removal is outside this contract, the result is **Not checked**. Incomplete reads remain **Not checked**. | Same shared core result where verification is available; Web-only QR and selected-PDF-text checks are supplemental and do not expand this matrix. |

## User-facing result meanings

- **Removed** means the eligible finding was present in the source and absent
  from a complete parse of the written copy.
- **Still present** means the output parser still found the finding, including
  when a different value of the same kind remains.
- **Not checked** means Hexscope cannot make a removal claim from its current
  capability, parse completeness, comparison evidence, or verification limit.
  The reason is shown next to the finding.

The human-readable Web result is available in English and Ukrainian. CLI output
uses the same three statuses and plain-language reasons. `--json` keeps the
version 1 schema and stable machine reason codes; scripts do not need to change.

## Regression evidence

- [`verification.rs`](../crates/hexscope-core/src/verification.rs) owns the
  capability list and tests that its exact combinations match the decision
  gate, while unknown kinds, formats, and scopes remain uncovered.
- [`verification.test.ts`](../apps/web/src/verification.test.ts) checks that
  every stable core reason decodes into its intended user-facing status and
  explanation.
- [`cli.rs`](../crates/hexscope-cli/tests/cli.rs) checks readable output,
  value-free reporting, and unchanged JSON behavior against written copies.
