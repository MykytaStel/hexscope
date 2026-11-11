# Local publication tools

Open **Before sharing: plan and tools** in a file's overview. Its optional code
loads when opened; the file and finding values remain local.

## Plan and rules

The plan lists only observed finding kinds whose format/scope is covered by
Core's removal contract after a complete source read. It does not prove that a
writer will accept the file or that an output removed anything. Keep the actual
copy result and read-back report as the authority.

Profiles specify blocked finding codes and whether a complete structural read
is required. Export/import the same JSON in Web and CLI:

```sh
hexscope plan photos/
hexscope policy hexscope-policy.json photos/
```

Rules evaluate Core findings. Unknown codes and supplemental QR rules remain
unchecked even when a profile does not require a complete read; OCR and visible
content are outside the evaluator.

These reports contain indexes and codes, without original names or values.
A profile passing is not a malware scan or safety certification. Unknown files
fail a profile requiring completeness even if their fallback byte map is whole.

## Evidence and comparison

Origin paths follow actual tree parents, bounded to 32 nodes and 80 findings.
Differing fields inspect up to 100,000 nodes, retain up to 16 occurrences per
label and show at most 20 groups. Different stored dates/authors/device fields
can be normal; the panel does not infer forgery or authorship.

Comparison reports finding kinds whose values changed even when both files
still contain the same kind. An absent finding in a partial read is not proof
of removal. Byte differences do not prove pixel/audio equivalence.

## Copy packages

After copy verification, save a ZIP containing the copy, JSON receipt and a
small HTML summary. Reports contain counts and codes, without metadata values
or source names. The copy's visible content remains its owner's responsibility.
Batch copy ZIPs also include the summary. The existing 10 MiB fingerprint and
Web read-back limits remain; unchecked outcomes stay unchecked.

## Experimental OCR

The image black-out editor offers English or Ukrainian OCR. A PDF page preview
offers OCR on its **first extractable picture**, not a complete PDF rendering.
Text represented by unsupported image encodings, vector outlines or other
page content may be absent. An OCR result does not change parser completeness.

The engine and models are generated from locked npm dependencies into the
versioned `ocr-v7-data1/` directory. They are loaded only after a direct request,
from the site's own origin; images are never uploaded. The language downloads
are approximately 10 MiB English and 5 MiB Ukrainian, plus the engine. These
assets are excluded from first-visit service-worker precaching; once fetched,
the normal asset cache can retain them for offline use. OCR result text is not
saved to localStorage or an OCR database.

Inputs are preview images of at most four million pixels; each OCR operation
has a 60-second budget and returns at most 200 word suggestions. Low-confidence
words are omitted, so important words can be missed. Preview every area and add
it manually. No automatic bulk redaction is performed. Image boxes support
keyboard editing, Undo and Redo; history is bounded to 80 changes.

For PDF images, all four corners are transformed through the actual image
matrix into a conservative page rectangle. Rotated/skewed content can require
a larger rectangle. Unsupported PDF rendering and clipping remain limits.

## Damage laboratory and limits

The laboratory changes only the shipped synthetic PNG's IHDR CRC and compares
its parser problems before/after in disposable workers. It never receives the
person's input. This is a format demonstration, not a repair guarantee.

Whole-file reads are limited to 128 MiB Web / 512 MiB CLI, with bounded reads
even if a CLI file grows. Supported large movies keep the sliced path. A tree
holds 500,000 ordinary nodes plus one limit error. This bounds tree storage,
not every allocation; format-specific decoded-byte limits remain necessary.
Heavy Web operations have a 60-second deadline. A cancelled parse gets 500 ms
to acknowledge cancellation before an otherwise idle worker is discarded. A
newer pending operation keeps its own deadline and is never discarded by an old
cancellation timer; the current file
may need to reopen from local bytes.

No physical iPhone/Android, HDR screen, VoiceOver/TalkBack, OCR recall or
real-world false-positive benchmark is claimed by these changes. Automated
engine/device profiles cannot replace those checks.
