import type { ParsedFile } from "./model";

/** Observations only: absence in an incomplete read never proves removal. */
export function semanticDiff(a: Pick<ParsedFile, "facts" | "coverageStatus">, b: Pick<ParsedFile, "facts" | "coverageStatus">): { changed: string[]; onlyA: string[]; onlyB: string[]; complete: boolean } {
  const group = (file: typeof a) => {
    const out = new Map<string, string[]>();
    for (const fact of file.facts) { const values = out.get(fact.kind) ?? []; values.push(fact.text); out.set(fact.kind, values); }
    return out;
  };
  const left = group(a); const right = group(b);
  return {
    changed: [...left.keys()].filter((kind) => right.has(kind) && JSON.stringify([...left.get(kind)!].sort()) !== JSON.stringify([...right.get(kind)!].sort())),
    onlyA: [...left.keys()].filter((kind) => !right.has(kind)),
    onlyB: [...right.keys()].filter((kind) => !left.has(kind)),
    complete: a.coverageStatus === "complete" && b.coverageStatus === "complete",
  };
}
