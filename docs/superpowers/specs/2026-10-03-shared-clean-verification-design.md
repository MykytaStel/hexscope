# Shared clean-copy verification across core, CLI, and web

**Status:** Approved for implementation after codebase correction on 2026-10-03.

## Goal

When a person prepares a file to send, Hexscope will inspect the copy it actually produced and say which parser-backed findings were removed, remain, or could not be checked. The same value-free classifier will serve the core, CLI, and WebAssembly bridge. It must never infer removal from unsupported or incomplete parser coverage.

The first verified coverage remains deliberately small: JPEG file-level camera, serial number, owner, and location findings. Other formats and kinds are reported as unchecked until parser completeness and fixtures prove that coverage.

## Current state

- PR #13 already added a lazy clean-copy verification report to Web, with output reparsing, QR rescanning, and PDF selected-text checks. Its parser-backed fact classifier is TypeScript-specific.
- `hexscope-core/src/summary.rs` extracts parser-backed facts, but there is no shared verification model or classifier.
- `hexscope-cli` writes clean copies but does not read the generated file back for verification.
- This milestone replaces only Web's parser-backed classifier. It preserves the QR and PDF selection supplements already in the UI.
- Processing stays local. Verification reports, logs, and JSON must not include finding values or scopes.
- PR #13 already bounds clean-copy verification at 10 MiB; the shared core classifier keeps that bound and skips its extra summary parse above the same limit. This does not change cleaning or the existing file-open path.

## Design

### Shared Rust contract

Add `hexscope-core::verification` with:

- `VerificationSnapshot`: format, parser completeness, and in-memory findings containing kind, scope, and optional stable value.
- `VerificationReport`: schema version 1 and `removed`, `present`, `unchecked` groups. Each item contains kind, a stable reason code, and an `unexpected` flag. It contains no values, scopes, parse tree, or bytes.
- `snapshot(&Summary)` to adapt the existing parser summary and `compare(source, output)` as the one classifier used by both clients.

Findings are compared as a multiset so duplicates stay distinct. Exact `(format, kind, scope, value)` matches remain present. An unmatched source finding with an unmatched output finding of the same format, kind, and scope remains present with `value_changed_same_kind`. A finding is removed only when the exact capability is covered, both parses are complete, and no same-kind output finding remains. Findings outside the capability table or from incomplete parses are unchecked. Output-only findings are present and marked unexpected. No comparable findings produce one unchecked `file` item rather than an all-clear.

The initial capability table is exactly JPEG, file scope, and kinds `camera`, `serial`, `owner`, and `location`. Summary completeness is false when parsing produced a warning or error node. Stable reasons are `removed`, `still_present`, `value_changed_same_kind`, `no_stable_value`, `coverage_incomplete`, `parse_incomplete`, `unexpected_output`, `no_comparable_findings`, and `verification_skipped`.

### WebAssembly and web app

- `Parsed` retains the source snapshot from its original parse. The worker adds its RPC request id as an opaque local token to the parsed-file model; a later verification request sends the actual `Blob` bytes back to the worker.
- After the asynchronous byte read, the worker looks up the still-open source parse and synchronously calls its existing `verifyCopy(bytes)` WASM method. That method checks the 10 MiB bound, summarizes the output bytes once, and calls the core classifier. If a newer parse already replaced the source, the worker reports it unchecked instead of comparing against the wrong file.
- Finding values and scopes stay in Rust for this comparison. The Web decoder accepts only the strict versioned value-free schema.
- PR #13's lazy accessible status and copy actions remain in place. The core report covers parser-backed findings; QR rescanning and PDF selected-text checks remain separate browser-specific supplements.
- Clean, redacted, and movie copies use the same verification request path; formats or kinds outside the whitelist remain unchecked.
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
2. WASM tests compare source findings with actual output bytes through the exported bridge, verify residual findings, and prove that oversized inputs return skipped without parsing.
3. Web tests check value-free report parsing, localized user-visible states, and existing save/share behavior. E2E exercises the clean-copy flow with the real fixture.
4. CLI integration tests prove it checks bytes on disk, preserves copies with residual/unchecked findings, emits valid versioned JSON, handles multiple files and `--in-place`, and returns the documented exit statuses.
5. Existing clean, redaction, accessibility, and large-file flows continue to work.
6. Run focused Rust and web checks, both WASM builds, the relevant web E2E path, and the repository gate. Compare gzip-9 sizes to the captured baseline; do not increase CI budgets.
7. Deployment is considered only after merge, hosted CI, and live artifact/file-flow checks.

## Self-review

- The spec now describes observed code rather than claiming an existing verifier.
- Web checks the actual copied bytes; the CLI checks the destination bytes read back from disk.
- Only explicitly complete JPEG fact coverage can be called removed; unsupported and incomplete cases remain visible.
- Finding values and scopes are absent from every report boundary.
- Browser extra work has a concrete 10 MiB bound without changing copy creation.
- CLI copy-by-default, `--out`, batch, and in-place behavior are addressed.
