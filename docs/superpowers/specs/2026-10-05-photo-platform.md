# Photo platform: verified copies, rolls and evidence

## Intended user

A person preparing files to send, including photographers digitizing a roll. Work remains local. The original is never replaced without an explicit existing CLI option.

## Deliverables

1. PNG and WebP share JPEG's conservative camera/serial/owner/location verification contract. Evidence uses real parsed metadata and actual cleaned bytes; malformed input, other facts and scopes stay unchecked. Image payloads stay unchanged.
2. Folder cleaning has per-file results, an aggregate value-free JSON receipt and no silent basename collisions. Web ZIPs include a receipt after readback. Unknown or damaged files cannot make a batch look safe.
3. Rust owns the density renderer for 8-bit and 16-bit rasters. CLI and web consume the same validated version-1 recipe. A roll tool applies one recipe serially and exports copies and a receipt. Native CLI and an optional web WASM codec handle TIFF; 16-bit TIFF output retains 16-bit processing. Source pixels are assumed sRGB-encoded: embedded profiles are reported, not silently described as calibrated archival color. Unsupported, oversized or multi-image TIFF inputs fail explicitly.
4. A reproducible evaluation records licensed real digitized scans, hashes, frame decisions and false positives separately from synthetic algorithm checks. Digitized glass negatives are labeled as such. No accuracy or color-recovery claim from this small corpus; OCR requires a later labeled evaluation.
5. A shared bounded photo-mosaic algorithm identifies repeated serials, nearby stored GPS points and dated-file ordering across a batch. It does not infer identity, home, route or authentic film origin. Export contains category counts and file indexes, never raw serials, coordinates, dates or filenames.

## Bounds and contracts

- Core has no image-codec or network dependency. Optional raster codec module is loaded only for the roll/TIFF tool.
- Sources: 50 MiB encoded, 30,000 px per side, 120 MP, decoder allocation capped at 256 MiB. Processing: 12 MP; normal web sharing output additionally caps the side at 4096. Roll and mosaic at most 1,000 items.
- Recipe schema/algorithm are unchanged. Crop uses displayed orientation; neutral positive conversion is identity at its bit depth. Negative conversion is a user-selected rendition, never automatic origin classification.
- Batch failures do not discard successful copies. Receipts distinguish written, not-created, removed, present and unchecked. Existing CLI NDJSON remains compatible.
- Production shipping requires targeted regressions, complete repo gates, an independent branch review, hosted CI and a live browser check. Merge/deploy authorization is already provided by the user.

## Non-goals

Film-stock recognition, grain authentication, geographic identity inference, scanner color calibration and OCR are not represented as proven features.
