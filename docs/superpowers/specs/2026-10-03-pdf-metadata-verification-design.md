# PDF metadata verification design

**Status:** Approved for implementation on 2026-10-03.

## Goal

Extend shared clean-copy verification to report a narrowly proven set of PDF Info and XMP metadata findings as removed after Hexscope reads the actual cleaned bytes back.

## Accepted coverage

For complete PDF parses and file scope only, shared verification may confirm removal of exactly these finding kinds:

`author`, `title`, `subject`, `keywords`, `application`, `producer`, `created`, `modified`, and `history`.

This is supported by the PDF cleaner removing the document Info dictionary and emptying XMP streams, and by the parser extracting these finding kinds from those sources, including object streams.

The PDF summary is complete for verification only when every declared Info/XMP source needed to extract these facts is readable. A declared Info reference that cannot be resolved, or an XMP stream with a missing stream body, unsupported filter, or exhausted decode budget, must leave findings unchecked. This metadata-source state is separate from structural parse problems, and it must not prevent the cleaner from writing an otherwise safe copy.

## Explicit non-coverage

PDF findings `updates`, `earlier`, hidden or covered text, comments, forms, attachments, links, and metadata embedded in photos are not covered by this change. They remain unchecked unless independently observed in the output, in which case the existing comparison may report them present. Existing Web QR rescanning and user-selected PDF text checks remain supplemental and separate from the shared core report.

Do not change the verifier's status rules: an exact residual match remains present, same-kind changed values remain present, and unsupported or incomplete cases remain unchecked. Do not add values or scopes to reports.

## Acceptance

1. A real `compact.pdf` clean-and-reparse comparison reports all its supported Info/XMP findings removed, with no present or unchecked findings.
2. A real `report.pdf` comparison reports covered Info/XMP findings removed while `updates` and `earlier` remain unchecked.
3. CLI `clean --verify` returns success for the compact fixture and a nonzero verification result for the report fixture, without exposing finding values.
4. Non-file scopes, other formats, unsupported kinds, and incomplete parses cannot be upgraded to removed.
5. A PDF with an unreadable declared XMP stream remains incomplete and its known metadata findings remain unchecked, while cleaning can still produce output.
6. A real Info dictionary containing Subject and Keywords is parsed and those values are confirmed removed after cleaning/reparsing.
7. No Web UI change is needed; the WebAssembly client inherits the shared core coverage and completeness decision.
