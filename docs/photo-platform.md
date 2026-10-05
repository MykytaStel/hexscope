# Photo tools: core, CLI and browser

## Verified metadata copies

JPEG, PNG and WebP now share the same four parser-backed removal capabilities:
camera, serial, owner and GPS location. PDF has its separate existing matrix.
See [coverage and result meanings](verification-coverage.md); other findings may
remain **Not checked** even when a cleaner removed their containers.

```sh
hexscope clean --verify --json photo.png
hexscope batch --out clean-copies ./photos
```

`batch` writes uniquely indexed copies and `hexscope-report.json`. The receipt
contains file indexes, counts and core verification reasons, without filenames
or private metadata values. A failed file does not discard successful copies.
Exit 1 means a failure, remaining finding or incomplete verification; exit 2
means invalid options or failed folder preflight. Use a fresh output folder for
each run. Input symlinks are not followed; at most 1000 files, 50 MiB each.

In the browser, the batch ZIP includes the same receipt shape. Every eligible
copy gets a parser readback up to 10 MiB. Larger copies remain unchecked. The
unknown/damaged-file state cannot imply that an entire batch is ready to send.
Visible image content is outside these metadata removal checks.

## Film rolls

Open **Film roll · JPEG / TIFF** on the start screen, or **Film roll** in a batch.
Choose scans, select their polarity, optionally import a calibrated recipe from
the single-frame lab, and download the resulting ZIP. The first successful
frame supplies a shared estimated film base if the recipe has none.

```sh
hexscope film --recipe roll.json --out developed --format jpeg ./scans
hexscope film --recipe roll.json --out developed16 --format tiff16 ./scans
```

The CLI and production web renderer use the same Rust `srgb-density-v1`
algorithm. The recipe's schema remains `hexscope.film-recipe`, version 1.
RGBA8 and RGBA16 processing live in the codec-free core. Codecs live in an
optional crate and browser module, loaded only for photo tools.

- TIFF supports one image, grayscale/RGB with 8/16-bit integer samples,
  supported uncompressed/Deflate/LZW/JPEG codec paths and EXIF orientation.
  Unsupported layouts, oversized decodes and multipage inputs fail explicitly.
- TIFF16 preserves 16-bit processing for 16-bit TIFF sources in both CLI/web;
  native PNG16 is also processed at 16 bits. Browser JPEG/PNG/WebP decoding uses
  8-bit pixels, even if the original PNG has 16-bit samples. A TIFF container
  cannot restore precision already discarded by that decode.
- Outputs cap at 12 MP. Sharing JPEG also caps the side at 4096. Sources cap at
  50 MiB, 30,000 px per side and 120 MP, with additional codec allocation bounds.
- Samples are assumed sRGB-encoded. TIFF ICC profiles are detected and reported,
  but not applied. Browser decoders may perform their own color management for
  other formats. These are adjustable renditions, without a calibrated color
  recovery claim. Source metadata is not copied by the encoders; the roll
  receipt still marks metadata verification **not checked**, rather than
  extending the clean-copy matrix to TIFF or certifying visible content.

The ZIP/native report records dimensions, source/output depth, resizing,
SHA-256 of actual output bytes and explicit verification limits. Originals are
preserved and output names are indexed. Processing is serial; closing the
browser dialog cancels subsequent jobs and discards that dialog's retained
output. There is no upload or saved private recipe history.

## Photo privacy mosaic

```sh
hexscope mosaic ./photos
hexscope mosaic --json ./photos > photo-mosaic.json
```

The browser's batch **Photo privacy mosaic** uses the same bounded Rust logic.
It groups matching stored serial values and GPS points within 100m of the first
file in each group. It keeps unspecified-zone dates separate from dates that
can be normalized to UTC. Invalid dates/coordinates are omitted; unparsed files
are counted separately. GPS distance uses spherical coordinates, including the
date line and poles. A chain of nearby points cannot grow one group indefinitely.

Local displays map indexes to input names. Export omits names, serial values,
coordinates and raw dates. Matching stored facts are observations; they do not
prove identity, a home, a route, scanner provenance or authentic film origin.

## Research evidence

[Real-scan evaluation](film-evaluation.md) includes provenance, hashes,
measurements and the separate research gate for raw negatives and OCR.
