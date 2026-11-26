// Exercise the real generated browser modules, not mocks or a native
// all-features build. Run after pnpm wasm / pnpm build.
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const parserFamilies = ["media", "pdf", "docs", "audio"];
const parserExports = ["parse", "parseOwned", "parseOwnedExternal", "parseMovie", "isMovie", "entropy", "cleanCopy"];
const rasterExports = ["photoMosaic", "renderRgba8", "processTiff", "encodeTiff8"];

async function load(family) {
  const stem = new URL(`../apps/web/src/wasm-${family}/hexscope_${family}`, import.meta.url);
  const module = await import(stem.href + ".js");
  module.initSync({ module: readFileSync(new URL(stem.href + "_bg.wasm")) });
  return module;
}

function sample(name) {
  return new Uint8Array(readFileSync(new URL("../apps/web/public/samples/" + name, import.meta.url)));
}

// These samples deliberately isolate one optional family. Media parsing is
// shared in every parser build (nested photos, PDF image inspection, etc.).
const cases = [
  ["photo.jpg", "jpeg", parserFamilies],
  ["sample.png", "png", parserFamilies],
  ["video.mov", "video", parserFamilies],
  ["drawing.svg", "svg", ["media"]],
  ["drawing.svgz", "svg", ["media"]],
  ["report.pdf", "pdf", ["pdf"]],
  ["report.docx", "zip", ["docs"]],
  ["deflate-demo.zip", "zip", ["docs"]],
  ["message.eml", "eml", ["docs"]],
  ["hello.wasm", "wasm", ["docs"]],
  ["sample.wav", "wav", ["audio"]],
  ["sample.mp3", "mp3", ["audio"]],
  ["sample.flac", "flac", ["audio"]],
  ["opus.ogg", "ogg", ["audio"]],
];

for (const family of parserFamilies) {
  test(family + " implements shared ABI and only its expected optional formats", async () => {
    const module = await load(family);
    for (const name of parserExports) assert.equal(typeof module[name], "function", family + ": " + name);
    for (const name of rasterExports) assert.equal(typeof module[name], "undefined", family + ": " + name);
    for (const [name, expected, enabled] of cases) {
      const parsed = module.parseOwnedExternal(sample(name));
      try {
        const label = family + ": " + name;
        assert.equal(parsed.format, enabled.includes(family) ? expected : "unknown", label);
        const nodes = parsed.starts.length;
        assert.ok(nodes > 0, label);
        assert.equal(parsed.lens.length, nodes, label);
        assert.equal(parsed.parents.length, nodes, label);
        assert.equal(parsed.kinds.length, nodes, label);
        assert.equal(typeof parsed.releasePresentation, "function", label);
        parsed.releasePresentation();
        assert.equal(parsed.format, enabled.includes(family) ? expected : "unknown", label);
      } finally { parsed.free(); }
    }
  });
}

test("raster module has its own ABI without the parser bridge", async () => {
  const module = await load("raster");
  for (const name of rasterExports) assert.equal(typeof module[name], "function", name);
  for (const name of parserExports) assert.equal(typeof module[name], "undefined", name);
});

test("audio module recognizes all four WAVE containers even for incomplete inputs", async () => {
  const module = await load("audio");
  for (const [signature, format] of [["RIFF","wav"], ["RIFX","rifx"], ["RF64","rf64"], ["BW64","bw64"]]) {
    const bytes = new Uint8Array(12);
    bytes.set(new TextEncoder().encode(signature));
    bytes.set(new TextEncoder().encode("WAVE"), 8);
    const parsed = module.parse(bytes);
    try { assert.equal(parsed.format, format); }
    finally { parsed.free(); }
  }
});
