### Added

- **Make a safe FLAC copy without Vorbis comments or embedded cover art.** Entire picture blocks are removed; other supported metadata blocks and compressed frames stay byte for byte. Files with metadata Hexscope cannot inspect remain read-only. The CLI can verify the copy after writing it; the Web confirms findings on copies within its read-back limit.
- **Clean FLAC files with supported metadata blocks in a batch.** Ogg and FLAC files with uninspected blocks remain read-only.
