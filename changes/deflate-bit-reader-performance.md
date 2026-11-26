### Changed

- Read DEFLATE extra bits in byte-sized chunks instead of bit-by-bit iteration.
- Index the Huffman prefix table in the stream's low-first bit order, moving bit reversal from decoding to table construction.
- Preserve exact traced code values, bit offsets, output budgets and malformed-stream errors.
