# DEFLATE bit reader candidate

The existing decoder already bulk-copies overlapping LZ77 matches and has a nine-bit Huffman lookup. This slice targets two remaining costs: a per-bit loop for extra fields and reversal of each nine-bit prefix. The new bounded reader consumes at most five bytes per read; the lookup table is populated in stream order while retaining canonical code metadata for explanation and checkpoints.

No changes to event ordering, output limits, zlib framing or checksum verification. Exhaustive offset/width tests compare a bit-at-a-time reference, and all 512 fixed-table prefixes at all eight alignments compare against the canonical fallback. Existing trace, checkpoint, corrupt-stream and roundtrip tests remain mandatory.

Measure Criterion parse_png, png_components and the existing Node WASM reports on identical builds and fixtures. Table construction may become more expensive (strided writes rather than contiguous fills); compare short and long streams before accepting. No measured speed or gzip gain claimed, no budget increases.
