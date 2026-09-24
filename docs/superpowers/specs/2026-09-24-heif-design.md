# HEIC and AVIF photos

## Goal

Photos from an iPhone are HEIC, and more and more of the web is AVIF. Both
are HEIF: ISO base media boxes, holding the image as coded items and its
metadata as items of their own. hexscope reads them like it reads a JPEG:
the structure, what the photo reveals (from its EXIF item), and a clean copy.

## Reading

- **Dispatch.** A file whose bytes 4–8 are `ftyp`, with a HEIF brand among
  its major and compatible brands (`heic heix heim heis hevc hevx mif1 msf1
  avif avis`). Other ISO files, such as MP4, are named but not read.
- **Boxes.** Read recursively, with the depth capped: `ftyp`, `meta` (a full
  box), `hdlr`, `dinf`/`dref`, `pitm`, `iinf`/`infe`, `iref` and its
  references, `iprp`/`ipco` (`ispe`, `irot`, `imir`, `colr`, `pixi`,
  `hvcC`, `av1C`, `ipma`), `iloc` (versions 0–2, field sizes 0/4/8,
  construction methods 0 and 1), `idat`, `mdat`. Anything else is named,
  with its length.
- **Items.** Every item's data becomes a node inside the box that holds it
  (`mdat`, or `idat` for construction method 1), labelled `item N · type`
  and valued by what it is: primary image, tile k of n, grid, thumbnail, EXIF
  metadata, XMP metadata.
- **EXIF item:** a 4-byte offset, then usually `Exif\0\0`, then TIFF, parsed
  by the existing EXIF reader and grafted in place. It gives the photo facts
  and the reveals card.
- **Dimensions:** the largest `ispe`. For a gridded image that is the grid's
  size, which is the picture's.
- **Problems.** A box that runs past its container, a size smaller than its
  header, an item whose data lies outside the file: Error. A construction
  method other than 0 or 1 is a limit of the tool, named without a problem.

## Clean copy

HEIF points at item data by absolute offset, so the copy keeps every length
and changes bytes in place. The EXIF item keeps its offset field and
`Exif\0\0`, then holds a minimal TIFF: an IFD0 with Orientation when one was
set, otherwise with no entries, followed by zeros. The XMP item becomes an
empty XMP packet, padded with spaces inside the packet. The copy is exactly
as long as the original, and no offset changes.

## Fixtures

`scripts/make-sample-heif.sh` uses macOS `sips` (Apple's own HEIF encoder)
to write, from the sample photo: a small HEIC, a 4032×3024 HEIC gridded into
tiles as an iPhone's are, and an AVIF.

## Testing

- The three fixtures parse with no problems; their facts match the sample
  photo's; their dimensions are right; the grid has all its tiles.
- Every truncation of the small HEIC survives; random bytes after a HEIF
  `ftyp` never panic.
- The docs coverage and composition tests take the fixtures automatically.
- The clean copy is the same length, parses cleanly, and reveals nothing.
