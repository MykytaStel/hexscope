### Changed

- **Inspect more HEIF item locations.** Construction method 2 now exposes an
  extent when its 1-based `iloc` reference points to a local item with one
  explicit extent contained in this file's `mdat` or `idat` data. Chained
  locations, external references, multiple source extents and implicit lengths
  remain unread. The parser does not copy or decode the image payload, and
  clean-copy behavior is unchanged.
