### Added

- **Clean copies are tested against damaged files**: a fuzz target and
  property tests make copies of broken HDR photos, MP3 and FLAC files and
  require every copy to read back as the same kind of file, or no copy at all.

### Fixed

- **A clean copy that would open as another kind of file is refused.**
  Removing an RTF's information group could bring a "%PDF-" written in its
  text into the place PDF readers look, so the copy opened as a PDF; now no
  copy is made, with the reason, in English and Ukrainian.
