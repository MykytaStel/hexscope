# Huffman Codes in the Player — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The DEFLATE player shows which bits of a step meant what, and where each Huffman code sits in its block's code table.

**Architecture:** The core decoder gains an opt-in recorder (`explain_next`) and a view of the current block's tables (`tables`), both computed on demand. The WASM bridge exposes `explain(index)` and `tables(index)` as flat `f64` arrays. A new web module, `codes.ts`, renders a bit row and a codes panel as DOM, and `player.ts` requests one explanation at a time for the step on screen.

**Tech Stack:** Rust 1.98 (edition 2024, no runtime deps), wasm-bindgen 0.2.128, TypeScript 7, Vite 8.

Spec: `docs/superpowers/specs/2026-09-23-huffman-player-design.md`.

## Global Constraints

- `#![forbid(unsafe_code)]`; no new dependencies in `hexscope-core` or `hexscope-wasm`.
- Nothing may panic on any input; every loop must terminate.
- Plain decoding (`step()`, `inflate`, `steps()` batches, stride 6) keeps its output and its speed: the 10 MB bench stays within noise.
- Code, comments, commits and UI text in English; comments explain *why*.
- Before every commit: `cargo fmt --all`, `cargo clippy --all-targets --all -- -D warnings`, `cargo test --all`; for web changes also `pnpm --filter web build`.

## File Structure

| File | Responsibility |
|---|---|
| `crates/hexscope-core/src/inflate/explain.rs` (new) | Types: `PartKind`, `Part`, `Explained`, `CodeGroup`, `DynamicHeader`, `BlockTables` |
| `crates/hexscope-core/src/inflate/huffman.rs` | `decode_traced`, `groups` |
| `crates/hexscope-core/src/inflate/engine.rs` | Recorder field, `explain_next`, `tables`, `next_index`; parts noted in header/symbol reads |
| `crates/hexscope-core/src/inflate/mod.rs` | Re-exports |
| `crates/hexscope-core/tests/golden.rs`, `tests/properties.rs` | PngSuite partition, never-panics property |
| `crates/hexscope-wasm/src/lib.rs` | `explain`, `tables`, `decoder_before`, codes for errors and block kinds |
| `apps/web/src/deflate.ts` (new) | `Step`, `StepKind`, `BLOCK_NAMES`, `STRIDE`, shared by player and codes |
| `apps/web/src/codes.ts` (new) | Decoding the bridge arrays; `CodesView` (bit row + codes panel) |
| `apps/web/src/player.ts`, `main.ts`, `worker.ts`, `style.css` | Wiring, messages, layout |
| `README.md`, `docs/known-issues.md` | Roadmap/known issues |

---

### Task 1: Canonical code groups and traced decoding

**Files:**
- Create: `crates/hexscope-core/src/inflate/explain.rs`
- Modify: `crates/hexscope-core/src/inflate/huffman.rs`, `crates/hexscope-core/src/inflate/mod.rs`

**Interfaces:**
- Produces: `explain::{PartKind, Part, Explained, CodeGroup, DynamicHeader, BlockTables}`; `Huffman::decode_traced(&self, &mut BitReader) -> Result<(u16, u16, u8), InflateError>` returning `(symbol, code, code_len)`; `Huffman::groups(&self) -> Vec<CodeGroup>`.

- [ ] **Step 1: Create `explain.rs` with the types**

```rust
//! What one decoding step read, bit by bit, and the code tables it read with.
//! Built only when asked for, so plain decoding pays nothing for it.

use crate::inflate::InflateError;
use crate::inflate::engine::{BlockKind, Step};

/// What a run of bits meant. The numbering is part of the WASM bridge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum PartKind {
    Final = 0,
    BlockType,
    /// Bits skipped to reach a byte boundary before a stored block's length.
    Padding,
    StoredLen,
    StoredNLen,
    StoredByte,
    HLit,
    HDist,
    HClen,
    /// The lengths of the code that encodes the code lengths.
    CodeLengthCode,
    /// The run-length-encoded code lengths of both tables.
    CodeLengths,
    LitLen,
    LengthExtra,
    Distance,
    DistanceExtra,
    /// Bits consumed by a read that failed.
    Unreadable,
}

/// One run of bits within a step: `[bit_start, bit_end)` of the stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Part {
    pub kind: PartKind,
    pub bit_start: u64,
    pub bit_end: u64,
    /// The decoded meaning: symbol, extra-bit value, count or byte.
    pub value: u32,
    /// For Huffman codes: the code as the decoder assembled it, most
    /// significant bit first, and its length. Zero otherwise.
    pub code: u16,
    pub code_len: u8,
}

impl Part {
    pub(crate) fn plain(kind: PartKind, bit_start: u64, bit_end: u64, value: u32) -> Self {
        Self { kind, bit_start, bit_end, value, code: 0, code_len: 0 }
    }

    pub(crate) fn code(
        kind: PartKind,
        bit_start: u64,
        bit_end: u64,
        symbol: u16,
        code: u16,
        code_len: u8,
    ) -> Self {
        Self { kind, bit_start, bit_end, value: symbol as u32, code, code_len }
    }
}

/// One step, explained.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Explained {
    /// `None` when the step failed.
    pub step: Option<Step>,
    pub error: Option<InflateError>,
    /// In reading order, contiguous. Empty for a step that reads nothing,
    /// such as the end of a stored block.
    pub parts: Vec<Part>,
    /// Bit position of the header of the block the step belongs to.
    pub block_start: u64,
}

/// The codes of one length: consecutive values from `first_code`, one per
/// symbol, in symbol order (RFC 1951 §3.2.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeGroup {
    pub len: u8,
    pub first_code: u16,
    pub symbols: Vec<u16>,
}

/// The counts a dynamic block's header declares.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DynamicHeader {
    pub hlit: u16,
    pub hdist: u8,
    pub hclen: u8,
}

/// The code tables a block decodes with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockTables {
    pub kind: BlockKind,
    /// Present for a dynamic block only.
    pub header: Option<DynamicHeader>,
    pub lit_len: Vec<CodeGroup>,
    pub distance: Vec<CodeGroup>,
}
```

In `mod.rs` add `pub mod explain;` (alphabetical, after `engine`) and
`pub use explain::{BlockTables, CodeGroup, DynamicHeader, Explained, Part, PartKind};`.

- [ ] **Step 2: Write the failing tests in `huffman.rs`'s test module**

```rust
    use crate::inflate::tables::fixed_literal_lengths;

    /// Packs Huffman codes, each most significant bit first, into DEFLATE's
    /// least-significant-bit-first byte order.
    fn pack(codes: &[(u16, u8)]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut n = 0usize;
        for &(code, len) in codes {
            for i in (0..len).rev() {
                if n % 8 == 0 {
                    out.push(0);
                }
                let bit = ((code >> i) & 1) as u8;
                *out.last_mut().unwrap() |= bit << (n % 8);
                n += 1;
            }
        }
        out
    }

    #[test]
    fn groups_reproduce_the_rfc_example() {
        // RFC 1951 §3.2.2: A..H with lengths 3,3,3,3,3,2,4,4 give
        // F=00, A=010, B=011, C=100, D=101, E=110, G=1110, H=1111.
        let h = Huffman::from_lengths(&[3, 3, 3, 3, 3, 2, 4, 4]).unwrap();
        let groups: Vec<_> = h
            .groups()
            .into_iter()
            .map(|g| (g.len, g.first_code, g.symbols))
            .collect();
        assert_eq!(
            groups,
            vec![(2, 0b00, vec![5]), (3, 0b010, vec![0, 1, 2, 3, 4]), (4, 0b1110, vec![6, 7])]
        );
    }

    #[test]
    fn groups_of_the_fixed_literal_table() {
        let h = Huffman::from_lengths(&fixed_literal_lengths()).unwrap();
        let g = h.groups();
        let shape: Vec<_> = g.iter().map(|g| (g.len, g.first_code, g.symbols.len())).collect();
        assert_eq!(
            shape,
            vec![(7, 0b000_0000, 24), (8, 0b0011_0000, 152), (9, 0b1_1001_0000, 112)]
        );
        // Within a length, symbols keep their order: 0..=143, then 280..=287.
        assert_eq!(g[1].symbols[0], 0);
        assert_eq!(g[1].symbols[144], 280);
    }

    #[test]
    fn a_traced_code_sits_at_its_symbol_in_its_group() {
        let h = Huffman::from_lengths(&fixed_literal_lengths()).unwrap();
        for g in h.groups() {
            for (k, &symbol) in g.symbols.iter().enumerate() {
                let code = g.first_code + k as u16;
                let data = pack(&[(code, g.len)]);
                let mut br = BitReader::new(&data);
                assert_eq!(h.decode_traced(&mut br), Ok((symbol, code, g.len)));
            }
        }
    }
```

- [ ] **Step 3: Run and see them fail**

Run: `cargo test -p hexscope-core --lib huffman`
Expected: compile error, `no method named groups` / `decode_traced`.

- [ ] **Step 4: Implement in `huffman.rs`**

Add `use crate::inflate::explain::CodeGroup;` and replace `decode` with:

```rust
    pub fn decode(&self, br: &mut BitReader) -> Result<u16, InflateError> {
        self.decode_traced(br).map(|(symbol, _, _)| symbol)
    }

    /// Like [`decode`](Self::decode), also returning the code as the loop
    /// assembled it — most significant bit first — and its length.
    pub fn decode_traced(&self, br: &mut BitReader) -> Result<(u16, u16, u8), InflateError> {
        // Walks lengths shortest-first: `first` is the smallest code of this
        // length, `index` the offset of that length's symbols in `symbols`.
        let mut code = 0i32;
        let mut first = 0i32;
        let mut index = 0i32;

        for len in 1..=MAX_BITS {
            code |= br.bits(1).map_err(|_| InflateError::Bits)? as i32;
            let count = self.counts[len] as i32;
            if code - first < count {
                let symbol = self.symbols[(index + (code - first)) as usize];
                return Ok((symbol, code as u16, len as u8));
            }
            index += count;
            first = (first + count) << 1;
            code <<= 1;
        }

        Err(InflateError::BadHuffmanCode)
    }

    /// The code grouped by length, each with its first canonical code — the
    /// same ranges `decode` walks, so what is shown is what decoding does.
    pub fn groups(&self) -> Vec<CodeGroup> {
        let mut out = Vec::new();
        let mut code = 0u32;
        let mut index = 0usize;
        for len in 1..=MAX_BITS {
            code = (code + self.counts[len - 1] as u32) << 1;
            let n = self.counts[len] as usize;
            if n > 0 {
                out.push(CodeGroup {
                    len: len as u8,
                    first_code: code as u16,
                    symbols: self.symbols[index..index + n].to_vec(),
                });
            }
            index += n;
        }
        out
    }
```

- [ ] **Step 5: Run the tests**

Run: `cargo test -p hexscope-core --lib huffman`
Expected: all pass, including the four existing Huffman tests.

- [ ] **Step 6: Commit**

```bash
cargo fmt --all && cargo clippy --all-targets --all -- -D warnings
git add crates/hexscope-core/src/inflate
git commit -m "feat(core): canonical code groups and traced Huffman decoding"
```

---

### Task 2: The decoder explains a step

**Files:**
- Modify: `crates/hexscope-core/src/inflate/engine.rs`
- Test: `engine.rs` test module, `crates/hexscope-core/tests/golden.rs`, `crates/hexscope-core/tests/properties.rs`

**Interfaces:**
- Consumes: Task 1 types, `Huffman::decode_traced`, `Huffman::groups`.
- Produces: `Decoder::explain_next(&mut self) -> Option<Explained>`, `Decoder::tables(&self) -> Option<BlockTables>`, `Decoder::next_index(&self) -> u64`.

- [ ] **Step 1: Record the bench baseline before touching the decoder**

Run: `cargo bench -p hexscope-core 2>&1 | grep -A2 time:`
Note the 10 MB time. Criterion stores it as the baseline for Step 7.

- [ ] **Step 2: Write the failing tests in `engine.rs`'s test module**

```rust
    use crate::inflate::explain::{Explained, PartKind};

    /// Bit fields packed in DEFLATE order. Header fields are numbers, read
    /// least significant bit first; Huffman codes go most significant first.
    fn pack_fields(fields: &[(u32, u8, bool)]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut n = 0usize;
        for &(value, len, msb_first) in fields {
            for i in 0..len {
                let bit = if msb_first { (value >> (len - 1 - i)) & 1 } else { (value >> i) & 1 };
                if n % 8 == 0 {
                    out.push(0);
                }
                *out.last_mut().unwrap() |= (bit as u8) << (n % 8);
                n += 1;
            }
        }
        out
    }

    /// Walks a plain and an explaining decoder side by side.
    fn explain_all(data: &[u8]) -> Vec<(Option<Result<Step, InflateError>>, Explained)> {
        let mut plain = Decoder::new(data, u64::MAX);
        let mut explaining = Decoder::new(data, u64::MAX);
        let mut out = Vec::new();
        while let Some(e) = explaining.explain_next() {
            out.push((plain.step(), e));
        }
        assert!(plain.step().is_none(), "both decoders end together");
        out
    }

    fn assert_partitions(e: &Explained, start: u64, end: u64) {
        let mut at = start;
        for p in &e.parts {
            assert_eq!(p.bit_start, at, "parts are contiguous: {e:?}");
            assert!(p.bit_end > p.bit_start, "no empty parts: {e:?}");
            at = p.bit_end;
        }
        assert_eq!(at, if e.parts.is_empty() { start } else { end }, "{e:?}");
    }

    fn sample_input() -> Vec<u8> {
        let mut input = b"hexscope hexscope hexscope! ".repeat(40);
        input.extend((0..3000u32).map(|i| (i * 7 % 251) as u8));
        input
    }

    #[test]
    fn explained_parts_partition_every_step() {
        let input = sample_input();
        let mut seen = std::collections::HashSet::new();
        for level in [Compression::none(), Compression::fast(), Compression::best()] {
            for (plain, e) in explain_all(&deflate(&input, level)) {
                let step = plain.unwrap().unwrap();
                assert_eq!(e.step, Some(step), "explaining changes nothing");
                assert_eq!(e.error, None);
                assert_eq!(e.block_start, step.block_start);
                assert_partitions(&e, step.bit_start, step.bit_end);
                seen.extend(e.parts.iter().map(|p| p.kind));
            }
        }
        for kind in [
            PartKind::Final, PartKind::BlockType, PartKind::StoredLen, PartKind::StoredNLen,
            PartKind::StoredByte, PartKind::HLit, PartKind::HDist, PartKind::HClen,
            PartKind::CodeLengthCode, PartKind::CodeLengths, PartKind::LitLen,
            PartKind::LengthExtra, PartKind::Distance, PartKind::DistanceExtra,
        ] {
            assert!(seen.contains(&kind), "{kind:?} never appeared");
        }
    }

    #[test]
    fn explained_codes_sit_in_their_tables() {
        let data = deflate(&sample_input(), Compression::best());
        let mut d = Decoder::new(&data, u64::MAX);
        let mut checked = 0;
        loop {
            // Tables before the step: a block's end code is read with them,
            // and afterwards the block is gone.
            let before = d.tables();
            let Some(e) = d.explain_next() else { break };
            let Some(tables) = before.or_else(|| d.tables()) else { continue };
            for p in &e.parts {
                let groups = match p.kind {
                    PartKind::LitLen => &tables.lit_len,
                    PartKind::Distance => &tables.distance,
                    _ => continue,
                };
                let g = groups.iter().find(|g| g.len == p.code_len).expect("a group of that length");
                let k = (p.code - g.first_code) as usize;
                assert_eq!(g.symbols[k], p.value as u16, "{p:?}");
                checked += 1;
            }
        }
        assert!(checked > 100, "only {checked} codes checked");
    }

    #[test]
    fn tables_describe_the_block_being_decoded() {
        // A fixed block holding only its end code (256 = 0000000).
        let fixed = pack_fields(&[(1, 1, false), (1, 2, false), (0, 7, true)]);
        let mut d = Decoder::new(&fixed, u64::MAX);
        assert_eq!(d.tables(), None, "no block before its header");
        d.explain_next();
        let t = d.tables().expect("fixed tables");
        assert_eq!(t.kind, BlockKind::Fixed);
        assert_eq!(t.header, None);
        assert_eq!(t.lit_len.iter().map(|g| g.len).collect::<Vec<_>>(), [7, 8, 9]);
        assert_eq!(d.next_index(), 1);

        let dynamic = deflate(&sample_input(), Compression::best());
        let mut d = Decoder::new(&dynamic, u64::MAX);
        d.explain_next();
        let t = d.tables().expect("dynamic tables");
        assert_eq!(t.kind, BlockKind::Dynamic);
        let h = t.header.expect("dynamic header");
        let lit_symbols: usize = t.lit_len.iter().map(|g| g.symbols.len()).sum();
        assert!(lit_symbols <= h.hlit as usize && h.hlit >= 257);

        let stored = deflate(b"abc", Compression::none());
        let mut d = Decoder::new(&stored, u64::MAX);
        d.explain_next();
        assert_eq!(d.tables(), None, "stored blocks have no codes");
    }

    #[test]
    fn a_truncated_stream_explains_where_it_ran_out() {
        let data = deflate(&sample_input(), Compression::best());
        let cut = &data[..data.len() / 2];
        let steps = explain_all(cut);
        let (_, last) = steps.last().unwrap();
        assert_eq!(last.error, Some(InflateError::Bits));
        let p = last.parts.last().expect("an unreadable part");
        assert_eq!(p.kind, PartKind::Unreadable);
        assert_eq!(p.bit_end, cut.len() as u64 * 8, "blames the bits that were left");
    }

    #[test]
    fn an_undefined_symbol_is_named_by_its_code() {
        // Fixed block, then code 286 (11000110): valid bits, undefined symbol.
        let data = pack_fields(&[(1, 1, false), (1, 2, false), (0b1100_0110, 8, true)]);
        let steps = explain_all(&data);
        let (_, e) = &steps[1];
        assert_eq!(e.error, Some(InflateError::BadSymbol));
        let kinds: Vec<_> = e.parts.iter().map(|p| (p.kind, p.value)).collect();
        assert_eq!(kinds, [(PartKind::LitLen, 286)], "every bit read was understood");
    }
```

- [ ] **Step 3: Run and see them fail**

Run: `cargo test -p hexscope-core --lib engine`
Expected: compile errors for `explain_next`, `tables`, `next_index`.

- [ ] **Step 4: Implement in `engine.rs`**

Imports: add `use crate::inflate::explain::{BlockTables, DynamicHeader, Explained, Part, PartKind};`.

`Block::Coded` keeps what `tables()` needs:

```rust
    Coded {
        kind: BlockKind,
        lit: Huffman,
        dist: Huffman,
        header: Option<DynamicHeader>,
    },
```

A free helper, next to `bit_err`:

```rust
/// Notes a part while a step is being explained; only a branch otherwise.
fn note(record: &mut Option<Vec<Part>>, part: impl FnOnce() -> Part) {
    if let Some(parts) = record {
        parts.push(part());
    }
}
```

`Decoder` gets a field, initialised to `None` in `new` and `resume`:

```rust
    /// Parts of the current step, collected only while explaining.
    record: Option<Vec<Part>>,
```

Public methods, after `into_output`:

```rust
    /// The index the next step will have.
    pub fn next_index(&self) -> u64 {
        self.index
    }

    /// Decodes one step like [`step`](Self::step), also recording what each
    /// run of bits it read meant. `None` once decoding has finished.
    pub fn explain_next(&mut self) -> Option<Explained> {
        let start = self.br.bit_pos();
        self.record = Some(Vec::new());
        let result = self.step();
        let mut parts = self.record.take().unwrap_or_default();
        let (step, error) = match result? {
            Ok(step) => (Some(step), None),
            Err(err) => {
                let from = parts.last().map_or(start, |p| p.bit_end);
                let mut to = self.br.bit_pos();
                // A read that ran out of input consumed nothing: blame the
                // bits that were left, since they were too few.
                if to == from && err == InflateError::Bits {
                    to = from + self.br.remaining_bits();
                }
                if to > from {
                    parts.push(Part::plain(PartKind::Unreadable, from, to, 0));
                }
                (None, Some(err))
            }
        };
        Some(Explained {
            step,
            error,
            parts,
            block_start: self.block_start,
        })
    }

    /// The code tables of the block being decoded: `None` for a stored block
    /// and between blocks.
    pub fn tables(&self) -> Option<BlockTables> {
        let Block::Coded { kind, lit, dist, header } = &self.block else {
            return None;
        };
        Some(BlockTables {
            kind: *kind,
            header: *header,
            lit_len: lit.groups(),
            distance: dist.groups(),
        })
    }
```

Stored byte in `advance`:

```rust
                let &[byte] = self.br.bytes(1).map_err(bit_err)? else {
                    return Err(InflateError::Bits);
                };
                let end = self.br.bit_pos();
                note(&mut self.record, || {
                    Part::plain(PartKind::StoredByte, end - 8, end, byte as u32)
                });
```

`open_block` becomes:

```rust
    fn open_block(&mut self) -> Result<(BlockKind, u64), InflateError> {
        let start = self.br.bit_pos();
        self.is_final = self.br.bits(1).map_err(bit_err)? == 1;
        let is_final = self.is_final as u32;
        note(&mut self.record, || Part::plain(PartKind::Final, start, start + 1, is_final));
        let btype = self.br.bits(2).map_err(bit_err)?;
        note(&mut self.record, || Part::plain(PartKind::BlockType, start + 1, start + 3, btype));
        let kind = match btype {
            0 => BlockKind::Stored,
            1 => BlockKind::Fixed,
            2 => BlockKind::Dynamic,
            _ => return Err(InflateError::BadSymbol),
        };
        let after_type = self.br.bit_pos();

        self.block = match kind {
            BlockKind::Stored => {
                let &[l0, l1, n0, n1] = self.br.bytes(4).map_err(bit_err)? else {
                    return Err(InflateError::Bits);
                };
                let len = u16::from_le_bytes([l0, l1]);
                let nlen = u16::from_le_bytes([n0, n1]);
                let end = self.br.bit_pos();
                let fields = end - 32;
                if fields > after_type {
                    note(&mut self.record, || {
                        Part::plain(PartKind::Padding, after_type, fields, 0)
                    });
                }
                note(&mut self.record, || {
                    Part::plain(PartKind::StoredLen, fields, fields + 16, len as u32)
                });
                note(&mut self.record, || {
                    Part::plain(PartKind::StoredNLen, fields + 16, end, nlen as u32)
                });
                if nlen != !len {
                    return Err(InflateError::BadSymbol);
                }
                Block::Stored { remaining: len as usize }
            }
            BlockKind::Fixed => Block::Coded {
                kind,
                lit: Huffman::from_lengths(&fixed_literal_lengths())?,
                dist: Huffman::from_lengths(&fixed_distance_lengths())?,
                header: None,
            },
            BlockKind::Dynamic => {
                let (lit, dist, header) = read_dynamic_tables(&mut self.br, &mut self.record)?;
                Block::Coded { kind, lit, dist, header: Some(header) }
            }
        };
        Ok((kind, after_type))
    }
```

`coded_symbol` notes each read (the `lit`/`dist` borrow of `self.block` and
`&mut self.record` are disjoint fields):

```rust
    fn coded_symbol(&mut self) -> Result<InflateEvent, InflateError> {
        let Block::Coded { lit, dist, .. } = &self.block else {
            return Err(InflateError::Bits);
        };
        let s = self.br.bit_pos();
        let (symbol, code, code_len) = lit.decode_traced(&mut self.br)?;
        let e = self.br.bit_pos();
        note(&mut self.record, || Part::code(PartKind::LitLen, s, e, symbol, code, code_len));
        match symbol {
            0..=255 => { /* unchanged */ }
            256 => Ok(self.close_block()),
            257..=285 => {
                let idx = symbol as usize - 257;
                let s = self.br.bit_pos();
                let extra = self.br.bits(LENGTH_EXTRA[idx] as u32).map_err(bit_err)?;
                let e = self.br.bit_pos();
                if e > s {
                    note(&mut self.record, || Part::plain(PartKind::LengthExtra, s, e, extra));
                }
                let length = LENGTH_BASE[idx] as u32 + extra;

                let s = self.br.bit_pos();
                let (dist_symbol, code, code_len) = dist.decode_traced(&mut self.br)?;
                let e = self.br.bit_pos();
                note(&mut self.record, || {
                    Part::code(PartKind::Distance, s, e, dist_symbol, code, code_len)
                });
                let dist_symbol = dist_symbol as usize;
                if dist_symbol >= DIST_BASE.len() {
                    return Err(InflateError::BadSymbol);
                }
                let s = self.br.bit_pos();
                let extra = self.br.bits(DIST_EXTRA[dist_symbol] as u32).map_err(bit_err)?;
                let e = self.br.bit_pos();
                if e > s {
                    note(&mut self.record, || Part::plain(PartKind::DistanceExtra, s, e, extra));
                }
                let distance = DIST_BASE[dist_symbol] as u32 + extra;
                /* the distance and output checks and the copy loop stay unchanged */
            }
            _ => Err(InflateError::BadSymbol),
        }
    }
```

`read_dynamic_tables` takes the recorder and returns the header:

```rust
fn read_dynamic_tables(
    br: &mut BitReader,
    record: &mut Option<Vec<Part>>,
) -> Result<(Huffman, Huffman, DynamicHeader), InflateError> {
    let start = br.bit_pos();
    let hlit = br.bits(5).map_err(bit_err)? as usize + 257;
    note(record, || Part::plain(PartKind::HLit, start, start + 5, hlit as u32));
    let hdist = br.bits(5).map_err(bit_err)? as usize + 1;
    note(record, || Part::plain(PartKind::HDist, start + 5, start + 10, hdist as u32));
    let hclen = br.bits(4).map_err(bit_err)? as usize + 4;
    note(record, || Part::plain(PartKind::HClen, start + 10, start + 14, hclen as u32));

    /* reading code_lengths unchanged */
    let lengths_start = br.bit_pos();
    note(record, || {
        Part::plain(PartKind::CodeLengthCode, start + 14, lengths_start, hclen as u32)
    });
    let code_huff = Huffman::from_lengths(&code_lengths)?;

    /* the lengths loop unchanged */
    let end = br.bit_pos();
    note(record, || Part::plain(PartKind::CodeLengths, lengths_start, end, total as u32));

    let lit = Huffman::from_lengths(&lengths[..hlit])?;
    let dist = Huffman::from_lengths(&lengths[hlit..])?;
    let header = DynamicHeader {
        hlit: hlit as u16,
        hdist: hdist as u8,
        hclen: hclen as u8,
    };
    Ok((lit, dist, header))
}
```

- [ ] **Step 5: Run the engine tests**

Run: `cargo test -p hexscope-core --lib engine`
Expected: all pass (old and new).

- [ ] **Step 6: Add the PngSuite partition and the property**

In `tests/golden.rs`:

```rust
#[test]
fn every_pngsuite_step_explains_its_bits_exactly() {
    use hexscope_core::inflate::Decoder;
    for (name, bytes) in pngsuite() {
        let doc = parse_png(&bytes);
        let stream: Vec<u8> = doc
            .idat
            .iter()
            .flat_map(|r| bytes[r.start as usize..r.end() as usize].iter().copied())
            .collect();
        if stream.len() < 6 {
            continue;
        }
        let body = &stream[2..stream.len() - 4];
        let mut plain = Decoder::new(body, u64::MAX);
        let mut explaining = Decoder::new(body, u64::MAX);
        while let Some(e) = explaining.explain_next() {
            let step = plain.step();
            let Some(Ok(step)) = step else {
                assert!(e.error.is_some(), "{name}: both fail together");
                break;
            };
            assert_eq!(e.step, Some(step), "{name}");
            let mut at = step.bit_start;
            for p in &e.parts {
                assert_eq!(p.bit_start, at, "{name}: step {}", step.index);
                at = p.bit_end;
            }
            if !e.parts.is_empty() {
                assert_eq!(at, step.bit_end, "{name}: step {}", step.index);
            }
        }
    }
}
```

Use the file's existing PngSuite iterator; if it is named differently than
`pngsuite()`, call that one. `r.end()` is `ByteRange::end`.

In `tests/properties.rs`, inside the `proptest!` block:

```rust
    #[test]
    fn explaining_never_panics(bytes in proptest::collection::vec(any::<u8>(), 0..2048)) {
        let mut d = hexscope_core::inflate::Decoder::new(&bytes, 1 << 20);
        while let Some(e) = d.explain_next() {
            let _ = d.tables();
            if e.error.is_some() {
                break;
            }
        }
    }
```

Run: `cargo test --all`
Expected: all pass.

- [ ] **Step 7: Check the bench against the baseline**

Run: `cargo bench -p hexscope-core 2>&1 | grep -A3 time:`
Expected: "No change in performance detected" or a change within ±5%. A
regression beyond that means the recorder branch landed in the hot loop
badly; fix before committing.

- [ ] **Step 8: Commit**

```bash
cargo fmt --all && cargo clippy --all-targets --all -- -D warnings && cargo test --all
git add crates/hexscope-core
git commit -m "feat(core): explain a DEFLATE step bit by bit, with its block's tables"
```

---

### Task 3: The bridge explains a step

**Files:**
- Modify: `crates/hexscope-wasm/src/lib.rs`

**Interfaces:**
- Consumes: `Decoder::{explain_next, tables, next_index}`, `PartKind as u8`.
- Produces (JS): `Parsed.explain(index: number): Float64Array` laid out
  `[block_start, error (-1 none), n, n × (kind, bit_start, bit_end, value, code, code_len)]`;
  `Parsed.tables(index: number): Float64Array` laid out
  `[kind, hlit, hdist, hclen, then twice: groups, then per group len, first_code, count, symbols…]`,
  empty for stored blocks. Error codes: Bits 0, BadHuffmanCode 1,
  BadCodeLengths 2, BadDistance 3, BadSymbol 4, BadZlibHeader 5,
  ChecksumMismatch 6, OutputTooLarge 7, BadCheckpoint 8. Block kinds: stored
  0, fixed 1, dynamic 2.

- [ ] **Step 1: Write the failing tests in the lib's test module**

```rust
    #[test]
    fn explain_agrees_with_steps() {
        let parsed = parse(&fixture("basn2c08.png"));
        let steps = every_step(&parsed);
        for (i, s) in steps.chunks(STEP_STRIDE).enumerate() {
            let e = parsed.explain(i as f64);
            assert_eq!(e[1], -1.0, "step {i} decodes");
            let n = e[2] as usize;
            assert_eq!(e.len(), 3 + n * PART_STRIDE);
            if n > 0 {
                assert_eq!(e[3 + 1], s[3], "step {i}: first part starts with the step");
                assert_eq!(e[3 + (n - 1) * PART_STRIDE + 2], s[4], "step {i}: last part ends with it");
            }
        }
    }

    #[test]
    fn tables_follow_the_block() {
        let parsed = parse(&fixture("basn2c08.png"));
        let t = parsed.tables(0.0);
        assert!(t[0] == 1.0 || t[0] == 2.0, "a coded block: {}", t[0]);
        assert!(t[4] > 0.0, "the literal/length table has groups");
    }

    #[test]
    fn explaining_nothing_is_empty() {
        let parsed = parse(&fixture("basn2c08.png"));
        for index in [-1.0, f64::NAN, 1e12] {
            assert!(parsed.explain(index).is_empty(), "{index}");
            assert!(parsed.tables(index).is_empty(), "{index}");
        }
    }
```

- [ ] **Step 2: Run and see them fail**

Run: `cargo test -p hexscope-wasm`
Expected: compile errors, `no method named explain`.

- [ ] **Step 3: Implement**

Next to `STEP_STRIDE`:

```rust
/// Numbers per part in `explain`: kind, bit_start, bit_end, value, code, code_len.
const PART_STRIDE: usize = 6;
```

In `#[wasm_bindgen] impl Parsed`:

```rust
    /// What step `index` read, part by part. Layout:
    /// `[block_start, error, n, then n × (kind, bit_start, bit_end, value,
    /// code, code_len)]`, `error` -1 when the step decoded. Empty when there
    /// is no such step.
    pub fn explain(&self, index: f64) -> Vec<f64> {
        let Some(e) = self.decoder_before(index).and_then(|mut d| d.explain_next()) else {
            return Vec::new();
        };
        let mut out = Vec::with_capacity(3 + e.parts.len() * PART_STRIDE);
        out.push(e.block_start as f64);
        out.push(e.error.map_or(-1.0, error_code));
        out.push(e.parts.len() as f64);
        for p in &e.parts {
            out.extend_from_slice(&[
                p.kind as u8 as f64,
                p.bit_start as f64,
                p.bit_end as f64,
                p.value as f64,
                p.code as f64,
                p.code_len as f64,
            ]);
        }
        out
    }

    /// The code tables of the block holding step `index`. Layout: `[kind,
    /// hlit, hdist, hclen, then for literal/length and for distance: groups,
    /// then per group len, first_code, count, symbols…]`. Empty for a stored
    /// block or when there is no such step.
    pub fn tables(&self, index: f64) -> Vec<f64> {
        let Some(mut d) = self.decoder_before(index) else {
            return Vec::new();
        };
        // A block's codes are in place before its steps; a header step builds
        // them, so for it they appear only after.
        let tables = d.tables().or_else(|| {
            d.step();
            d.tables()
        });
        let Some(t) = tables else {
            return Vec::new();
        };
        let h = t.header;
        let mut out = vec![
            block_kind_code(t.kind),
            h.map_or(0.0, |h| h.hlit as f64),
            h.map_or(0.0, |h| h.hdist as f64),
            h.map_or(0.0, |h| h.hclen as f64),
        ];
        for groups in [&t.lit_len, &t.distance] {
            out.push(groups.len() as f64);
            for g in groups {
                out.extend_from_slice(&[g.len as f64, g.first_code as f64, g.symbols.len() as f64]);
                out.extend(g.symbols.iter().map(|&s| s as f64));
            }
        }
        out
    }
```

In the non-exported `impl Parsed`, after `decoder_at`:

```rust
    /// A decoder whose next step is `index`, or `None` when decoding stops
    /// before reaching it.
    fn decoder_before(&self, index: f64) -> Option<Decoder<'_>> {
        let body = self.body()?;
        if !(index.is_finite() && index >= 0.0) {
            return None;
        }
        let index = index as u64;
        let mut d = self.decoder_at(body, index);
        while d.next_index() < index {
            d.step()?.ok()?;
        }
        Some(d)
    }
```

Free functions; `encode` switches to `block_kind_code`:

```rust
fn block_kind_code(kind: BlockKind) -> f64 {
    match kind {
        BlockKind::Stored => 0.0,
        BlockKind::Fixed => 1.0,
        BlockKind::Dynamic => 2.0,
    }
}

/// The error's number on the JS side, where `codes.ts` words it.
fn error_code(err: InflateError) -> f64 {
    match err {
        InflateError::Bits => 0.0,
        InflateError::BadHuffmanCode => 1.0,
        InflateError::BadCodeLengths => 2.0,
        InflateError::BadDistance => 3.0,
        InflateError::BadSymbol => 4.0,
        InflateError::BadZlibHeader => 5.0,
        InflateError::ChecksumMismatch => 6.0,
        InflateError::OutputTooLarge => 7.0,
        InflateError::BadCheckpoint => 8.0,
    }
}
```

- [ ] **Step 4: Run the tests and the wasm check**

Run: `cargo test -p hexscope-wasm && cargo check -p hexscope-wasm --target wasm32-unknown-unknown`
Expected: pass.

- [ ] **Step 5: Commit**

```bash
cargo fmt --all && cargo clippy --all-targets --all -- -D warnings
git add crates/hexscope-wasm
git commit -m "feat(wasm): explain a step and its block's tables on demand"
```

---

### Task 4: The player shows bits and codes

**Files:**
- Create: `apps/web/src/deflate.ts`, `apps/web/src/codes.ts`
- Modify: `apps/web/src/player.ts`, `apps/web/src/worker.ts`, `apps/web/src/main.ts`, `apps/web/src/style.css`

**Interfaces:**
- Consumes: `Parsed.explain`, `Parsed.tables` from Task 3.
- Produces: `PlayerSource.explain(index: number, knownBlock: number): Promise<{ parts: Float64Array; tables: Float64Array | null }>`; `CodesView` with `bits: HTMLElement`, `panel: HTMLElement`, `show(step, explained, tables)`, `clear()`.

- [ ] **Step 1: Move the step model to `deflate.ts`**

```ts
/** The DEFLATE steps the worker sends, as the player and the codes view read them. */

/** Numbers per step in a batch from the worker; see `Parsed::steps`. */
export const STRIDE = 6;

export const StepKind = { BlockStart: 0, Literal: 1, Match: 2, BlockEnd: 3, Failure: 4 } as const;
export const BLOCK_NAMES = ["stored", "fixed Huffman", "dynamic Huffman"];

export interface Step {
  index: number;
  kind: number;
  /** Block kind, literal byte or distance, by `kind`. */
  a: number;
  /** Match length. */
  b: number;
  bitStart: number;
  bitEnd: number;
  outStart: number;
}
```

In `player.ts` delete those four definitions and add
`import { BLOCK_NAMES, STRIDE, StepKind, type Step } from "./deflate";`.
Run `pnpm --filter web exec tsc --noEmit`: expected clean.

- [ ] **Step 2: Worker message and source**

`worker.ts`: add to `WorkerRequest`
`| { id: number; type: "explain"; index: number; knownBlock: number }` and to
`WorkerResponse`
`| { id: number; type: "explain"; parts: Float64Array; tables: Float64Array | null }`.
Replace the final `else` branch with:

```ts
  } else if (req.type === "explain") {
    const parts = current.explain(req.index);
    // Tables change once per block: send them only when the page's are stale.
    const tables = parts.length > 0 && parts[0] !== req.knownBlock ? current.tables(req.index) : null;
    post({ id: req.id, type: "explain", parts, tables }, tables ? [parts.buffer, tables.buffer] : [parts.buffer]);
  } else {
    const bytes = current.inflated;
    post({ id: req.id, type: "inflated", bytes }, [bytes.buffer]);
  }
```

`main.ts`, in the `player.open` source object:

```ts
    explain: async (index, knownBlock) => {
      const r = await call({ type: "explain", index, knownBlock });
      return r.type === "explain" ? { parts: r.parts, tables: r.tables } : { parts: new Float64Array(0), tables: null };
    },
```

`player.ts`, `PlayerSource` gains:

```ts
  /** Step `index` part by part; the block's tables unless `knownBlock` is its block. */
  explain(index: number, knownBlock: number): Promise<{ parts: Float64Array; tables: Float64Array | null }>;
```

- [ ] **Step 3: Write `codes.ts`**

```ts
import { HEX } from "./canvas";
import { BLOCK_NAMES, StepKind, type Step } from "./deflate";

/** What a run of bits meant; numbered as `PartKind` in the core. */
const Part = {
  Final: 0, BlockType: 1, Padding: 2, StoredLen: 3, StoredNLen: 4, StoredByte: 5,
  HLit: 6, HDist: 7, HClen: 8, CodeLengthCode: 9, CodeLengths: 10,
  LitLen: 11, LengthExtra: 12, Distance: 13, DistanceExtra: 14, Unreadable: 15,
} as const;

const PART_STRIDE = 6;
/** Indexed by the bridge's error code. */
const ERRORS = [
  "the input ran out",
  "no code in this block's table starts with these bits",
  "the code lengths do not form a valid code",
  "the distance reaches before the start of the output",
  "the symbol is not defined in DEFLATE",
  "the zlib header does not describe DEFLATE",
  "the checksum does not match",
  "the output would exceed the size limit",
  "the decoder could not resume here",
];

export interface CodePart {
  kind: number;
  bitStart: number;
  bitEnd: number;
  value: number;
  code: number;
  codeLen: number;
}

export interface Explained {
  blockStart: number;
  error: string | null;
  parts: CodePart[];
}

export interface CodeGroup {
  len: number;
  firstCode: number;
  symbols: number[];
}

export interface BlockTables {
  kind: number;
  hlit: number;
  hdist: number;
  hclen: number;
  litLen: CodeGroup[];
  distance: CodeGroup[];
}

export function readExplained(a: Float64Array): Explained | null {
  if (a.length < 3) return null;
  const parts: CodePart[] = [];
  for (let i = 0; i < a[2]; i++) {
    const o = 3 + i * PART_STRIDE;
    parts.push({ kind: a[o], bitStart: a[o + 1], bitEnd: a[o + 2], value: a[o + 3], code: a[o + 4], codeLen: a[o + 5] });
  }
  return { blockStart: a[0], error: a[1] < 0 ? null : (ERRORS[a[1]] ?? "decoding failed"), parts };
}

export function readTables(a: Float64Array): BlockTables | null {
  if (a.length < 4) return null;
  let o = 4;
  const table = (): CodeGroup[] => {
    const groups: CodeGroup[] = [];
    const n = a[o++];
    for (let g = 0; g < n; g++) {
      const len = a[o++];
      const firstCode = a[o++];
      const count = a[o++];
      groups.push({ len, firstCode, symbols: Array.from(a.subarray(o, o + count)) });
      o += count;
    }
    return groups;
  };
  const litLen = table();
  const distance = table();
  return { kind: a[0], hlit: a[1], hdist: a[2], hclen: a[3], litLen, distance };
}

const fmt = (n: number) => n.toLocaleString();
const bin = (v: number, width: number) => v.toString(2).padStart(width, "0");

function el(tag: string, cls: string, text?: string): HTMLElement {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (text !== undefined) e.textContent = text;
  return e;
}

/** A Huffman code as the decoder assembled it; a field as its number; long runs as a count. */
function bitsText(p: CodePart): string {
  const width = p.bitEnd - p.bitStart;
  if (p.kind === Part.LitLen || p.kind === Part.Distance) return bin(p.code, p.codeLen);
  if (p.kind === Part.Unreadable || width > 16) return `${fmt(width)} bits`;
  return bin(p.value, width);
}

function partLabel(p: CodePart, step: Step): string {
  const match = step.kind === StepKind.Match;
  switch (p.kind) {
    case Part.Final: return p.value ? "BFINAL · last block" : "BFINAL · more follow";
    case Part.BlockType: return `BTYPE · ${BLOCK_NAMES[p.value] ?? "reserved"}`;
    case Part.Padding: return "padding to a byte";
    case Part.StoredLen: return `LEN · ${fmt(p.value)} bytes`;
    case Part.StoredNLen: return "NLEN · LEN inverted";
    case Part.StoredByte: return `byte 0x${HEX[p.value]}`;
    case Part.HLit: return `HLIT · ${p.value} lit/len codes`;
    case Part.HDist: return `HDIST · ${p.value} distance codes`;
    case Part.HClen: return `HCLEN · ${p.value} code-length codes`;
    case Part.CodeLengthCode: return "code-length code";
    case Part.CodeLengths: return `code lengths · ${p.value} symbols`;
    case Part.LitLen:
      return p.value < 256 ? `literal 0x${HEX[p.value]}` : p.value === 256 ? "end of block" : `length code ${p.value}`;
    case Part.LengthExtra: return match ? `+${p.value} → length ${fmt(step.b)}` : `+${p.value}`;
    case Part.Distance: return `distance code ${p.value}`;
    case Part.DistanceExtra: return match ? `+${p.value} → ${fmt(step.a)} back` : `+${p.value}`;
    default: return "unreadable";
  }
}

function partClass(kind: number): string {
  if (kind === Part.LitLen || kind === Part.Distance) return "is-code";
  if (kind === Part.LengthExtra || kind === Part.DistanceExtra) return "is-extra";
  if (kind === Part.StoredByte) return "is-byte";
  if (kind === Part.Unreadable) return "is-bad";
  return "is-header";
}

/** Symbol positions worth showing: all of a short list, else the ends and the hit's neighbours; -1 marks a gap. */
function shownIndices(n: number, hit: number): number[] {
  if (n <= 14) return Array.from({ length: n }, (_, i) => i);
  const keep = new Set([0, 1, 2, n - 2, n - 1]);
  const centre = hit >= 0 ? hit : 5;
  for (let i = centre - 3; i <= centre + 3; i++) if (i >= 0 && i < n) keep.add(i);
  const sorted = [...keep].sort((x, y) => x - y);
  const out: number[] = [];
  sorted.forEach((i, k) => {
    if (k > 0 && i !== sorted[k - 1] + 1) out.push(-1);
    out.push(i);
  });
  return out;
}

function symbolName(table: "lit" | "dist", s: number): string {
  if (table === "dist") return String(s);
  return s < 256 ? HEX[s] : s === 256 ? "end" : String(s);
}

/**
 * The bits of the step on screen, split into what they meant, and the code
 * tables of its block with the path its codes took through them.
 */
export class CodesView {
  readonly bits = el("div", "player-bits");
  readonly panel = el("div", "player-codes");

  clear(): void {
    this.bits.replaceChildren();
    this.panel.replaceChildren();
  }

  show(step: Step, ex: Explained | null, tables: BlockTables | null): void {
    if (!ex) {
      this.clear();
      return;
    }
    this.bits.replaceChildren(
      ...ex.parts.map((p) => {
        const part = el("span", `bits-part ${partClass(p.kind)}`);
        part.append(el("code", "", bitsText(p)), el("small", "", partLabel(p, step)));
        return part;
      }),
    );
    if (ex.error) this.bits.append(el("span", "bits-error", `Stopped: ${ex.error}.`));

    const sections: HTMLElement[] = [];
    const codes = ex.parts.filter((p) => p.kind === Part.LitLen || p.kind === Part.Distance);
    if (!tables) {
      if (ex.parts.some((p) => p.kind === Part.StoredLen || p.kind === Part.StoredByte)) {
        sections.push(el("p", "codes-note", "Stored block: no codes — its bytes are copied as they are."));
      }
    } else if (step.kind === StepKind.BlockStart) {
      if (tables.hlit) {
        sections.push(el("p", "codes-note", `This header builds ${tables.hlit} literal/length and ${tables.hdist} distance codes, using a ${tables.hclen}-symbol code for their lengths.`));
      }
      sections.push(this.table("Literal/length codes", "lit", tables.litLen, null));
      sections.push(this.table("Distance codes", "dist", tables.distance, null));
    } else {
      for (const p of codes) {
        sections.push(
          p.kind === Part.LitLen
            ? this.table("Literal/length codes", "lit", tables.litLen, p)
            : this.table("Distance codes", "dist", tables.distance, p),
        );
      }
    }
    this.panel.replaceChildren(...sections);
  }

  private table(title: string, kind: "lit" | "dist", groups: CodeGroup[], hit: CodePart | null): HTMLElement {
    const sec = el("section", "codes-table");
    sec.append(el("h4", "", title), el("small", "codes-hint", kind === "lit" ? "literals in hex · 256 ends the block · 257+ are lengths" : "symbols are distance codes"));
    for (const g of groups) {
      const count = g.symbols.length;
      const last = g.firstCode + count - 1;
      const row = el("div", "codes-row");
      let state = "";
      let why = "";
      let hitIndex = -1;
      if (hit) {
        if (g.len < hit.codeLen) {
          // Canonical codes: a longer code's prefix lies past every code of a shorter length.
          const prefix = hit.code >> (hit.codeLen - g.len);
          state = "is-passed";
          why = `${bin(prefix, g.len)} is past this range`;
        } else if (g.len === hit.codeLen) {
          hitIndex = hit.code - g.firstCode;
          state = "is-hit";
          why = `${bin(hit.code, g.len)} is #${hitIndex + 1} here`;
        } else {
          state = "is-after";
        }
      }
      if (state) row.classList.add(state);
      const symbols = el("span", "codes-symbols");
      for (const i of shownIndices(count, hitIndex)) {
        symbols.append(i < 0 ? el("span", "codes-gap", "…") : el("span", i === hitIndex ? "codes-sym is-hit" : "codes-sym", symbolName(kind, g.symbols[i])));
      }
      row.append(
        el("span", "codes-len", `${g.len} bits`),
        el("span", "codes-count", `×${count}`),
        el("span", "codes-range", count > 1 ? `${bin(g.firstCode, g.len)}–${bin(last, g.len)}` : bin(g.firstCode, g.len)),
        symbols,
      );
      if (why) row.append(el("span", "codes-why", why));
      sec.append(row);
    }
    return sec;
  }
}
```

- [ ] **Step 4: Wire `player.ts`**

Template: replace `<canvas class="player-strip"></canvas>` with
`<div class="player-stage"><canvas class="player-strip"></canvas></div>`.
After `host.append(this.root)`:

```ts
    this.root.querySelector(".player-meta")!.after(this.codes.bits);
    this.root.querySelector(".player-stage")!.append(this.codes.panel);
```

Fields:

```ts
  private readonly codes = new CodesView();
  /** The tables of the block last explained; the worker resends them only on a new block. */
  private tables: { block: number; tables: BlockTables | null } = { block: -1, tables: null };
  private explaining = false;
  private wanted = -1;
```

In `open` and `close`, reset: `this.codes.clear(); this.tables = { block: -1, tables: null }; this.wanted = -1;`.

At the end of `render()`, after `this.draw()`: `this.explain(step);`

New method in the data section:

```ts
  /**
   * Asks for the breakdown of the step on screen. One request is in flight at
   * a time; when it returns and the player has moved on, the newest step is
   * asked for next, so fast playback costs one request per round trip.
   */
  private explain(step: Step): void {
    this.wanted = step.index;
    const source = this.source;
    if (this.explaining || !source) return;
    this.explaining = true;
    const gen = this.generation;
    void source
      .explain(step.index, this.tables.block)
      .then(({ parts, tables }) => {
        if (gen !== this.generation) return;
        const ex = readExplained(parts);
        if (ex && tables) this.tables = { block: ex.blockStart, tables: readTables(tables) };
        if (this.wanted === step.index) {
          const current = ex && ex.blockStart === this.tables.block ? this.tables.tables : null;
          this.codes.show(step, ex, current);
        }
      })
      .finally(() => {
        this.explaining = false;
        if (gen !== this.generation || this.wanted === step.index) return;
        const next = this.stepAt(this.wanted);
        if (next) this.explain(next);
      });
  }
```

Import: `import { CodesView, readExplained, readTables, type BlockTables } from "./codes";`.

- [ ] **Step 5: CSS**

In `style.css`: change `body.is-playing .layout` rows to
`minmax(0, 1fr) 400px`; `.drawer-player` rows to
`auto auto auto auto minmax(0, 1fr) auto`. Append to the player section:

```css
.player-stage {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(300px, 40%);
  gap: 16px;
  min-height: 0;
}

.player-bits {
  display: flex;
  flex-wrap: wrap;
  gap: 4px 6px;
  min-height: 38px;
}

.bits-part {
  display: grid;
  gap: 1px;
  padding: 3px 7px;
  border-radius: 6px;
  background: color-mix(in srgb, var(--part) 14%, transparent);
  border: 1px solid color-mix(in srgb, var(--part) 35%, transparent);
}

.bits-part code {
  font: 600 12.5px var(--mono);
  color: var(--part);
  letter-spacing: 0.04em;
}

.bits-part small {
  font-size: 11px;
  color: var(--muted);
  white-space: nowrap;
}

.bits-part.is-code { --part: var(--tint-idat); }
.bits-part.is-extra { --part: var(--tint-sig); }
.bits-part.is-header { --part: var(--tint-ihdr); }
.bits-part.is-byte { --part: var(--tint-anc); }
.bits-part.is-bad { --part: var(--tint-error); }

.bits-error {
  align-self: center;
  font-size: 12.5px;
  color: var(--tint-error);
}

.player-codes {
  min-height: 0;
  overflow: auto;
  font-size: 12px;
}

.codes-table + .codes-table {
  margin-top: 10px;
}

.codes-table h4 {
  display: inline;
  margin: 0 8px 0 0;
  font-size: 12px;
}

.codes-hint,
.codes-note {
  color: var(--faint);
  font-size: 11px;
}

.codes-note {
  margin: 0 0 8px;
  font-size: 12px;
  color: var(--muted);
}

.codes-row {
  display: grid;
  grid-template-columns: 44px 40px auto minmax(0, 1fr);
  align-items: baseline;
  gap: 2px 8px;
  padding: 2px 6px;
  border-radius: 5px;
  font-family: var(--mono);
}

.codes-len { color: var(--muted); }
.codes-count { color: var(--faint); }
.codes-range { color: var(--text); }

.codes-symbols {
  display: flex;
  flex-wrap: wrap;
  gap: 0 5px;
  color: var(--muted);
}

.codes-gap { color: var(--faint); }

.codes-why {
  grid-column: 3 / -1;
  font-size: 11px;
  color: var(--faint);
}

.codes-row.is-passed { opacity: 0.55; }
.codes-row.is-passed .codes-why::before { content: "✗ "; color: var(--tint-error); }
.codes-row.is-after { opacity: 0.35; }
.codes-row.is-hit { background: color-mix(in srgb, var(--tint-idat) 12%, transparent); }
.codes-row.is-hit .codes-why::before { content: "✓ "; color: var(--tint-sig); }

.codes-sym.is-hit {
  padding: 0 4px;
  border-radius: 4px;
  background: var(--tint-idat);
  color: var(--bg);
  font-weight: 600;
}
```

Inside the existing `@media (max-width: 900px)` block:

```css
  .player-stage {
    grid-template-columns: minmax(0, 1fr);
    grid-template-rows: 110px minmax(0, 1fr);
  }
```

If `--bg` is not a defined token, use the token the stylesheet uses for the
page background.

- [ ] **Step 6: Build and check in the browser**

Run: `pnpm --filter web build` (expected: clean). Start the `web` preview,
open "Try a sample", press Play, then pause and step through:
- step 1 (block header): BFINAL/BTYPE/HLIT/HDIST/HCLEN chips, the note, both tables;
- a literal: one code chip; the lit/len table with ruled-out shorter lengths and the hit;
- step 42 (back-reference): four chips, both tables;
- "Try a broken file" → last step: an unreadable chip and "Stopped: …".
Then resize to 375 px width and confirm the panel stacks under the strip.

- [ ] **Step 7: Commit**

```bash
git add apps/web/src
git commit -m "feat(web): show each step's bits and its block's Huffman codes"
```

---

### Task 5: Docs, CI, merge

**Files:**
- Modify: `README.md`, `docs/known-issues.md`

- [ ] **Step 1:** In `docs/known-issues.md`, delete the "Huffman codes in the
  player" paragraph from "Not yet built". In `README.md`'s "What it does",
  extend the DEFLATE player sentence to say it shows each step's bits and
  the block's Huffman codes, grouped by length.
- [ ] **Step 2:** Run every CI check locally: fmt check, clippy, `cargo test --all`, the wasm32 check, `pnpm --filter web build`.
- [ ] **Step 3:** Commit (`docs: the player explains Huffman codes`), push
  `feat/huffman-player`, wait for CI (`gh run watch`), merge into `main`
  with `--no-ff`, push, and wait for CI on `main`.
