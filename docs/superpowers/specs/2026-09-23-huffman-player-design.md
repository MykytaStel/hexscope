# Huffman codes in the DEFLATE player

## Goal

The player says *what* each step did ("Back-reference — copy 12 bytes from 97
back") and *how many* bits it read. It does not say which bits meant what, or
where the codes came from. This milestone shows both:

1. **The bits of a step, split into their parts** — the literal/length code,
   its extra bits, the distance code, its extra bits — each with its value.
2. **The block's code tables, grouped by code length**, with the path the
   current code took through them: too long for every shorter length, inside
   the range of its own.

The dynamic block header stays one step. It gains a breakdown (BFINAL, BTYPE,
HLIT, HDIST, HCLEN, the code-length code, the code lengths) and a summary of
the tables it builds, but its internals are not stepped through.

## Non-goals

- Stepping through the code-length alphabet and the 16/17/18 repeats.
- A drawn binary tree. The decoder is canonical and never builds one; the
  grouped view is what it actually does.
- Any change to the cost of parsing or of fetching step batches.

## Approach: explanation on demand

Only the step on screen needs a breakdown. Recording parts for every step
would make every batch larger for one step's benefit, so the breakdown is
computed when asked: the worker resumes the decoder just before the step and
decodes that one step with a recorder switched on.

### Core (`crates/hexscope-core/src/inflate`)

**`explain.rs`** (new) holds the types:

```rust
pub enum PartKind {
    Final, BlockType, Padding, StoredLen, StoredNLen, StoredByte,
    HLit, HDist, HClen, CodeLengthCode, CodeLengths,
    LitLen, LengthExtra, Distance, DistanceExtra,
    /// Bits consumed by a read that failed.
    Unreadable,
}

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

pub struct Explained {
    pub step: Option<Step>,           // None when the step failed
    pub error: Option<InflateError>,
    pub parts: Vec<Part>,
    pub block_start: u64,
}

pub struct CodeGroup { pub len: u8, pub first_code: u16, pub symbols: Vec<u16> }

pub struct DynamicHeader { pub hlit: u16, pub hdist: u8, pub hclen: u8 }

pub struct BlockTables {
    pub kind: BlockKind,
    /// HLIT, HDIST, HCLEN for a dynamic block.
    pub header: Option<DynamicHeader>,
    pub lit_len: Vec<CodeGroup>,
    pub distance: Vec<CodeGroup>,
}
```

**`Decoder`** gains:

- `explain_next(&mut self) -> Option<Explained>`: decodes one step with
  recording on; `None` once decoding has finished.
- `next_index(&self) -> u64`: the index the next step will have.
- `tables(&self) -> Option<BlockTables>`: the current block's tables, `None`
  for stored blocks or between blocks.

Recording is an `Option<Vec<Part>>` field. `step()` leaves it `None`, so the
hot loop only pays for a branch per part. The 10 MB benchmark confirms no
regression.

**`Huffman`** gains `decode_traced`, which also returns the assembled code and
its length, and `groups()`, which derives each length's first canonical code
from `counts` (RFC 1951 §3.2.2) and slices `symbols`. Nothing new is stored.

A read that fails still yields a part: `Unreadable`, from where the read began
to where the reader stopped. A code matching nothing in the table is the most
common kind of damage, and this names exactly its bits.

### Bridge (`crates/hexscope-wasm`)

- `explain(index) -> Vec<f64>`: resumes at the nearest checkpoint via
  `decoder_at`, steps to `index`, then calls `explain_next`. The layout is
  `[block_start, error (−1 if none), n, then n × (kind, bit_start, bit_end,
  value, code, code_len)]`.
- `tables(index) -> Vec<f64>`: the tables of the block holding step `index`:
  `[kind, hlit, hdist, hclen, then per table: groups, then per group: len,
  first_code, count, symbols…]`. Empty for stored blocks.

The worker answers one `explain` message with both: the parts, and the tables
only when the block differs from the one the page already holds. One request
is in flight at a time; when it returns, the newest wanted step is asked for
next.

### Web (`apps/web/src`)

A new module, **`codes.ts`**, renders both views as DOM, not canvas: they are
text and tables, and DOM gives selection, wrapping and accessibility.

- **Bit row**, under the step sentence: the step's bits in reading order,
  grouped into parts, each part tinted and labelled underneath ("len 265 · 7
  bits", "+1", "dist 9", "+89"). A Huffman code shows as the decoder assembled
  it. Extra bits show as a number, since they are one.
- **Codes panel**, beside the strip where the width allows and below it when
  it does not: one row per code length with symbol count, code range and
  symbols. Shorter lengths the code passed through are marked as ruled out,
  and the matching length highlights the symbol. A back-reference shows the
  literal/length table, then the distance table. A block header shows HLIT,
  HDIST and HCLEN with both tables. Stored blocks show that there are no
  codes.

Tables are cached by `block_start`, so crossing into a new block costs one
fetch. During playback an explanation is requested at most once per drawn
frame; paused, every step is explained. `player.ts` only wires this up, which
keeps the new code out of the file that draws the strip.

## Errors

- **The step failed:** the parts up to the failure plus an `Unreadable` part,
  and the error in words, e.g. "no code in this block's table starts with
  these 15 bits".
- **Resuming fails** (a checkpoint is inconsistent, which the fuzzer has never
  produced): `explain` returns an empty result, and the panel hides instead of
  showing something wrong.
- **Large or hostile input:** `explain` replays at most one checkpoint
  interval (512 steps). Tables are at most 288 + 30 symbols.

## Testing

- **Partition:** for every step of every PngSuite file, the explained parts
  are contiguous and cover exactly `[bit_start, bit_end)`, and `explain_next`
  yields the same `Step` as `step()`.
- **Codes agree with tables:** every `LitLen` and `Distance` part's
  `(code, code_len)` sits inside the group for that length, at the symbol's
  position.
- **Canonical codes:** `groups()` reproduces the RFC 1951 §3.2.2 example
  (lengths 3,3,3,3,3,2,4,4 give F=00, A=010 … H=1111) and the fixed table
  (7 bits from 0000000, 8 bits from 00110000, 9 bits from 110010000).
- **Damage:** a truncated stream explains its failing step with an
  `Unreadable` part ending at the end of the input. An undefined symbol
  (fixed-Huffman code 286) is named by its `LitLen` part, with no
  `Unreadable` part after it, because every bit it read was understood.
- **Bridge:** `explain(i)` agrees with `steps(i, 1)` on bit range and kind.
- **Properties/fuzz:** `explain` never panics on arbitrary bytes. It joins the
  existing proptest suite.
- **Performance:** the 10 MB parse benchmark is unchanged within noise.
- **Browser:** the sample PNG shows literal, back-reference, block header and
  failure steps (via "Try a broken file"), checked at desktop and 375 px width.
