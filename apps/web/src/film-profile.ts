import type { ParsedFile } from "./model";

/** Presence of a source ICC container, not validation of its color transform. */
export function profileFromTree(source: Pick<ParsedFile, "format" | "labels" | "kinds">): boolean | null {
  if (!["jpeg", "png", "webp", "tiff"].includes(source.format)) return null;
  const isProfile = (label: string) => {
    if (source.format === "jpeg") return /^APP\d+ · ICC$/.test(label);
    if (source.format === "png") return label === "iCCP";
    if (source.format === "webp") return label === "ICCP";
    return label === "ICC profile";
  };
  if (source.labels.some((label, i) => source.kinds[i] === 0 && isProfile(label))) return true;
  return source.kinds.includes(3) ? null : false;
}
