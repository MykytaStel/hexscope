/** Node kinds, matching the codes `hexscope-wasm` emits. */
export const Kind = { Container: 0, Field: 1, Warning: 2, Error: 3 } as const;
export type Kind = (typeof Kind)[keyof typeof Kind];

/** What the worker hands back: the flattened tree plus file-level facts. */
export interface ParsedFile {
  starts: Float64Array;
  lens: Float64Array;
  parents: Int32Array;
  kinds: Uint8Array;
  labels: string[];
  values: string[];
  /** [width, height, bitDepth, colorType, interlace], or null. */
  ihdr: number[] | null;
  /** [totalEvents, literals, matches, outputBytes], or null. */
  trace: number[] | null;
  idatBytes: number;
  parseMs: number;
}

/** Colour families for the hex view, keyed by the top-level chunk. */
export type Tint =
  | "sig"
  | "ihdr"
  | "plte"
  | "idat"
  | "iend"
  | "text"
  | "anc"
  | "warning"
  | "error";

const CHUNK_TINTS: Record<string, Tint> = {
  signature: "sig",
  IHDR: "ihdr",
  PLTE: "plte",
  tRNS: "plte",
  IDAT: "idat",
  IEND: "iend",
  tEXt: "text",
  zTXt: "text",
  iTXt: "text",
};

/**
 * The parse tree plus the indexes the UI needs to stay fast: children in
 * compressed-sparse-row form, and per node the spatial children sorted by
 * offset so "which node owns byte N" is a short descent of binary searches
 * instead of a scan.
 */
export class FileModel {
  readonly count: number;
  readonly depth: Uint16Array;
  /** Top-level ancestor of each node (a direct child of the root). */
  readonly top: Int32Array;
  readonly problems: number[];

  private readonly childOffsets: Uint32Array;
  private readonly childIds: Int32Array;
  /** Children with a non-empty range, sorted by start: the descent index. */
  private readonly spatial: Int32Array[];

  constructor(
    readonly file: ParsedFile,
    readonly bytes: Uint8Array,
    readonly name: string,
  ) {
    const { parents, kinds, starts, lens } = file;
    const n = parents.length;
    this.count = n;

    // Parents always precede children in the arena, so one forward pass
    // gives depth and top-level ancestry.
    this.depth = new Uint16Array(n);
    this.top = new Int32Array(n);
    this.top[0] = 0;
    for (let i = 1; i < n; i++) {
      const p = parents[i];
      this.depth[i] = this.depth[p] + 1;
      this.top[i] = p === 0 ? i : this.top[p];
    }

    const counts = new Uint32Array(n + 1);
    for (let i = 1; i < n; i++) counts[parents[i] + 1]++;
    for (let i = 0; i < n; i++) counts[i + 1] += counts[i];
    this.childOffsets = counts.slice();
    this.childIds = new Int32Array(Math.max(0, n - 1));
    const cursor = counts.slice();
    for (let i = 1; i < n; i++) this.childIds[cursor[parents[i]]++] = i;

    this.spatial = new Array(n);
    for (let i = 0; i < n; i++) {
      const kids = Array.from(this.children(i)).filter((c) => lens[c] > 0);
      // Ties on start: problems sort last, so a warning on the very byte a
      // field decodes wins the hover — that byte's most important fact.
      kids.sort((a, b) => starts[a] - starts[b] || kinds[a] - kinds[b]);
      this.spatial[i] = Int32Array.from(kids);
    }

    this.problems = [];
    for (let i = 0; i < n; i++) if (kinds[i] >= Kind.Warning) this.problems.push(i);
  }

  children(id: number): Int32Array {
    return this.childIds.subarray(this.childOffsets[id], this.childOffsets[id + 1]);
  }

  hasChildren(id: number): boolean {
    return this.childOffsets[id + 1] > this.childOffsets[id];
  }

  label(id: number): string {
    return this.file.labels[id];
  }

  value(id: number): string {
    return this.file.values[id];
  }

  kind(id: number): Kind {
    return this.file.kinds[id] as Kind;
  }

  start(id: number): number {
    return this.file.starts[id];
  }

  len(id: number): number {
    return this.file.lens[id];
  }

  end(id: number): number {
    return this.file.starts[id] + this.file.lens[id];
  }

  /** Root-to-node chain of ids, root first. */
  path(id: number): number[] {
    const out: number[] = [];
    for (let i = id; i >= 0; i = this.file.parents[i]) out.push(i);
    return out.reverse();
  }

  /** Deepest node whose range contains `offset`, or -1 outside the file. */
  nodeAt(offset: number): number {
    const { starts, lens } = this.file;
    if (offset < 0 || offset >= this.bytes.length) return -1;

    let node = 0;
    for (;;) {
      const kids = this.spatial[node];
      // Last child starting at or before `offset`.
      let lo = 0;
      let hi = kids.length - 1;
      let hit = -1;
      while (lo <= hi) {
        const mid = (lo + hi) >> 1;
        if (starts[kids[mid]] <= offset) {
          hit = mid;
          lo = mid + 1;
        } else {
          hi = mid - 1;
        }
      }
      if (hit < 0) return node;
      const kid = kids[hit];
      if (offset >= starts[kid] + lens[kid]) return node;
      node = kid;
    }
  }

  /** Colour family for a node, from its own kind or its top-level chunk. */
  tint(id: number): Tint {
    const kind = this.kind(id);
    if (kind === Kind.Error) return "error";
    if (kind === Kind.Warning) return "warning";
    const top = this.top[id];
    if (top <= 0) return "anc";
    if (this.kind(top) === Kind.Error) return "error";
    return CHUNK_TINTS[this.label(top)] ?? "anc";
  }
}
