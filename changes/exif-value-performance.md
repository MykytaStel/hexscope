### Changed

- Borrow bounded EXIF value bytes and decode scalar numbers without temporary vectors.
- Share numeric and rational decoding across metadata display and fact collection.
- Separate TIFF traversal and rendering for measurable code-size attribution; preserve full-value range checks.
