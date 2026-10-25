export type ViewRange = readonly [start: number, end: number];

/** Notify view consumers only when the visible byte range actually changes. */
export function reportViewRange(
  previous: ViewRange | null,
  next: ViewRange,
  notify: (start: number, end: number) => void,
): ViewRange {
  if (previous === null || previous[0] !== next[0] || previous[1] !== next[1]) notify(...next);
  return next;
}
