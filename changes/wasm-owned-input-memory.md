### Changed

- **Large files use less memory while they are inspected.** The parser now reuses its owned input buffer for JPEG block analysis, ZIP navigation, and attachment extraction instead of making another full-size copy inside WebAssembly.
