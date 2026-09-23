# ZIP, Part 2: The Player for Any Entry — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Any deflated entry of a ZIP can be played in the DEFLATE player, with the reading head on the archive's own bytes.

**Architecture:** The bridge's stream stops assuming zlib: it keeps a header and trailer length (2 and 4 for PNG, 0 and 0 for a ZIP entry). `select_entry(i)` decodes an entry into the stream, output and checkpoints the player already reads. The page learns which entry a node belongs to, shows an entry panel with a play button, and hands the stream's header length to `bitsToFile`.

Spec: `docs/superpowers/specs/2026-09-23-zip-design.md`, "Bridge and worker" and "Web".

## Global Constraints

As Part 1. Additionally: PNG behaviour is unchanged (the existing bridge tests stay green unmodified), and nothing is decompressed until an entry is played.

## Tasks

### Task 1: Bridge
- [ ] Failing tests (`crates/hexscope-wasm`): a built ZIP of two entries (stored + deflate) gives `entries` = 2 × 6 numbers with `playable` 0 then 1; `select_entry(1)` returns true, then `steps` rebuild the entry's data byte for byte, `segments` is the entry's data range, `streamHeader` is 0; `select_entry(0)` (stored) and an out-of-range index return false; a PNG's `streamHeader` is 2.
- [ ] `Parsed` gains `header_len`, `trailer_len`, `zip: Option<ZipState { source, entries }>`; `body()` uses the lengths; getters `entries`, `streamHeader`; `select_entry`. The ZIP test builder is `#[cfg(test)]` in the core, so the bridge tests build their archive with `flate2` directly (a local helper).
- [ ] Commit: `feat(wasm): play any deflated ZIP entry`.

### Task 2: Page
- [ ] `ParsedFile` gains `entries` and `streamHeader`; `FileModel` gains `entryOf(id)`, `entry(i)`, `setStream(segments, trace, bytes)`, and `bitsToFile` uses `streamHeader`.
- [ ] Worker: `selectEntry` request → `{ type: "stream", playable, trace, segments, idatBytes }`.
- [ ] Drawer: for a node inside an entry, an "Entry" group (name, method, sizes, ratio) with *Watch it decompress*, disabled with a reason when the entry cannot be played. File panel's DEFLATE stats only for PNG.
- [ ] Main: the top play button and **P** follow the selected entry for ZIP; playing an entry closes any open player, selects the stream, sets it on the model and opens the player.
- [ ] Browser: play `word/document.xml` in the sample, step to a back-reference, check the reading head sits inside that entry's data; the photo (stored) shows the disabled reason.
- [ ] Commit: `feat(web): play a ZIP entry`.
