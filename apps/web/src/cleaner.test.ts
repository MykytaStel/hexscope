import { describe, expect, it } from "vitest";
import { cleanCopyNote } from "./cleaner";

describe("clean-copy note selection", () => {
  it("explains that an Ultra HDR copy keeps its gain map", () => {
    expect(
      cleanCopyNote({
        format: "jpeg",
        labels: ["JPEG", "picture listed in the MPF index"],
        values: ["", "HDR gain map"],
      }),
    ).toMatch(/HDR gain map/);
  });

  it("uses the regular picture note for a JPEG without an HDR gain map", () => {
    expect(cleanCopyNote({ format: "jpeg", labels: ["JPEG"], values: [""] })).toMatch(/camera data/);
  });

  it("warns when the JPEG has an unsupported indexed picture", () => {
    expect(
      cleanCopyNote({
        format: "jpeg",
        labels: ["JPEG", "picture listed in the MPF index"],
        values: ["", "depth map"],
      }),
    ).toMatch(/will not make a clean copy/);
  });
});
