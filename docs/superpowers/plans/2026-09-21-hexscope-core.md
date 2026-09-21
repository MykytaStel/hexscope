# hexscope Core Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build `hexscope-core`, a Rust crate that turns a PNG byte slice into a structure tree, decoded pixels, and a step-by-step trace of the DEFLATE decompression — without ever panicking on malformed input.

**Architecture:** One crate, no `unsafe`. A `Reader` gives fallible byte access; a `ParseTree` arena collects nodes; a hand-written inflate emits events to a sink so the caller decides whether to keep a trace. `parse_png` is infallible: every input yields a tree, with `Warning`/`Error` nodes where the file is broken.

**Tech Stack:** Rust 2024 edition, `insta` (golden snapshots), `proptest` (property tests), `criterion` (benchmarks), `cargo-fuzz` (fuzzing), `flate2` (dev-only oracle for inflate correctness).

## Global Constraints

- Crate is `#![forbid(unsafe_code)]` — no exceptions.
- **The parser never panics and never loops forever.** All byte access goes through `Reader`, which returns `Result`. No direct slice indexing in parsing code.
- **Every input returns a tree.** `parse_png` has no `Result` — failure is represented by `NodeKind::Error` nodes, not by an error return.
- No runtime dependencies for parsing. `flate2` is a `dev-dependency` only, used to verify our inflate against a reference.
- License fields in every `Cargo.toml`: `license = "MIT OR Apache-2.0"`.
- Performance budget: parsing a 10 MB PNG completes in under 300 ms (enforced in Task 11).
- Commit after every task. Conventional commits (`feat:`, `test:`, `chore:`).

---

### Task 1: Workspace, core crate, and the node model

**Files:**
- Create: `Cargo.toml` (workspace root)
- Create: `crates/hexscope-core/Cargo.toml`
- Create: `crates/hexscope-core/src/lib.rs`
- Create: `crates/hexscope-core/src/model.rs`
- Create: `rust-toolchain.toml`

**Interfaces:**
- Consumes: nothing.
- Produces: `ByteRange`, `Value`, `NodeKind`, `Node`, `NodeId`, `ParseTree` with methods `new()`, `add(parent, label, range, kind, value) -> NodeId`, `get(id) -> &Node`, `root() -> Option<NodeId>`, `len() -> usize`.

- [ ] **Step 1: Create the workspace root**

`Cargo.toml`:

```toml
[workspace]
members = ["crates/hexscope-core"]
resolver = "3"

[workspace.package]
edition = "2024"
license = "MIT OR Apache-2.0"
repository = "https://github.com/hexscope/hexscope"

[profile.release]
opt-level = 3
lto = true
codegen-units = 1
```

`rust-toolchain.toml`:

```toml
[toolchain]
channel = "stable"
components = ["rustfmt", "clippy"]
```

- [ ] **Step 2: Create the core crate manifest**

`crates/hexscope-core/Cargo.toml`:

```toml
[package]
name = "hexscope-core"
version = "0.1.0"
description = "Parses binary files into a structure tree with byte ranges"
edition.workspace = true
license.workspace = true
repository.workspace = true

[dependencies]

[dev-dependencies]
flate2 = "1"
insta = "1"
proptest = "1"
```

- [ ] **Step 3: Write the failing test for the tree model**

`crates/hexscope-core/src/model.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_a_parent_child_tree() {
        let mut tree = ParseTree::new();
        let root = tree.add(None, "PNG", ByteRange::new(0, 100), NodeKind::Container, None);
        let field = tree.add(
            Some(root),
            "width",
            ByteRange::new(16, 4),
            NodeKind::Field,
            Some(Value::U64(1920)),
        );

        assert_eq!(tree.root(), Some(root));
        assert_eq!(tree.get(root).children, vec![field]);
        assert_eq!(tree.get(field).parent, Some(root));
        assert_eq!(tree.get(field).value, Some(Value::U64(1920)));
        assert_eq!(tree.len(), 2);
    }

    #[test]
    fn byte_range_end_is_exclusive() {
        assert_eq!(ByteRange::new(8, 25).end(), 33);
    }
}
```

- [ ] **Step 4: Run the test to verify it fails**

Run: `cargo test -p hexscope-core`
Expected: FAIL — `cannot find type ParseTree in this scope` (and similar for the other items).

- [ ] **Step 5: Implement the model**

Prepend to `crates/hexscope-core/src/model.rs`:

```rust
/// Index into `ParseTree::nodes`.
pub type NodeId = u32;

/// A half-open span of the source file: `[start, start + len)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ByteRange {
    pub start: u64,
    pub len: u64,
}

impl ByteRange {
    pub fn new(start: u64, len: u64) -> Self {
        Self { start, len }
    }

    pub fn end(&self) -> u64 {
        self.start + self.len
    }
}

/// A decoded field value. Large byte payloads are not copied — only their
/// length is recorded, because the bytes stay addressable via the node range.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    U64(u64),
    Text(String),
    /// Length in bytes of an opaque payload.
    Bytes(u64),
    /// A numeric value with a known meaning, e.g. color type 6 = "RGBA".
    Enum { raw: u64, name: &'static str },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeKind {
    /// Groups other nodes, e.g. a chunk.
    Container,
    /// A decoded leaf value.
    Field,
    /// File is readable but suspicious, e.g. a CRC mismatch.
    Warning,
    /// This region could not be read; parsing continued elsewhere.
    Error,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub id: NodeId,
    pub parent: Option<NodeId>,
    pub label: String,
    pub range: ByteRange,
    pub value: Option<Value>,
    pub kind: NodeKind,
    pub children: Vec<NodeId>,
}

/// Arena of nodes. The first node added becomes the root.
#[derive(Debug, Default)]
pub struct ParseTree {
    nodes: Vec<Node>,
}

impl ParseTree {
    pub fn new() -> Self {
        Self { nodes: Vec::new() }
    }

    pub fn add(
        &mut self,
        parent: Option<NodeId>,
        label: impl Into<String>,
        range: ByteRange,
        kind: NodeKind,
        value: Option<Value>,
    ) -> NodeId {
        let id = self.nodes.len() as NodeId;
        self.nodes.push(Node {
            id,
            parent,
            label: label.into(),
            range,
            value,
            kind,
            children: Vec::new(),
        });
        if let Some(p) = parent {
            self.nodes[p as usize].children.push(id);
        }
        id
    }

    pub fn get(&self, id: NodeId) -> &Node {
        &self.nodes[id as usize]
    }

    pub fn root(&self) -> Option<NodeId> {
        if self.nodes.is_empty() { None } else { Some(0) }
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Nodes in insertion order. Used by the WASM bridge to flatten the tree.
    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }
}
```

`crates/hexscope-core/src/lib.rs`:

```rust
#![forbid(unsafe_code)]

pub mod model;

pub use model::{ByteRange, Node, NodeId, NodeKind, ParseTree, Value};
```

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test -p hexscope-core`
Expected: PASS — `test result: ok. 2 passed`.

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml rust-toolchain.toml crates/
git commit -m "feat(core): add workspace and parse tree model"
```

---

### Task 2: Fallible byte reader

**Files:**
- Create: `crates/hexscope-core/src/reader.rs`
- Modify: `crates/hexscope-core/src/lib.rs`

**Interfaces:**
- Consumes: nothing.
- Produces: `ReadError`, `Reader<'a>` with `new(&'a [u8])`, `pos() -> u64`, `remaining() -> usize`, `seek(u64)`, `u8() -> Result<u8, ReadError>`, `u16_be()`, `u32_be()`, `bytes(usize) -> Result<&'a [u8], ReadError>`, `array::<N>() -> Result<[u8; N], ReadError>`, `bytes_until(u8) -> Option<&'a [u8]>`, `rest() -> &'a [u8]`.

`Reader` is the crate's single bounds-checked chokepoint: slicing lives here so that no parser above it ever indexes a byte slice by hand.

- [ ] **Step 1: Write the failing tests**

`crates/hexscope-core/src/reader.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_big_endian_integers() {
        let data = [0x00, 0x00, 0x07, 0x80, 0x12, 0x34];
        let mut r = Reader::new(&data);
        assert_eq!(r.u32_be(), Ok(1920));
        assert_eq!(r.u16_be(), Ok(0x1234));
        assert_eq!(r.pos(), 6);
    }

    #[test]
    fn reports_eof_instead_of_panicking() {
        let data = [0x01, 0x02];
        let mut r = Reader::new(&data);
        assert_eq!(
            r.u32_be(),
            Err(ReadError::Eof { needed: 4, available: 2 })
        );
        // A failed read must not consume anything.
        assert_eq!(r.pos(), 0);
        assert_eq!(r.u8(), Ok(0x01));
    }

    #[test]
    fn seek_past_end_clamps_and_reports_eof() {
        let data = [0x01, 0x02];
        let mut r = Reader::new(&data);
        r.seek(9999);
        assert_eq!(r.remaining(), 0);
        assert_eq!(r.u8(), Err(ReadError::Eof { needed: 1, available: 0 }));
    }

    #[test]
    fn bytes_borrows_without_copying() {
        let data = [1, 2, 3, 4, 5];
        let mut r = Reader::new(&data);
        assert_eq!(r.bytes(3), Ok(&data[0..3]));
        assert_eq!(r.remaining(), 2);
    }

    #[test]
    fn array_reads_a_fixed_size_field() {
        let data = *b"IHDRxx";
        let mut r = Reader::new(&data);
        assert_eq!(r.array::<4>(), Ok(*b"IHDR"));
        assert_eq!(r.pos(), 4);
        // Too few bytes left: reports EOF and stays put, like every other read.
        assert_eq!(r.array::<4>(), Err(ReadError::Eof { needed: 4, available: 2 }));
        assert_eq!(r.pos(), 4);
    }

    #[test]
    fn bytes_until_splits_on_the_delimiter() {
        let data = *b"Author\0Ada";
        let mut r = Reader::new(&data);
        assert_eq!(r.bytes_until(0), Some(&b"Author"[..]));
        // The delimiter itself is consumed.
        assert_eq!(r.pos(), 7);
        assert_eq!(r.rest(), &b"Ada"[..]);
        assert_eq!(r.remaining(), 0);
    }

    #[test]
    fn bytes_until_reports_a_missing_delimiter_without_moving() {
        let data = *b"no-null-here";
        let mut r = Reader::new(&data);
        assert_eq!(r.bytes_until(0), None);
        assert_eq!(r.pos(), 0, "a failed search must not consume input");
    }

    #[test]
    fn bytes_until_handles_an_empty_leading_field() {
        let data = [0u8, b'x'];
        let mut r = Reader::new(&data);
        assert_eq!(r.bytes_until(0), Some(&[][..]));
        assert_eq!(r.rest(), &[b'x'][..]);
    }

    #[test]
    fn rest_on_exhausted_input_is_empty() {
        let data = [1u8];
        let mut r = Reader::new(&data);
        assert_eq!(r.bytes(1), Ok(&data[..]));
        assert_eq!(r.rest(), &[][..]);
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p hexscope-core reader`
Expected: FAIL — `cannot find type Reader in this scope`.

- [ ] **Step 3: Implement the reader**

Prepend to `crates/hexscope-core/src/reader.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadError {
    Eof { needed: usize, available: usize },
}

/// A cursor over a byte slice. Every read is bounds-checked and a failed read
/// leaves the position untouched, so a caller can recover and try something
/// smaller.
#[derive(Debug, Clone)]
pub struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    pub fn pos(&self) -> u64 {
        self.pos as u64
    }

    pub fn remaining(&self) -> usize {
        self.data.len() - self.pos
    }

    /// Clamps to end-of-input rather than failing; subsequent reads report EOF.
    pub fn seek(&mut self, pos: u64) {
        self.pos = usize::try_from(pos).unwrap_or(usize::MAX).min(self.data.len());
    }

    pub fn bytes(&mut self, n: usize) -> Result<&'a [u8], ReadError> {
        if self.remaining() < n {
            return Err(ReadError::Eof { needed: n, available: self.remaining() });
        }
        let out = &self.data[self.pos..self.pos + n];
        self.pos += n;
        Ok(out)
    }

    pub fn u8(&mut self) -> Result<u8, ReadError> {
        Ok(self.bytes(1)?[0])
    }

    pub fn u16_be(&mut self) -> Result<u16, ReadError> {
        let b = self.bytes(2)?;
        Ok(u16::from_be_bytes([b[0], b[1]]))
    }

    pub fn u32_be(&mut self) -> Result<u32, ReadError> {
        let b = self.bytes(4)?;
        Ok(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    }

    /// Reads up to the next occurrence of `delim` and consumes the delimiter.
    /// Returns `None` when the delimiter is absent, leaving the position
    /// untouched so the caller can record the damage and move on.
    pub fn bytes_until(&mut self, delim: u8) -> Option<&'a [u8]> {
        let offset = self.data[self.pos..].iter().position(|&b| b == delim)?;
        let out = self.bytes(offset).ok()?;
        // `position` already proved the delimiter is the next byte.
        self.pos += 1;
        Some(out)
    }

    /// Consumes and returns everything left, which may be empty.
    pub fn rest(&mut self) -> &'a [u8] {
        let out = &self.data[self.pos..];
        self.pos = self.data.len();
        out
    }

    /// Reads a fixed-size field as an owned array. Parsers use this for things
    /// like a four-byte chunk type so they never index a slice by hand.
    pub fn array<const N: usize>(&mut self) -> Result<[u8; N], ReadError> {
        let available = self.remaining();
        let slice = self.bytes(N)?;
        // `bytes` already guaranteed exactly N bytes, so this conversion cannot
        // fail; writing it as a fallible conversion keeps the function total
        // and leaves no panicking path in the crate's read layer.
        slice
            .try_into()
            .map_err(|_| ReadError::Eof { needed: N, available })
    }
}
```

Add to `crates/hexscope-core/src/lib.rs`:

```rust
pub mod reader;

pub use reader::{ReadError, Reader};
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p hexscope-core reader`
Expected: PASS — `test result: ok. 9 passed`.

- [ ] **Step 5: Commit**

```bash
git add crates/hexscope-core/src/reader.rs crates/hexscope-core/src/lib.rs
git commit -m "feat(core): add fallible byte reader"
```

---

### Task 3: CRC-32 and the PNG chunk walker

**Files:**
- Create: `crates/hexscope-core/src/crc32.rs`
- Create: `crates/hexscope-core/src/png/mod.rs`
- Create: `crates/hexscope-core/src/png/chunks.rs`
- Modify: `crates/hexscope-core/src/lib.rs`

**Interfaces:**
- Consumes: `Reader`, `ByteRange` (Tasks 1–2).
- Produces: `crc32(&[u8]) -> u32`; `PNG_SIGNATURE: [u8; 8]`; `Chunk<'a> { range, kind: [u8; 4], data: &'a [u8], data_range, declared_crc, actual_crc }` with `Chunk::kind_str() -> String` and `Chunk::crc_ok() -> bool`; `next_chunk<'a>(&mut Reader<'a>) -> Option<Result<Chunk<'a>, ChunkError>>`; `ChunkError::{Truncated, LengthTooLarge}`.

- [ ] **Step 1: Write the failing CRC test**

`crates/hexscope-core/src/crc32.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_known_vectors() {
        assert_eq!(crc32(b""), 0x0000_0000);
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        // "IEND" with no data — the CRC every PNG ends with.
        assert_eq!(crc32(b"IEND"), 0xAE42_6082);
    }
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p hexscope-core crc32`
Expected: FAIL — `cannot find function crc32 in this scope`.

- [ ] **Step 3: Implement CRC-32**

Prepend to `crates/hexscope-core/src/crc32.rs`:

```rust
/// Standard CRC-32 (IEEE 802.3, reflected, polynomial 0xEDB88320) — the one
/// PNG uses for every chunk.
pub fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}
```

- [ ] **Step 4: Run it to verify it passes**

Run: `cargo test -p hexscope-core crc32`
Expected: PASS — `test result: ok. 1 passed`.

- [ ] **Step 5: Write the failing chunk walker tests**

`crates/hexscope-core/src/png/chunks.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::crc32::crc32;
    use crate::reader::Reader;

    /// Builds a well-formed chunk: length, type, data, CRC over type+data.
    fn chunk(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        out.extend_from_slice(kind);
        out.extend_from_slice(data);
        let mut crc_input = kind.to_vec();
        crc_input.extend_from_slice(data);
        out.extend_from_slice(&crc32(&crc_input).to_be_bytes());
        out
    }

    #[test]
    fn walks_a_valid_chunk() {
        let bytes = chunk(b"IHDR", &[1, 2, 3]);
        let mut r = Reader::new(&bytes);
        let c = next_chunk(&mut r).unwrap().unwrap();

        assert_eq!(c.kind_str(), "IHDR");
        assert_eq!(c.data, &[1, 2, 3]);
        assert!(c.crc_ok());
        assert_eq!(c.range.start, 0);
        assert_eq!(c.range.len, 15); // 4 + 4 + 3 + 4
        assert_eq!(c.data_range.start, 8);
        assert_eq!(c.data_range.len, 3);
        assert!(next_chunk(&mut r).is_none());
    }

    #[test]
    fn detects_a_bad_crc_without_failing() {
        let mut bytes = chunk(b"tEXt", b"hi");
        let last = bytes.len() - 1;
        bytes[last] ^= 0xFF;
        let mut r = Reader::new(&bytes);
        let c = next_chunk(&mut r).unwrap().unwrap();

        assert_eq!(c.kind_str(), "tEXt");
        assert!(!c.crc_ok());
        assert_eq!(c.data, b"hi");
    }

    #[test]
    fn reports_truncation_instead_of_panicking() {
        let full = chunk(b"IDAT", &[9; 20]);
        let mut r = Reader::new(&full[..12]);
        assert_eq!(next_chunk(&mut r), Some(Err(ChunkError::Truncated)));
        assert_eq!(next_chunk(&mut r), None, "damage must end the walk");
    }

    #[test]
    fn a_truncated_length_field_ends_the_walk() {
        // Three stray trailing bytes: not even enough for the length field.
        // A failed read leaves the position untouched, so unless the walker
        // consumes them itself a looping caller would spin forever here.
        let bytes = [0u8, 0, 0];
        let mut r = Reader::new(&bytes);
        assert_eq!(next_chunk(&mut r), Some(Err(ChunkError::Truncated)));
        assert_eq!(next_chunk(&mut r), None, "the same error must not repeat");
    }

    #[test]
    fn rejects_an_absurd_declared_length() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&0xFFFF_FFFFu32.to_be_bytes());
        bytes.extend_from_slice(b"IDAT");
        let mut r = Reader::new(&bytes);
        assert_eq!(next_chunk(&mut r), Some(Err(ChunkError::LengthTooLarge)));
        assert_eq!(next_chunk(&mut r), None, "damage must end the walk");
    }
}
```

- [ ] **Step 6: Run them to verify they fail**

Run: `cargo test -p hexscope-core chunks`
Expected: FAIL — `cannot find function next_chunk in this scope`.

- [ ] **Step 7: Implement the chunk walker**

Prepend to `crates/hexscope-core/src/png/chunks.rs`:

```rust
use crate::crc32::crc32;
use crate::model::ByteRange;
use crate::reader::Reader;

pub const PNG_SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

/// The PNG spec caps chunk length at 2^31 - 1.
const MAX_CHUNK_LEN: u32 = 0x7FFF_FFFF;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChunkError {
    /// The file ended inside this chunk.
    Truncated,
    /// The declared length exceeds what the format allows.
    LengthTooLarge,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chunk<'a> {
    /// The whole chunk: length + type + data + CRC.
    pub range: ByteRange,
    pub kind: [u8; 4],
    pub data: &'a [u8],
    pub data_range: ByteRange,
    pub declared_crc: u32,
    pub actual_crc: u32,
}

impl Chunk<'_> {
    pub fn kind_str(&self) -> String {
        self.kind.iter().map(|&b| b as char).collect()
    }

    pub fn crc_ok(&self) -> bool {
        self.declared_crc == self.actual_crc
    }
}

/// Reads the next chunk. Returns `None` at a clean end of input, and
/// `Some(Err(..))` when the file is damaged — in both cases the caller keeps
/// control and decides what to record.
///
/// **Damage ends the walk.** Every error path seeks to end-of-input before
/// returning, so a caller that keeps looping receives `None` on the next call
/// rather than the same error forever. Without this, a file ending in one to
/// three stray bytes would fail the length read without consuming them — a
/// failed read deliberately leaves the position untouched — and spin any naive
/// loop indefinitely. The no-infinite-loop guarantee belongs here, not in the
/// memory of every future caller.
pub fn next_chunk<'a>(r: &mut Reader<'a>) -> Option<Result<Chunk<'a>, ChunkError>> {
    if r.remaining() == 0 {
        return None;
    }
    let start = r.pos();

    let Ok(len) = r.u32_be() else {
        r.seek(u64::MAX);
        return Some(Err(ChunkError::Truncated));
    };
    if len > MAX_CHUNK_LEN {
        r.seek(u64::MAX);
        return Some(Err(ChunkError::LengthTooLarge));
    }

    let Ok(kind) = r.array::<4>() else {
        r.seek(u64::MAX);
        return Some(Err(ChunkError::Truncated));
    };

    let data_start = r.pos();
    let Ok(data) = r.bytes(len as usize) else {
        r.seek(u64::MAX);
        return Some(Err(ChunkError::Truncated));
    };

    let Ok(declared_crc) = r.u32_be() else {
        r.seek(u64::MAX);
        return Some(Err(ChunkError::Truncated));
    };

    let mut crc_input = Vec::with_capacity(4 + data.len());
    crc_input.extend_from_slice(&kind);
    crc_input.extend_from_slice(data);

    Some(Ok(Chunk {
        range: ByteRange::new(start, r.pos() - start),
        kind,
        data,
        data_range: ByteRange::new(data_start, len as u64),
        declared_crc,
        actual_crc: crc32(&crc_input),
    }))
}
```

`crates/hexscope-core/src/png/mod.rs`:

```rust
pub mod chunks;
```

Add to `crates/hexscope-core/src/lib.rs`:

```rust
pub mod crc32;
pub mod png;
```

- [ ] **Step 8: Run all tests to verify they pass**

Run: `cargo test -p hexscope-core`
Expected: PASS — `test result: ok. 17 passed`.

- [ ] **Step 9: Commit**

```bash
git add crates/hexscope-core/src/crc32.rs crates/hexscope-core/src/png crates/hexscope-core/src/lib.rs
git commit -m "feat(core): add crc32 and png chunk walker"
```

---

### Task 4: Chunk payload decoding

**Files:**
- Create: `crates/hexscope-core/src/png/fields.rs`
- Modify: `crates/hexscope-core/src/png/mod.rs`

**Interfaces:**
- Consumes: `Chunk` (Task 3), `ParseTree`, `Value`, `NodeKind` (Task 1).
- Produces: `Ihdr { width, height, bit_depth, color_type, interlace }` with `Ihdr::bytes_per_pixel() -> usize`; `decode_ihdr(&Chunk, &mut ParseTree, NodeId) -> Option<Ihdr>`; `decode_text`, `decode_phys`, `decode_plte`, `decode_gama` all `(&Chunk, &mut ParseTree, NodeId)`; `decode_trns(&Chunk, &mut ParseTree, NodeId, Option<u8>)`.

Covers the spec's ancillary chunk list: IHDR, PLTE, tEXt, pHYs, gAMA, tRNS.

- [ ] **Step 1: Write the failing tests**

`crates/hexscope-core/src/png/fields.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ByteRange, NodeKind, ParseTree, Value};
    use crate::png::chunks::Chunk;

    fn fake_chunk<'a>(kind: &[u8; 4], data: &'a [u8]) -> Chunk<'a> {
        Chunk {
            range: ByteRange::new(0, (data.len() + 12) as u64),
            kind: *kind,
            data,
            data_range: ByteRange::new(8, data.len() as u64),
            declared_crc: 0,
            actual_crc: 0,
        }
    }

    #[test]
    fn decodes_ihdr_fields() {
        // 1920x1080, 8-bit, colour type 6 (RGBA), no interlace.
        let data = [0, 0, 7, 128, 0, 0, 4, 56, 8, 6, 0, 0, 0];
        let chunk = fake_chunk(b"IHDR", &data);
        let mut tree = ParseTree::new();
        let root = tree.add(None, "PNG", ByteRange::new(0, 0), NodeKind::Container, None);

        let ihdr = decode_ihdr(&chunk, &mut tree, root).expect("valid IHDR");

        assert_eq!(ihdr.width, 1920);
        assert_eq!(ihdr.height, 1080);
        assert_eq!(ihdr.bit_depth, 8);
        assert_eq!(ihdr.color_type, 6);
        assert_eq!(ihdr.bytes_per_pixel(), 4);

        let labels: Vec<&str> = tree.get(root).children.iter()
            .map(|&id| tree.get(id).label.as_str())
            .collect();
        assert_eq!(labels, ["width", "height", "bitDepth", "colorType", "compression", "filter", "interlace"]);

        let color_node = tree.get(tree.get(root).children[3]);
        assert_eq!(color_node.value, Some(Value::Enum { raw: 6, name: "RGBA" }));
        // Field ranges point at the real file offsets, not at chunk-local ones.
        assert_eq!(tree.get(tree.get(root).children[0]).range, ByteRange::new(8, 4));
    }

    #[test]
    fn marks_a_truncated_ihdr_without_panicking() {
        let chunk = fake_chunk(b"IHDR", &[0, 0, 7]);
        let mut tree = ParseTree::new();
        let root = tree.add(None, "PNG", ByteRange::new(0, 0), NodeKind::Container, None);

        assert!(decode_ihdr(&chunk, &mut tree, root).is_none());
        let child = tree.get(tree.get(root).children[0]);
        assert_eq!(child.kind, NodeKind::Error);
        assert_eq!(child.label, "IHDR truncated");
    }

    #[test]
    fn decodes_text_keyword_and_value() {
        let chunk = fake_chunk(b"tEXt", b"Author\0Ada");
        let mut tree = ParseTree::new();
        let root = tree.add(None, "tEXt", ByteRange::new(0, 0), NodeKind::Container, None);

        decode_text(&chunk, &mut tree, root);

        let children = &tree.get(root).children;
        assert_eq!(tree.get(children[0]).value, Some(Value::Text("Author".into())));
        assert_eq!(tree.get(children[1]).value, Some(Value::Text("Ada".into())));
    }

    #[test]
    fn flags_text_with_no_separator() {
        let chunk = fake_chunk(b"tEXt", b"no-null-here");
        let mut tree = ParseTree::new();
        let root = tree.add(None, "tEXt", ByteRange::new(0, 0), NodeKind::Container, None);

        decode_text(&chunk, &mut tree, root);

        let child = tree.get(tree.get(root).children[0]);
        assert_eq!(child.kind, NodeKind::Warning);
        assert_eq!(child.label, "missing keyword separator");
    }

    #[test]
    fn decodes_palette_entries_as_hex_colors() {
        let data = [0xFF, 0x00, 0x00, 0x00, 0x80, 0xFF];
        let chunk = fake_chunk(b"PLTE", &data);
        let mut tree = ParseTree::new();
        let root = tree.add(None, "PLTE", ByteRange::new(0, 0), NodeKind::Container, None);

        decode_plte(&chunk, &mut tree, root);

        let children = &tree.get(root).children;
        assert_eq!(tree.get(children[0]).value, Some(Value::U64(2)));
        assert_eq!(tree.get(children[1]).value, Some(Value::Text("#FF0000".into())));
        assert_eq!(tree.get(children[2]).value, Some(Value::Text("#0080FF".into())));
        // Second entry starts three bytes into the payload, which begins at 8.
        assert_eq!(tree.get(children[2]).range, ByteRange::new(11, 3));
    }

    #[test]
    fn warns_on_a_palette_length_that_is_not_a_multiple_of_three() {
        let chunk = fake_chunk(b"PLTE", &[1, 2, 3, 4]);
        let mut tree = ParseTree::new();
        let root = tree.add(None, "PLTE", ByteRange::new(0, 0), NodeKind::Container, None);

        decode_plte(&chunk, &mut tree, root);

        assert_eq!(tree.get(tree.get(root).children[0]).kind, NodeKind::Warning);
    }

    #[test]
    fn decodes_physical_pixel_dimensions() {
        // 2835 pixels per metre is 72 dpi — what most editors write.
        let mut data = Vec::new();
        data.extend_from_slice(&2835u32.to_be_bytes());
        data.extend_from_slice(&2835u32.to_be_bytes());
        data.push(1);
        let chunk = fake_chunk(b"pHYs", &data);
        let mut tree = ParseTree::new();
        let root = tree.add(None, "pHYs", ByteRange::new(0, 0), NodeKind::Container, None);

        decode_phys(&chunk, &mut tree, root);

        let children = &tree.get(root).children;
        assert_eq!(tree.get(children[0]).value, Some(Value::U64(2835)));
        assert_eq!(tree.get(children[1]).value, Some(Value::U64(2835)));
        assert_eq!(
            tree.get(children[2]).value,
            Some(Value::Enum { raw: 1, name: "metre" })
        );
        // The unit byte sits 8 bytes into a payload that starts at file offset 8.
        assert_eq!(tree.get(children[2]).range, ByteRange::new(16, 1));
    }

    #[test]
    fn marks_a_truncated_phys_without_panicking() {
        let chunk = fake_chunk(b"pHYs", &[0, 0]);
        let mut tree = ParseTree::new();
        let root = tree.add(None, "pHYs", ByteRange::new(0, 0), NodeKind::Container, None);

        decode_phys(&chunk, &mut tree, root);

        let child = tree.get(tree.get(root).children[0]);
        assert_eq!(child.kind, NodeKind::Error);
        assert_eq!(child.label, "pHYs truncated");
    }

    #[test]
    fn decodes_gamma_as_raw_and_decimal() {
        // 45455 is the value nearly every PNG writes: gamma 1/2.2.
        // Bound to a variable: a temporary would not outlive the borrow.
        let gamma = 45455u32.to_be_bytes();
        let chunk = fake_chunk(b"gAMA", &gamma);
        let mut tree = ParseTree::new();
        let root = tree.add(None, "gAMA", ByteRange::new(0, 0), NodeKind::Container, None);

        decode_gama(&chunk, &mut tree, root);

        let children = &tree.get(root).children;
        assert_eq!(tree.get(children[0]).value, Some(Value::U64(45455)));
        assert_eq!(tree.get(children[1]).value, Some(Value::Text("0.45455".into())));
    }

    #[test]
    fn marks_a_truncated_gama_without_panicking() {
        let chunk = fake_chunk(b"gAMA", &[0, 0]);
        let mut tree = ParseTree::new();
        let root = tree.add(None, "gAMA", ByteRange::new(0, 0), NodeKind::Container, None);

        decode_gama(&chunk, &mut tree, root);

        let child = tree.get(tree.get(root).children[0]);
        assert_eq!(child.kind, NodeKind::Error);
        assert_eq!(child.label, "gAMA truncated");
    }

    #[test]
    fn reads_trns_according_to_color_type() {
        let chunk = fake_chunk(b"tRNS", &[0, 64, 128]);
        let mut tree = ParseTree::new();

        let palette_root = tree.add(None, "tRNS", ByteRange::new(0, 0), NodeKind::Container, None);
        decode_trns(&chunk, &mut tree, palette_root, Some(3));
        let child = tree.get(tree.get(palette_root).children[0]);
        assert_eq!(child.label, "paletteAlphaCount");
        assert_eq!(child.value, Some(Value::U64(3)));

        let rgb_root = tree.add(None, "tRNS", ByteRange::new(0, 0), NodeKind::Container, None);
        decode_trns(&chunk, &mut tree, rgb_root, Some(2));
        assert_eq!(tree.get(tree.get(rgb_root).children[0]).label, "transparentColor");

        let orphan_root = tree.add(None, "tRNS", ByteRange::new(0, 0), NodeKind::Container, None);
        decode_trns(&chunk, &mut tree, orphan_root, None);
        assert_eq!(tree.get(tree.get(orphan_root).children[0]).kind, NodeKind::Warning);

        // Colour types 4 and 6 already carry an alpha channel, so the PNG spec
        // forbids tRNS there — the realistic way this warning fires in the wild.
        for forbidden in [4u8, 6] {
            let root = tree.add(None, "tRNS", ByteRange::new(0, 0), NodeKind::Container, None);
            decode_trns(&chunk, &mut tree, root, Some(forbidden));
            let child = tree.get(tree.get(root).children[0]);
            assert_eq!(child.kind, NodeKind::Warning, "colour type {forbidden}");
            assert_eq!(child.label, "tRNS without a usable colour type");
        }
    }
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p hexscope-core fields`
Expected: FAIL — `cannot find function decode_ihdr in this scope`.

- [ ] **Step 3: Implement the decoders**

Prepend to `crates/hexscope-core/src/png/fields.rs`:

```rust
use crate::model::{ByteRange, NodeId, NodeKind, ParseTree, Value};
use crate::png::chunks::Chunk;
use crate::reader::Reader;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ihdr {
    pub width: u32,
    pub height: u32,
    pub bit_depth: u8,
    pub color_type: u8,
    pub interlace: u8,
}

impl Ihdr {
    /// Bytes each pixel occupies once unfiltered. Sub-byte depths round up to
    /// 1, which is what the filter algorithm needs.
    pub fn bytes_per_pixel(&self) -> usize {
        let channels = match self.color_type {
            0 => 1, // greyscale
            2 => 3, // RGB
            3 => 1, // palette index
            4 => 2, // greyscale + alpha
            6 => 4, // RGBA
            _ => 1,
        };
        ((channels * self.bit_depth as usize) / 8).max(1)
    }
}

fn color_type_name(raw: u8) -> &'static str {
    match raw {
        0 => "Greyscale",
        2 => "RGB",
        3 => "Palette",
        4 => "Greyscale+Alpha",
        6 => "RGBA",
        _ => "unknown",
    }
}

/// Adds one field node whose range is expressed in file coordinates.
fn field(
    tree: &mut ParseTree,
    parent: NodeId,
    label: &str,
    chunk: &Chunk,
    offset: u64,
    len: u64,
    value: Value,
) {
    let range = ByteRange::new(chunk.data_range.start + offset, len);
    tree.add(Some(parent), label, range, NodeKind::Field, Some(value));
}

pub fn decode_ihdr(chunk: &Chunk, tree: &mut ParseTree, parent: NodeId) -> Option<Ihdr> {
    let mut r = Reader::new(chunk.data);

    // All thirteen bytes are read up front; if any read fails the chunk is
    // damaged and we record one Error node rather than a half-filled tree.
    let (Ok(width), Ok(height), Ok(bit_depth), Ok(color_type), Ok(compression), Ok(filter), Ok(interlace)) = (
        r.u32_be(),
        r.u32_be(),
        r.u8(),
        r.u8(),
        r.u8(),
        r.u8(),
        r.u8(),
    ) else {
        tree.add(
            Some(parent),
            "IHDR truncated",
            chunk.data_range,
            NodeKind::Error,
            None,
        );
        return None;
    };

    field(tree, parent, "width", chunk, 0, 4, Value::U64(width as u64));
    field(tree, parent, "height", chunk, 4, 4, Value::U64(height as u64));
    field(tree, parent, "bitDepth", chunk, 8, 1, Value::U64(bit_depth as u64));
    field(
        tree,
        parent,
        "colorType",
        chunk,
        9,
        1,
        Value::Enum { raw: color_type as u64, name: color_type_name(color_type) },
    );
    field(tree, parent, "compression", chunk, 10, 1, Value::U64(compression as u64));
    field(tree, parent, "filter", chunk, 11, 1, Value::U64(filter as u64));
    field(tree, parent, "interlace", chunk, 12, 1, Value::U64(interlace as u64));

    Some(Ihdr { width, height, bit_depth, color_type, interlace })
}

/// PLTE is a flat array of RGB triples. Each entry becomes a field so hovering
/// a palette index in the hex view lights up the exact three bytes.
pub fn decode_plte(chunk: &Chunk, tree: &mut ParseTree, parent: NodeId) {
    if !chunk.data.len().is_multiple_of(3) {
        tree.add(
            Some(parent),
            "PLTE length is not a multiple of 3",
            chunk.data_range,
            NodeKind::Warning,
            None,
        );
        return;
    }

    field(
        tree,
        parent,
        "entries",
        chunk,
        0,
        chunk.data.len() as u64,
        Value::U64((chunk.data.len() / 3) as u64),
    );

    // `array::<3>` yields a fixed-size array, so indexing it is checked at
    // compile time rather than being a raw slice index.
    let mut r = Reader::new(chunk.data);
    let mut i = 0u64;
    while let Ok(rgb) = r.array::<3>() {
        field(
            tree,
            parent,
            &format!("entry {i}"),
            chunk,
            i * 3,
            3,
            Value::Text(format!("#{:02X}{:02X}{:02X}", rgb[0], rgb[1], rgb[2])),
        );
        i += 1;
    }
}

/// gAMA stores gamma × 100000 as a big-endian u32.
pub fn decode_gama(chunk: &Chunk, tree: &mut ParseTree, parent: NodeId) {
    let mut r = Reader::new(chunk.data);
    let Ok(raw) = r.u32_be() else {
        tree.add(
            Some(parent),
            "gAMA truncated",
            chunk.data_range,
            NodeKind::Error,
            None,
        );
        return;
    };

    field(tree, parent, "gamma", chunk, 0, 4, Value::U64(raw as u64));
    field(
        tree,
        parent,
        "gammaDecimal",
        chunk,
        0,
        4,
        Value::Text(format!("{:.5}", raw as f64 / 100_000.0)),
    );
}

/// tRNS means different things per colour type, so the label says which
/// reading applies instead of silently guessing.
pub fn decode_trns(chunk: &Chunk, tree: &mut ParseTree, parent: NodeId, color_type: Option<u8>) {
    match color_type {
        Some(3) => {
            field(
                tree,
                parent,
                "paletteAlphaCount",
                chunk,
                0,
                chunk.data.len() as u64,
                Value::U64(chunk.data.len() as u64),
            );
        }
        Some(0) | Some(2) => {
            field(
                tree,
                parent,
                "transparentColor",
                chunk,
                0,
                chunk.data.len() as u64,
                Value::Bytes(chunk.data.len() as u64),
            );
        }
        _ => {
            tree.add(
                Some(parent),
                "tRNS without a usable colour type",
                chunk.data_range,
                NodeKind::Warning,
                None,
            );
        }
    }
}

pub fn decode_text(chunk: &Chunk, tree: &mut ParseTree, parent: NodeId) {
    let mut r = Reader::new(chunk.data);

    let Some(keyword_bytes) = r.bytes_until(0) else {
        tree.add(
            Some(parent),
            "missing keyword separator",
            chunk.data_range,
            NodeKind::Warning,
            None,
        );
        return;
    };
    let text_bytes = r.rest();

    let sep = keyword_bytes.len() as u64;
    let keyword = String::from_utf8_lossy(keyword_bytes).into_owned();
    let text = String::from_utf8_lossy(text_bytes).into_owned();

    field(tree, parent, "keyword", chunk, 0, sep, Value::Text(keyword));
    field(
        tree,
        parent,
        "text",
        chunk,
        sep + 1,
        text_bytes.len() as u64,
        Value::Text(text),
    );
}

pub fn decode_phys(chunk: &Chunk, tree: &mut ParseTree, parent: NodeId) {
    let mut r = Reader::new(chunk.data);
    let (Ok(x), Ok(y), Ok(unit)) = (r.u32_be(), r.u32_be(), r.u8()) else {
        tree.add(
            Some(parent),
            "pHYs truncated",
            chunk.data_range,
            NodeKind::Error,
            None,
        );
        return;
    };

    field(tree, parent, "pixelsPerUnitX", chunk, 0, 4, Value::U64(x as u64));
    field(tree, parent, "pixelsPerUnitY", chunk, 4, 4, Value::U64(y as u64));
    field(
        tree,
        parent,
        "unit",
        chunk,
        8,
        1,
        Value::Enum { raw: unit as u64, name: if unit == 1 { "metre" } else { "unknown" } },
    );
}
```

Note the `chunk.data[10]` and `chunk.data[11]` indexing is safe here: reaching that line means all thirteen IHDR bytes were read successfully.

Add to `crates/hexscope-core/src/png/mod.rs`:

```rust
pub mod fields;
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p hexscope-core fields`
Expected: PASS — `test result: ok. 11 passed`.

- [ ] **Step 5: Commit**

```bash
git add crates/hexscope-core/src/png/
git commit -m "feat(core): decode IHDR, PLTE, tEXt, pHYs, gAMA and tRNS payloads"
```

---

### Task 5: LSB-first bit reader

**Files:**
- Create: `crates/hexscope-core/src/bits.rs`
- Modify: `crates/hexscope-core/src/lib.rs`

**Interfaces:**
- Consumes: nothing.
- Produces: `BitReader<'a>` with `new(&'a [u8])`, `bits(u32) -> Result<u32, BitError>`, `align()`, `bit_pos() -> u64`, `byte_pos() -> usize`, `seek_bits(u64)`, `bytes(usize) -> Result<&'a [u8], BitError>`; `BitError::Eof`.

- [ ] **Step 1: Write the failing tests**

`crates/hexscope-core/src/bits.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_bits_least_significant_first() {
        // 0b1011_0101 — DEFLATE consumes from the low end.
        let data = [0b1011_0101];
        let mut br = BitReader::new(&data);
        assert_eq!(br.bits(1), Ok(1));
        assert_eq!(br.bits(2), Ok(0b10));
        assert_eq!(br.bits(5), Ok(0b1011_0));
        assert_eq!(br.bit_pos(), 8);
    }

    #[test]
    fn spans_byte_boundaries() {
        let data = [0xFF, 0x01];
        let mut br = BitReader::new(&data);
        assert_eq!(br.bits(10), Ok(0b01_1111_1111));
    }

    #[test]
    fn zero_bits_is_always_zero() {
        let data = [0xAB];
        let mut br = BitReader::new(&data);
        assert_eq!(br.bits(0), Ok(0));
        assert_eq!(br.bit_pos(), 0);
    }

    #[test]
    fn align_moves_to_the_next_byte() {
        let data = [0xFF, 0xAA];
        let mut br = BitReader::new(&data);
        assert_eq!(br.bits(3), Ok(0b111));
        br.align();
        assert_eq!(br.byte_pos(), 1);
        assert_eq!(br.bits(8), Ok(0xAA));
    }

    #[test]
    fn reports_eof_instead_of_panicking() {
        let data = [0x01];
        let mut br = BitReader::new(&data);
        assert_eq!(br.bits(9), Err(BitError::Eof));
    }
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p hexscope-core bits`
Expected: FAIL — `cannot find type BitReader in this scope`.

- [ ] **Step 3: Implement the bit reader**

Prepend to `crates/hexscope-core/src/bits.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BitError {
    Eof,
}

/// Reads bits low-to-high within each byte, which is the order DEFLATE uses.
/// The position is tracked in bits so the UI can point at an exact bit offset.
#[derive(Debug, Clone)]
pub struct BitReader<'a> {
    data: &'a [u8],
    bit_pos: u64,
}

impl<'a> BitReader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, bit_pos: 0 }
    }

    pub fn bit_pos(&self) -> u64 {
        self.bit_pos
    }

    pub fn byte_pos(&self) -> usize {
        (self.bit_pos / 8) as usize
    }

    /// Used to resume decoding from a checkpoint.
    pub fn seek_bits(&mut self, bit_pos: u64) {
        self.bit_pos = bit_pos.min(self.data.len() as u64 * 8);
    }

    pub fn bits(&mut self, n: u32) -> Result<u32, BitError> {
        debug_assert!(n <= 32);
        let mut out = 0u32;
        for i in 0..n {
            let byte = self
                .data
                .get((self.bit_pos / 8) as usize)
                .ok_or(BitError::Eof)?;
            let bit = (byte >> (self.bit_pos % 8)) & 1;
            out |= (bit as u32) << i;
            self.bit_pos += 1;
        }
        Ok(out)
    }

    /// Discards bits up to the next byte boundary (used before stored blocks).
    pub fn align(&mut self) {
        self.bit_pos = self.bit_pos.div_ceil(8) * 8;
    }

    /// Reads whole bytes from the current (aligned) position.
    pub fn bytes(&mut self, n: usize) -> Result<&'a [u8], BitError> {
        self.align();
        let start = self.byte_pos();
        let end = start.checked_add(n).ok_or(BitError::Eof)?;
        if end > self.data.len() {
            return Err(BitError::Eof);
        }
        self.bit_pos = end as u64 * 8;
        Ok(&self.data[start..end])
    }
}
```

Add to `crates/hexscope-core/src/lib.rs`:

```rust
pub mod bits;
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p hexscope-core bits`
Expected: PASS — `test result: ok. 5 passed`.

- [ ] **Step 5: Commit**

```bash
git add crates/hexscope-core/src/bits.rs crates/hexscope-core/src/lib.rs
git commit -m "feat(core): add LSB-first bit reader"
```

---

### Task 6: Canonical Huffman decoding

**Files:**
- Create: `crates/hexscope-core/src/inflate/mod.rs`
- Create: `crates/hexscope-core/src/inflate/huffman.rs`
- Modify: `crates/hexscope-core/src/lib.rs`

**Interfaces:**
- Consumes: `BitReader`, `BitError` (Task 5).
- Produces: `InflateError::{Bits, BadHuffmanCode, BadCodeLengths, BadDistance, BadSymbol, BadZlibHeader, ChecksumMismatch, OutputTooLarge}`; `Huffman` with `from_lengths(&[u8]) -> Result<Huffman, InflateError>` and `decode(&self, &mut BitReader) -> Result<u16, InflateError>`.

- [ ] **Step 1: Write the failing tests**

`crates/hexscope-core/src/inflate/huffman.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_a_three_symbol_canonical_code() {
        // Lengths [1, 2, 2] give codes A=0, B=10, C=11 (MSB-first on the wire).
        let h = Huffman::from_lengths(&[1, 2, 2]).unwrap();

        // Bits are emitted LSB-first, so "0" then "10" then "11" is
        // 0, 0,1, 1,1 => 0b11010 = 0x1A in the first byte.
        let data = [0b0001_1010];
        let mut br = BitReader::new(&data);
        assert_eq!(h.decode(&mut br), Ok(0));
        assert_eq!(h.decode(&mut br), Ok(1));
        assert_eq!(h.decode(&mut br), Ok(2));
    }

    #[test]
    fn ignores_symbols_with_zero_length() {
        let h = Huffman::from_lengths(&[0, 1, 0, 1]).unwrap();
        let data = [0b0000_0010];
        let mut br = BitReader::new(&data);
        assert_eq!(h.decode(&mut br), Ok(1));
        assert_eq!(h.decode(&mut br), Ok(3));
    }

    #[test]
    fn rejects_an_oversubscribed_code() {
        // Three symbols of length 1 cannot fit in a binary tree.
        assert_eq!(
            Huffman::from_lengths(&[1, 1, 1]),
            Err(InflateError::BadCodeLengths)
        );
    }

    #[test]
    fn reports_an_invalid_code_instead_of_looping() {
        let h = Huffman::from_lengths(&[1, 2, 2]).unwrap();
        let data = [];
        let mut br = BitReader::new(&data);
        assert_eq!(h.decode(&mut br), Err(InflateError::Bits));
    }
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p hexscope-core huffman`
Expected: FAIL — `cannot find type Huffman in this scope`.

- [ ] **Step 3: Implement Huffman decoding**

Prepend to `crates/hexscope-core/src/inflate/huffman.rs`:

```rust
use crate::bits::BitReader;
use crate::inflate::InflateError;

pub const MAX_BITS: usize = 15;

/// A canonical Huffman code stored as symbol counts per length plus symbols in
/// canonical order — the decode walks lengths instead of building a tree, which
/// keeps construction cheap and allocation-free per symbol.
#[derive(Debug, Clone)]
pub struct Huffman {
    counts: [u16; MAX_BITS + 1],
    symbols: Vec<u16>,
}

impl Huffman {
    pub fn from_lengths(lengths: &[u8]) -> Result<Self, InflateError> {
        let mut counts = [0u16; MAX_BITS + 1];
        for &len in lengths {
            if len as usize > MAX_BITS {
                return Err(InflateError::BadCodeLengths);
            }
            counts[len as usize] += 1;
        }
        counts[0] = 0;

        // Kraft inequality: a code is valid when it is not oversubscribed.
        let mut left = 1i32;
        for len in 1..=MAX_BITS {
            left <<= 1;
            left -= counts[len] as i32;
            if left < 0 {
                return Err(InflateError::BadCodeLengths);
            }
        }

        // `cursor[len]` is where the next symbol of that length goes, so
        // symbols end up grouped by length in canonical order.
        let mut cursor = [0u16; MAX_BITS + 1];
        for len in 2..=MAX_BITS {
            cursor[len] = cursor[len - 1] + counts[len - 1];
        }

        let mut symbols = vec![0u16; lengths.len()];
        for (symbol, &len) in lengths.iter().enumerate() {
            if len != 0 {
                let slot = cursor[len as usize] as usize;
                symbols[slot] = symbol as u16;
                cursor[len as usize] += 1;
            }
        }

        Ok(Self { counts, symbols })
    }

    pub fn decode(&self, br: &mut BitReader) -> Result<u16, InflateError> {
        // Walks lengths shortest-first: `first` is the smallest code of this
        // length, `index` the offset of that length's symbols in `symbols`.
        let mut code = 0i32;
        let mut first = 0i32;
        let mut index = 0i32;

        for len in 1..=MAX_BITS {
            code |= br.bits(1).map_err(|_| InflateError::Bits)? as i32;
            let count = self.counts[len] as i32;
            if code - first < count {
                return Ok(self.symbols[(index + (code - first)) as usize]);
            }
            index += count;
            first = (first + count) << 1;
            code <<= 1;
        }

        Err(InflateError::BadHuffmanCode)
    }
}
```


`crates/hexscope-core/src/inflate/mod.rs`:

```rust
pub mod huffman;

pub use huffman::Huffman;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InflateError {
    /// Ran out of input bits.
    Bits,
    /// Bit pattern matches no code in the table.
    BadHuffmanCode,
    /// Code lengths do not form a valid canonical code.
    BadCodeLengths,
    /// Back-reference points before the start of the output.
    BadDistance,
    /// Symbol outside the defined alphabet.
    BadSymbol,
    /// zlib CMF/FLG bytes are not a DEFLATE stream.
    BadZlibHeader,
    /// Adler-32 over the output does not match the stored value.
    ChecksumMismatch,
    /// Output exceeded the caller's limit.
    OutputTooLarge,
}
```

Add to `crates/hexscope-core/src/lib.rs`:

```rust
pub mod inflate;
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p hexscope-core huffman`
Expected: PASS — `test result: ok. 4 passed`.

- [ ] **Step 5: Commit**

```bash
git add crates/hexscope-core/src/inflate/ crates/hexscope-core/src/lib.rs
git commit -m "feat(core): add canonical huffman decoder"
```

---

### Task 7: Inflate with an event sink

**Files:**
- Create: `crates/hexscope-core/src/inflate/tables.rs`
- Create: `crates/hexscope-core/src/inflate/engine.rs`
- Modify: `crates/hexscope-core/src/inflate/mod.rs`

**Interfaces:**
- Consumes: `BitReader` (Task 5), `Huffman`, `InflateError` (Task 6).
- Produces: `BlockKind::{Stored, Fixed, Dynamic}`; `InflateEvent::{BlockStart { kind, bit_pos }, Literal { byte }, Match { distance, length }, BlockEnd}`; `EventSink` trait with `emit(&mut self, InflateEvent)`; `NoTrace` sink; `inflate(&[u8], u64, &mut dyn EventSink) -> Result<Vec<u8>, InflateError>` where the `u64` is the output size limit.

- [ ] **Step 1: Write the tables**

`crates/hexscope-core/src/inflate/tables.rs`:

```rust
/// Length codes 257..=285: base length and extra bits (RFC 1951 §3.2.5).
pub const LENGTH_BASE: [u16; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131,
    163, 195, 227, 258,
];
pub const LENGTH_EXTRA: [u8; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];

/// Distance codes 0..=29: base distance and extra bits.
pub const DIST_BASE: [u16; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];
pub const DIST_EXTRA: [u8; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13,
    13,
];

/// The order code lengths appear in a dynamic block header.
pub const CODE_LENGTH_ORDER: [usize; 19] = [
    16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
];

/// Literal/length code lengths for a fixed-Huffman block.
pub fn fixed_literal_lengths() -> [u8; 288] {
    let mut lengths = [0u8; 288];
    for (symbol, len) in lengths.iter_mut().enumerate() {
        *len = match symbol {
            0..=143 => 8,
            144..=255 => 9,
            256..=279 => 7,
            _ => 8,
        };
    }
    lengths
}

/// Distance code lengths for a fixed-Huffman block: all five bits.
pub fn fixed_distance_lengths() -> [u8; 30] {
    [5u8; 30]
}
```

- [ ] **Step 2: Write the failing engine tests**

`crates/hexscope-core/src/inflate/engine.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use flate2::Compression;
    use flate2::write::DeflateEncoder;
    use std::io::Write;

    fn deflate(input: &[u8], level: Compression) -> Vec<u8> {
        let mut enc = DeflateEncoder::new(Vec::new(), level);
        enc.write_all(input).unwrap();
        enc.finish().unwrap()
    }

    fn roundtrip(input: &[u8], level: Compression) {
        let compressed = deflate(input, level);
        let out = inflate(&compressed, u64::MAX, &mut NoTrace).expect("inflate succeeds");
        assert_eq!(out, input, "roundtrip mismatch at level {level:?}");
    }

    #[test]
    fn inflates_stored_blocks() {
        roundtrip(b"hexscope stored block", Compression::none());
    }

    #[test]
    fn inflates_fixed_blocks() {
        roundtrip(b"aaaaaaaaaaaaaaaaaaaaaaaabbbbcccc", Compression::fast());
    }

    #[test]
    fn inflates_dynamic_blocks() {
        let mut input = Vec::new();
        for i in 0..8000u32 {
            input.extend_from_slice(format!("line {} of the corpus\n", i % 97).as_bytes());
        }
        roundtrip(&input, Compression::best());
    }

    #[test]
    fn inflates_empty_input() {
        roundtrip(b"", Compression::fast());
    }

    #[test]
    fn emits_events_that_reconstruct_the_output() {
        // Highly repetitive input guarantees at least one back-reference,
        // without depending on exactly how flate2 chooses to split matches.
        let compressed = deflate(b"abcabcabcabcabcabcabcabc", Compression::best());
        let mut trace = CollectEvents::default();
        let out = inflate(&compressed, u64::MAX, &mut trace).unwrap();

        assert_eq!(out, b"abcabcabcabcabcabcabcabc");
        assert!(matches!(trace.events.first(), Some(InflateEvent::BlockStart { .. })));
        assert_eq!(trace.events.last(), Some(&InflateEvent::BlockEnd));

        let match_count = trace
            .events
            .iter()
            .filter(|e| matches!(e, InflateEvent::Match { .. }))
            .count();
        assert!(match_count >= 1, "repetitive input must produce a back-reference");

        // Replaying the events by hand must rebuild the exact output — this is
        // the property the animation depends on.
        let mut replayed: Vec<u8> = Vec::new();
        for event in &trace.events {
            match *event {
                InflateEvent::Literal { byte } => replayed.push(byte),
                InflateEvent::Match { distance, length } => {
                    let start = replayed.len() - distance as usize;
                    for i in 0..length as usize {
                        let byte = replayed[start + i];
                        replayed.push(byte);
                    }
                }
                _ => {}
            }
        }
        assert_eq!(replayed, out);
    }

    #[test]
    fn returns_an_error_for_a_truncated_stream() {
        let compressed = deflate(b"some reasonably compressible content here", Compression::best());
        // Cut the stream in half: the decoder must give up cleanly.
        let result = inflate(&compressed[..compressed.len() / 2], u64::MAX, &mut NoTrace);
        assert!(result.is_err(), "expected an error, got {result:?}");
    }

    #[test]
    fn returns_an_error_for_a_reserved_block_type() {
        // Block type 3 is reserved and must be rejected, not guessed at.
        // Bits: BFINAL=1, BTYPE=11 -> 0b111 in the low three bits.
        let result = inflate(&[0b0000_0111], u64::MAX, &mut NoTrace);
        assert_eq!(result, Err(InflateError::BadSymbol));
    }

    #[test]
    fn stops_at_the_output_limit() {
        let input = vec![b'x'; 100_000];
        let compressed = deflate(&input, Compression::best());
        assert_eq!(
            inflate(&compressed, 1000, &mut NoTrace),
            Err(InflateError::OutputTooLarge)
        );
    }

    #[test]
    fn never_panics_on_random_bytes() {
        for seed in 0..500u32 {
            let bytes: Vec<u8> = (0..64u32)
                .map(|i| (seed.wrapping_mul(2654435761).wrapping_add(i * 40503) >> 13) as u8)
                .collect();
            let _ = inflate(&bytes, 1 << 20, &mut NoTrace);
        }
    }

    #[derive(Default)]
    struct CollectEvents {
        events: Vec<InflateEvent>,
    }

    impl EventSink for CollectEvents {
        fn emit(&mut self, event: InflateEvent) {
            self.events.push(event);
        }
    }
}
```

- [ ] **Step 3: Run them to verify they fail**

Run: `cargo test -p hexscope-core engine`
Expected: FAIL — `cannot find function inflate in this scope`.

- [ ] **Step 4: Implement the engine**

Prepend to `crates/hexscope-core/src/inflate/engine.rs`:

```rust
use crate::bits::{BitError, BitReader};
use crate::inflate::huffman::Huffman;
use crate::inflate::tables::{
    CODE_LENGTH_ORDER, DIST_BASE, DIST_EXTRA, LENGTH_BASE, LENGTH_EXTRA, fixed_distance_lengths,
    fixed_literal_lengths,
};
use crate::inflate::InflateError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockKind {
    Stored,
    Fixed,
    Dynamic,
}

/// One decoding step, as the UI wants to animate it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InflateEvent {
    BlockStart { kind: BlockKind, bit_pos: u64 },
    Literal { byte: u8 },
    Match { distance: u16, length: u16 },
    BlockEnd,
}

/// Receives events as decoding proceeds. Keeping this a sink rather than a
/// returned `Vec` is what lets a caller decode a 200 MB stream without holding
/// millions of events in memory.
pub trait EventSink {
    fn emit(&mut self, event: InflateEvent);
}

/// Sink that discards everything — for plain decoding.
pub struct NoTrace;

impl EventSink for NoTrace {
    fn emit(&mut self, _event: InflateEvent) {}
}

fn bit_err(_: BitError) -> InflateError {
    InflateError::Bits
}

/// Decompresses a raw DEFLATE stream. `max_output` caps the result so a crafted
/// file cannot exhaust memory.
pub fn inflate(
    data: &[u8],
    max_output: u64,
    sink: &mut dyn EventSink,
) -> Result<Vec<u8>, InflateError> {
    let mut br = BitReader::new(data);
    let mut out: Vec<u8> = Vec::new();

    loop {
        let is_final = br.bits(1).map_err(bit_err)? == 1;
        let kind = match br.bits(2).map_err(bit_err)? {
            0 => BlockKind::Stored,
            1 => BlockKind::Fixed,
            2 => BlockKind::Dynamic,
            _ => return Err(InflateError::BadSymbol),
        };
        sink.emit(InflateEvent::BlockStart { kind, bit_pos: br.bit_pos() });

        match kind {
            BlockKind::Stored => {
                let header = br.bytes(4).map_err(bit_err)?;
                let len = u16::from_le_bytes([header[0], header[1]]) as usize;
                let nlen = u16::from_le_bytes([header[2], header[3]]);
                if nlen != !(len as u16) {
                    return Err(InflateError::BadSymbol);
                }
                if out.len() as u64 + len as u64 > max_output {
                    return Err(InflateError::OutputTooLarge);
                }
                let payload = br.bytes(len).map_err(bit_err)?;
                for &byte in payload {
                    sink.emit(InflateEvent::Literal { byte });
                }
                out.extend_from_slice(payload);
            }
            BlockKind::Fixed => {
                let lit = Huffman::from_lengths(&fixed_literal_lengths())?;
                let dist = Huffman::from_lengths(&fixed_distance_lengths())?;
                decode_block(&mut br, &lit, &dist, &mut out, max_output, sink)?;
            }
            BlockKind::Dynamic => {
                let (lit, dist) = read_dynamic_tables(&mut br)?;
                decode_block(&mut br, &lit, &dist, &mut out, max_output, sink)?;
            }
        }

        sink.emit(InflateEvent::BlockEnd);
        if is_final {
            break;
        }
    }

    Ok(out)
}

fn read_dynamic_tables(br: &mut BitReader) -> Result<(Huffman, Huffman), InflateError> {
    let hlit = br.bits(5).map_err(bit_err)? as usize + 257;
    let hdist = br.bits(5).map_err(bit_err)? as usize + 1;
    let hclen = br.bits(4).map_err(bit_err)? as usize + 4;

    let mut code_lengths = [0u8; 19];
    for &slot in CODE_LENGTH_ORDER.iter().take(hclen) {
        code_lengths[slot] = br.bits(3).map_err(bit_err)? as u8;
    }
    let code_huff = Huffman::from_lengths(&code_lengths)?;

    let total = hlit + hdist;
    let mut lengths = vec![0u8; total];
    let mut i = 0;
    while i < total {
        let symbol = code_huff.decode(br)?;
        match symbol {
            0..=15 => {
                lengths[i] = symbol as u8;
                i += 1;
            }
            16 => {
                if i == 0 {
                    return Err(InflateError::BadCodeLengths);
                }
                let prev = lengths[i - 1];
                let repeat = 3 + br.bits(2).map_err(bit_err)? as usize;
                if i + repeat > total {
                    return Err(InflateError::BadCodeLengths);
                }
                lengths[i..i + repeat].fill(prev);
                i += repeat;
            }
            17 => {
                let repeat = 3 + br.bits(3).map_err(bit_err)? as usize;
                if i + repeat > total {
                    return Err(InflateError::BadCodeLengths);
                }
                i += repeat;
            }
            18 => {
                let repeat = 11 + br.bits(7).map_err(bit_err)? as usize;
                if i + repeat > total {
                    return Err(InflateError::BadCodeLengths);
                }
                i += repeat;
            }
            _ => return Err(InflateError::BadSymbol),
        }
    }

    let lit = Huffman::from_lengths(&lengths[..hlit])?;
    let dist = Huffman::from_lengths(&lengths[hlit..])?;
    Ok((lit, dist))
}

fn decode_block(
    br: &mut BitReader,
    lit: &Huffman,
    dist: &Huffman,
    out: &mut Vec<u8>,
    max_output: u64,
    sink: &mut dyn EventSink,
) -> Result<(), InflateError> {
    loop {
        let symbol = lit.decode(br)?;
        match symbol {
            0..=255 => {
                if out.len() as u64 + 1 > max_output {
                    return Err(InflateError::OutputTooLarge);
                }
                let byte = symbol as u8;
                out.push(byte);
                sink.emit(InflateEvent::Literal { byte });
            }
            256 => return Ok(()),
            257..=285 => {
                let idx = symbol as usize - 257;
                let length = LENGTH_BASE[idx] as u32
                    + br.bits(LENGTH_EXTRA[idx] as u32).map_err(bit_err)?;

                let dist_symbol = dist.decode(br)? as usize;
                if dist_symbol >= DIST_BASE.len() {
                    return Err(InflateError::BadSymbol);
                }
                let distance = DIST_BASE[dist_symbol] as u32
                    + br.bits(DIST_EXTRA[dist_symbol] as u32).map_err(bit_err)?;

                if distance as usize > out.len() {
                    return Err(InflateError::BadDistance);
                }
                if out.len() as u64 + length as u64 > max_output {
                    return Err(InflateError::OutputTooLarge);
                }

                let start = out.len() - distance as usize;
                for i in 0..length as usize {
                    // Byte-by-byte on purpose: overlapping copies are legal and
                    // are how run-length encoding falls out of LZ77.
                    let byte = out[start + i];
                    out.push(byte);
                }
                sink.emit(InflateEvent::Match {
                    distance: distance as u16,
                    length: length as u16,
                });
            }
            _ => return Err(InflateError::BadSymbol),
        }
    }
}
```

Add to `crates/hexscope-core/src/inflate/mod.rs`:

```rust
pub mod engine;
pub mod tables;

pub use engine::{BlockKind, EventSink, InflateEvent, NoTrace, inflate};
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p hexscope-core engine`
Expected: PASS — `test result: ok. 9 passed`.

- [ ] **Step 6: Commit**

```bash
git add crates/hexscope-core/src/inflate/
git commit -m "feat(core): implement inflate with event sink"
```

---

### Task 8: Checkpoints and trace replay

**Files:**
- Create: `crates/hexscope-core/src/inflate/trace.rs`
- Modify: `crates/hexscope-core/src/inflate/mod.rs`

**Interfaces:**
- Consumes: `EventSink`, `InflateEvent`, `inflate` (Task 7).
- Produces: `Checkpoint { event_index: u64, bit_pos: u64, out_pos: u64 }`; `CheckpointSink::new(interval: u64)` implementing `EventSink`, plus `finish(self) -> TraceSummary`; `TraceSummary { total_events, literals, matches, output_bytes, checkpoints }` with `coding_events() -> u64`.

- [ ] **Step 1: Write the failing tests**

`crates/hexscope-core/src/inflate/trace.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::inflate::engine::inflate;
    use flate2::Compression;
    use flate2::write::DeflateEncoder;
    use std::io::Write;

    fn deflate(input: &[u8]) -> Vec<u8> {
        let mut enc = DeflateEncoder::new(Vec::new(), Compression::best());
        enc.write_all(input).unwrap();
        enc.finish().unwrap()
    }

    fn corpus() -> Vec<u8> {
        let mut input = Vec::new();
        for i in 0..4000u32 {
            input.extend_from_slice(format!("row {} value {}\n", i, i % 31).as_bytes());
        }
        input
    }

    #[test]
    fn records_checkpoints_at_the_requested_interval() {
        let input = corpus();
        let compressed = deflate(&input);
        let mut sink = CheckpointSink::new(512);
        let out = inflate(&compressed, u64::MAX, &mut sink).unwrap();
        let summary = sink.finish();

        assert_eq!(out, input);
        assert!(summary.total_events > 2000, "got {}", summary.total_events);
        // The event stream must account for every decompressed byte.
        assert_eq!(summary.output_bytes, input.len() as u64);
        assert!(summary.matches > 0, "a repetitive corpus must produce matches");

        // One checkpoint per interval, give or take the final partial one.
        let expected = summary.total_events / 512;
        assert!(
            summary.checkpoints.len() as u64 >= expected
                && summary.checkpoints.len() as u64 <= expected + 1,
            "{} checkpoints for {} events",
            summary.checkpoints.len(),
            summary.total_events
        );
    }

    #[test]
    fn checkpoints_record_increasing_positions() {
        let compressed = deflate(&corpus());
        let mut sink = CheckpointSink::new(256);
        inflate(&compressed, u64::MAX, &mut sink).unwrap();
        let summary = sink.finish();

        for pair in summary.checkpoints.windows(2) {
            assert!(pair[1].event_index > pair[0].event_index);
            assert!(pair[1].out_pos >= pair[0].out_pos);
        }
    }

    #[test]
    fn an_empty_stream_produces_no_checkpoints() {
        let compressed = deflate(b"");
        let mut sink = CheckpointSink::new(64);
        inflate(&compressed, u64::MAX, &mut sink).unwrap();
        assert!(sink.finish().checkpoints.is_empty());
    }
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p hexscope-core trace`
Expected: FAIL — `cannot find type CheckpointSink in this scope`.

- [ ] **Step 3: Implement checkpoints**

Prepend to `crates/hexscope-core/src/inflate/trace.rs`:

```rust
use crate::inflate::engine::{EventSink, InflateEvent};

/// A resume point. Replaying from here needs the output produced so far, which
/// the caller already holds, plus the bit position in the compressed stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Checkpoint {
    pub event_index: u64,
    pub bit_pos: u64,
    pub out_pos: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceSummary {
    pub total_events: u64,
    pub literals: u64,
    pub matches: u64,
    /// Total decompressed size. Paired with the compressed size by the caller
    /// to show a ratio.
    pub output_bytes: u64,
    pub checkpoints: Vec<Checkpoint>,
}

impl TraceSummary {
    /// Events that produce output, i.e. excluding block markers.
    pub fn coding_events(&self) -> u64 {
        self.literals + self.matches
    }
}

/// Counts events and records a checkpoint every `interval` events, so the UI
/// can jump into the middle of a long stream without keeping every step.
pub struct CheckpointSink {
    interval: u64,
    index: u64,
    out_pos: u64,
    last_bit_pos: u64,
    literals: u64,
    matches: u64,
    checkpoints: Vec<Checkpoint>,
}

impl CheckpointSink {
    pub fn new(interval: u64) -> Self {
        assert!(interval > 0, "checkpoint interval must be positive");
        Self {
            interval,
            index: 0,
            out_pos: 0,
            last_bit_pos: 0,
            literals: 0,
            matches: 0,
            checkpoints: Vec::new(),
        }
    }

    pub fn finish(self) -> TraceSummary {
        TraceSummary {
            total_events: self.index,
            literals: self.literals,
            matches: self.matches,
            output_bytes: self.out_pos,
            checkpoints: self.checkpoints,
        }
    }
}

impl EventSink for CheckpointSink {
    fn emit(&mut self, event: InflateEvent) {
        match event {
            InflateEvent::BlockStart { bit_pos, .. } => self.last_bit_pos = bit_pos,
            InflateEvent::Literal { .. } => {
                self.literals += 1;
                self.out_pos += 1;
            }
            InflateEvent::Match { length, .. } => {
                self.matches += 1;
                self.out_pos += length as u64;
            }
            InflateEvent::BlockEnd => {}
        }

        self.index += 1;
        if self.index % self.interval == 0 {
            self.checkpoints.push(Checkpoint {
                event_index: self.index,
                bit_pos: self.last_bit_pos,
                out_pos: self.out_pos,
            });
        }
    }
}
```

Add to `crates/hexscope-core/src/inflate/mod.rs`:

```rust
pub mod trace;

pub use trace::{Checkpoint, CheckpointSink, TraceSummary};
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p hexscope-core trace`
Expected: PASS — `test result: ok. 3 passed`.

- [ ] **Step 5: Commit**

```bash
git add crates/hexscope-core/src/inflate/
git commit -m "feat(core): add checkpoint sink for deflate traces"
```

---

### Task 9: zlib wrapper and scanline unfiltering

**Files:**
- Create: `crates/hexscope-core/src/inflate/zlib.rs`
- Create: `crates/hexscope-core/src/png/unfilter.rs`
- Modify: `crates/hexscope-core/src/inflate/mod.rs`, `crates/hexscope-core/src/png/mod.rs`

**Interfaces:**
- Consumes: `inflate`, `EventSink`, `InflateError` (Tasks 6–7).
- Produces: `adler32(&[u8]) -> u32`; `zlib_decompress(&[u8], u64, &mut dyn EventSink) -> Result<Vec<u8>, InflateError>`; `unfilter(raw: &[u8], width: u32, height: u32, bpp: usize) -> Result<Vec<u8>, UnfilterError>`; `UnfilterError::{ShortData, BadFilterType(u8), BadDimensions}`.

- [ ] **Step 1: Write the failing zlib tests**

`crates/hexscope-core/src/inflate/zlib.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::inflate::NoTrace;
    use flate2::Compression;
    use flate2::write::ZlibEncoder;
    use std::io::Write;

    fn zlib(input: &[u8]) -> Vec<u8> {
        let mut enc = ZlibEncoder::new(Vec::new(), Compression::best());
        enc.write_all(input).unwrap();
        enc.finish().unwrap()
    }

    #[test]
    fn adler_matches_known_vector() {
        assert_eq!(adler32(b"Wikipedia"), 0x11E6_0398);
        assert_eq!(adler32(b""), 1);
    }

    #[test]
    fn decompresses_a_zlib_stream() {
        let input = b"hexscope zlib wrapper test, repeated repeated repeated";
        let out = zlib_decompress(&zlib(input), u64::MAX, &mut NoTrace).unwrap();
        assert_eq!(out, input);
    }

    #[test]
    fn rejects_a_non_deflate_method() {
        let bad = [0x79, 0x01, 0x00];
        assert_eq!(
            zlib_decompress(&bad, u64::MAX, &mut NoTrace),
            Err(InflateError::BadZlibHeader)
        );
    }

    #[test]
    fn detects_a_corrupted_checksum() {
        let mut stream = zlib(b"payload");
        let last = stream.len() - 1;
        stream[last] ^= 0xFF;
        assert_eq!(
            zlib_decompress(&stream, u64::MAX, &mut NoTrace),
            Err(InflateError::ChecksumMismatch)
        );
    }

    #[test]
    fn rejects_a_stream_too_short_for_a_header() {
        assert_eq!(
            zlib_decompress(&[0x78], u64::MAX, &mut NoTrace),
            Err(InflateError::BadZlibHeader)
        );
    }
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p hexscope-core zlib`
Expected: FAIL — `cannot find function adler32 in this scope`.

- [ ] **Step 3: Implement the zlib wrapper**

Prepend to `crates/hexscope-core/src/inflate/zlib.rs`:

```rust
use crate::inflate::engine::{EventSink, inflate};
use crate::inflate::InflateError;

const ADLER_MOD: u32 = 65521;

pub fn adler32(data: &[u8]) -> u32 {
    let mut a: u32 = 1;
    let mut b: u32 = 0;
    for &byte in data {
        a = (a + byte as u32) % ADLER_MOD;
        b = (b + a) % ADLER_MOD;
    }
    (b << 16) | a
}

/// Unwraps a zlib stream (RFC 1950) and inflates its payload. PNG stores IDAT
/// data in exactly this form.
pub fn zlib_decompress(
    data: &[u8],
    max_output: u64,
    sink: &mut dyn EventSink,
) -> Result<Vec<u8>, InflateError> {
    if data.len() < 6 {
        return Err(InflateError::BadZlibHeader);
    }
    let cmf = data[0];
    let flg = data[1];

    // Low nibble 8 means DEFLATE; the two header bytes must be a multiple of 31.
    if cmf & 0x0F != 8 || (((cmf as u16) << 8) | flg as u16) % 31 != 0 {
        return Err(InflateError::BadZlibHeader);
    }
    // A preset dictionary is legal zlib but never appears in PNG.
    if flg & 0x20 != 0 {
        return Err(InflateError::BadZlibHeader);
    }

    let body = &data[2..data.len() - 4];
    let stored = u32::from_be_bytes([
        data[data.len() - 4],
        data[data.len() - 3],
        data[data.len() - 2],
        data[data.len() - 1],
    ]);

    let out = inflate(body, max_output, sink)?;
    if adler32(&out) != stored {
        return Err(InflateError::ChecksumMismatch);
    }
    Ok(out)
}
```

Add to `crates/hexscope-core/src/inflate/mod.rs`:

```rust
pub mod zlib;

pub use zlib::{adler32, zlib_decompress};
```

- [ ] **Step 4: Run them to verify they pass**

Run: `cargo test -p hexscope-core zlib`
Expected: PASS — `test result: ok. 5 passed`.

- [ ] **Step 5: Write the failing unfilter tests**

`crates/hexscope-core/src/png/unfilter.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_none_passes_bytes_through() {
        // One row, 2 px, 1 byte per pixel, filter type 0.
        let raw = [0, 10, 20];
        assert_eq!(unfilter(&raw, 2, 1, 1).unwrap(), vec![10, 20]);
    }

    #[test]
    fn filter_sub_adds_the_pixel_to_the_left() {
        // filter 1: each byte is a delta from the byte one pixel back.
        let raw = [1, 10, 5, 5];
        assert_eq!(unfilter(&raw, 3, 1, 1).unwrap(), vec![10, 15, 20]);
    }

    #[test]
    fn filter_up_adds_the_row_above() {
        let raw = [0, 10, 20, 2, 5, 5];
        assert_eq!(unfilter(&raw, 2, 2, 1).unwrap(), vec![10, 20, 15, 25]);
    }

    #[test]
    fn filter_average_reconstructs_from_left_and_up() {
        // 2 px wide, 2 rows, 1 byte per pixel.
        // Row 0, filter 0: [8, 16].
        // Row 1, filter 3: out = delta + floor((left + up) / 2)
        //   x=0: left=0 (no pixel to the left), up=8  -> 4 + 4  = 8
        //   x=1: left=8,                        up=16 -> 4 + 12 = 16
        let raw = [0, 8, 16, 3, 4, 4];
        assert_eq!(unfilter(&raw, 2, 2, 1).unwrap(), vec![8, 16, 8, 16]);
    }

    #[test]
    fn filter_paeth_picks_the_closest_predictor() {
        // Row 0, filter 0: [8, 16].
        // Row 1, filter 4, deltas [0, 0]:
        //   x=0: paeth(left=0, up=8, upleft=0)  -> 8  -> out 8
        //   x=1: paeth(left=8, up=16, upleft=8) -> 16 -> out 16
        let raw = [0, 8, 16, 4, 0, 0];
        assert_eq!(unfilter(&raw, 2, 2, 1).unwrap(), vec![8, 16, 8, 16]);
    }

    #[test]
    fn paeth_predictor_matches_the_spec_examples() {
        assert_eq!(paeth(0, 0, 0), 0);
        assert_eq!(paeth(10, 20, 30), 10); // p=0:  pa=10, pb=20, pc=30
        assert_eq!(paeth(30, 20, 10), 30); // p=40: pa=10, pb=20, pc=30
        // p = 1 + 200 - 100 = 101; pa=100, pb=99, pc=1 -> upper-left wins.
        assert_eq!(paeth(1, 200, 100), 100);
    }

    #[test]
    fn rejects_an_unknown_filter_type() {
        let raw = [9, 1, 2];
        assert_eq!(unfilter(&raw, 2, 1, 1), Err(UnfilterError::BadFilterType(9)));
    }

    #[test]
    fn reports_short_data_instead_of_panicking() {
        let raw = [0, 1];
        assert_eq!(unfilter(&raw, 5, 1, 1), Err(UnfilterError::ShortData));
    }

    #[test]
    fn rejects_zero_dimensions() {
        assert_eq!(unfilter(&[], 0, 1, 1), Err(UnfilterError::BadDimensions));
    }
}
```

- [ ] **Step 6: Run them to verify they fail**

Run: `cargo test -p hexscope-core unfilter`
Expected: FAIL — `cannot find function unfilter in this scope`.

- [ ] **Step 7: Implement unfiltering**

Prepend to `crates/hexscope-core/src/png/unfilter.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnfilterError {
    /// The decompressed data is smaller than width × height requires.
    ShortData,
    BadFilterType(u8),
    BadDimensions,
}

/// PNG's Paeth predictor (RFC 2083 §6.6).
fn paeth(a: u8, b: u8, c: u8) -> u8 {
    let p = a as i32 + b as i32 - c as i32;
    let pa = (p - a as i32).abs();
    let pb = (p - b as i32).abs();
    let pc = (p - c as i32).abs();
    if pa <= pb && pa <= pc {
        a
    } else if pb <= pc {
        b
    } else {
        c
    }
}

/// Reverses the per-scanline filters PNG applies before compression. `raw` is
/// the decompressed IDAT payload: one filter-type byte per row, then the row.
pub fn unfilter(
    raw: &[u8],
    width: u32,
    height: u32,
    bpp: usize,
) -> Result<Vec<u8>, UnfilterError> {
    if width == 0 || height == 0 || bpp == 0 {
        return Err(UnfilterError::BadDimensions);
    }

    let row_len = (width as usize)
        .checked_mul(bpp)
        .ok_or(UnfilterError::BadDimensions)?;
    let needed = (row_len + 1)
        .checked_mul(height as usize)
        .ok_or(UnfilterError::BadDimensions)?;
    if raw.len() < needed {
        return Err(UnfilterError::ShortData);
    }

    let mut out = vec![0u8; row_len * height as usize];

    for y in 0..height as usize {
        let filter = raw[y * (row_len + 1)];
        let src = &raw[y * (row_len + 1) + 1..y * (row_len + 1) + 1 + row_len];

        for x in 0..row_len {
            let left = if x >= bpp { out[y * row_len + x - bpp] } else { 0 };
            let up = if y > 0 { out[(y - 1) * row_len + x] } else { 0 };
            let up_left = if y > 0 && x >= bpp {
                out[(y - 1) * row_len + x - bpp]
            } else {
                0
            };

            let value = match filter {
                0 => src[x],
                1 => src[x].wrapping_add(left),
                2 => src[x].wrapping_add(up),
                3 => src[x].wrapping_add(((left as u16 + up as u16) / 2) as u8),
                4 => src[x].wrapping_add(paeth(left, up, up_left)),
                other => return Err(UnfilterError::BadFilterType(other)),
            };
            out[y * row_len + x] = value;
        }
    }

    Ok(out)
}
```

Add to `crates/hexscope-core/src/png/mod.rs`:

```rust
pub mod unfilter;
```

- [ ] **Step 8: Run all tests to verify they pass**

Run: `cargo test -p hexscope-core`
Expected: PASS — all tests green, including the 9 new unfilter tests.

- [ ] **Step 9: Commit**

```bash
git add crates/hexscope-core/src/inflate/zlib.rs crates/hexscope-core/src/inflate/mod.rs crates/hexscope-core/src/png/
git commit -m "feat(core): add zlib wrapper and scanline unfiltering"
```

---

### Task 10: End-to-end `parse_png` with golden snapshots

**Files:**
- Modify: `crates/hexscope-core/src/png/mod.rs`
- Create: `crates/hexscope-core/tests/golden.rs`
- Create: `crates/hexscope-core/tests/fixtures/` (populated in this task)
- Modify: `crates/hexscope-core/src/lib.rs`

**Interfaces:**
- Consumes: everything from Tasks 1–9.
- Produces: `PngDocument { tree: ParseTree, ihdr: Option<Ihdr>, pixels: Option<Vec<u8>>, trace: Option<TraceSummary> }`; `parse_png(&[u8]) -> PngDocument` (infallible).

- [ ] **Step 1: Fetch the PngSuite fixtures**

PngSuite is Willem van Schaik's PNG conformance corpus. The archive filename
carries a release date that changes, so check the listing at
`http://www.schaik.com/pngsuite/` and use the current `PngSuite-*.zip` link:

```bash
mkdir -p crates/hexscope-core/tests/fixtures
curl -sSL -o /tmp/PngSuite.zip http://www.schaik.com/pngsuite/PngSuite-2017jul19.zip
unzip -o -q /tmp/PngSuite.zip -d crates/hexscope-core/tests/fixtures/pngsuite
ls crates/hexscope-core/tests/fixtures/pngsuite/*.png | wc -l
```

Expected: roughly 150–180 `.png` files. If the URL 404s, take the current link
from the page above.

Verify the two files the tests name by hand exist, and that the corrupt set is
present:

```bash
ls crates/hexscope-core/tests/fixtures/pngsuite/basn2c08.png
ls crates/hexscope-core/tests/fixtures/pngsuite/x*.png | wc -l
```

Expected: `basn2c08.png` exists (basic, non-interlaced, colour type 2, 8-bit,
32×32), and at least 10 files beginning with `x` — those are intentionally
corrupt and are the ones that matter most.

Commit the fixtures so CI does not depend on a third-party download:

```bash
git add crates/hexscope-core/tests/fixtures
git commit -m "test(core): vendor the PngSuite conformance corpus"
```

- [ ] **Step 2: Write the failing end-to-end tests**

`crates/hexscope-core/tests/golden.rs`:

```rust
use hexscope_core::png::{parse_png, PngDocument};
use hexscope_core::model::{NodeKind, ParseTree};
use std::fs;
use std::path::Path;

fn fixture(name: &str) -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/pngsuite")
        .join(name);
    fs::read(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

/// Renders the tree as indented text so snapshots stay readable in review.
fn render(tree: &ParseTree) -> String {
    fn walk(tree: &ParseTree, id: u32, depth: usize, out: &mut String) {
        let node = tree.get(id);
        out.push_str(&"  ".repeat(depth));
        out.push_str(&format!(
            "{} [{}..{}] {:?}",
            node.label,
            node.range.start,
            node.range.end(),
            node.kind
        ));
        if let Some(value) = &node.value {
            out.push_str(&format!(" = {value:?}"));
        }
        out.push('\n');
        for &child in &node.children {
            walk(tree, child, depth + 1, out);
        }
    }

    let mut out = String::new();
    if let Some(root) = tree.root() {
        walk(tree, root, 0, &mut out);
    }
    out
}

#[test]
fn basic_rgb_snapshot() {
    let doc = parse_png(&fixture("basn2c08.png"));
    insta::assert_snapshot!(render(&doc.tree));
}

#[test]
fn decodes_pixels_for_a_basic_image() {
    let doc = parse_png(&fixture("basn2c08.png"));
    let ihdr = doc.ihdr.expect("IHDR present");
    assert_eq!((ihdr.width, ihdr.height), (32, 32));
    assert_eq!(doc.pixels.expect("pixels decoded").len(), 32 * 32 * 3);
}

#[test]
fn corrupt_files_are_flagged_and_still_produce_a_tree() {
    // Every PngSuite file starting with `x` is intentionally broken. We do not
    // depend on any single filename — only on the guarantee that damage is
    // reported rather than hidden, and that a tree comes back regardless.
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/pngsuite");
    let mut corrupt_seen = 0;
    let mut flagged = 0;

    for entry in fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        if !name.starts_with('x') || !name.ends_with(".png") {
            continue;
        }
        corrupt_seen += 1;

        let doc = parse_png(&fs::read(&path).unwrap());
        assert!(!doc.tree.is_empty(), "{name} produced an empty tree");

        let problems = doc
            .tree
            .nodes()
            .iter()
            .filter(|n| matches!(n.kind, NodeKind::Warning | NodeKind::Error))
            .count();
        if problems > 0 {
            flagged += 1;
        }
    }

    assert!(corrupt_seen >= 10, "only found {corrupt_seen} corrupt fixtures");
    assert!(
        flagged * 2 >= corrupt_seen,
        "only {flagged} of {corrupt_seen} corrupt files were flagged"
    );
}

#[test]
fn every_pngsuite_file_produces_a_tree_without_panicking() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/pngsuite");
    let mut checked = 0;
    for entry in fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("png") {
            continue;
        }
        let bytes = fs::read(&path).unwrap();
        let doc: PngDocument = parse_png(&bytes);
        assert!(
            !doc.tree.is_empty(),
            "{} produced an empty tree",
            path.display()
        );
        checked += 1;
    }
    assert!(checked > 100, "only checked {checked} files");
}

#[test]
fn arbitrary_bytes_still_produce_a_tree() {
    for input in [
        vec![],
        vec![0u8; 8],
        b"not a png at all".to_vec(),
        vec![0x89, b'P', b'N', b'G'],
    ] {
        let doc = parse_png(&input);
        assert!(!doc.tree.is_empty(), "empty tree for {input:?}");
    }
}
```

- [ ] **Step 3: Run them to verify they fail**

Run: `cargo test -p hexscope-core --test golden`
Expected: FAIL — `cannot find function parse_png in hexscope_core::png`.

- [ ] **Step 4: Implement `parse_png`**

Prepend to `crates/hexscope-core/src/png/mod.rs` (keeping the existing `pub mod` lines):

```rust
pub mod chunks;
pub mod fields;
pub mod unfilter;

use crate::inflate::trace::{CheckpointSink, TraceSummary};
use crate::inflate::zlib::zlib_decompress;
use crate::model::{ByteRange, NodeKind, ParseTree, Value};
use crate::png::chunks::{Chunk, ChunkError, PNG_SIGNATURE, next_chunk};
use crate::png::fields::{
    Ihdr, decode_gama, decode_ihdr, decode_phys, decode_plte, decode_text, decode_trns,
};
use crate::png::unfilter::unfilter;
use crate::reader::Reader;

/// Caps decompressed IDAT output at 512 MB so a compression bomb cannot
/// exhaust memory.
const MAX_PIXEL_BYTES: u64 = 512 * 1024 * 1024;

/// One checkpoint per 512 events keeps the list small while staying fine
/// enough for a scrubber.
const CHECKPOINT_INTERVAL: u64 = 512;

#[derive(Debug)]
pub struct PngDocument {
    pub tree: ParseTree,
    pub ihdr: Option<Ihdr>,
    pub pixels: Option<Vec<u8>>,
    pub trace: Option<TraceSummary>,
}

/// Parses a PNG. Never fails: damage is recorded as `Warning`/`Error` nodes.
pub fn parse_png(data: &[u8]) -> PngDocument {
    let mut tree = ParseTree::new();
    let root = tree.add(
        None,
        "PNG",
        ByteRange::new(0, data.len() as u64),
        NodeKind::Container,
        None,
    );

    let mut r = Reader::new(data);

    match r.bytes(8) {
        Ok(sig) if sig == PNG_SIGNATURE => {
            tree.add(
                Some(root),
                "signature",
                ByteRange::new(0, 8),
                NodeKind::Field,
                Some(Value::Bytes(8)),
            );
        }
        _ => {
            tree.add(
                Some(root),
                "not a PNG signature",
                ByteRange::new(0, data.len().min(8) as u64),
                NodeKind::Error,
                None,
            );
            return PngDocument { tree, ihdr: None, pixels: None, trace: None };
        }
    }

    let mut ihdr: Option<Ihdr> = None;
    let mut idat: Vec<u8> = Vec::new();

    while let Some(result) = next_chunk(&mut r) {
        let chunk = match result {
            Ok(c) => c,
            Err(err) => {
                let label = match err {
                    ChunkError::Truncated => "truncated chunk",
                    ChunkError::LengthTooLarge => "chunk length out of range",
                };
                tree.add(
                    Some(root),
                    label,
                    ByteRange::new(r.pos(), r.remaining() as u64),
                    NodeKind::Error,
                    None,
                );
                break;
            }
        };

        let node = tree.add(
            Some(root),
            chunk.kind_str(),
            chunk.range,
            NodeKind::Container,
            Some(Value::Bytes(chunk.data.len() as u64)),
        );

        if !chunk.crc_ok() {
            tree.add(
                Some(node),
                format!(
                    "CRC mismatch: stored {:08X}, computed {:08X}",
                    chunk.declared_crc, chunk.actual_crc
                ),
                ByteRange::new(chunk.range.end() - 4, 4),
                NodeKind::Warning,
                None,
            );
        }

        match &chunk.kind {
            b"IHDR" => ihdr = decode_ihdr(&chunk, &mut tree, node),
            b"PLTE" => decode_plte(&chunk, &mut tree, node),
            b"tEXt" => decode_text(&chunk, &mut tree, node),
            b"pHYs" => decode_phys(&chunk, &mut tree, node),
            b"gAMA" => decode_gama(&chunk, &mut tree, node),
            b"tRNS" => decode_trns(&chunk, &mut tree, node, ihdr.map(|h| h.color_type)),
            b"IDAT" => idat.extend_from_slice(chunk.data),
            _ => {}
        }
    }

    let (pixels, trace) = decode_pixels(&idat, ihdr, &mut tree, root);

    PngDocument { tree, ihdr, pixels, trace }
}

fn decode_pixels(
    idat: &[u8],
    ihdr: Option<Ihdr>,
    tree: &mut ParseTree,
    root: crate::model::NodeId,
) -> (Option<Vec<u8>>, Option<TraceSummary>) {
    let Some(ihdr) = ihdr else {
        return (None, None);
    };
    if idat.is_empty() {
        return (None, None);
    }
    // Interlaced images use a seven-pass layout; v1 shows the tree but not the
    // pixels for them.
    if ihdr.interlace != 0 {
        tree.add(
            Some(root),
            "interlaced image: pixel preview unavailable",
            ByteRange::new(0, 0),
            NodeKind::Warning,
            None,
        );
        return (None, None);
    }

    let mut sink = CheckpointSink::new(CHECKPOINT_INTERVAL);
    let raw = match zlib_decompress(idat, MAX_PIXEL_BYTES, &mut sink) {
        Ok(raw) => raw,
        Err(err) => {
            tree.add(
                Some(root),
                format!("IDAT decompression failed: {err:?}"),
                ByteRange::new(0, 0),
                NodeKind::Error,
                None,
            );
            return (None, Some(sink.finish()));
        }
    };
    let summary = sink.finish();

    match unfilter(&raw, ihdr.width, ihdr.height, ihdr.bytes_per_pixel()) {
        Ok(pixels) => (Some(pixels), Some(summary)),
        Err(err) => {
            tree.add(
                Some(root),
                format!("unfiltering failed: {err:?}"),
                ByteRange::new(0, 0),
                NodeKind::Error,
                None,
            );
            (None, Some(summary))
        }
    }
}
```

Add to `crates/hexscope-core/src/lib.rs`:

```rust
pub use png::{PngDocument, parse_png};
```

- [ ] **Step 5: Run the tests and accept the snapshot**

Run: `cargo test -p hexscope-core --test golden`
Expected: the snapshot test reports a new snapshot; the other four pass.

Then review and accept:

```bash
cargo insta review
```

Read the rendered tree before accepting. It must show `signature`, `IHDR` with seven fields, `IDAT`, and `IEND`.

- [ ] **Step 6: Re-run to verify everything passes**

Run: `cargo test -p hexscope-core`
Expected: PASS — all unit and golden tests green.

- [ ] **Step 7: Commit**

```bash
git add crates/hexscope-core/ 
git commit -m "feat(core): add end-to-end png parsing with golden snapshots"
```

---

### Task 11: Fuzzing, property tests, benchmark, and CI

**Files:**
- Create: `crates/hexscope-core/fuzz/Cargo.toml`
- Create: `crates/hexscope-core/fuzz/fuzz_targets/parse_png.rs`
- Create: `crates/hexscope-core/tests/properties.rs`
- Create: `crates/hexscope-core/benches/parse.rs`
- Create: `.github/workflows/ci.yml`
- Modify: `crates/hexscope-core/Cargo.toml`

**Interfaces:**
- Consumes: `parse_png` (Task 10), `inflate`, `NoTrace` (Task 7).
- Produces: no library API — this task locks in the guarantees the earlier tasks claim.

- [ ] **Step 1: Write the property tests**

`crates/hexscope-core/tests/properties.rs`:

```rust
use hexscope_core::inflate::{NoTrace, inflate};
use hexscope_core::png::parse_png;
use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2000))]

    /// The central guarantee of the whole crate.
    #[test]
    fn parse_png_always_returns_a_tree(bytes in proptest::collection::vec(any::<u8>(), 0..4096)) {
        let doc = parse_png(&bytes);
        prop_assert!(!doc.tree.is_empty());
    }

    /// A valid signature followed by garbage is the nastiest realistic input:
    /// it gets past the sniff and into the chunk walker.
    #[test]
    fn png_signature_plus_garbage_is_survivable(
        tail in proptest::collection::vec(any::<u8>(), 0..4096)
    ) {
        let mut bytes = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        bytes.extend_from_slice(&tail);
        let doc = parse_png(&bytes);
        prop_assert!(!doc.tree.is_empty());
    }

    #[test]
    fn inflate_never_panics(bytes in proptest::collection::vec(any::<u8>(), 0..2048)) {
        let _ = inflate(&bytes, 1 << 20, &mut NoTrace);
    }
}
```

- [ ] **Step 2: Run them to verify they pass**

Run: `cargo test -p hexscope-core --test properties`
Expected: PASS — `test result: ok. 3 passed`. If any case fails, proptest prints a minimal reproducer; fix the parser, not the test.

- [ ] **Step 3: Add the fuzz target**

```bash
cargo install cargo-fuzz
cd crates/hexscope-core && cargo fuzz init && cd ../..
```

Replace `crates/hexscope-core/fuzz/fuzz_targets/fuzz_target_1.rs` with `crates/hexscope-core/fuzz/fuzz_targets/parse_png.rs`:

```rust
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let doc = hexscope_core::png::parse_png(data);
    // The only assertion that matters: we got here without panicking, and the
    // tree always describes something.
    assert!(!doc.tree.is_empty());
});
```

Update `crates/hexscope-core/fuzz/Cargo.toml` so the target name matches:

```toml
[[bin]]
name = "parse_png"
path = "fuzz_targets/parse_png.rs"
test = false
doc = false
```

- [ ] **Step 4: Seed the corpus and run the fuzzer**

```bash
mkdir -p crates/hexscope-core/fuzz/corpus/parse_png
cp crates/hexscope-core/tests/fixtures/pngsuite/*.png crates/hexscope-core/fuzz/corpus/parse_png/
cargo +nightly fuzz run parse_png -- -max_total_time=120
```

Expected: `Done ... runs`, with no crash artifacts written. If a crash appears, `cargo fuzz fmt parse_png <artifact>` prints the input — fix the parser and add that input as a unit test.

- [ ] **Step 5: Add the benchmark**

Append to `crates/hexscope-core/Cargo.toml`:

```toml
[dev-dependencies.criterion]
version = "0.5"

[[bench]]
name = "parse"
harness = false
```

`crates/hexscope-core/benches/parse.rs`:

```rust
use criterion::{Criterion, criterion_group, criterion_main};
use hexscope_core::png::parse_png;
use std::hint::black_box;
use std::time::Duration;

/// Builds a ~10 MB PNG in memory: noisy pixel data so DEFLATE has real work.
fn big_png() -> Vec<u8> {
    use flate2::Compression;
    use flate2::write::ZlibEncoder;
    use std::io::Write;

    let (width, height) = (1600u32, 1600u32);
    let mut raw = Vec::with_capacity(((width * 3 + 1) * height) as usize);
    for y in 0..height {
        raw.push(0u8); // filter type None
        for x in 0..width {
            let v = (x ^ y) as u8;
            raw.extend_from_slice(&[v, v.wrapping_mul(7), v.wrapping_add(31)]);
        }
    }

    let mut enc = ZlibEncoder::new(Vec::new(), Compression::fast());
    enc.write_all(&raw).unwrap();
    let idat = enc.finish().unwrap();

    fn chunk(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
        let mut out = (data.len() as u32).to_be_bytes().to_vec();
        out.extend_from_slice(kind);
        out.extend_from_slice(data);
        let mut crc_input = kind.to_vec();
        crc_input.extend_from_slice(data);
        out.extend_from_slice(&hexscope_core::crc32::crc32(&crc_input).to_be_bytes());
        out
    }

    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);

    let mut png = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    png.extend_from_slice(&chunk(b"IHDR", &ihdr));
    png.extend_from_slice(&chunk(b"IDAT", &idat));
    png.extend_from_slice(&chunk(b"IEND", &[]));
    png
}

fn bench_parse(c: &mut Criterion) {
    let png = big_png();
    let mb = png.len() as f64 / 1_048_576.0;
    println!("benchmark input: {mb:.1} MB");

    let mut group = c.benchmark_group("parse_png");
    // The spec's budget: 10 MB in under 300 ms.
    group.measurement_time(Duration::from_secs(20));
    group.bench_function("10mb", |b| b.iter(|| parse_png(black_box(&png))));
    group.finish();
}

criterion_group!(benches, bench_parse);
criterion_main!(benches);
```

- [ ] **Step 6: Run the benchmark and check the budget**

Run: `cargo bench -p hexscope-core`
Expected: the `parse_png/10mb` line reports a time. **It must be under 300 ms.** If it is not, the likely cause is the byte-at-a-time `BitReader::bits` — buffer 32 bits at a time before optimising anything else.

Record the measured number in the commit message so regressions are visible in `git log`.

- [ ] **Step 7: Add CI**

`.github/workflows/ci.yml`:

```yaml
name: CI

on:
  push:
    branches: [main]
  pull_request:

env:
  CARGO_TERM_COLOR: always

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy
      - uses: Swatinem/rust-cache@v2
      - name: Format
        run: cargo fmt --all --check
      - name: Clippy
        run: cargo clippy --all-targets -- -D warnings
      - name: Test
        run: cargo test --all

  fuzz:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@nightly
      - uses: Swatinem/rust-cache@v2
      - name: Install cargo-fuzz
        run: cargo install cargo-fuzz
      - name: Fuzz smoke run
        run: cargo fuzz run parse_png -- -max_total_time=60
        working-directory: crates/hexscope-core
```

- [ ] **Step 8: Verify the whole suite locally**

Run these three and confirm each passes before committing:

```bash
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --all
```

Expected: no formatting diff, no clippy warnings, all tests pass.

- [ ] **Step 9: Commit**

```bash
git add crates/hexscope-core/ .github/
git commit -m "test(core): add fuzzing, property tests, benchmark and CI"
```

---

## Deliberate deviations from the spec

**The `Format` trait is not built yet.** §4 of the spec sketches
`trait Format { fn sniff(head: &[u8]) -> bool; fn parse(r: &mut Reader) -> ParseResult; }`.
This plan ships `parse_png(&[u8]) -> PngDocument` directly instead.

A trait with exactly one implementor is a guess, not a design: its shape gets
decided by the one format that exists, and the second format then either
contorts to fit or forces a rewrite. `parse_png` already has the right
signature to sit behind that trait, so introducing it when EXIF arrives (v1.1)
costs one small refactor and produces a trait shaped by two real cases rather
than one.

Nothing about the spec's goal is lost: the frontend still consumes only
`ParseTree`, and adding a format still means adding a module without touching
the UI.

**The spec's remaining testing requirements are out of scope here by design.**
§10 items 5 (Playwright frame-time trace) and §9's hover, scroll and
drop-to-first-frame budgets are frontend guarantees; they belong to Plan 2.
This plan covers §10 items 1, 2, 3, 4 and 6 and the parse-time budget.

## Done criteria for this plan

- `cargo test --all` passes, including golden snapshots over the PngSuite corpus.
- `cargo fuzz run parse_png -- -max_total_time=120` finds no crash.
- `cargo bench` reports `parse_png/10mb` under 300 ms.
- `cargo clippy --all-targets -- -D warnings` is clean.
- No `unsafe` anywhere (`#![forbid(unsafe_code)]` is in `lib.rs`).

## What comes next

Plan 2 (`hexscope-wasm` + web shell) consumes exactly three things from this crate: `parse_png`, `ParseTree::nodes()` for flattening, and `TraceSummary` for the animation scrubber. Nothing else crosses the boundary.
