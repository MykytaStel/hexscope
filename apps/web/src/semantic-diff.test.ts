import { expect, it } from "vitest";
import { semanticDiff } from "./semantic-diff";

it("detects changed values even when both files have the same finding kinds", () => {
  const a = { facts: [{ kind: "owner", text: "A", node: 0 }], coverageStatus: "complete" as const };
  const b = { facts: [{ kind: "owner", text: "B", node: 0 }], coverageStatus: "partial" as const };
  expect(semanticDiff(a, b)).toEqual({ changed: ["owner"], onlyA: [], onlyB: [], complete: false });
});
