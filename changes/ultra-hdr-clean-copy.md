### Added

- **Ultra HDR clean copies** keep one bounded MPF-listed gain map. The core
  reads the map's photo metadata into the parent file's evidence, preserves
  MPF offsets and both pictures, and removes personal metadata from both JPEGs
  while retaining supported Ultra HDR or ISO 21496-1 settings. The Web explains
  this behavior in English and Ukrainian before the copy is made.

### Fixed

- **Unknown or incomplete gain-map metadata** now makes cleaning refuse the
  copy; an unrecognized MPF-listed picture is never replaced with blank bytes.
  Core tests verify safe metadata retention and read-back. The synthetic
  iPhone-format fixture does not establish visual behavior on a physical
  iPhone.
