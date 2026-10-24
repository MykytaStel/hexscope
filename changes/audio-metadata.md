### Added

- **Audio tag inspection:** see titles, artists, albums, dates, track details and other tags in MP3, FLAC, Ogg Vorbis and Opus files. The read-only parser keeps each finding linked to source bytes and says when metadata could not be fully inspected; it does not decode or play audio.

### Fixed

- **Partial ID3 reads are explicit:** unsupported frames and reserved flags no longer make an MP3 look completely inspected, and later supported tags remain visible.
