### Added
- **Make an MP3 copy without its edge metadata tags.** Leading ID3v2 and trailing APE, ID3v1 Plus and ID3v1 blocks are removed; compressed MPEG audio bytes stay unchanged. The copy is read back, and findings are confirmed removed only when both metadata reads are complete.
- **Clean tagged MP3 files in a batch.** Only formats with a safe clean-copy path are selectable.
