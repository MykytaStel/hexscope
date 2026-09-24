# Explanations and Verdict — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Every node explains itself in one plain sentence with a link to its specification, and a verdict card says what the file is and gives away.

**Architecture:** A `docs` module in the core holds `Doc`, `Spec`, `Concern` and a glob matcher; each format owns a table of `(pattern, Doc)` next to its parser, and `describe(tree, id, format)` picks by label and parent. The bridge deduplicates docs into shared tables with a per-node index. The page shows the sentence under the breadcrumbs, a Spec row, tree tooltips, and composes the verdict from problems, concerns and facts.

Spec: `docs/superpowers/specs/2026-09-24-explanations-design.md`.

## Global Constraints

- No new runtime dependencies; nothing panics; English UI text.
- Explanation style: one sentence, ≤ 200 characters, ends with a full stop, plain words first; say what it means for the person, not how the parser works.
- Every spec URL is https and was checked to return 200 (anchors checked against the document where it has ids).
- Before each commit: fmt, clippy `-D warnings`, `cargo test --all`, wasm32 check, web build; check exit codes.

## Tasks

### Task 1: `docs` core and the matcher
- [ ] `crates/hexscope-core/src/docs/mod.rs`: `Doc`, `Spec`, `Concern`; `const fn` helpers; `fn glob(pattern, label) -> bool` (`*` matches any run, including empty); `describe(tree, id, format) -> Option<Doc>` dispatching to format tables, then `generic(kind)` fallbacks for problems.
- [ ] Unit tests: glob (prefix, infix, several stars, no star), fallback for an unknown problem, `None` for an unknown field.

### Task 2: Tables
- [ ] PNG (`png/docs.rs`): signature, every chunk type the parser names (critical + the ancillary ones PngSuite has), IHDR/PLTE/tRNS/gAMA/pHYs/tEXt fields, IDAT parts (`zlib header*`, `block * · *`, `Adler-32*`), every PNG problem.
- [ ] JPEG (`jpeg` docs): markers/segments (SOI, APPn by kind, DQT, DHT, SOFn, DRI, SOS, scan data, EOI, COM), JFIF and SOF fields, problems.
- [ ] EXIF (`exif/tags.rs`): the tag table gains a description per tag; TIFF header, IFD structure, `* value`, thumbnail; problems.
- [ ] ZIP (`zip/docs.rs`): root, entry (child of root that is none of the records), local header, data, data descriptor, extra fields (known ids + `extra 0x*`), central directory and its records, ZIP64 records, EOCD, every field, every problem; concerns per spec table.
- [ ] Unknown format root and messages (`document.rs`).
- [ ] Coverage test (`tests/docs.rs`): all fixtures + built damaged archives + damaged PNG/JPEG cases from the golden tests → every node described, no problem on the fallback; every EXIF tag described; shape rules on every table entry.

### Task 3: Bridge
- [ ] `Parsed`: `docIds`, `docTexts`, `docCites`, `docUrls`, `docConcerns`, deduplicated by pointer/text. Tests: ids length = node count; the IHDR node's text is the IHDR doc; a CRC warning's concern is Damage.

### Task 4: Page
- [ ] Model: `doc(id)` → `{ text, cite, url, concern } | null`; worker passes the new arrays.
- [ ] Drawer: sentence under the crumbs; problem note uses the doc text and concern colour; Spec row with the link; verdict card first in the file panel with *Show me* links and the footnote.
- [ ] Tree: row tooltip gains the sentence.
- [ ] Browser: the four samples' verdicts; a damaged ZIP built in the page (prefix) shows "Something hidden"; hover/pin; 375 px.
- [ ] Link check script `scripts/check-spec-links.py`: every URL → 200.
- [ ] Docs: README section on the verdict and explanations; merge.
