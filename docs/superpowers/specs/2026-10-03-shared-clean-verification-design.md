# Shared clean-copy verification across core, CLI, and web

**Status:** Approved for implementation after codebase correction on 2026-10-03.

## Goal

When a person prepares a file to send, Hexscope will inspect the copy it actually produced and say which parser-backed findings were removed, remain, or could not be checked. The same value-free classifier will serve the core, CLI, and WebAssembly bridge. It must never infer removal from unsupported or incomplete parser coverage.

The first verified coverage remains deliberately small: JPEG file-level camera, serial number, owner, and location findings. Other formats and kinds are reported as unchecked until parser completeness and fixtures prove that coverage.

## Current state

- The web worker makes a cleaned copy and returns the removed-block list from the cleaner. The page displays those operation results but does not reparse the copy.
- `hexscope-core/src/summary.rs` extracts parser-backed facts for the CLI, but there is no verification model or classifier.
- `hexscope-cli` writes clean copies but does not read the generated file back for verification.
- Existing web functions also support PDF redaction, QR inspection, and opening a copy. They are separate operations; this milestone does not claim verification parity for QR or PDF text-selection checks.
- Processing stays local. Verification reports, logs, and JSON must not include finding values or scopes.
- The web worker has no current clean-verification size bound. This milestone will skip re-parsing copies larger than 10 MiB and report that verification was skipped. This bounds the additional parser memory; it does not change cleaning or the existing file-open path.

## Design

### Shared Rust contract

Add `hexscope-core::verification` with:

- `VerificationSnapshot`: format, parser completeness, and in-memory findings containing kind, scope, and optional stable value.
- `VerificationReport`: schema version 1 and `removed`, `present`, `unchecked` groups. Each item contains kind, a stable reason code, and an `unexpected` flag. It contains no values, scopes, parse tree, or bytes.
- `snapshot(&Summary)` to adapt the existing parser summary and `compare(source, output)` as the one classifier used by both clients.

Findings are compared as a multiset so duplicates stay distinct. Exact `(format, kind, scope, value)` matches remain present. An unmatched source finding with an unmatched output finding of the same format, kind, and scope remains present with `value_changed_same_kind`. A finding is removed only when the exact capability is covered, both parses are complete, and no same-kind output finding remains. Findings outside the capability table or from incomplete parses are unchecked. Output-only findings are present and marked unexpected. No comparable findings produce one unchecked `file` item rather than an all-clear.

The initial capability table is exactly JPEG, file scope, and kinds `camera`, `serial`, `owner`, and `location`. Summary completeness is false when parsing produced a warning or error node. Stable reasons are `removed`, `still_present`, `value_changed_same_kind`, `no_stable_value`, `coverage_incomplete`, `parse_incomplete`, `unexpected_output`, `no_comparable_findings`, and `verification_skipped`.

### WebAssembly and web app

- `Parsed` retains an in-memory source snapshot derived from the document it already parsed. A WASM method accepts the cleaner's `CleanCopy`, checks its existing output bytes, parses that output once, and calls the core classifier. This does not parse the original bytes again or copy finding values to the page.
- The WASM method skips output parsing when the copy exceeds 10 MiB and returns a `verification_skipped` report. A cleaner error means there is no copy and therefore no verification report.
- The worker attaches the report to its existing clean-copy response. The page adds an accessible, localized status with removed, still-present, and unchecked groups, showing finding kinds and reasons only.
- Clean, redacted, and movie copies use the same response path; formats or kinds outside the whitelist remain unchecked. PDF selected-text and QR findings are not claimed as verified by this contract.
- No new runtime dependency, telemetry, network call, OCR, or server processing.

### CLI

- Add `hexscope clean --verify [--json] PATH...`; default `clean` behavior remains unchanged.
- Before cleaning, retain the source snapshot. After the copy is atomically written, read the destination back from disk and compare its summary with the source snapshot. This also works with `--in-place` because the snapshot is retained before replacement.
- Human output names paths, kinds, statuses, and reason codes, never values. JSON emits one object per input with `schema_version: 1`, source/output paths, operation state, and the three groups.
- Do not retract a produced copy when findings remain or verification is incomplete. A made copy can be used even when its report is not clear.
- Exit code with `--verify`: `0` only when a copy was produced and all findings were verified removed; `1` for cleaner refusal/failure, any present or unchecked finding, or no output; `2` for usage and filesystem I/O errors. Without `--verify`, existing exit behavior stays unchanged.

## Failure handling and privacy

- Do not print finding values or scopes in verification output, including JSON. Values are held in memory only for comparison.
- A copy remains available when verification finds residual data, incomplete parsing, unsupported coverage, or a browser size limit.
- An output parse error or warning makes relevant source findings unchecked. A parser crash or unavailable verifier maps to an unchecked/skipped state in the worker.
- The browser's 10 MiB limit applies only to the extra verification parse; it does not prevent making, saving, or sharing a copy.
- The CLI verifies the bytes written to the destination; write/read errors remain I/O failures.

## Validation and acceptance

1. Core tests cover duplicate findings, changed-value same-kind findings, unsupported and incomplete coverage, output-only findings, empty input, and report serialization with no values or scopes.
2. WASM tests compare source and `CleanCopy` through the exported bridge, verify residual findings, and prove that oversized copies return skipped without parsing.
3. Web tests check value-free report parsing, localized user-visible states, and existing save/share behavior. E2E exercises the clean-copy flow with the real fixture.
4. CLI integration tests prove it checks bytes on disk, preserves copies with residual/unchecked findings, emits valid versioned JSON, handles multiple files and `--in-place`, and returns the documented exit statuses.
5. Existing clean, redaction, accessibility, and large-file flows continue to work.
6. Run focused Rust and web checks, both WASM builds, the relevant web E2E path, and the repository gate. Compare gzip-9 sizes to the captured baseline; do not increase CI budgets.
7. Deployment is considered only after merge, hosted CI, and live artifact/file-flow checks.

## Self-review

- The spec now describes observed code rather than claiming an existing verifier.
- The output object is the cleaner's actual copy in Web and the destination read back from disk in CLI.
- Only explicitly complete JPEG fact coverage can be called removed; unsupported and incomplete cases remain visible.
- Finding values and scopes are absent from every report boundary.
- Browser extra work has a concrete 10 MiB bound without changing copy creation.
- CLI copy-by-default, `--out`, batch, and in-place behavior are addressed.
