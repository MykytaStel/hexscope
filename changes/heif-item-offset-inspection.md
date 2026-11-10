### Changed

- **Inspect more HEIF item locations.** Construction method 2 maps an explicit
  requested range through its 1-based `iloc` reference into a local item, even
  when that item has several explicit `mdat` or `idat` extents. A range must
  stay inside one source extent; ranges crossing a boundary, chained or
  external references, and implicit lengths remain unread. The parser does not
  copy or decode the image payload, and clean-copy behavior is unchanged.
