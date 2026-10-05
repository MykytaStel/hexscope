// QR codes in a file, as facts about it. The pictures are decoded and
// searched in the worker; what each code says is read in qr/meaning.ts.

import type { FileModel, ParsedFile, PhotoFact } from "./model";
import { meaning } from "./qr/meaning";
import { call } from "./rpc";

const PICTURES = ["jpeg", "png", "heif", "webp", "gif"];
const IMAGE_NAME = /\.(png|jpe?g|gif|webp|bmp|heic|avif)$/i;
/** Pictures inside a document or message looked through, at most. */
const MAX_INSIDE = 30;

export async function placeFilmScanCard(target: ParentNode, file: ParsedFile): Promise<void> {
  if (!file.filmScan) return;
  const reveals = target.querySelector(".reveals");
  if (!reveals) return;
  const { filmScanCard } = await import("./filmscan-card");
  reveals.after(filmScanCard(file.filmScan));
}

type Look = { picture: Blob; inspectFilmScan?: boolean } | { pdf: Uint8Array } | { entries: { index: number; name: string; node: number }[] };

/** What in the file is looked through for codes: the picture itself, a PDF's pictures, or pictures inside it. */
function request(m: FileModel, inspectFilmScan: boolean): Look | null {
  const f = m.file;
  if (PICTURES.includes(f.format)) {
    return {
      picture: m.source ?? new Blob([m.bytes as BlobPart]),
      ...(f.format === "jpeg" && inspectFilmScan ? { inspectFilmScan: true } : {}),
    };
  }
  if (f.format === "pdf") return { pdf: m.bytes.slice() };
  const inside: { index: number; name: string; node: number }[] = [];
  if (f.format === "eml" || f.format === "msg") {
    f.attachments.forEach((name, index) => IMAGE_NAME.test(name) && inside.push({ index, name, node: 0 }));
  } else if (f.format === "zip") {
    for (let i = 0; i * 7 < f.entries.length; i++) {
      const e = m.entry(i);
      const name = m.label(e.node);
      if (IMAGE_NAME.test(name) && e.openable) inside.push({ index: i, name, node: e.node });
    }
  }
  return inside.length ? { entries: inside.slice(0, MAX_INSIDE) } : null;
}

/**
 * Looks for QR codes and optional JPEG scan clues. Resolves with whether the
 * file gained visible analysis. A file is looked through once.
 */
const looked = new WeakSet<FileModel>();
type CodeFactUpdate = false | "film-scan" | "codes";
export async function addCodeFacts(m: FileModel, inspectFilmScan = false): Promise<CodeFactUpdate> {
  if (looked.has(m)) return false;
  looked.add(m);
  const what = request(m, inspectFilmScan);
  if (!what) return false;
  const r = await call({ type: "codes", ...what });
  const filmScanRequested = "picture" in what && what.inspectFilmScan === true;
  if (r.type !== "codes") {
    if (filmScanRequested) {
      m.file.filmScan = {
        availability: "unavailable",
        perforationEdges: [],
        perforationRepeats: 0,
        frameEdges: [],
        evidenceStrength: "none",
      };
      return "film-scan";
    }
    return false;
  }
  const scanCluesAdded = r.filmScan !== undefined;
  if (r.filmScan) m.file.filmScan = r.filmScan;
  if (r.codes.length === 0) return scanCluesAdded ? "film-scan" : false;
  const nodes = "entries" in what ? new Map(what.entries.map((e) => [e.name, e.node])) : new Map<string, number>();
  const facts: PhotoFact[] = r.codes.map((c) => {
    const said = meaning(c.text);
    return { kind: said.kind, text: c.where ? `${said.text} (in ${c.where})` : said.text, node: nodes.get(c.where) ?? 0 };
  });
  m.file.facts.push(...facts);
  if ("picture" in what) {
    // A little of the quiet zone too, so no module shows at the edge.
    const pad = (b: number[]) => [b[0] - 0.01, b[1] - 0.01, b[2] + 0.01, b[3] + 0.01].map((v) => Math.min(1, Math.max(0, v)));
    for (const c of r.codes) m.codeBoxes.push(pad(c.box) as [number, number, number, number]);
  }
  return "codes";
}
