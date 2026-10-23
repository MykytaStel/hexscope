import { describe, expect, it } from "vitest";
import { PARSER_BUILDS } from "./parser-builds";

describe("parser build registry", () => {
  it("has one lazy loader for every parser family", () => {
    expect(Object.keys(PARSER_BUILDS).sort()).toEqual(["audio", "docs", "media", "pdf"]);
  });
});
