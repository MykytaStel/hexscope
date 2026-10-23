### Added

- **A clean-copy receipt you can check offline.** After the copy has been read back, save an optional record of what its verification found. It contains no file name, path or metadata values; the CLI can check whether the copy's bytes still match its SHA-256 fingerprint. A match confirms the bytes only, not who made the receipt or whether the file is safe. Batch fingerprints are opt-in; the existing batch receipt stays unchanged by default.

### Changed

- **SHA-256 can be calculated with fixed memory.** The CLI hashes large copies in bounded chunks instead of holding a second copy of the file in memory. The browser creates a single-copy fingerprint only when a receipt is requested and the copy is within its existing 10 MiB verification limit.
- **Optional comparisons load on demand.** The comparison tool is downloaded only when a second file is selected, reducing the analyzer's initial compressed bundle by about 2.5 KiB.
- **Unavailable batch fingerprints are explicit.** If a clean copy was written but its requested fingerprint could not be computed, the receipt keeps the copy and records why the fingerprint is absent.

### Fixed

- **ZIP extraction now checks the archive's declared output size.** An entry is rejected when its actual decompressed length disagrees with the central directory, even if its CRC happens to match.
- **Web copy verification recognizes changed values of the same kind.** It no longer reports a replaced metadata value as both removed and unexpectedly present; exact matches are reserved before changed values are paired.
- **Batch copies keep their original file indexes when items repeat.** Naming collisions and receipt rows no longer rely on repeatedly searching the input list.
