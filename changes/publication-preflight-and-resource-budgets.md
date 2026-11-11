### Added

- **Plan changes before making a copy**, with observed finding kinds from the shared Core removal contract, separate from proof that an output was cleaned.
- **Local publication profiles** in the Web and `hexscope policy PROFILE.json PATH...`, with portable JSON rules and a shared Core evaluator. `hexscope plan PATH...` exports value-free preflight reports.
- **Finding origin paths and differing stored fields**, with links to real source nodes and no inference of tampering.
- **Copy publication packages** containing the copy, value-free receipt and a small HTML summary; batch ZIPs also include the summary.
- **Opt-in local English/Ukrainian OCR** for image redaction and a PDF page's first extractable picture. Suggestions require manual confirmation; the engine and models load from the same site, without uploading images.
- **A damaged-sample laboratory** that demonstrates CRC damage without changing the person's file.

### Changed

- **Whole-file reading has explicit budgets:** 128 MiB in Web, 512 MiB in CLI; supported large movies retain sliced inspection. The shared tree stops allocating after 500,000 ordinary nodes and records an incomplete-read error.
- **Long operations recover through worker deadlines.** Unresponsive cancelled parses are discarded after a short grace period; late replies cannot replace a newer session.
- **Comparisons explain changed finding values** and distinguish byte differences from proof of visible-content preservation.
- **Photo area editing supports Redo**, with bounded undo history and keyboard controls.

### Fixed

- **Comparison workers ignore progress messages** until a terminal result, report failures and release overdue workers.
- **Audio formats can produce verification receipts** consistently in Core and Web.
- **Ukrainian PDF revision text preserves “one”** instead of displaying an incomplete phrase.
