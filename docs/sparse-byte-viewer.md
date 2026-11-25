# Sparse movie byte viewer

The worker transfers the compact movie bytes, omitted ranges and logical size. FileModel validates the size against the source Blob, maps original offsets to compact bytes and loads deferred media in 64 KiB pages. The per-model cache retains at most 64 pages (4 MiB), with at most four in-flight page reads. Missing data is not represented as zero. Small files retain their existing contiguous representation.

Search continues to search non-media bytes, comparisons explicitly skip omitted media, and embedded signature checks inspect original-offset ranges. An explicit embedded-file search may still allocate a temporary prefix of up to 256 MiB (the existing search ceiling); it does not populate the viewer cache. Saving a found range uses a Blob slice; opening one is bounded to 128 MiB. This explicit search buffer remains a separate optimization target.

The 4 MiB limit is a cache payload limit, not a measured total browser RSS. Compact metadata, parse trees, WASM linear memory, temporary search buffers and browser-managed Blob storage are additional costs. Multiple models each own a cache. The overall browser admission limit and WASM download budgets are unchanged. Run browser end-to-end tests and measure memory on actual devices before claiming a physical RAM saving or a new supported file-size limit.
