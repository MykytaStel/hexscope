# HEIF item-offset inspection

Construction method 2 (`iloc` item offsets) now exposes an extent when its
1-based `iloc` reference points to a local item with one explicit extent
contained in this file's `mdat` or `idat` data. The parser checks every
offset and length before creating the item range. It does not copy or decode
the image payload.

Chained item locations, external references, source items with multiple
extents, and implicit lengths stay unread and are named in the item-location
tree. This bounded step improves structural inspection without claiming full
HEIF/AVIF item reconstruction. No clean-copy behavior changes.
