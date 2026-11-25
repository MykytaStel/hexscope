// Measure generated WASM directly in Node, one isolated process per fixture
// and repetition. Browser/Node numbers are separate evidence, never pooled.
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { readFileSync, mkdirSync, writeFileSync } from "node:fs";
import { fileURLToPath, pathToFileURL } from "node:url";
import { resolve, dirname } from "node:path";
import { cpus, arch, platform, totalmem } from "node:os";
import { performance } from "node:perf_hooks";

const root = fileURLToPath(new URL("../", import.meta.url));
const fixtures = [
  ["jpeg","media","photo.jpg"], ["png","media","sample.png"], ["mov","media","video.mov"],
  ["pdf","pdf","report.pdf"], ["zip","docs","deflate-demo.zip"], ["office","docs","report.docx"], ["audio","audio","sample.mp3"],
];

if (process.argv[2] === "--child") {
  const item = fixtures.find(f => f[0] === process.argv[3]);
  if (!item) throw new Error("Unknown fixture");
  const [,family,name] = item;
  const dir = resolve(root,"apps/web/src/wasm-"+family);
  const bytes = new Uint8Array(readFileSync(resolve(root,"apps/web/public/samples",name)));
  const module = await import(pathToFileURL(resolve(dir,"hexscope_"+family+".js")).href);
  const wasm = readFileSync(resolve(dir,"hexscope_"+family+"_bg.wasm"));
  const initStart = performance.now();
  const exports = module.initSync({ module: wasm });
  const initMs = performance.now()-initStart;
  const memory = () => exports.memory.buffer.byteLength;
  const memoryAfterInit = memory();
  const parse = () => {
    const start = performance.now();
    const parsed = module.parse(bytes);
    try { return { ms: performance.now()-start, memory: memory() }; }
    finally { parsed.free(); }
  };
  const cold = parse(), warm = parse();
  const cleanStart = performance.now();
  const copy = module.cleanCopy(bytes,false);
  let copyBytes, cleanMs, memoryAfterClean;
  try {
    if (copy.error) throw new Error("Fixture clean failed: "+copy.error);
    copyBytes = copy.bytes.length;
    cleanMs = performance.now()-cleanStart;
    memoryAfterClean = memory();
  } finally { copy.free(); }
  console.log(JSON.stringify({ initMs, coldParseMs:cold.ms, warmParseMs:warm.ms, cleanMs, copyBytes, memoryAfterInit, memoryAfterCold:cold.memory, memoryAfterWarm:warm.memory, memoryAfterClean, memoryAfterFree:memory() }));
} else {
  const repeats = Number(process.env.HEXSCOPE_PERF_REPEATS ?? 5);
  if (!Number.isInteger(repeats) || repeats < 1 || repeats > 20) throw new Error("Repeat count must be 1–20");
  const cases = fixtures.map(([id,,name]) => {
    const bytes = readFileSync(resolve(root,"apps/web/public/samples",name));
    const metrics = Array.from({length:repeats}, () => JSON.parse(execFileSync(process.execPath,[fileURLToPath(import.meta.url),"--child",id],{encoding:"utf8",timeout:60000})));
    return { id,inputBytes:bytes.length,inputSha256:createHash("sha256").update(bytes).digest("hex"),metrics };
  });
  const report = { schema:"hexscope.performance", version:1, kind:"wasm", revision:execFileSync("git",["rev-parse","HEAD"],{cwd:root,encoding:"utf8"}).trim(),
    environment:{ platform:platform(),arch:arch(),cpu:cpus()[0]?.model,cores:cpus().length,ramBytes:totalmem(),node:process.version,repeats },cases };
  const output = resolve(root,process.env.HEXSCOPE_PERF_OUTPUT ?? ".perf/wasm.json");
  mkdirSync(dirname(output),{recursive:true});
  writeFileSync(output,JSON.stringify(report,null,2)+"\n");
  console.log("Performance evidence: "+output);
}
