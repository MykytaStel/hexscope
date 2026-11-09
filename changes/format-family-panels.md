### Added

- **Read media and code structure by family.** Video files group tracks and
  stored timing; audio files group tags, pictures, stream blocks and incomplete
  parts; SVG groups scripts, references and embedded images; email groups
  sender and route clues, authentication claims, links and attachments; WASM
  shows imports, exports and debug references. These views do not decode or
  play media, execute code, verify email claims or visit links.

### Fixed

- **Describe unknown formats accurately.** Hexscope no longer implies that an
  unrecognized file has no EXIF metadata; it says structural metadata could
  not be checked and does not offer a clean copy it cannot safely rewrite.
