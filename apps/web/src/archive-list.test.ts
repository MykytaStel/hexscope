import { expect, it } from "vitest";
import { windowOf } from "./archive-list";

it("draws the rows in view and a few around them, never past either end", () => {
  expect(windowOf(0, 400, 40, 100_000)).toEqual([0, 18]);
  expect(windowOf(40 * 50_000, 400, 40, 100_000)).toEqual([49_992, 50_018]);
  expect(windowOf(40 * 99_990, 400, 40, 100_000)).toEqual([99_982, 100_000]);
  expect(windowOf(0, 400, 40, 3)).toEqual([0, 3]);
  expect(windowOf(0, 0, 40, 10)).toEqual([0, 8]);
});
