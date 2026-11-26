### Fixed

- Reject out-of-width and overlong WASM LEB128 encodings while preserving valid padded unsigned and signed operands.
- Check function-index arithmetic and retain damaged-section reporting.
