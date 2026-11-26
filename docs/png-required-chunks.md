# PNG required chunk framing

IHDR must occur exactly once as the first chunk; IEND must have a zero-length payload. Record errors for missing, duplicate or displaced headers and nonempty terminators. The first header remains the authority for subsequent image decoding and later metadata chunks can still be read. The existing cleaner refuses inputs with error nodes. Regression fixtures derive from PNGSuite and include a malformed IEND with a correct CRC. This does not implement all PNG chunk ordering or ancillary-chunk constraints. Existing size limits remain unchanged.
