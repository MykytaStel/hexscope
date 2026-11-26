# EXIF IFD termination

Each classic TIFF IFD ends with a four-byte next-IFD offset, including zero for no following directory. Previously an exact entry boundary with an absent or partial pointer produced no error. Preserve the parsed entries and report damage for the unread pointer. Existing over-declared entry-count errors remain separate. The regression covers both byte orders and each missing-byte count. This does not add support for BigTIFF or change which directory links are followed.
