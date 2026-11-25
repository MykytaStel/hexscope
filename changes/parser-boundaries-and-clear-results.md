### Fixed

- Reject overflowing ZIP ranges and unrepresentable embedded offsets.
- Preserve the last byte of unterminated media box strings and validate box boundaries.
- Stop on unsupported HEIF location versions and oversized location tables rather than silently clipping them.
- Show an explicit partial-inspection warning instead of a healthy verdict when checks are incomplete.
- Name MP3, FLAC and Ogg correctly when their extensions do not match.
