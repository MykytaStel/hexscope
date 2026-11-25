# Engineering roadmap: 0.24.0–0.33.0

Baseline: workspace 0.23.0, main commit `d115b9698d4dd77b6f4fd51150eae0f065a2be8b`.
This is a flexible engineering sequence, not a release schedule or a race to 1.0.
Quality, accessibility, plain-language explanations and local privacy are
ongoing work in every phase, not a final milestone.
It complements the historical [roadmap](roadmap.md). The earlier private
learning milestone moves to 0.32; small learning/UX fixes can land sooner
when they do not block correctness. Do not bump the version until its gate holds.

| Release | Scope | Acceptance evidence |
| --- | --- | --- |
| 0.24.0 | Core hardening: malformed framing, arithmetic/range checks, parser error classification, regression corpus | Focused regressions, all existing CI gates, no false complete status in tested damaged fixtures |
| 0.25.0 | Shared resource accounting: bytes, nodes, expanded output, nesting, work and cancellation; preserve existing limits until measured | Adversarial budget fixtures; deterministic partial status; native and WASM budget tests |
| 0.26.0 | Deep container inspection: bounded nested ZIP/Office traversal, source paths, duplicate/ambiguous entries, native/browser compression parity where practical | Nested corpus, aggregate budgets and correct evidence origins; unsupported methods explicitly unchecked |
| 0.27.0 | PDF intelligence: xref stream entries, indirect filter resolution, incremental revisions, attachments, redaction verification | Valid/damaged PDF fixtures, independent renderer/text checks; fail closed for unsupported encrypted or ambiguous rewrites |
| 0.28.0 | Photo intelligence: HEIF/AVIF extent/reference coverage, EXIF/XMP/ICC/HDR, Live Photo pair identifiers | Independent metadata and pixel checks; physical HDR display evidence or explicit unchecked status |
| 0.29.0 | Media intelligence: RIFF/RF64/BW64, MP3/FLAC/Ogg and MOV/MP4 coverage and preservation | Per-format corpus, malformed boundary fixtures, independent metadata and media-payload comparison |
| 0.30.0 | Verification engine 2: shared typed evidence, source/copy fingerprints, bounded readback beyond current browser cap where measured | Independent verification; no verified claim for unread data; CLI/WASM parity, versioned receipt compatibility |
| 0.31.0 | Recovery lab: conservative recovery of damaged PNG/JPEG/ZIP, explain changes and losses, expand only with evidence | Synthetic damage matrix, independent readback, original never overwritten by default |
| 0.32.0 | Workbench: semantic comparison, local policies, private guided learning, evidence navigation and accessible UI | Keyboard/touch journeys, EN/UK parity, no private values in sample links, screenshots and E2E |
| 0.33.0 | Production readiness: API/CLI/WASM contracts, performance regression gates, physical-device matrix and release process | All release gates; recorded device/memory measurements; remaining unsupported cases documented; ongoing quality and accessibility review |

## Release discipline

- Work in reviewable branches; keep main deployable. No automatic release or merge.
- Every confirmed defect gets a failing-before/passing-after regression when feasible.
- Reuse existing format coverage, golden, property, fuzz, independent and large-file
  gates. Add missing fixtures before claiming new support.
- Keep supported, partial, unsupported and not checked distinct. No universal
  safety, malware or anonymization claim.
- Preserve local-only analysis and privacy-safe reports. Never upload user files
  or use real private data as committed fixtures.
- Record measurements (environment, file sizes, time, peak memory) before changing
  limits. Automated mobile emulation does not replace physical tests.
- Preserve existing CLI output and WASM contracts or document migrations explicitly.
- Use changes/ fragments for user-visible changes. Keep Cargo, site, CLI and
  changelog versions aligned through the existing release process.

## 0.24 first slice

- Reject zlib headers with invalid CINFO even when FCHECK is valid.
- Distinguish truncated gzip trailers from trailing bytes.
- Reject ZIP extraction when its declared expanded size exceeds the caller's
  budget before invoking a decoder; keep actual output and CRC verification.
- Run focused regressions and existing CI before merge. This slice is not the
  complete 0.24 release.

## 0.24 execution phases

| Phase | Work | Exit gate |
| --- | --- | --- |
| 0.24-A | Compressed framing and extraction budgets (this first slice) | Regression tests for zlib, gzip and ZIP; branch-push CI passes |
| 0.24-B | Audit checked arithmetic, offsets, lengths and allocation boundaries | Boundary fixtures for each confirmed defect; no panic or silent clipping |
| 0.24-C | Audit parser completion and error propagation across CLI and WASM | Truncated and malformed fixtures never report unsupported or unread evidence as complete |
| 0.24-D | Expand regression corpus and verify release readiness | Full CI, independent checks, native/browser evidence, documented remaining limitations |

Close each phase only with its recorded evidence. Keep the release version at
0.23.0 until the 0.24 release gate is satisfied. The later roadmap entries
remain proposed and can be refined using findings from earlier phases.

### Continuous improvement

- Treat version numbers as labels, not delivery targets. Bring accessibility,
  keyboard navigation, understandable coverage and Ukrainian parity into each slice.
- Phase B first slice: checked ranges and embedded offsets, ZIP extraction bounds,
  BMFF field limits and text preservation, HEIF item-table limits and partial
  inspection messages. Phase B remains open until broader audit and CI evidence.

### Performance and memory slice

- Release WASM presentation data once the page owns the flattened arrays;
  preserve verification snapshots, nested archive navigation and playback.
- Move cleaned output out of WASM once, avoiding repeated full-copy getters.
- Avoid writing to untouched media pages in the sparse movie byte view;
  reject over-budget non-media data before allocation.
- Prefetch documents only on reported fast connections with sufficient RAM.
- Use the same gzip size measurement locally and in CI; keep all budgets fixed.
- Remaining: true sparse byte views, worker lifetime/high-water memory,
  per-format code-size reductions, physical-device peak-memory measurements.

### Compact range migration

- Map known compact movie bytes to original offsets without materializing media.
- Entropy works even with no full-offset buffer; the current hex viewer still
  uses a full logical backing array. Migrate view, comparison, export and
  search consumers explicitly before switching the worker protocol.
- Avoid allocating an array of all missing chunks merely to answer a boolean
  comparison query; never display unread bytes as literal zeroes.

### Streaming embedded discovery

- Replace the sparse movie's explicit prefix allocation with bounded Blob windows.
- Keep one signature engine for contiguous and streamed inputs; preserve original offsets, marker search, cancellation and coverage reporting.
- Validate parity and physical read bounds before moving on to actual WASM binary profiling and browser memory benchmarks.
