# Film photo lab

Hexscope can prepare an explicit new sharing rendition of a JPEG, PNG or WebP
scan, locally in the browser. Open a photo, then **Prepare a film photo to
share** below the metadata findings. The landing page offers a synthetic
negative to demonstrate the workflow without a private photograph.

1. Choose an already developed positive, color negative, or monochrome negative.
2. Review the original and its retained crop. Adjust the four margins to exclude
   edge lettering or unwanted border content. A detected frame is only a crop
   suggestion and is applied by a separate button.
3. For a negative, sample clear unexposed film with the pointer, enter a sample
   position in percentages, or enter RGB values. Automatic base estimation is
   approximate. Adjust exposure and contrast while comparing the preview.
4. Create a JPEG copy. Hexscope re-reads the actual output bytes with its shared
   Rust/WASM parser and scans output pixels for QR codes and border patterns.
   The report distinguishes detected metadata from visible image content. A
   failed output check is reported; a successfully produced copy remains available.
5. Download the copy or save/load its versioned JSON recipe to reuse adjustments
   on other scans. Recipes contain only whitelisted controls, no filename,
   original metadata, source fingerprint or image bytes. Rendering may differ
   across browsers; recipes are not a byte-identical reproducibility guarantee.

## Algorithms and limits

For negatives, `srgb-density-v1` linearizes sRGB channel values, normalizes
transmission by the sampled film base, computes `-ln(transmission / base)`,
and derives shared tone bounds from the cropped raster's 1st/99th density
percentiles. Shared bounds preserve relative channel calibration. Exposure and
contrast are applied through 256-entry channel lookup tables; the monochrome
mode produces equal RGB channels. Sampling uses median patches, with a bounded
histogram estimate as the automatic fallback.

Film-base sampling and density conversion are established approaches, described
in [darktable's negadoctor manual](https://docs.darktable.org/usermanual/5.6/en/module-reference/processing-modules/negadoctor/)
and [RawTherapee's film-negative documentation](https://rawpedia.rawtherapee.com/Film_Negative).
This implementation is an adjustable 8-bit sharing rendition. Scans may already
have tone curves, clipping or a color cast; it cannot recover clipped data or
claim calibrated archival colors. RAW, TIFF, 16-bit color and edge-text OCR are
not supported by this lab.

The preview is limited to 1200 pixels on the longest side. Export is limited to
4096 pixels and 12 megapixels, including the border before cropping. Source
limits are 50 MiB, 120 megapixels and 30,000 pixels on either side. The rendered
orientation is baked into the new JPEG. Export re-encodes pixels with JPEG
quality 0.94; it is a lossy rendition, so preserve the master. Output parsing
uses the existing 10 MiB verification limit. Canvas encoding can add fresh
format/color/resolution metadata; the report describes supported findings
detected by the parser, not a promise that every metadata byte is absent.

Rendering runs in a worker with one active raster job and only the newest
pending request. Disposal releases object URLs and stale results cannot replace
the current view or calibration. No photographs, settings or reports are
uploaded or retained in storage by the lab.

Repeated openings and frame boundaries remain visual heuristics. Digital
borders can look alike. The synthetic sample and regression fixtures exercise
geometry, calibration, rotation and failure handling; they do not establish
classifier precision on real photographs. A consented/lawfully reusable labeled
corpus and false-positive evaluation remain a gate before any provenance claim.

The CLI continues to inspect/clean digital metadata with the same core parser;
the browser supplies raster codecs for the interactive lab. This change does
not add a CLI pixel-conversion command.

## Rolls and TIFF16

The production renderer now uses the shared Rust core. The optional [roll tool](photo-platform.md) applies saved recipes to multiple frames in the browser or CLI and supports 16-bit TIFF sources and outputs. Color-space and decoder limits are explicit.
