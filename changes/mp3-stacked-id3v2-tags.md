### Added

- **Read and safely clean stacked MP3 ID3v2 tags.** The core inspects each
  consecutive leading tag, keeps byte ranges tied to the correct tag, and
  removes all inspected tags while copying compressed audio unchanged.
  Inspection is capped at 16 tags and 65,536 frames across the chain; files
  beyond the tag limit stay read-only, and frame-limited reads remain incomplete.
