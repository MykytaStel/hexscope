import { expect, it } from "vitest";
import type { FileModel, ParsedFile } from "./model";
import { heifGraph, sequenceTimings } from "./heif-map";

function model(labels: string[], values: string[], parents: number[]): FileModel {
  const file = { format: "heif", labels, values, parents: Int32Array.from(parents) } as ParsedFile;
  const children = new Map<number, number[]>();
  parents.forEach((parent, id) => {
    if (id === 0) return;
    const list = children.get(parent) ?? [];
    list.push(id);
    children.set(parent, list);
  });
  return {
    file,
    count: labels.length,
    label: (id: number) => labels[id],
    value: (id: number) => values[id],
    len: () => 0,
    children: (id: number) => Int32Array.from(children.get(id) ?? []),
  } as unknown as FileModel;
}

it("shows primary, constructed, tile and auxiliary relationships without claiming assembled bytes", () => {
  const m = model(
    [
      "HEIF", "majorBrand", "pitm", "itemId", "infe", "itemId", "itemType", "infe", "itemId", "itemType",
      "infe", "itemId", "itemType", "item 1 location", "constructionMethod", "construction method 2: data held by other items, not read",
      "item 2 location", "constructionMethod", "item 2 · hvc1", "item 3 · hvc1", "dimg", "auxl",
    ],
    [
      "", "heic", "", "1", "", "1", "grid", "", "2", "hvc1", "", "3", "hvc1", "", "2", "", "", "0", "tile 1 of 2", "tile 2 of 2", "item 1 → 2, 3", "item 3 → 1",
    ],
    [0, 0, 0, 2, 0, 4, 4, 0, 7, 7, 0, 10, 10, 0, 13, 13, 0, 16, 0, 0, 0, 0],
  );

  const graph = heifGraph(m)!;
  expect(graph.sequence).toBe(false);
  expect(graph.refs).toEqual([
    { type: "dimg", from: 1, to: [2, 3] },
    { type: "auxl", from: 3, to: [1] },
  ]);
  expect(graph.items.map(({ id, type, primary, role, constructionMethod, node }) => ({ id, type, primary, role, constructionMethod, node }))).toEqual([
    { id: 1, type: "grid", primary: true, role: "primary image", constructionMethod: 2, node: -1 },
    { id: 2, type: "hvc1", primary: false, role: "tile", constructionMethod: 0, node: 18 },
    { id: 3, type: "hvc1", primary: false, role: "tile", constructionMethod: null, node: 19 },
  ]);
});

it("recognizes an AVIS compatible brand even when AVIF is the major brand", () => {
  const m = model(["HEIF", "majorBrand", "compatibleBrands"], ["", "mif1", "avif, avis, miaf"], [0, 0, 0]);
  expect(heifGraph(m)?.sequence).toBe(true);
});

it("extracts a bounded AVIS timeline from pict sample timing", () => {
  const m = model(
    [
      "HEIF", "majorBrand", "moov", "trak", "mdia", "hdlr", "handlerType", "mdhd", "timescale",
      "minf", "stbl", "stts", "entryCount", "sampleCount", "sampleDelta", "sampleCount", "sampleDelta",
    ],
    [
      "", "avis", "", "AV1 image sequence · pict · 3 samples · 400 ticks at 1000 ticks/s", "", "", "pict", "", "1000",
      "", "", "", "2", "2", "100", "1", "200",
    ],
    [0, 0, 0, 2, 3, 4, 5, 4, 7, 4, 9, 10, 11, 11, 11, 11, 11],
  );

  expect(sequenceTimings(m)).toEqual({
    trackCount: 1,
    tracks: [{
      handler: "pict",
      timescale: 1000,
      sampleCount: 3,
      seconds: 0.4,
      runs: [{ samples: 2, delta: 100 }, { samples: 1, delta: 200 }],
    }],
  });
});
