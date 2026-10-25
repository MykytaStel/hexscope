import { describe, expect, it, vi } from "vitest";
import { reportViewRange, type ReportedViewRange } from "./view-range";

describe("visible byte range notifications", () => {
  it("reports the initial range and only reports again when it changes", () => {
    const notify = vi.fn();
    let previous: ReportedViewRange | null = null;

    previous = reportViewRange(previous, [0, 16], notify);
    previous = reportViewRange(previous, [0, 16], notify);
    previous = reportViewRange(previous, [16, 32], notify);
    previous = reportViewRange(previous, [16, 40], notify);

    expect(notify.mock.calls).toEqual([[0, 16], [16, 32], [16, 40]]);
  });

  it("reports the same initial range after a new model resets the view", () => {
    const notify = vi.fn();
    let previous: ReportedViewRange | null = null;

    previous = reportViewRange(previous, [0, 16], notify);
    previous = reportViewRange(previous, [0, 16], notify);
    previous = reportViewRange(null, [0, 16], notify);

    expect(notify.mock.calls).toEqual([[0, 16], [0, 16]]);
  });

  it("reports the current range to a replacement view consumer", () => {
    const first = vi.fn();
    const replacement = vi.fn();
    let previous: ReportedViewRange | null = null;

    previous = reportViewRange(previous, [0, 16], first);
    reportViewRange(previous, [0, 16], replacement);

    expect(first).toHaveBeenCalledOnce();
    expect(replacement).toHaveBeenCalledWith(0, 16);
  });
});
