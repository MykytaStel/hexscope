# File map: what a file is made of, and how random its bytes are

## Goal

Two views of the whole file at once, one for everyone and one for the
curious:

1. **What it's made of**: a bar and a legend in the file panel. "Picture 86%,
   metadata 11%, thumbnail 2%, structure 1%", or, for a Word document, how
   much is text, images, metadata and ZIP framing.
2. **An entropy minimap** beside the hex view, covering the whole file. Each
   stretch is coloured by how random its bytes are: zeros and padding, text
   and structure, compressed or encrypted. It doubles as a scrollbar, and
   marks where the problems are. An encrypted blob inside a text file, or
   data after the end of an image, shows at a glance.

Second of the four milestones on "two depths" (see
`2026-09-24-explanations-design.md`).

## Non-goals

- A separate map screen or treemap. Both views live in the existing layout.
- Byte-value histograms, or entropy per node in the tree.
- Guessing what unknown bytes are (a zip inside a PNG, and so on). Bytes no
  part claims are shown as such, and the verdict already names hidden data.

## Core: `crates/hexscope-core/src/map.rs`

### Composition

```rust
pub enum Role { Content, Metadata, Thumbnail, Structure, Hidden, Damaged }

pub struct Slice { pub start: u64, pub len: u64, pub role: Role, pub node: Option<NodeId> }

/// Every byte of the file in exactly one slice, in order, with neighbours of
/// the same role merged.
pub fn composition(tree: &ParseTree, format: Format, file_len: u64) -> Vec<Slice>;
```

A node's role comes from its format and label, and the deepest node with a
role wins its bytes: the EXIF thumbnail inside APP1 is Thumbnail, while the
rest of APP1 is Metadata.

| Format | Content | Metadata | Thumbnail | Structure |
|---|---|---|---|---|
| PNG | IDAT | tEXt, zTXt, iTXt, eXIf, tIME | — | signature and every other chunk |
| JPEG | scan data | APPn except JFIF, COM | `thumbnail` | SOI, EOI, JFIF, DQT, DHT, SOF, SOS, DRI, … |
| ZIP | each entry's `data` | the `data` of entries under `docProps/` or `META-INF/` | — | local headers, descriptors, central directory, end records |

Across every format:

- **Hidden**: a problem node whose concern is Hidden, and any bytes that no
  top-level node covers.
- **Damaged**: an Error node, or a problem node whose concern is Damage, when
  it has bytes.

For a file in a format hexscope does not read, the whole file is one
Structure slice named after the format. It has no parts to tell apart.

Composition is a sweep over role intervals, `O(n log n)` in the number of
nodes with a role, so a ZIP with 100,000 entries is fine. It is computed at
parse time.

### Entropy

```rust
/// Shannon entropy in bits per byte (0 to 8) of `bins` consecutive windows
/// covering `data`. Windows are at least 256 bytes, so that 8 is reachable;
/// a smaller file gets fewer bins.
pub fn entropy(data: &[u8], bins: usize) -> Vec<f32>;
```

It is a single pass with a 256-counter histogram per window. The bridge
exposes it as a free function that the worker calls after `parse`, outside
the timed parse, with 1024 bins.

## Bridge

- `Parsed.composition: Float64Array`, laid out
  `[start, len, role, node (-1 none)] × n`. Roles are numbered 0 to 5 in the
  order above.
- `entropy(bytes, bins) -> Float32Array`.

## Web

### "What it's made of" card

In the file panel, under the verdict:

- a bar of the roles in file order, 10 px high, each slice's width
  proportional to its bytes, with a 1 px minimum so a tiny slice still shows;
- a legend below it, one row per role present, largest first: swatch, name,
  percentage, size. Names follow the format: Content reads "Picture" for
  PNG/JPEG and "Files" for ZIP.
- Clicking a legend row selects that role's largest slice's node. Hovering a
  bar slice highlights its node.

Role colours come from the existing tints, so the bar matches the hex view:

| Role | Tint |
|---|---|
| Content | IDAT orange |
| Metadata | text teal |
| Thumbnail | GPS pink |
| Structure | IHDR blue |
| Hidden | PLTE purple |
| Damaged | error red |

### Entropy minimap

A canvas column 14 px wide, to the right of the hex view and full height:

- **Colour.** Each pixel row is the bins it covers, coloured by their mean on
  a sequential ramp: faint at 0 bits (zeros and padding), mid-tone around 4–6
  (text, structure), hot above 7.5 (compressed or encrypted).
- **Problems.** A 2 px red tick at each problem's offset, plus the
  selection's position.
- **Viewport.** An outline shows which part of the file the hex view has on
  screen. Clicking or dragging scrolls there, with the same mapping the hex
  view uses past 8 MB of scroll height.
- **Tooltip.** Offset, entropy ("7.98 bits/byte: compressed or encrypted"),
  and the node there.

The minimap redraws on theme change and on resize, and stays at 375 px width.

## Testing

- **Composition:** for every fixture and damaged file in the coverage set,
  the slices are sorted, contiguous from 0 to the file length, never empty,
  and never have two neighbours with the same role. Samples: in `sample.png`
  Content is the largest role; `photo.jpg` has Thumbnail and Metadata;
  `report.docx` has Metadata, from docProps; a prefixed ZIP starts with
  Hidden; a cut PNG has Damaged.
- **Entropy:** all zeros give 0; bytes 0..=255 repeated give 8; deflated
  random data is above 7.5; the bin count follows the 256-byte minimum; an
  empty input gives no bins.
- **Bridge:** the composition layout, and `entropy` from JS.
- **Browser:** the card on all four samples and a disguised ZIP; the
  minimap's colours, problem ticks, click and drag, and tooltip; at 375 px.
