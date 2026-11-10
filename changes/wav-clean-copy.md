### Added

- **Make a clean copy of a fully inspected WAV recording.** The copy removes
  complete WAVE INFO and Broadcast Wave metadata chunks while preserving the
  format and audio data byte for byte. Removing Broadcast Wave also removes
  its timing, loudness values and coding history; unsupported or incomplete
  chunks keep the file read-only.
