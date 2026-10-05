# Real scan evaluation

The reproducible corpus contains **four NASA Apollo 11 flight-film scans**, at
published medium resolution. These are positive, post-processed renditions of
70mm Hasselblad photographs, not unprocessed orange-mask negatives. NASA's
[image catalog](https://images.nasa.gov/) supplies the files; the
[Apollo image library](https://www.nasa.gov/wp-content/uploads/static/history/alsj/a11/a11_eva_thumbs.html)
describes their film provenance. Source URLs, NASA media-use guidance, polarity,
dimensions and SHA-256 hashes are recorded in the
[manifest](../crates/hexscope-raster/tests/fixtures/real-scans/manifest.json).

## Measured results

| Check | Result | What it proves |
| --- | --- | --- |
| Complete frame false positives | 0 of 4 | No full crop suggestion on these four published positives. |
| Perforation false positives | 0 of 4 | No repeated film-hole claim on these four images. |
| Neutral positive RGB difference | maximum mean absolute error 0.10752 on a 0–255 channel scale | The native TIFF rendition closely preserves decoded JPEG pixels. JPEG decoders differ slightly; this is not a colorimetric measurement. |
| Synthetic RGB16 TIFF | exact sample equality, including 300 and 32001 | The core/codec/native and browser paths preserve sub-byte precision with neutral settings. |
| Raw color-negative recovery | not measured | No calibrated original-negative/target-color pairs are available in this corpus. |

Detailed per-file observations are in [the JSON results](film-evaluation-results.json).
The four images are from one mission and one family of photographic equipment.
They cannot establish general classifier accuracy, frame recall, grain
authenticity or scanner color calibration. Narrow edge artifacts in these
published copies are not labeled as a complete film-frame boundary.

## Reproduce

Requires Python with Pillow and Playwright, plus the standard project toolchain.
The checked-in samples mean the evaluation performs no network requests.

```sh
cargo build -p hexscope-cli
pnpm build
pnpm --filter web preview --host 127.0.0.1 --port 4176
# In a second terminal:
python3 scripts/evaluate-film.py --base-url http://127.0.0.1:4176
```

The script verifies input hashes, uses the native shared renderer to create
neutral TIFF copies, compares decoded pixels and invokes the production browser
worker's frame heuristic. Original files are preserved.

## Next research gate

Before OCR or a color-recovery claim, acquire consented/licensed raw 8/16-bit
color negatives from several scanners and film stocks, with manual frame and
edge-text labels, plus color targets or expert reference renditions. Measure
frame precision/recall, OCR character error and color difference independently.
Recipes currently assume sRGB-encoded samples; TIFF ICC profiles are reported
but not applied. Linear scanner data requires a different, explicitly versioned
algorithm rather than silently reusing this recipe.
