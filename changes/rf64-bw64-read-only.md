### Added

- **Inspect RF64 and BW64 WAVE files.** The parser reads the mandatory first
  `ds64` chunk and resolves 64-bit form, data and table sizes with checked
  bounds and a 4,096-entry limit. Audio is not decoded or played; BW64 XML and
  channel metadata remain unread, and neither format inherits WAV clean-copy
  support.
