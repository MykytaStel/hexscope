import type { ParsedFile } from "./model";

/** Presence of a source ICC container, not validation of its color transform. */
export function profileFromTree(source: Pick<ParsedFile, "format" | "labels" | "kinds">): boolean | null {
  if (!["jpeg", "png", "webp"].includes(source.format)) return null;
  if (source.labels.some((label, i) => source.kinds[i] === 0 &&
    (source.format === "jpeg" ? /^APP\d+ · ICC$/.test(label) : label === (source.format === "png" ? "iCCP" : "ICCP")))) return true;
  return source.kinds.includes(3) ? null : false;
}
