# Office XML performance

Named Documents profiling identified zip::office::document_facts at 11884 raw body bytes. The candidate shares XML loading and avoids a second decompression of word/document.xml when inspecting links. Findings retain their original order, source node, text and admission limits. Valid UTF-8 uses the already-owned Vec; malformed bytes use exactly the previous lossy conversion. Comment counting avoids a per-tag pointer vector. The tag recognizer is unchanged, including its existing handling of malformed and lookalike tags.

Tests cover allocation identity, malformed UTF-8 parity and 200000 matching tags. Existing Office, ZIP, corpus translation and clean-copy evidence tests remain mandatory. Rebuild all WASM families and compare gzip and latency before claiming improvements. No format support or limits changed.
