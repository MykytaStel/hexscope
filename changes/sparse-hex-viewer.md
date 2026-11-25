### Changed

- Large movie inspection now transfers compact metadata instead of allocating a full logical file buffer.
- The hex viewer loads deferred media through a bounded page cache; unknown bytes remain visibly unknown.
- Search, comparison, byte inspection, size reporting and signature discovery use original offsets.
