# Explanations and verdict: two depths for every file

## Goal

hexscope shows everything but explains little. A person who does not know
what IHDR is sees names; one who does has to look up which section of which
specification defines it. This milestone gives every node two depths, both
always visible, the plainer one first:

1. **What it is**, in one sentence anyone can read.
2. **Where it is defined**: the section of the specification, linked.

Above the tree, a **verdict** answers the question people arrive with, "is
this file all right, and what does it say?", in a few plain lines, each
leading to the bytes behind it.

It is the first of four milestones on the same principle, each with its own
spec: this one, a file map with entropy, cleaning metadata from a copy, and
linking pixels to bytes.

## Non-goals

- A Simple/Expert switch, or hiding technical detail. Both depths are shown;
  the deeper one sits lower.
- Saying a file is safe. hexscope is not a virus scanner: the verdict reports
  what was found, and says so.
- Translating the interface. English, as now.

## Where explanations live: in the core, next to the parsers

Each format has a table from label to explanation, in the crate that emits
the label. A test then proves that every label a parser can produce has one.

```rust
// crates/hexscope-core/src/docs/mod.rs
pub struct Doc {
    /// One sentence, plain language, no jargon it does not explain.
    pub text: &'static str,
    /// Where the format defines it, e.g. "PNG §11.2.2", with a URL.
    pub spec: Option<Spec>,
    /// For problem nodes only: what kind of trouble it is.
    pub concern: Option<Concern>,
}
pub struct Spec { pub cite: &'static str, pub url: &'static str }

pub enum Concern {
    /// Bytes are corrupt or missing: the file will not open properly.
    Damage,
    /// Something is hidden, disguised or contradictory: data outside the
    /// format, two records that disagree, bytes read twice.
    Hidden,
    /// A rule is broken in a way that is usually harmless.
    Oddity,
}

pub fn describe(tree: &ParseTree, id: NodeId, format: Format) -> Option<Doc>;
```

`describe` works from the node's label and its parent's label. That is
enough to tell a ZIP entry, whose label is a file name, from a field: an
entry is a child of the root that is not one of the ZIP's own records. Labels
that carry numbers match by shape: `block N · …`, `entry N`, `IFDN`, `… value`
(an EXIF value stored away from its entry), `… (continued)`.

Tables:

| Format | File | Covers |
|---|---|---|
| PNG | `png/docs.rs` | chunks, IHDR/PLTE/tRNS/gAMA/pHYs/tEXt fields, the IDAT stream's parts, problems |
| JPEG | `jpeg` docs | markers and segments, JFIF, SOF/DHT/DQT/SOS, problems |
| EXIF | `exif/tags.rs` | each tag's name gains its explanation in the same table; TIFF header and IFD structure |
| ZIP | `zip/docs.rs` | records, fields, entries, data descriptors, extra fields, problems |
| Other | `document.rs` | the unknown-format root and its message |

**Problem nodes** carry dynamic text ("CRC mismatch: stored …, computed …"),
so they match by prefix. Each gets an explanation of what it means and a
`Concern`. Examples:

- *name differs from the central directory's* → Hidden: "Two parts of the
  archive disagree on this file's name, so different tools can extract
  different files. Android malware has used exactly this."
- *CRC mismatch* → Damage: "The checksum does not match the data: these
  bytes changed after the file was written."
- *bit depth … is not allowed for colour type …* → Oddity.

A problem with no specific entry falls back to a generic sentence for its
kind, so the UI never shows nothing. The coverage test does not accept the
fallback for any problem the fixtures produce.

**Specifications cited:** W3C PNG (third edition), RFC 1950 (zlib),
RFC 1951 (DEFLATE), ITU T.81 (JPEG), JFIF 1.02, CIPA DC-008 (EXIF 2.32),
PKWARE APPNOTE (ZIP). Every URL is checked to resolve while this is built;
links point at a section anchor where the document has them, and at the
document with the section named in `cite` where it does not.

## Bridge

Tens of thousands of nodes share a few hundred explanations, so they are sent
once, not per node:

- `docIds: Int32Array`, one per node: an index into the tables below, or -1.
- `docTexts`, `docCites`, `docUrls`: strings joined by U+001F.
- `docConcerns: Uint8Array`: 0 none, 1 damage, 2 hidden, 3 oddity.

## Verdict

The page composes it from what the file model already has (problems, facts,
the concern of each problem), in this order, showing only the lines that
apply:

| Line | When | Example |
|---|---|---|
| **Damaged** | any Error, or a Warning with Concern Damage | "Damaged: 2 places could not be read. It may not open, or open partly." |
| **Something hidden** | any Concern Hidden | "Something is hidden or disguised: data before the archive." |
| **Reveals** | facts exist | "Reveals where it was taken and the camera's serial number." / "Reveals who wrote it and their company." |
| **Unusual** | only Oddity warnings | "Breaks 1 rule of the format, usually harmless." |
| **Looks healthy** | none of the above | "Looks healthy: every part reads the way the format says it should." |

Each line has a *Show me* link to its first node; the problem count's **N**
still steps through all of them. A footnote reads: "What hexscope found by
reading the structure. It is not a virus scan."

## Web

- **Verdict card**: first in the file panel, above "File".
- **Node panel**: under the breadcrumbs, the explanation sentence; for a
  problem, it replaces the generic "Damage — this region could not be read"
  note and takes the note's colour from its Concern. In the facts grid,
  a **Spec** row links the citation (new tab, `noopener noreferrer`).
- **Tree**: a row's tooltip gains the explanation, so hovering already tells
  what a thing is.

## Testing

- **Coverage:** for every node of every fixture — PngSuite, the photos, the
  `.docx` fixture and sample, and the damaged archives the ZIP tests build —
  `describe` returns an explanation, and no problem node gets the fallback.
  Every EXIF tag in the table has an explanation.
- **Shape:** every explanation is one sentence, at most 200 characters, and
  ends with a full stop; every URL is `https://`.
- **Verdict:** on the samples: `sample.png` looks healthy, `broken.png` is
  damaged, `photo.jpg` reveals, `report.docx` reveals, a ZIP with a prefix
  hides something.
- **Links:** a script (not CI; it needs the network) requests every spec URL
  and fails on anything but 200.
- **Browser:** each sample shows its verdict; hovering and pinning show the
  sentence and the Spec link; checked at 375 px.
