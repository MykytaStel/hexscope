### Changed

- Stream embedded signature searches in large movies using bounded 1 MiB windows instead of a full prefix allocation.
- Share header and end-marker validation across contiguous and Blob-backed sources.
- Preserve original offsets, scan limits, and cancellation; reject short reads and truncated signature headers.
