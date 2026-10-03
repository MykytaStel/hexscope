# Format and clean-copy verification coverage

This matrix records three separate abilities: what Hexscope can extract from the original file, what its cleaner can change, and what it can prove about the bytes of the produced copy. A supported parser or cleaner does not by itself mean a finding is verified as removed.

Code references: [summary](../crates/hexscope-core/src/summary.rs), [format dispatch](../crates/hexscope-core/src/document.rs), [cleaner](../crates/hexscope-core/src/clean.rs), [verification contract](../crates/hexscope-core/src/verification.rs), [CLI copy verification](../crates/hexscope-cli/src/main.rs), [Web copy verification](../apps/web/src/worker.ts), [Web QR inspection](../apps/web/src/qrfacts.ts), and the [GitHub Action definition](../action.yml).

## Status key

- **Extracted** means the parser can emit a finding into the core summary for supported structures. The exact facts depend on what the file contains and on parser limits.
- **Cleaned** means `hexscope-core::clean()` has a format-specific transformation. It does not mean every finding in that format is removed.
- **Verified removed** means the shared comparison has explicit coverage for that format, finding kind, and scope, both reads are complete, and the produced bytes are parsed again.
- **Unchecked** means Hexscope cannot make a complete removal claim for that item today. This is the correct result when coverage is missing or a read is incomplete.

`Summary.complete` currently means that the parser emitted no warning or error nodes. It is a useful structural guard, but it is not a general proof that every possible field or nested object in a format was understood.

The shared verifier has only the explicit JPEG removal whitelist shown below. If a matching finding remains in the copy, it can be reported as **present** even when its removal is not covered. If an unmatched source finding would otherwise be called removed but its format or kind is outside the whitelist, it stays **unchecked**. “No removal proof” therefore does not mean every result item has the status `unchecked`.

## Images and video

The image and video summary uses a shared fact vocabulary: location, camera, lens, serial, owner, place, taken, caption, software, original, history, thumbnail, shutter, uptime, linked, screenshot, AI, credentials, and prompt. Each parser exposes only the facts it can read from that format and file; the list is not a promise that every field exists in every format.

| Format | Extracted findings | Cleaner | Shared core proof after cleaning | Additional Web check | Current limit |
| --- | --- | --- | --- | --- | --- |
| JPEG | Supported photo facts and parser warnings/errors | Yes; rewrites supported metadata segments while preserving required image data | **Camera, serial, owner, location** only, at file scope, with complete source and output parses | QR image rescan when a retained QR finding needs checking | Other JPEG kinds have no removal proof: retained findings are present, and absent findings stay unchecked. This applies to parser-visible metadata, not text or marks baked into pixels. |
| PNG | Supported photo facts and parser warnings/errors | Yes; removes supported metadata chunks and trailing data while preserving image structure | None; no PNG finding kind is covered for a removal claim | QR image rescan when applicable | Matching parser findings can still be reported as present; absent findings stay unchecked. |
| HEIF / AVIF | Supported photo facts and parser warnings/errors | Yes; supported EXIF/XMP metadata is cleaned in place | None; no HEIF finding kind is covered for a removal claim | QR image rescan when applicable | Matching parser findings can still be reported as present; absent findings stay unchecked. |
| WebP | Supported photo facts and parser warnings/errors | Yes; supported EXIF/XMP chunks are removed | None; no WebP finding kind is covered for a removal claim | QR image rescan when applicable | Matching parser findings can still be reported as present; absent findings stay unchecked. |
| GIF | Supported photo facts and parser warnings/errors | Yes; supported metadata/comment blocks are cleaned | None; no GIF finding kind is covered for a removal claim | QR image rescan when applicable | Matching parser findings can still be reported as present; absent findings stay unchecked. |
| Video | Supported container photo facts and parser warnings/errors | Yes; metadata is cleaned; a gapped large-file path also exists | None; no video finding kind is covered for a removal claim | No dedicated video QR recheck in the copy-verification path | Matching parser findings can still be reported as present; absent findings stay unchecked. |

## Documents, archives, and other files

| Format | Extracted findings | Cleaner | Shared core proof after cleaning | Additional Web check | Current limit |
| --- | --- | --- | --- | --- | --- |
| PDF | Document facts such as author/title, actions and links, forms, attachments, encryption/updates, plus hidden or covered page content and parser problems | Yes; general clean-copy path. Redaction is a separate operation | None; shared comparison cannot confirm removal of PDF parser findings | Rechecks QR codes in supported PDF pictures and only the text items the user explicitly selected for checking | Selected text and QR are narrower Web-only checks; they do not prove every PDF object or all hidden content was removed. CLI `redact` has no `--verify` mode. |
| ZIP / OOXML Office | Archive and Office facts such as title, author/editor, company, template, comments, tracked/deleted/hidden text, links, embedded files, and photos in supported entries | Yes; rewrites the archive and can clean supported nested entries. Word comments and tracked changes are handled by default; Excel comments and PowerPoint comments/speaker notes are opt-in | None; shared comparison cannot confirm removal of ZIP parser findings | Web can inspect QR codes in up to 30 supported archive images, but does not currently recheck QR codes in a cleaned ZIP copy | Facts are flattened to file scope in the snapshot, so nested-entry identity is not represented in the comparison. |
| Legacy Office / CFB (`office97`) | Supported Word, Excel, and PowerPoint document properties and related findings | Yes, when the `documents` feature is enabled | None; shared comparison cannot confirm removal of these findings | No extra copy-verification path | Matching findings may be reported as present; absent findings stay unchecked. |
| Outlook message / CFB (`msg`) | Supported message headers, attachments, and related document facts | No; the CFB cleaner currently supports legacy Word, Excel, and PowerPoint documents, not Outlook messages | Not applicable; no cleaned copy is produced | Web can scan up to 30 supported attached images for QR codes | No clean-copy verification path for messages. |
| EML | Supported message headers, links, attachment names, and parser problems | No clean-copy implementation | Not applicable; no cleaned copy is produced | Web can scan supported attached images for QR codes during inspection | Inspection support is not cleaning support. |
| WebAssembly | Supported module sections, names, and other parser facts | Yes, when the `documents` feature is enabled | None; shared comparison cannot confirm removal of WASM parser findings | No extra copy-verification path | Matching findings may be reported as present; absent findings stay unchecked. |
| Unrecognised / unsupported | Format hints or plain-text/byte view; no format-specific summary facts | No | Not applicable | No | Hexscope does not claim to inspect or clean an unknown format. |

## What the interfaces prove today

- **Rust core:** owns parsing, cleaning, the shared summary, and the conservative comparison. The current complete-removal contract is exactly `jpeg` + file scope + `camera`, `serial`, `owner`, or `location`.
- **CLI:** `clean --verify` reads each written copy back and reports the shared comparison. It can report matched residuals as present; possible removals outside explicit coverage stay unchecked. `--verify` is available only with `clean`, not `redact`.
- **Web:** uses the shared core comparison on the actual copy bytes, plus independent QR and explicitly selected PDF-text checks. Output verification is bounded at 10 MiB; if it cannot run, the UI reports an unchecked result.
- **GitHub Action:** the composite Action in [`action.yml`](../action.yml) builds the CLI and runs `hexscope check` on the requested files/folders, honoring `fail-on` and writing GitHub workflow annotations. It is an inspection/CI interface, but it does not make clean copies or verify their metadata removal.

## Cross-format limits that affect future work

1. `snapshot()` currently assigns `scope = "file"` to every summary fact. It does not preserve ZIP entry, PDF page/object, or other nested identity; expand scopes before claiming scoped or nested coverage.
2. A matching parser fact is evidence about what that parser recognizes. Verification must also require an explicit format/finding/scope contract and complete source/output reads.
3. Browser-only QR and selected PDF-text checks are separate from the core report. Keep their limits visible and do not describe them as complete format coverage.
4. The browser's document-heavy parser module is loaded separately from its small first module. Feature availability and size/performance budgets need to be evaluated per build path.

## Next coverage step

Use this matrix to define fixtures before expanding the verifier. PNG metadata is a reasonable next small candidate because parsing and cleaning already exist, but it should move ahead of the PDF work only after a fixture audit shows a narrowly provable set of PNG fact kinds. PDF remains the next larger investigation because selected-text redaction, hidden content, and parser completeness need separate evidence.
