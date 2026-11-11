import { expect, it } from "vitest";
import { readProfile, evidencePath, differingFields } from "./preflight-model";

it("imports a value-free profile and rejects malformed rules", () => {
  const profile = readProfile(JSON.stringify({ schema: "hexscope.policy", version: 1, require_complete: true, blocked_kinds: ["location", "serial", "serial"] }));
  expect(profile.blocked_kinds).toEqual(["location", "serial"]);
  expect(() => readProfile(JSON.stringify({ ...profile, blocked_kinds: ["private name"] }))).toThrow();
});

it("follows only real parent edges and stops on cycles", () => {
  expect(evidencePath({ parents: new Int32Array([-1, 0, 1]), labels: [] }, 2)).toEqual([0, 1, 2]);
  expect(evidencePath({ parents: new Int32Array([1, 0]), labels: [] }, 1)).toHaveLength(2);
});

it("lists conflicting stored fields without guessing their origin", () => {
  expect(differingFields({ labels: ["Author", "Author", "Author"], values: ["A", "A", "B"], kinds: new Uint8Array([1, 1, 1]) })).toEqual([{ label: "Author", nodes: [0, 1, 2] }]);
});
