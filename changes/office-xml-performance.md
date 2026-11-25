### Changed

- Reuse valid UTF-8 decompression buffers for Office XML metadata and reuse the Word document body for link and tracked-change analysis.
- Count comment tags without retaining a vector of every matching tag, and reuse the tag search prefix within list scans.
