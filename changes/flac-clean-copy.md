### Added

- **Make a safe FLAC copy without Vorbis comments.** The compressed frames and other recognized metadata blocks stay byte for byte; files with metadata Hexscope cannot inspect remain read-only. The CLI can verify the copy after writing it; the Web confirms findings on copies within its read-back limit.
- **Clean fully inspected FLAC files in a batch.** Ogg and incomplete FLAC files remain read-only.
