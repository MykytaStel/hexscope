import type { FileModel } from "./model";
import { CATEGORIES as CORE_CATEGORIES } from "./finding-kinds";

/** Each kind of fact as a category, most telling first. */
export const CATEGORIES = CORE_CATEGORIES;

/** How many distinct known categories these fact kinds fall in. */
export function categoryCount(kinds: Iterable<string>): number {
  const set = new Set(kinds);
  return CATEGORIES.filter(([kind]) => set.has(kind)).length;
}

/** The categories a file gives away, in core-defined priority order. */
export function categories(model: FileModel): string[] {
  const kinds = new Set([
    ...(model.file.location ? ["location"] : []),
    ...model.file.facts.map((fact) => fact.kind),
  ]);
  return CATEGORIES.filter(([kind]) => kinds.has(kind)).map(([, words]) => words);
}
