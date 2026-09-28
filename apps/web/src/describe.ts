// What the page needs out of a parsed document, as plain arrays and
// objects: the one place that knows how the WebAssembly side packs them.
// No DOM and no worker globals, so it runs in tests as it runs in the
// worker.
import type { Parsed } from "./wasm/hexscope_wasm.js";
import type { Blackout, PageArea, ParsedFile } from "./model";

/** Joins the strings the WebAssembly side packs into one. */
export const SEPARATOR = "\u001f";

/** Reads everything the page needs out of a parsed document. */
export function describe(parsed: Parsed): ParsedFile {
  const result: ParsedFile = {
    starts: parsed.starts,
    lens: parsed.lens,
    parents: parsed.parents,
    kinds: parsed.kinds,
    labels: parsed.labels.split(SEPARATOR),
    values: parsed.values.split(SEPARATOR),
    ihdr: null,
    trace: null,
    idatBytes: parsed.idatBytes,
    segments: parsed.segments,
    streamHeader: parsed.streamHeader,
    entries: parsed.entries,
    docIds: parsed.docIds,
    docTexts: parsed.docTexts.split(SEPARATOR),
    docCites: parsed.docCites.split(SEPARATOR),
    docUrls: parsed.docUrls.split(SEPARATOR),
    docConcerns: parsed.docConcerns,
    composition: parsed.composition,
    entropy: new Float32Array(0),
    entropyWindow: 0,
    format: parsed.format as ParsedFile["format"],
    dimensions: null,
    orientation: parsed.orientation,
    facts: [],
    location: null,
    blackouts: [],
    attachments: parsed.attachments ? parsed.attachments.split(SEPARATOR) : [],
    parseMs: 0,
    preview: null,
    rowFilters: parsed.rowFilters,
  };
  const size = Array.from(parsed.previewSize);
  if (size.length === 2) {
    result.preview = { width: size[0], height: size[1], pixels: parsed.previewPixels };
  }
  const dims = Array.from(parsed.dimensions);
  result.dimensions = dims.length === 2 ? [dims[0], dims[1]] : null;
  const facts = parsed.facts ? parsed.facts.split(SEPARATOR) : [];
  for (let i = 0; i + 2 < facts.length; i += 3) {
    result.facts.push({ kind: facts[i], text: facts[i + 1], node: Number(facts[i + 2]) });
  }
  const loc = Array.from(parsed.location);
  if (loc.length === 4) {
    result.location = {
      latitude: loc[0],
      longitude: loc[1],
      altitude: Number.isNaN(loc[2]) ? null : loc[2],
      node: loc[3],
    };
  }
  result.blackouts = blackouts(parsed.blackouts, parsed.blackoutTexts.split(SEPARATOR));
  const ihdr = Array.from(parsed.ihdr);
  const trace = Array.from(parsed.trace);
  result.ihdr = ihdr.length ? ihdr : null;
  result.trace = trace.length ? trace : null;
  return result;
}

/** The layout `Parsed.blackouts` documents, as objects. */
function blackouts(n: Float64Array, texts: string[]): Blackout[] {
  const out: Blackout[] = [];
  const area = (i: number): PageArea => [n[i], n[i + 1], n[i + 2], n[i + 3]];
  let i = 0;
  let t = 0;
  while (i + 8 <= n.length) {
    const b: Blackout = { page: n[i], media: area(i + 1), boxes: [], texts: [], context: [] };
    const [boxes, pieces, beside] = [n[i + 5], n[i + 6], n[i + 7]];
    i += 8;
    for (let k = 0; k < boxes && i + 4 <= n.length; k++, i += 4) b.boxes.push(area(i));
    for (let k = 0; k < pieces && i + 4 <= n.length; k++, i += 4) b.texts.push({ area: area(i), text: texts[t++] ?? "" });
    for (let k = 0; k < beside && i + 4 <= n.length; k++, i += 4) b.context.push({ area: area(i), text: texts[t++] ?? "" });
    out.push(b);
  }
  return out;
}

