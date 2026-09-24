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
  /** Compressed bytes of the stream the player steps through. */
  idatBytes: number;
  /** That stream's pieces in the file as [start, len, start, len, ...]. */
  segments: Float64Array;
  /** Bytes of wrapper before its DEFLATE data: 2 for zlib, 0 for a ZIP entry. */
  streamHeader: number;
  /** Per ZIP entry: node, method, flags, compressed, uncompressed, playable, openable. */
  entries: Float64Array;
  /** Per node, an index into the doc tables, or -1. */
  docIds: Int32Array;
  docTexts: string[];
  docCites: string[];
  docUrls: string[];
  /** Per doc: 0 not a problem, 1 damage, 2 something hidden, 3 an oddity. */
  docConcerns: Uint8Array;
  format: "png" | "jpeg" | "zip" | "unknown";
  /** [width, height], or null when the file does not say. */
  dimensions: [number, number] | null;
  /** What a photo's metadata reveals; empty for anything else. */
  facts: PhotoFact[];
  location: PhotoLocation | null;
  parseMs: number;
}

/** How worried to be about a problem, as the core classifies it. */
export const Concern = { None: 0, Damage: 1, Hidden: 2, Oddity: 3 } as const;

/** What a node is, in one sentence, and where its format defines it. */
export interface NodeDoc {
  text: string;
  cite: string;
  url: string;
  concern: number;
}

/** Numbers per entry in `ParsedFile.entries`. */
const ENTRY_STRIDE = 7;

export interface ZipEntryInfo {
  node: number;
  method: number;
  flags: number;
  compressed: number;
  uncompressed: number;
  playable: boolean;
  openable: boolean;
}

export interface PhotoFact {
  kind: string;
  text: string;
  node: number;
}

export interface PhotoLocation {
  latitude: number;
  longitude: number;
  altitude: number | null;
  /** The GPS IFD. */
  node: number;
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
  | "gps"
  | "warning"
  | "error";

/** Colour by top-level node: PNG chunks and JPEG segments. */
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
  SOI: "sig",
  EOI: "iend",
  APP1: "text",
  COM: "text",
  DQT: "plte",
  DHT: "plte",
  SOS: "idat",
  "scan data": "idat",
};

/** "APP1 · EXIF" is keyed by "APP1"; every SOFn is a frame header. */
/** An archive's top level: its entries, then its directory and end records. */
function zipTint(label: string): Tint {
  if (label === "central directory") return "ihdr";
  if (label.includes("end of central directory") || label === "ZIP64 locator") return "iend";
  return "text";
}

function chunkTint(label: string): Tint {
  const key = label.split(" · ")[0];
  if (/^SOF\d+$/.test(key)) return "ihdr";
  return CHUNK_TINTS[key] ?? "anc";
}

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
  private readonly tints: Tint[];

  private readonly childOffsets: Uint32Array;
  private readonly childIds: Int32Array;
  /** Children with a non-empty range, sorted by start: the descent index. */
  private readonly spatial: Int32Array[];
  /** Offset of each IDAT segment within the reassembled zlib stream. */
  private segmentStreamStart: number[] = [];
  /** Entry index by entry node, for finding the entry a node sits in. */
  private readonly entryByNode = new Map<number, number>();

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
      // The descent assumes siblings do not partially overlap. A node that
      // straddles its neighbours would win the search for some of their bytes
      // and lose it for others; leave it out of the index instead. It stays in
      // the tree, and selecting it still outlines its whole range.
      const disjoint: number[] = [];
      for (const k of kids) {
        const prev = disjoint[disjoint.length - 1];
        const sameRange = prev !== undefined && starts[k] === starts[prev] && lens[k] === lens[prev];
        if (prev === undefined || sameRange || starts[k] >= starts[prev] + lens[prev]) disjoint.push(k);
      }
      this.spatial[i] = Int32Array.from(disjoint);
    }

    this.problems = [];
    for (let i = 0; i < n; i++) if (kinds[i] >= Kind.Warning) this.problems.push(i);

    // Colour depends only on the node, so work it out once rather than for
    // every byte on every frame. Anything under a GPS node gets the GPS
    // colour: that is the part of a photo people most need to see.
    this.tints = new Array(n);
    const topTint = file.format === "zip" ? zipTint : chunkTint;
    for (let i = 0; i < n; i++) {
      const kind = kinds[i];
      const p = parents[i];
      if (kind === Kind.Error) this.tints[i] = "error";
      else if (kind === Kind.Warning) this.tints[i] = "warning";
      else if (file.labels[i].startsWith("GPS") || (p > 0 && this.tints[p] === "gps")) this.tints[i] = "gps";
      else if (i === 0) this.tints[i] = "anc";
      else if (p === 0) this.tints[i] = topTint(file.labels[i]);
      else this.tints[i] = this.tints[p] === "warning" || this.tints[p] === "error" ? topTint(file.labels[this.top[i]]) : this.tints[p];
    }

    this.indexSegments();
    for (let i = 0; i * ENTRY_STRIDE < file.entries.length; i++) {
      this.entryByNode.set(file.entries[i * ENTRY_STRIDE], i);
    }
  }

  private indexSegments(): void {
    this.segmentStreamStart = [];
    let acc = 0;
    for (let i = 0; i < this.file.segments.length; i += 2) {
      this.segmentStreamStart.push(acc);
      acc += this.file.segments[i + 1];
    }
  }

  /** Points the player's stream at a ZIP entry the worker has just decoded. */
  setStream(segments: Float64Array, trace: number[] | null, compressed: number): void {
    this.file.segments = segments;
    this.file.trace = trace;
    this.file.idatBytes = compressed;
    this.file.streamHeader = 0;
    this.indexSegments();
  }

  /** The ZIP entry node `id` sits in, as an index into the entries, or -1. */
  entryOf(id: number): number {
    if (id < 0) return -1;
    return this.entryByNode.get(this.top[id]) ?? -1;
  }

  /** What node `id` is, or null when nothing explains it. */
  doc(id: number): NodeDoc | null {
    const f = this.file;
    const d = id >= 0 ? f.docIds[id] : -1;
    if (d === undefined || d < 0) return null;
    return { text: f.docTexts[d], cite: f.docCites[d], url: f.docUrls[d], concern: f.docConcerns[d] };
  }

  entry(i: number): ZipEntryInfo {
    const e = this.file.entries.subarray(i * ENTRY_STRIDE, (i + 1) * ENTRY_STRIDE);
    return { node: e[0], method: e[1], flags: e[2], compressed: e[3], uncompressed: e[4], playable: e[5] === 1, openable: e[6] === 1 };
  }

  /** Whether the file has a compressed stream the player can step through. */
  get playable(): boolean {
    return this.file.segments.length > 0 && (this.file.trace?.[0] ?? 0) > 0;
  }

  /** File offset of byte `n` of the reassembled zlib stream, or -1. */
  streamToFile(n: number): number {
    const starts = this.segmentStreamStart;
    let lo = 0;
    let hi = starts.length - 1;
    let seg = -1;
    while (lo <= hi) {
      const mid = (lo + hi) >> 1;
      if (starts[mid] <= n) {
        seg = mid;
        lo = mid + 1;
      } else {
        hi = mid - 1;
      }
    }
    if (seg < 0) return -1;
    const within = n - starts[seg];
    if (within >= this.file.segments[seg * 2 + 1]) return -1;
    return this.file.segments[seg * 2] + within;
  }

  /**
   * File range `[start, end)` holding DEFLATE bits `[bitStart, bitEnd)`. The
   * DEFLATE data begins after the stream's wrapper header, if it has one. A
   * range that crosses from one IDAT chunk into the next also spans the chunk
   * framing between them.
   */
  bitsToFile(bitStart: number, bitEnd: number): [number, number] {
    const h = this.file.streamHeader;
    const first = this.streamToFile(h + Math.floor(bitStart / 8));
    const last = this.streamToFile(h + Math.max(Math.ceil(bitEnd / 8), Math.floor(bitStart / 8) + 1) - 1);
    if (first < 0 || last < 0) return [-1, -1];
    return [first, last + 1];
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

  /** Colour family for a node, from its own kind, GPS, or its top-level chunk. */
  tint(id: number): Tint {
    return this.tints[id] ?? "anc";
  }
}
