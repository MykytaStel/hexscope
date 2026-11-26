import { describe, expect, it } from "vitest";
import { FileModel, Kind, type ParsedFile } from "./model";

type N = { parent: number; start: number; len: number; kind: number };

function model(nodes: N[], size: number): FileModel {
  const n = nodes.length;
  const file: ParsedFile = {
    starts: Float64Array.from(nodes, x => x.start),
    lens: Float64Array.from(nodes, x => x.len),
    parents: Int32Array.from(nodes, x => x.parent),
    kinds: Uint8Array.from(nodes, x => x.kind),
    labels: Array(n).fill("field"),
    values: Array(n).fill(""),
    ihdr: null, trace: null, idatBytes: 0, segments: new Float64Array(),
    streamHeader: 0, entries: new Float64Array(),
    docIds: new Int32Array(n).fill(-1), docKeys: new Uint32Array(), docConcerns: new Uint8Array(),
    composition: new Float64Array(), entropy: new Float32Array(), entropyWindow: 0,
    missing: new Float64Array(), format: "unknown", coverageStatus: "complete",
    preview: null, rowFilters: new Uint8Array(), dimensions: null, orientation: 1,
    facts: [], location: null, attachments: [], blackouts: [], parseMs: 0,
  };
  return new FileModel(file, new Uint8Array(size), "synthetic.bin");
}

// Independent reproduction of the previous spatial index and descent,
// including stable ties and partial overlap exclusion.
function reference(nodes: N[], size: number) {
  const children: number[][] = nodes.map(() => []);
  for (let i = 1; i < nodes.length; i++) {
    if (nodes[i].len > 0) children[nodes[i].parent].push(i);
  }
  const spatial = children.map(kids => {
    const kept: number[] = [];
    kids.sort((a,b) => nodes[a].start - nodes[b].start || nodes[a].kind - nodes[b].kind);
    for (const k of kids) {
      const prev = nodes[kept[kept.length - 1]];
      const curr = nodes[k];
      if (!prev || (curr.start === prev.start && curr.len === prev.len) || curr.start >= prev.start + prev.len) kept.push(k);
    }
    return kept;
  });
  return (offset: number): number => {
    if (offset < 0 || offset >= size) return -1;
    let node = 0;
    for (;;) {
      let hit: number | undefined;
      for (const k of spatial[node]) {
        if (nodes[k].start > offset) break;
        hit = k;
      }
      if (hit === undefined || offset >= nodes[hit].start + nodes[hit].len) return node;
      node = hit;
    }
  };
}

describe("flat spatial index", () => {
  it("preserves overlapping siblings, same-range problem precedence and zero-length fields", () => {
    const nodes: N[] = [
      { parent: -1, start: 0, len: 100, kind: Kind.Container },
      { parent: 0, start: 10, len: 20, kind: Kind.Field },
      { parent: 0, start: 15, len: 30, kind: Kind.Field },
      { parent: 0, start: 10, len: 20, kind: Kind.Warning },
      { parent: 0, start: 50, len: 40, kind: Kind.Container },
      { parent: 4, start: 60, len: 5, kind: Kind.Field },
      { parent: 4, start: 70, len: 0, kind: Kind.Error },
      { parent: 0, start: 99, len: 1, kind: Kind.Field },
    ];
    const actual = model(nodes, 100);
    const expected = reference(nodes, 100);
    for (let offset = -1; offset <= 100; offset++) expect(actual.nodeAt(offset)).toBe(expected(offset));
    expect(actual.nodeAt(10)).toBe(3);
    expect(actual.nodeAt(60)).toBe(5);
    expect([...actual.children(0)]).toEqual([1,2,3,4,7]);
  });

  it("matches the old index across generated trees with unsorted siblings", () => {
    let seed = 42;
    const rand = () => (seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0);
    for (let run = 0; run < 10; run++) {
      const nodes: N[] = [{ parent: -1, start: 0, len: 512, kind: Kind.Container }];
      for (let i = 1; i < 200; i++) {
        const parent = rand() % i;
        const p = nodes[parent];
        const start = p.start + rand() % (p.len + 1);
        const len = rand() % (p.start + p.len - start + 1);
        nodes.push({ parent, start, len, kind: rand() % 4 });
      }
      const actual = model(nodes, 512);
      const expected = reference(nodes, 512);
      for (let offset = 0; offset < 512; offset++) expect(actual.nodeAt(offset)).toBe(expected(offset));
    }
  });

  it("handles a hundred thousand leaf nodes and exact end boundaries", () => {
    const size = 100_000;
    const nodes: N[] = [{ parent: -1, start: 0, len: size, kind: Kind.Container }];
    for (let i = 0; i < size; i++) nodes.push({ parent: 0, start: i, len: 1, kind: Kind.Field });
    const actual = model(nodes, size);
    expect(actual.count).toBe(size + 1);
    expect(actual.nodeAt(0)).toBe(1);
    expect(actual.nodeAt(50_000)).toBe(50_001);
    expect(actual.nodeAt(size - 1)).toBe(size);
    expect(actual.nodeAt(size)).toBe(-1);
  });
});
