import { expect, it } from "vitest";
import { profileFromTree } from "./film-profile";

it("recognizes source ICC containers in all three browser photo formats", () => {
  for (const [format, label] of [["jpeg", "APP2 · ICC"], ["png", "iCCP"], ["webp", "ICCP"]] as const)
    expect(profileFromTree({ format, labels: [label], kinds: new Uint8Array([0]) })).toBe(true);
});
it("keeps profile absence unknown for incomplete and unsupported sources", () => {
  expect(profileFromTree({ format: "jpeg", labels: ["truncated"], kinds: new Uint8Array([3]) })).toBeNull();
  expect(profileFromTree({ format: "heif", labels: [], kinds: new Uint8Array() })).toBeNull();
  expect(profileFromTree({ format: "jpeg", labels: ["APP2 · ICC"], kinds: new Uint8Array([1]) })).toBe(false);
  expect(profileFromTree({ format: "png", labels: ["IHDR", "IEND"], kinds: new Uint8Array([0, 0]) })).toBe(false);
});
