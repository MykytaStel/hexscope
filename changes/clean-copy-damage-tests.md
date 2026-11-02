### Added

- **Clean copies are tested against damaged files**: a fuzz target and
  property tests make copies of broken HDR photos, MP3 and FLAC files and
  require every copy to read back as the same kind of file, or no copy at all.
