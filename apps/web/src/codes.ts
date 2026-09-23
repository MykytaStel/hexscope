import { HEX } from "./canvas";
import { BLOCK_NAMES, StepKind, type Step } from "./deflate";

/** What a run of bits meant; numbered as `PartKind` in the core. */
const Part = {
  Final: 0,
  BlockType: 1,
  Padding: 2,
  StoredLen: 3,
  StoredNLen: 4,
  StoredByte: 5,
  HLit: 6,
  HDist: 7,
  HClen: 8,
  CodeLengthCode: 9,
  CodeLengths: 10,
  LitLen: 11,
  LengthExtra: 12,
  Distance: 13,
  DistanceExtra: 14,
  Unreadable: 15,
} as const;

/** Numbers per part in `Parsed::explain`. */
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

/** What the header counts are stored as: the count less its minimum (RFC 1951 §3.2.7). */
const COUNT_BIAS: Record<number, number> = { [Part.HLit]: 257, [Part.HDist]: 1, [Part.HClen]: 4 };

/** A Huffman code as the decoder assembled it; a field as its number; long runs as a count. */
function bitsText(p: CodePart): string {
  const width = p.bitEnd - p.bitStart;
  if (p.kind === Part.LitLen || p.kind === Part.Distance) return bin(p.code, p.codeLen);
  if (p.kind === Part.Unreadable || width > 16) return `${fmt(width)} bits`;
  return bin(p.value - (COUNT_BIAS[p.kind] ?? 0), width);
}

function partLabel(p: CodePart, step: Step): string {
  const match = step.kind === StepKind.Match;
  switch (p.kind) {
    case Part.Final:
      return p.value ? "BFINAL · last block" : "BFINAL · more follow";
    case Part.BlockType:
      return `BTYPE · ${BLOCK_NAMES[p.value] ?? "reserved"}`;
    case Part.Padding:
      return "padding to a byte";
    case Part.StoredLen:
      return `LEN · ${fmt(p.value)} bytes`;
    case Part.StoredNLen:
      return "NLEN · LEN inverted";
    case Part.StoredByte:
      return `byte 0x${HEX[p.value]}`;
    case Part.HLit:
      return `HLIT · ${p.value - 257} + 257 = ${p.value} lit/len codes`;
    case Part.HDist:
      return `HDIST · ${p.value - 1} + 1 = ${p.value} distance codes`;
    case Part.HClen:
      return `HCLEN · ${p.value - 4} + 4 = ${p.value} code-length codes`;
    case Part.CodeLengthCode:
      return "code-length code";
    case Part.CodeLengths:
      return `code lengths · ${p.value} symbols`;
    case Part.LitLen:
      return p.value < 256 ? `literal 0x${HEX[p.value]}` : p.value === 256 ? "end of block" : `length code ${p.value}`;
    case Part.LengthExtra:
      return match ? `+${p.value} → length ${fmt(step.b)}` : `+${p.value}`;
    case Part.Distance:
      return `distance code ${p.value}`;
    case Part.DistanceExtra:
      return match ? `+${p.value} → ${fmt(step.a)} back` : `+${p.value}`;
    default:
      return "unreadable";
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
    if (!tables) {
      if (ex.parts.some((p) => p.kind === Part.StoredLen || p.kind === Part.StoredByte)) {
        sections.push(el("p", "codes-note", "Stored block: no codes — its bytes are copied as they are."));
      }
    } else if (step.kind === StepKind.BlockStart) {
      if (tables.hlit) {
        sections.push(
          el(
            "p",
            "codes-note",
            `This header builds ${tables.hlit} literal/length and ${tables.hdist} distance codes, using a ${tables.hclen}-symbol code for their lengths.`,
          ),
        );
      }
      sections.push(this.table("Literal/length codes", "lit", tables.litLen, null));
      sections.push(this.table("Distance codes", "dist", tables.distance, null));
    } else {
      for (const p of ex.parts) {
        if (p.kind === Part.LitLen) sections.push(this.table("Literal/length codes", "lit", tables.litLen, p));
        if (p.kind === Part.Distance) sections.push(this.table("Distance codes", "dist", tables.distance, p));
      }
    }
    this.panel.replaceChildren(...sections);
  }

  private table(title: string, kind: "lit" | "dist", groups: CodeGroup[], hit: CodePart | null): HTMLElement {
    const sec = el("section", "codes-table");
    sec.append(
      el("h4", "", title),
      el(
        "small",
        "codes-hint",
        kind === "lit" ? "literals in hex · 256 ends the block · 257+ are lengths" : "symbols are distance codes",
      ),
    );
    for (const g of groups) {
      const count = g.symbols.length;
      const last = g.firstCode + count - 1;
      const row = el("div", "codes-row");
      let why = "";
      let hitIndex = -1;
      if (hit) {
        if (g.len < hit.codeLen) {
          // Canonical codes: a longer code's prefix lies past every code of a shorter length.
          row.classList.add("is-passed");
          why = `${bin(hit.code >> (hit.codeLen - g.len), g.len)} is past this range`;
        } else if (g.len === hit.codeLen) {
          hitIndex = hit.code - g.firstCode;
          row.classList.add("is-hit");
          why = `${bin(hit.code, g.len)} is #${hitIndex + 1} here`;
        } else {
          row.classList.add("is-after");
        }
      }
      const symbols = el("span", "codes-symbols");
      for (const i of shownIndices(count, hitIndex)) {
        symbols.append(
          i < 0
            ? el("span", "codes-gap", "…")
            : el("span", i === hitIndex ? "codes-sym is-hit" : "codes-sym", symbolName(kind, g.symbols[i])),
        );
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
