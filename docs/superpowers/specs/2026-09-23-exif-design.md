# EXIF from JPEG — design

**Date:** 2026-09-23
**Status:** proposed

## Goal

Drop a photo, see what it reveals about whoever took it — camera, serial
number, when, and where — each fact on the exact bytes that hold it. The
second format, and the one with the widest audience: anyone with a phone.

## Scope

In:

- Recognise the format by its first bytes: PNG, JPEG, or unknown.
- Walk JPEG segments: markers, lengths, the entropy-coded scan data after SOS
  (skipping stuffed `FF 00` bytes and restart markers), through to EOI.
- Decode the headers worth reading: APP0 (JFIF), SOF0/1/2 (dimensions,
  precision, components), APP1 (EXIF).
- Parse EXIF fully: the TIFF header in either byte order, IFD0, the Exif,
  GPS and Interop sub-IFDs, and IFD1 (thumbnail). Every entry as a node on its
  12 bytes; a value stored elsewhere as its own child node on its real bytes,
  so hovering a camera model highlights the string itself.
- Name the common tags and render values for people: strings, rationals,
  exposure times, GPS coordinates as decimal degrees.
- A privacy summary: location, camera make and model, serial numbers, owner
  name, software, date taken, embedded thumbnail.

Out:

- Decoding JPEG pixels.
- XMP, IPTC and vendor MakerNotes — shown as opaque byte ranges.
- Removing or editing metadata. hexscope stays read-only.

## Decisions

**A `Document` enum, not the `Format` trait.** The original spec planned a
trait once a second format existed. With PNG and JPEG in hand, the trait would
reduce to `fn parse(&[u8]) -> ParseTree`, and everything format-specific —
pixels and a DEFLATE trace for PNG, EXIF facts for JPEG — would still need an
enum to carry. So: `parse(bytes) -> Document`, where `Document` is
`Png | Jpeg | Unknown`, each with a `tree()`. Adding a format is still one
module and one enum arm; the interface consumes the tree as before.

**Same guarantees as PNG.** Never panics, never loops, always returns a tree.
EXIF is hostile territory: offsets point anywhere, IFDs can chain into cycles,
counts can claim gigabytes. Every offset is checked against the TIFF block,
IFD chains track visited offsets, and entry counts are capped.

**Damage points at bytes.** Truncated segment, length overrun, missing EOI,
IFD offset out of range, IFD loop, value out of range — each an error node on
the bytes responsible.

## Interface

- File info names the format: `JPEG · 4032×3024 · EXIF`.
- A **"What this photo reveals"** card in the details panel. Each fact is a
  link into the tree and the hex view.
- A badge in the top bar when the photo carries a location — separate from
  the problems badge, because it is not damage.
- JPEG tints: EXIF teal, GPS its own colour, SOF blue, tables purple, scan
  data orange.
- A sample photo with a location on the empty screen.

## Testing

- EXIF built byte by byte in tests: both byte orders, every value type, GPS,
  out-of-range offsets, IFD cycles, truncation at every boundary.
- `parse()` never panics on arbitrary bytes (property test and fuzz target).
- Every existing PNG test keeps passing through the new dispatcher.
