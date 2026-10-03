# Shared clean-copy verification across core, CLI, and web

**Status:** Draft for user review; implementation has not started.

## Goal

Move the classification of parser-backed findings in a cleaned copy into
`hexscope-core`, then expose the same result to the web app and the CLI. A user
must be able to tell which known findings were removed, remain in the copy, or
could not be checked. Hexscope must never infer removal from an incomplete
parser result or describe a partially checked file as fully clear.

The primary user remains a person preparing a file to send. CLI users and file
format enthusiasts are secondary users. This is the first shared-core milestone
in the approved Hexscope roadmap, not a claim that every feature or format will
reach parity in one change.

## Current state

- `apps/web/src/verification.ts` owns the finding-matching algorithm, explicit
  JPEG coverage whitelist, PDF selected-text verification, and compact report
  types.
- `apps/web/src/worker.ts` reparses the generated copy in WebAssembly. It keeps
  file values in the worker, sends a compact report to the page, and maps
  parser and worker errors to an unchecked result.
- `crates/hexscope-core/src/summary.rs` already extracts parser-backed facts
  for the CLI, but it has no output-verification model.
- `crates/hexscope-cli/src/main.rs` supports `check`, `clean`, `repair`, and
  `redact`; `clean` writes a copy but does not reparse and compare it.
- The Web UI also checks selected PDF redaction text and QR findings. Those
  checks include UI-specific inputs that the CLI does not currently collect.
- The approved product constraints are local processing, no telemetry, no
  raw finding values in the displayed verification report, a bounded browser
  check, and no universal safety verdict.

## Candidate approaches

1. **Keep verification in TypeScript and duplicate it in the CLI.** This is
   quick, but rules, coverage, and edge cases can drift between clients. Reject.
2. **Move the pure finding classifier and capability rules into Rust core.**
   Each client supplies observations already produced by the shared parser;
   core returns a value-free report. This preserves one algorithm without
   reparsing the browser's source file. Recommend.
3. **Make core accept both byte buffers and parse both files for every check.**
   This centralizes extraction too, but repeats parsing in the web worker and
   increases time and memory for the 10 MiB browser path. Reject for this
   milestone.

## Design

### Shared Rust contract

Add a focused verification module to `hexscope-core` with types equivalent to:

- **Finding observation:** format, kind, optional stable value, and optional
  scope. Values are inputs to in-memory comparison only.
- **Coverage:** an explicit format/kind/scope capability and whether the
  source and output parses are complete for this comparison.
- **Report item:** kind, `removed` / `present` / `unchecked` status, a stable
  reason code, and an `unexpected` marker for output-only findings.
- **Report:** the three groups consumed by the existing UI and by CLI output.

The serialized report must never contain a finding value, scope, parser tree,
or file bytes. Client code maps kind and reason codes to localized or
human-readable labels. The core remains free of UI strings, network access,
filesystem I/O, and new runtime dependencies.

The initial reason-code set is `still_present`, `value_changed_same_kind`,
`no_stable_value`, `coverage_incomplete`, `parse_incomplete`,
`retained_by_clean_policy`, `unexpected_output`,
`no_comparable_findings`, and `verification_skipped`. A matching retained
finding stays in the `present` group with `retained_by_clean_policy`; client
copy-knowledge supplies its human explanation. The code set is versioned with
the public report schema.

The classifier compares occurrences as a multiset so duplicate findings are
not collapsed. It matches exact `(format, kind, scope, value)` identities. If
an unmatched source finding has an unmatched output finding of the same kind
and scope, classify it as present: the value may have changed, but the type of
information remains. Only classify an unmatched finding as removed when its
format/kind/scope capability is explicitly complete and both parses are
complete. Otherwise report it as unchecked. Output-only findings are present
and marked unexpected. An empty comparison produces an unchecked item rather
than an all-clear.

The first capability table preserves the currently tested whitelist:
JPEG file-level camera, serial number, owner, and location. Other
format/kind/scope pairs remain unchecked until parser completeness and a
regression fixture demonstrate coverage. A JPEG parse with structural
warnings or errors is incomplete for this check.

### WebAssembly and web app

- Add a small WebAssembly bridge method that accepts the already parsed source
  and output observations and invokes the Rust classifier. Do not parse the
  source bytes a second time solely for classification.
- Keep output parsing in the existing worker and keep the existing 10 MiB
  browser limit and 10-second timeout. A limit, malformed response, worker
  failure, or incomplete parse maps to unchecked.
- Keep the existing deferred loading of the verification UI after a copy is
  created. Measure the main JavaScript and both WASM artifacts; do not raise
  CI budgets to accommodate the bridge.
- Keep localized labels, specific explanatory text, and the existing
  three-group report in TypeScript. The page receives no raw values or scopes.
- Preserve the existing PDF selected-text and QR checks in the web app for
  this milestone. They remain clearly separate from core-classified
  parser-backed findings and are not represented as CLI parity.

### CLI

- Add `hexscope clean --verify [--json] PATH...` without changing the default
  behavior of `clean`.
- For each generated copy, summarize the source, clean it, write the copy, then
  summarize the actual output and call the same core classifier. Keep only one
  parsed document tree alive at a time.
- Print file and output paths plus finding kinds, statuses, and reason codes;
  omit finding values. `--json` emits one object per input with
  `schema_version: 1`, source and output paths, operation state, and the
  removed/present/unchecked arrays.
- Do not retract a generated copy when verification finds residual data or
  cannot complete. Report the status and use a conservative exit code so
  scripts cannot mistake present or unchecked findings for verified removal.
- If no copy is produced, report that no output was available to verify; do
  not emit a successful verification result.
- Keep batch processing, `--out`, and copy-by-default behavior. Add tests for
  mixed results across multiple files and for output write failures.

Exit status with `--verify`: `0` when every generated output has no present or
unchecked findings; `1` when cleaning fails, any finding remains, a check is
unchecked, or no output was created to verify; `2` remains reserved for usage
and input/output I/O errors. Without `--verify`, existing exit behavior is
unchanged.

## Failure handling and security

- Core parsing continues to return a tree; verification represents incomplete
  input as unchecked instead of adding a panic or an optimistic fallback.
- A copy remains available when its verification report contains present or
  unchecked findings.
- Values stay in memory and are not included in reports, logs, JSON, or
  telemetry. Avoid formatting parser errors that may contain file-derived
  text into user-facing verification reasons.
- The browser size and time limits remain outside the pure core classifier.
  The CLI verifies observations without adding another copy-size limit; its
  memory behavior must be measured on existing large-file fixtures.
- No OCR, password collection, server processing, general risk score, new
  format parser, or changes to what the cleaner removes are included.

## Validation and acceptance

1. Rust unit tests cover duplicate occurrences, changed-value same-kind
   findings, unsupported and incomplete coverage, output-only findings,
   intentional retention, empty input, and the absence of values/scopes in the
   report.
2. CLI integration tests prove that `clean --verify` checks the bytes actually
   written, reports residual and unchecked findings, preserves copies, emits
   valid versioned JSON, and returns the documented exit status.
3. Web worker tests prove it calls the shared Rust classifier, preserves
   timeout/error behavior, and never sends raw values back to the page.
4. Cross-client tests run the same fixture observations through core, CLI, and
   WASM and compare the normalized statuses and reason codes.
5. Existing PDF redaction, QR, clean-copy, keyboard, and accessibility flows
   continue to work unchanged.
6. `cargo test --all`, Clippy, both WASM builds, web tests, E2E, and the full
   JavaScript/WASM size budgets pass. Record bundle and large-file measurements
   before and after.
7. Production deployment occurs only after the reviewed change is merged and
   hosted CI and deployment succeed; check the live assets and a real file
   flow before claiming the feature is live.

## Self-review

- The scope distinguishes shared parser-backed findings from the existing
  Web-only PDF-selection and QR checks.
- Coverage is explicit and conservative; changed values cannot silently be
  called removed when a same-kind output finding remains.
- The report contract excludes values and scopes, and failure states are
  distinct from successful removal.
- CLI exit status, no-copy behavior, browser limits, memory constraints, and
  validation commands are stated without adding a new format or server.
