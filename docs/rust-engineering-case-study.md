# Hexscope: an evidence-led Rust engineering case study

This page is a source-linked technical walkthrough for engineers and reviewers. It describes the codebase and its published contracts, not an independent audit or a claim about adoption.

## Problem and product boundary

People routinely send photos, PDFs, documents and archives without knowing what metadata, hidden content or structural damage they contain. Hexscope inspects local files, explains supported findings, and creates cleaned or recovered copies **only when the supported path can verify its result**.

The key constraint is privacy: file contents are parsed on-device. The [contribution principles](../CONTRIBUTING.md) explain the deliberately narrow website-visit counter exception. The product is not a malware scanner or a guarantee that a file is safe.

## Rust design: one core, multiple interfaces

The same Rust parsing model powers CLI, WASM and the browser UI.

| Layer | Source | Main engineering decision |
| --- | --- | --- |
| Bounded reads | [`reader.rs`](../crates/hexscope-core/src/reader.rs) | Centralize checked byte access; parsers do not index slices directly |
| Parse model | [`model.rs`](../crates/hexscope-core/src/model.rs) | Return a tree of byte ranges, including damaged areas, instead of aborting on a malformed input |
| Format dispatch | [`document.rs`](../crates/hexscope-core/src/document.rs) | Select a parser from evidence in file bytes, not just the file extension |
| DEFLATE/zlib | [`hexscope-inflate`](../crates/hexscope-inflate) | Explain decompression one step at a time |
| CLI | [`hexscope-cli`](../crates/hexscope-cli) | Stable commands, exit codes and structured output contracts |
| Browser bridge | [`hexscope-wasm`](../crates/hexscope-wasm) | Share the Rust core with a browser worker using compact typed arrays |

The core, inflate library and CLI are intentionally free of runtime dependencies. The repository prohibits `unsafe` and treats parser hangs, panics and out-of-bounds reads as security issues. Those are **design rules**, not a claim that all possible inputs have been mathematically proven safe.

## A reproducible CLI walkthrough

Using a disposable file or a file you are authorized to inspect:

```sh
cargo install --locked hexscope-cli
hexscope --version
hexscope check path/to/sample.jpg
hexscope check --json --fail-on none path/to/sample.jpg
hexscope clean --verify --out /tmp/hexscope-clean path/to/sample.jpg
```

The exact command/option/exit-code behavior is documented in the tested [CLI contract](cli.md). `clean` writes copies by default, not modifications to the original. Some file types are read-only; an unsupported change is refused rather than silently presented as complete.

A useful interview discussion: why bounded reads matter for untrusted input, why structured evidence is different from a security verdict, and what a verified output actually establishes.

## Quality gates and observable limits

The documented development checks are:

```sh
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --all
pnpm build
pnpm --filter web test
```

They are commands to run on a checked-out repository, **not a claim that they were executed while writing this page**. [CONTRIBUTING.md](../CONTRIBUTING.md) documents property tests, fuzz targets, size/performance budgets, and the requirement to add a fixture for every confirmed parsing bug.

For structured tooling, `hexscope check --sarif` and `--json` are supported. [cli.md](cli.md) defines their compatibility guarantees; the CLI contract tests are in [`contract.rs`](../crates/hexscope-cli/tests/contract.rs).

Important limitations remain visible in [known issues](known-issues.md): unsupported parsers and image/video codecs, bounded handling of unusually large or complex files, incomplete audio interpretation, and specific physical-device verification gaps. Avoid describing an incomplete check as a clean result.

## Engineering decisions worth discussing in an interview

1. **Bounds over convenience.** How centralized checked byte reads constrain the parser and make malformed-file behavior testable.
2. **Explainable state.** Why a parse tree keeps damaged byte ranges visible rather than throwing away all evidence after an error.
3. **One semantic core.** What belongs in Rust versus browser TypeScript, and why CLI/browser findings should not drift.
4. **Verified transformations.** Why a clean or repaired copy must be re-read, and what that read-back does not prove.
5. **Compatibility contracts.** How stable exit codes, JSON/SARIF schemas and CLI tests help consumers integrate safely.
6. **Resource budgets.** How bounded scans, memory and WASM size budgets shape implementation choices.

## Public evidence

- [README](../README.md) — user-facing functionality, installation and distributions
- [CONTRIBUTING.md](../CONTRIBUTING.md) — code architecture, constraints, checks and how to add supported formats
- [CLI contract](cli.md) — commands, exit codes, machine-readable results
- [Known issues](known-issues.md) — what the project intentionally does not claim
- [Release notes / roadmap](roadmap.md) — milestone status and remaining verification gates

**Scope note:** This is a reviewer guide. It does not replace checking a specific release, source revision, CI logs or local tests.