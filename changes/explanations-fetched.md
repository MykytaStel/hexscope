### Changed

- **A file's answer arrives sooner**: the parser every visitor downloads
  first is 15 KB smaller for photos and movies (148 → 133 KB gzipped), 17 KB
  for PDFs and 21 KB for Office files and email. The sentence that says what
  each part of a file is, and where its format defines it, now comes in a
  file of its own once the file is open, for the structure tree, the
  inspector and the structure JSON.

### Fixed

- **The structure tree's tooltips were in English on the Ukrainian page**:
  what each part is now reads as the inspector says it.
