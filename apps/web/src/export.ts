// The whole structure, as JSON, for a person's own tools: every part with
// its offset, length, kind, value and explanation, nested as in the tree,
// with what the file gives away and what is wrong with it. Unlike the
// report for a bug, it keeps the values: it is for the file's owner.
import { FileModel, Kind } from "./model";

const KIND = ["container", "field", "warning", "error"];

interface Part {
  label: string;
  offset: number;
  length: number;
  kind: string;
  value?: string;
  explanation?: string;
  children?: Part[];
}

export function structureJson(m: FileModel): string {
  const n = m.file.parents.length;
  const parts: Part[] = new Array(n);
  for (let id = 0; id < n; id++) {
    const p: Part = { label: m.label(id), offset: m.start(id), length: m.len(id), kind: KIND[m.kind(id)] ?? String(m.kind(id)) };
    const value = m.value(id);
    if (value) p.value = value;
    const doc = m.doc(id);
    if (doc?.text) p.explanation = doc.text;
    parts[id] = p;
  }
  // Parents come before their children: one pass nests them.
  for (let id = 1; id < n; id++) (parts[m.file.parents[id]].children ??= []).push(parts[id]);
  const f = m.file;
  return JSON.stringify(
    {
      file: m.name,
      format: f.format,
      size: m.bytes.length,
      reveals: [
        ...(f.location ? [{ kind: "location", text: `${f.location.latitude}, ${f.location.longitude}` }] : []),
        ...f.facts.map((x) => ({ kind: x.kind, text: x.text })),
      ],
      problems: m.problems.map((id) => ({
        label: m.label(id),
        offset: m.start(id),
        length: m.len(id),
        kind: m.kind(id) === Kind.Error ? "error" : "warning",
      })),
      structure: parts[0],
    },
    null,
    1,
  );
}

/** Hands the JSON to the browser as a download. */
export function saveStructure(m: FileModel): void {
  const url = URL.createObjectURL(new Blob([structureJson(m)], { type: "application/json" }));
  const a = document.createElement("a");
  a.href = url;
  a.download = `${m.name}.structure.json`;
  a.click();
  setTimeout(() => URL.revokeObjectURL(url), 10_000);
}
