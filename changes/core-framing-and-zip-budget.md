### Fixed

- **Reject malformed zlib window declarations.** A header with an invalid
  window size is rejected even if its header checksum is correct.
- **Distinguish truncated gzip trailers from extra data.** The gzip reader
  now reports the correct framing failure internally.
- **Reject over-budget ZIP entries before decoding.** Entries declaring an
  expanded size above the caller's budget no longer start decompression.
  Actual expanded length and CRC checks remain in place.
- **Validate the entire zlib frame.** Reject extra bytes before the Adler-32
  trailer and back-references exceeding the declared window.
