# Film renderer sampling and parity

**Status:** Proposed for review
**Date:** 2026-10-21

## Goal

Make the bounded film-renderer histogram less vulnerable to periodic sampling aliasing and keep one authoritative production implementation.

## Behavior

- Keep the existing ceiling of 131,072 samples and 12,000,000 rendered pixels.
- Replace fixed linear stepping for base-color, tone-bound, and clipping estimates with deterministic stratified sampling over the whole raster. Reuse identical sample positions across passes and keep output deterministic for the same dimensions and pixels.
- Skip base-color histogram work when the caller supplied a calibrated base.
- Keep Rust `hexscope-core::film` as the production algorithm used by native and Web/WASM paths.
- Remove the unused duplicate TypeScript pixel renderer after moving its behavioral coverage to Rust tests. Keep TypeScript helpers that the UI still needs for crop geometry, sampling a base, and settings.
- Continue to describe output as an adjustable sharing rendition, not calibrated archival color recovery or proof of film origin.

## Acceptance

- Unit tests compare sampled histograms with full-raster results for periodic stripes, flat fields, gradients, very wide/tall rasters, and sizes below the cap.
- The sampler always stays in bounds, covers the raster's strata, is deterministic, and does not exceed the cap.
- Rust tests cover neutral positive output, mono-negative output, color-base calibration, clipping bounds, 8-bit and 16-bit inputs, and invalid dimensions/settings.
- The checked-in four Apollo positives remain documented as a narrow smoke corpus; no classifier accuracy or negative-color recovery claim is added.

## Out of scope

New film recognition, OCR, calibrated color recovery, and expansion of the real-scan corpus without lawful, labeled examples.
