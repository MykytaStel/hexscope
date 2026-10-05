# Hexscope roadmap

This is the project's shared product and engineering roadmap. It describes priorities and decision gates, not release dates. Work moves forward in small, reviewable slices; a feature is not considered complete until the user-facing behavior, CLI/core parity where applicable, tests, and hosted state agree.

## Product direction

Hexscope should help a person understand what a file reveals before they share it, then let them act safely. It should also make file formats understandable to curious people, down to the bytes. Analysis stays on the user's device, conclusions stay tied to parser evidence, and unsupported cases are described as unchecked rather than clear.

## Principles

- **Local by default:** no file uploads, accounts, telemetry, or server-side inspection.
- **Evidence before verdict:** explain what was read, what was changed, and what could not be checked.
- **One core, several interfaces:** keep format facts and conservative decisions in Rust core; expose consistent results to the Web, CLI, and GitHub Action.
- **Copies stay under user control:** never silently alter the source; keep a successfully produced copy available when post-write verification is incomplete, and refuse to produce one when source content cannot be inspected safely.
- **Measure before expanding:** protect parser safety, browser memory, gzip size budgets, accessibility, and real-device usability.
- **Teach without getting in the way:** give the plain-language answer first; let curious users open the structure, algorithms, and bytes.

## Shipped foundation

### Shared clean-copy verification — shipped in PR #14

- Rust core compares parser-backed facts before and after cleaning and emits a value-free report.
- Web verifies the actual produced bytes in the worker and retains its QR and selected-PDF-text checks.
- CLI `clean --verify` reads the written output back and returns a conservative status.
- PR #14 first shipped confirmed removal for complete JPEG file-scope findings: camera, serial number, owner, and location. The PDF Info/XMP extension below adds a second evidence-backed slice; other formats and findings remain unchecked until parser and cleaner behavior have proof and fixtures.
- Web output verification skips the additional parse above 10 MiB; copy creation remains available.

## Near-term priorities

### P0 — Expand verification without creating false reassurance

1. **Publish a format/finding/cleaning coverage matrix.** The narrower verified-removal contract is documented in [`verification-coverage.md`](verification-coverage.md) and enforced by the core. A full inventory of parser extraction and cleaning behavior across all formats remains follow-up work.
2. **Expand PDF verification in narrow slices.** Shared core verification now confirms removal only for complete, file-scope Info/XMP findings: author, title, subject, keywords, application, producer, created, modified, and history. An unsupported fact observed in the cleaned output is reported as present; claims that unsupported revision findings, hidden or covered text, or other PDF content were removed remain unchecked until parser and cleaner evidence proves coverage. Keep Web QR rescanning and selected-PDF-text checks supplemental.
3. **Add real regression cases for incomplete documents.** The PDF slice now covers surviving hidden text, hidden and revision text absent from copies but left unchecked, unreadable page streams, and findings present only in the compared output. Extend equivalent cases to other formats as their verification coverage grows; every uncertain path must stay unchecked.
4. **Make coverage understandable in the result UI and CLI.** The shared clean-copy result now names what was removed, remains, or was not checked, with a plain-language reason. Keep any successfully produced copy available when post-write verification is incomplete.

### P1 — Make the core and CLI a dependable platform

1. **Keep a single capability contract.** Derive coverage labels and reason codes from the shared core contract so Web, CLI, JSON, and Action output cannot silently disagree.
2. **Improve batch workflows.** Make per-file results, aggregate exit status, and machine-readable output useful for folders and CI without hiding which files were unchecked.
3. **Harden parser and cleaner boundaries.** Extend property tests, fuzz targets, malformed-file fixtures, and read-after-write verification as new format coverage is added.
4. **Protect performance and package budgets.** Benchmark parsing and cleaning on representative large files; lazy-load optional work and keep existing JavaScript/WASM limits enforced in CI.
5. **Finish user-facing accessibility and localization as features grow.** Preserve keyboard operation, screen-reader announcements, reduced-motion behavior, mobile layouts, and matching Ukrainian/English explanations.

## Film Scan Inspector and local film photo lab

The feature is for a person preparing a scan of a film photograph to send. Hexscope receives a digital scan, so it can inspect both the digital file and evidence visible from the physical film.

### Two layers to inspect

- **Scan-file data:** camera/scanner and software fields, dates, location, embedded previews, and other metadata that the scanning or editing workflow may have written.
- **Visible film evidence:** if the scan includes the film border, identify frame boundaries, perforations, and readable edge markings. These may expose film type, emulsion/frame codes, or roll indexing. Such markings are clues, not necessarily personal identifiers.

Scanning creates a digital image file, so the scan can also carry metadata from the digitizing or editing workflow; embedded metadata is not guaranteed to be present or consistent across scanners. Fujifilm's guide documents edge markings, DX codes, and frame numbers on specific roll films. Kodak KEYKODE describes a motion-picture workflow for linking frames and rolls, so it must not be generalized to every still-photo negative. Realistic film grain can also be rendered onto digital images, which makes grain a weak clue rather than proof of origin. See the [Library of Congress scanning guide](https://blogs.loc.gov/thesignal/2014/03/personal-digital-archiving-the-basics-of-scanning/), [CIPA Exif standard](https://www.cipa.jp/e/std/std-sec.html), [Fujifilm Professional Data Guide](https://asset.fujifilm.com/www/us/files/2020-03/85d928f44b0df3b2a95913e46608881d/ProfessionalFilmDataGuide.pdf), [Kodak KEYKODE reference](https://www.kodak.com/en/motion/page/keykode-numbers/), and [film-grain rendering research](https://onlinelibrary.wiley.com/doi/abs/10.1111/cgf.13159).

### Current implementation

- The web app locally inspects JPEG, PNG and WebP pixels in its worker for repeated high-contrast openings and frame-like edge boundaries. The result lists the observed edges and signal agreement; it does not infer film origin from grain.
- The [film photo lab](film-lab.md) provides oriented before/after previews, explicit border cropping, film-base-calibrated color/monochrome negative conversion, exposure and contrast, and validated local recipes. It creates a bounded JPEG sharing rendition and re-reads the actual output with the shared core plus supplemental QR/border analysis. The source remains intact.
- The first regression fixtures are synthetic. The detector has not yet been measured against a labeled set of genuine film scans and digital images with simulated borders, so it is a visual clue rather than a validated classifier.
- Before making provenance claims or adding edge-marking recognition, validate precision and false-positive rates on consented or public examples of full-border scans, cropped scans, digital frames, and low-contrast scans. RAW/TIFF, 16-bit processing, roll-wide calibration and photographic evaluation remain future work.

### Stages and decision gates

1. **Discovery:** audit current JPEG facts and cleaners; gather lawful, consented or synthetic examples of flatbed scans, camera scans, negatives, positives, cropped scans, full borders, and digitally simulated film looks. Do not add private user photographs to the repository.
2. **MVP:** start with the existing JPEG path. Report embedded metadata separately from visible image content. Detect only evidence with measured accuracy; label uncertain origin as likely/uncertain rather than verified.
3. **User action:** offer an explicit new copy to remove supported metadata. If visible edge markings should be hidden, offer a separate crop/redaction preview; never imply that metadata cleaning removes text baked into pixels or change the master scan silently.
4. **Recognition expansion:** evaluate local OCR for edge markings only after a labeled sample set, precision/recall targets, language coverage, worker isolation, and bundle/runtime cost are known. Add TIFF or other scan formats only after parser and cleaner coverage is proven.
5. **Acceptance:** test genuine film scans, digital photographs with simulated grain, partial/cropped borders, unreadable markings, and damaged metadata. Grain alone must never be used as proof of film origin.

## Research and differentiation candidates

These are ideas to evaluate, not committed milestones. Select them by user value, evidence quality, and fit with local-first constraints.

### Local privacy mosaic for a batch of photos

Show when a group of photos reveals a pattern that a single file does not: repeated device identifiers, a location cluster, or a sequence of places. Keep analysis on-device, show the exact evidence and its limits, and let the user decide whether to clean or exclude files. Avoid inferring a home or identity from weak signals.

### Reproducible clean-copy receipt

Create an optional, value-free receipt that records the verifier/schema version, capabilities applied, and outcome for a specific produced copy. Design the privacy model before making receipts shareable; a receipt must not look like a safety certification or leak file contents.

### File-format lab

Extend Hexscope's byte-to-meaning experiences with more step-through views: how a JPEG scan is assembled, where a PDF page draws hidden text, or how an archive entry maps to compressed bytes. Keep each lesson connected to the actual file under inspection and its specification.

### Trustworthy format coverage scorecard

Build a continuously tested corpus from synthetic fixtures and redistributable samples. Track coverage, false reassurance, false positives, parse completeness, performance, and size per format. Use it to decide what can move from “unchecked” to “verified.”

## Longer-term areas

- **Architecture:** keep parsers, facts, cleaning, and verification separated behind small stable interfaces; add abstraction only where multiple formats need it.
- **Science and algorithms:** measure detection quality and failure modes; use deterministic methods first. Keep OCR or machine learning local and optional, with published evaluation data and explicit uncertainty.
- **Web product:** keep the main path short—summary, evidence, safe action—while preserving the detailed byte inspector as an optional depth layer.
- **CLI and CI:** maintain predictable exit codes, stable JSON/SARIF, batch support, and parity with Web for the same supported evidence.
- **Security and reliability:** continue fuzzing hostile inputs, testing corrupt and oversized files, and verifying real output bytes rather than trusting the requested operation.

## Out of scope unless the product direction changes

- Uploading user files or reports to a service for analysis.
- Accounts, analytics, or background storage of inspected files.
- Calling Hexscope a malware scanner or treating a heuristic as proof of authenticity.
- Silently cropping, redacting, or overwriting a source image.

## How to use this roadmap

Start with the highest-priority unfinished user problem. Before each substantial feature, verify the parser and cleaner can support the claim, define fixtures and failure behavior, then implement one bounded end-to-end slice across the necessary interfaces. Update this file when evidence changes priority or when work ships; keep speculative candidates clearly marked until selected.
