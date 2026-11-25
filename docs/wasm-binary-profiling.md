# WASM binary profiling

This slice attributes bytes without changing parser behavior, Rust features, or existing transfer budgets.

## Production evidence

```sh
pnpm build
pnpm perf:test
pnpm perf:size --check
pnpm perf:profile
```

Report: `.perf/wasm-profile-production.json`. All five artifacts must be present and unique. The profiler checks the module with the Node WebAssembly engine, bounds every structural read, and accounts for the eight-byte header plus all section envelopes. Gzip uses the same command as the existing gate.

Inspect code and data payloads, type/import/export overhead, function counts, body lengths (including locals), defined initial/maximum memory pages (64 KiB each), and the largest functions. Function names are taken only from optional name metadata or exported aliases. Stripped internal bodies remain anonymous. A function index is local to a specific artifact; never compare indices across builds.

## Diagnostic names

```sh
pnpm perf:profile:named pdf docs
```

Report: `.perf/wasm-profile-diagnostic.json`. Separate Cargo target and wasm-bindgen output under `.perf/`, same feature selection and size profile, no producer section, no explicit removal of names. The compiler may still omit some names. Do not deploy these artifacts or substitute their gzip sizes for production gates. Named crate groups are best-effort textual attribution, not exclusive retained size: LTO and inlining can mix work from several crates.

## Investigation order

1. Compare production code/data and raw/gzip sizes, and inspect largest named PDF/Docs bodies.
2. Identify candidates with explicit evidence: duplicated generic instantiations, large tables, hot/cold separation, shared media paths.
3. Change one mechanism at a time; preserve parse coverage, cleaning safety, nested dispatch, and CLI feature behavior.
4. Rebuild all five modules; run the full correctness suites and the same-machine latency benchmark comparison.
5. Require measured compressed savings without a material latency or correctness regression. No automatic budget increases.

Raw body bytes are not independent gzip contributions. Initial memory limits are not measured process RSS. This is not a call graph, disassembler, or heap profiler. The profiler currently supports the core Wasm v1 encodings used by these builds; unsupported import/reference/memory encodings fail rather than guessing.
