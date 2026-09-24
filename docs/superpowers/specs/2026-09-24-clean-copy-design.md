# Clean copy: remove what a file reveals

## Goal

Where hexscope shows "What this photo reveals" or "What this document
reveals", one button saves a copy without it. The copy is made in the
browser, and nothing leaves the tab. The page lists what was removed and can
open the copy in hexscope, so anyone can check the card is now empty.

Third of the four "two depths" milestones.

## Scope

- **JPEG.** Removed: APP1 EXIF (location, serial numbers, owner, camera,
  dates, thumbnail), APP1 XMP, APP13 Photoshop/IPTC, APP2 MPF, JFXX, every
  other APPn hexscope does not name, COM, and any data after EOI (phones keep
  extra pictures there). Kept: SOI, APP0 JFIF, APP2 ICC colour profile, APP14
  Adobe (colour transform), the frame, tables, scan data and EOI. The image
  data is copied byte for byte: nothing is re-encoded and no quality is lost.
- **Orientation.** When EXIF held an Orientation other than 1, the copy gets
  a minimal APP1 EXIF holding only that tag, so the picture does not turn
  sideways.
- **Office documents** (a ZIP with `docProps/core.xml`, `app.xml` or
  `custom.xml`): those entries are replaced with empty property files,
  stored uncompressed. Every other entry's data is copied as it is. Headers
  are rewritten without extra fields, which can hold owners and times,
  without data descriptors, and without the archive comment; bytes outside
  the entries are dropped. The page says plainly that comments and tracked
  changes inside the text keep their authors.
- **Refused, with the reason:** encrypted entries, ZIP64 archives, entries
  whose data is cut short, and files with nothing to remove.
- PNG is not cleaned yet, because it has no "reveals" card.

## Core: `crates/hexscope-core/src/clean.rs`

```rust
pub struct Cleaned { pub bytes: Vec<u8>, pub removed: Vec<Removed>, pub orientation_kept: Option<u16> }
pub struct Removed { pub what: String, pub bytes: u64 }
pub enum CleanError { NothingToRemove, Encrypted, Zip64, Damaged, Unsupported }
pub fn clean(data: &[u8]) -> Result<Cleaned, CleanError>;
```

`PhotoFacts` gains the raw `orientation`, read from IFD0.

## Bridge and page

- `cleanCopy(bytes) -> Uint8Array` (empty when refused), with
  `cleanReport` and `cleanError` getters. It is a free function, since the
  page already holds the bytes.
- In the reveals card: **Remove it — save a clean copy**. After saving, the
  card lists what was removed and offers **Open the clean copy**.

## Testing

- **Photo:** after cleaning, the parse has no facts and no location, no new
  warnings, and scan data identical to the original. Orientation 6 survives
  as the only tag. Data after EOI is gone.
- **Office:** after cleaning the `.docx` fixture there are no facts and no
  warnings, the same entries, and every entry except the property files
  extracts to the same bytes.
- **Robustness:** every file in the coverage set gives `Ok` or `Err`, never
  a panic, and every `Ok` parses without Error nodes.
- **Browser:** clean the photo and the document, open the copies, and check
  the cards are empty.
