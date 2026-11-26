### Changed

- Validate external archive ranges and exact read lengths before handing compressed bytes to WASM.
- Share a 128 MiB in-flight compressed-read budget across extraction and player selection; release the reservation on success, cancellation or failure.
