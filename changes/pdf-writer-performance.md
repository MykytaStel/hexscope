### Changed

- Write PDF object headers and cross-reference fields directly into the output with a shared decimal encoder, avoiding temporary formatting strings.
- Blank JPEG metadata in the already-owned PDF output rather than copying the entire JPEG into a separate temporary buffer.
- Reject offsets that cannot fit a classic cross-reference entry and avoid object-number arithmetic overflow.
