// A name that says another format than the bytes is told by the core (for
// the command line) and by the page (verdict.ts) from tables of their own:
// this holds the two together, so a new extension is added to both.
import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { EXTENSIONS, ZIP_KINDS } from "./verdict";

const rust = readFileSync(new URL("../../../crates/hexscope-core/src/naming.rs", import.meta.url), "utf8");
const strings = (s: string) => [...s.matchAll(/"([^"]*)"/g)].map((m) => m[1]);

describe("the names a format goes by", () => {
  it("are the same on the page as in the core", () => {
    const table = rust.slice(rust.indexOf("const EXTENSIONS"), rust.indexOf("];", rust.indexOf("const EXTENSIONS")));
    const core: Record<string, string[]> = {};
    for (const row of table.matchAll(/\(\s*"(\w+)",\s*&\[([^\]]*)\]/g)) core[row[1]] = strings(row[2]);
    expect(Object.keys(core).length).toBeGreaterThan(10);
    expect(EXTENSIONS).toEqual(core);
  });

  it("are the same for the documents a ZIP can be", () => {
    const table = rust.slice(rust.indexOf("const ZIP_KINDS"), rust.indexOf("];", rust.indexOf("const ZIP_KINDS")));
    const core = [...table.matchAll(/\(\s*"([^"]+)",\s*"([^"]+)",\s*&\[([^\]]*)\]\)/g)].map((r) => [r[1], r[2], strings(r[3])]);
    expect(core.length).toBeGreaterThan(5);
    expect(ZIP_KINDS).toEqual(core);
  });
});
