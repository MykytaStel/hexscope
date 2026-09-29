// QR codes in a file, as facts about it. The pictures are decoded and
// searched in the worker; what each code says is read in qr/meaning.ts.

import type { FileModel, PhotoFact } from "./model";
import { meaning } from "./qr/meaning";
import { call } from "./rpc";

const PICTURES = ["jpeg", "png", "heif", "webp", "gif"];
const IMAGE_NAME = /\.(png|jpe?g|gif|webp|bmp|heic|avif)$/i;
/** Pictures inside a document or message looked through, at most. */
const MAX_INSIDE = 30;

type Look = { picture: Blob } | { pdf: Uint8Array } | { entries: { index: number; name: string; node: number }[] };

/** What in the file is looked through for codes: the picture itself, a PDF's pictures, or pictures inside it. */
function request(m: FileModel): Look | null {
  const f = m.file;
  if (PICTURES.includes(f.format)) return { picture: m.source ?? new Blob([m.bytes as BlobPart]) };
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
 * Looks for QR codes in `m` and adds what they say to its facts. Resolves
 * with whether it found any. A file is looked through once.
 */
const looked = new WeakSet<FileModel>();
export async function addCodeFacts(m: FileModel): Promise<boolean> {
  if (looked.has(m)) return false;
  looked.add(m);
  const what = request(m);
  if (!what) return false;
  const r = await call({ type: "codes", ...what });
  if (r.type !== "codes" || r.codes.length === 0) return false;
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
  return true;
}
