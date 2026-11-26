import { test } from "node:test";
import assert from "node:assert/strict";
import { wasmFamilies, familyNames, buildArguments, artifactPattern } from "./wasm-build-manifest.mjs";

test("production features and compressed budgets are the existing five contracts", () => {
  assert.deepEqual(Object.entries(wasmFamilies).map(([family, spec]) => [family, spec.features, spec.budget]), [
    ["media", "hdr,svg,strings", 178000], ["pdf", "pdf", 229000], ["docs", "office,mail,module", 240000],
    ["audio", "audio", 168000], ["raster", "web", 216000],
  ]);
  for (const family of Object.keys(wasmFamilies)) {
    const args = buildArguments(family);
    assert.ok(args.includes("--locked"));
    assert.ok(args.includes("--no-default-features"));
    assert.equal(args[args.indexOf("--features") + 1], wasmFamilies[family].features);
    assert.equal(args[args.indexOf("-p") + 1], family === "raster" ? "hexscope-raster" : "hexscope-wasm");
    assert.ok(artifactPattern(family).test("hexscope_" + family + "_bg-abcd.wasm"));
    assert.ok(artifactPattern(family, true).test("hexscope_" + family + "_bg.wasm"));
    assert.ok(!artifactPattern(family, true).test("hexscope_" + family + "_bg-abcd.wasm"));
    for (const other of Object.keys(wasmFamilies).filter(f => f !== family)) {
      assert.ok(!artifactPattern(other).test("hexscope_" + family + "_bg-abcd.wasm"));
    }
  }
});
test("rejects empty, duplicate and prototype-named requests", () => {
  for (const names of [[], ["pdf", "pdf"], ["unknown"], ["toString"]]) assert.throws(() => familyNames(names));
  assert.deepEqual(familyNames(["pdf", "docs"]), ["pdf", "docs"]);
});
