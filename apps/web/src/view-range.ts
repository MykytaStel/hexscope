export type ViewRange = readonly [start: number, end: number];
export type ViewRangeListener = (start: number, end: number) => void;

export interface ReportedViewRange {
  range: ViewRange;
  listener: ViewRangeListener;
}

/** Notify view consumers only when the visible byte range actually changes. */
export function reportViewRange(
  previous: ReportedViewRange | null,
  next: ViewRange,
  notify: ViewRangeListener,
): ReportedViewRange {
  if (previous && previous.listener === notify && previous.range[0] === next[0] && previous.range[1] === next[1]) {
    return previous;
  }
  notify(...next);
  return { range: next, listener: notify };
}
