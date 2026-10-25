import { describe, expect, it, vi } from "vitest";
import { reportViewRange, type ViewRange } from "./view-range";

describe("visible byte range notifications", () => {
  it("reports the initial range and only reports again when it changes", () => {
    const notify = vi.fn();
    let previous: ViewRange | null = null;

    previous = reportViewRange(previous, [0, 16], notify);
    previous = reportViewRange(previous, [0, 16], notify);
    previous = reportViewRange(previous, [16, 32], notify);
    previous = reportViewRange(previous, [16, 40], notify);

    expect(notify.mock.calls).toEqual([[0, 16], [16, 32], [16, 40]]);
  });

  it("reports the same initial range after a new model resets the view", () => {
    const notify = vi.fn();
    let previous: ViewRange | null = null;

    previous = reportViewRange(previous, [0, 16], notify);
    previous = reportViewRange(previous, [0, 16], notify);
    previous = reportViewRange(null, [0, 16], notify);

    expect(notify.mock.calls).toEqual([[0, 16], [0, 16]]);
  });
});
