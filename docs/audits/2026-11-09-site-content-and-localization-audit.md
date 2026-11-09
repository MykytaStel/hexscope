# Site behavior, file display and localization audit

- **Date:** 2026-11-09
- **Checkout:** `feat/0-16-human-readable-reports`
- **Scope:** source and product-doc review of the file workspace, content labels,
  embedded-file search, Ukrainian copy and known limitations. This was not a
  full parser audit or a manual device/browser review.

## Corrections made

1. The landing page said **every byte is explained**. That overpromised for
   unsupported formats, intentionally unparsed payloads and unread movie
   media. Copy now says the site shows parts it recognizes, where their bytes
   sit and what the format says about them.
2. The button **Find files inside** implied a structural guarantee. The action
   actually searches available bytes for a bounded set of known signatures.
   It is now named **Search for embedded files**, reports no matches only for
   the bytes it searched, and labels matches as possible with a note that a
   signature can occur incidentally.
3. The Ukrainian copy for three in-place cleaning operations used singular
   verbs with plural subjects. The sentences now agree grammatically.
4. The Ukrainian AVIS description called sample timing a frame table. It now
   describes timing data without asserting that every temporal unit is one
   frame.
5. `known-issues.md` said HEIF sequences were read only as boxes. It now records
   the 0.16 item graph and AVIS timing view, plus the still-unimplemented item
   reconstruction and sequence decoding.
6. The workspace label **Map** was ambiguous outside the file context. It is
   now **File map** / **Мапа файла**.
7. The initial pass found that signature scans ran synchronously from the
   drawer, including the hidden-byte pass used while drawing the overview. The
   release candidate fixes that risk: scans yield and can be cancelled; the
   automatic hidden-data scan is capped at 16 MiB, while manual media reads
   stay local and stop at the first 256 MiB of the file. The search has one
   total byte budget and a 100-match limit. These bounds were checked in code;
   phone responsiveness was not measured on a physical device.

## What the site currently exposes

- **Overview:** file verdict, detected privacy findings, suggested actions,
  previews and supported clean-copy/repair actions.
- **Metadata:** grouped facts the parser recognized. It does not imply that
  every metadata namespace or field in the format is understood.
- **Content:** supported text, hidden text, attached files, QR content and
  image/page evidence, depending on format. Image OCR is not performed.
- **File map:** byte-order composition by role; selecting a segment leads to
  the relevant byte range. It is an overview, not an independent decode of
  compressed data.
- **Structure and Bytes:** a virtualized, expandable parser tree plus a
  byte-oriented view. Selecting a node links its explanation and range to the
  corresponding bytes. Large movie media is read on demand.
- **ZIP Files and Archive Trace:** names, directory metadata and bounded
  inspection of local header, compressed data and unpacked output.
- **HEIF:** item roles and references plus AVIS timing. Construction-method-2
  bytes are not assembled; AV1 image samples are not decoded or played.

The source tree already represents more than the overview can show: parser
nodes, ranges, explanations, warnings, decoded facts and some rendered pixels.
The main gap is a consistent way to tell a person whether a selected part is
decoded, only structurally identified, not read yet, or unsupported.

## Next work, in priority order

### P0 — show the evidence state for each part

Give selected parts an explicit state such as **decoded**, **identified, not
decoded**, **not read yet**, **partially read**, or **unsupported**. Include the
byte range and a clear next action when one exists. This would prevent an empty
value from looking like an empty payload and make limitations visible without
requiring a separate known-issues page.

### P1 — decode useful views on top of the byte tree

- For HEIF, connect item extents to their source bytes and expose more item
  properties; evaluate grid reconstruction as a separate, fixture-backed step.
- For PDF, make page → content stream → font/image resources easier to follow,
  retaining the existing incomplete-page warnings and clean-copy refusal.
- For images, unify pixel/block previews with the tree so a person can move
  from a visible region to its source bytes and back.
- For video, make track/sample timing, chunk offsets and not-yet-read media
  ranges legible together. Keep the media read-on-demand budget explicit.
- For nested containers, preserve a clear breadcrumb and parent/child return
  path when an attachment or archive entry is opened.
- Move signature scanning off the render path or make it cancellable and
  incremental. Keep the byte and result bounds visible; don't replace the
  current cap with an unbounded whole-file scan.

These are candidate investigations, not claims that the underlying parsers
already provide complete evidence. Each should begin with a format-specific
fixture and a written boundary for what remains unread or undecoded. The
signature-search item was implemented in the 0.16 release candidate with the
bounds above.

## Review limits

The audit itself was source-led. Follow-up release checks include Web unit
tests, the production build, selected Playwright flows and Rust release gates;
they do not substitute for physical iPhone/Android, screen-reader, or dedicated
parser-fuzz evidence. The embedded-file action remains a signature heuristic,
not attachment verification or a malware scan.
