import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { readdirSync, readFileSync, mkdirSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { resolve, dirname, basename } from "node:path";
import { profileWasm } from "./wasm-profile-core.mjs";
import { wasmFamilies, artifactPattern } from "./wasm-build-manifest.mjs";

const root=fileURLToPath(new URL("../",import.meta.url));
const args=process.argv.slice(2);
const option=(name,fallback)=>args.find(a=>a.startsWith("--"+name+"="))?.slice(name.length+3)??fallback;
if(args.some(a=>!["--json"].includes(a)&&!["dir","output","top","kind"].some(k=>a.startsWith("--"+k+"=")))) throw new Error("Usage: pnpm perf:profile [--dir=...] [--output=...] [--top=30] [--kind=production|diagnostic] [--json]");
const dir=resolve(root,option("dir","apps/web/dist/assets"));
const kind=option("kind","production");
if(!["production","diagnostic"].includes(kind)) throw new Error("Unknown profile kind");
const top=Number(option("top","30"));
const known=Object.keys(wasmFamilies);
const familyFor=file=>known.find(f=>artifactPattern(f,kind==="diagnostic").test(file));
const files=readdirSync(dir).filter(f=>familyFor(f)).sort();
if(!files.length) throw new Error("No Hexscope WASM artifacts found; run pnpm build first");
const families=new Set();
const modules=files.map(file=>{
  const family=familyFor(file);
  if(families.has(family)) throw new Error("Duplicate WASM artifact for "+family);
  families.add(family);
  const path=resolve(dir,file), bytes=readFileSync(path);
  if(!WebAssembly.validate(bytes)) throw new Error("Invalid or unsupported WASM: "+file);
  const gzipBytes=execFileSync("gzip",["-9c",path],{maxBuffer:16*1024*1024}).length;
  return {family,file:basename(file),sha256:createHash("sha256").update(bytes).digest("hex"),gzipBytes,...profileWasm(bytes,{top})};
});
if(kind==="production"&&families.size!==known.length) throw new Error("Expected all five production families");
const report={schema:"hexscope.wasm-profile",version:1,kind,revision:execFileSync("git",["rev-parse","HEAD"],{cwd:root,encoding:"utf8"}).trim(),node:process.version,modules};
const output=resolve(root,option("output",".perf/wasm-profile-"+kind+".json"));
mkdirSync(dirname(output),{recursive:true});
writeFileSync(output,JSON.stringify(report,null,2)+"\n");
if(args.includes("--json")) console.log(JSON.stringify(report,null,2));
else {
  console.table(modules.map(m=>({family:m.family,rawBytes:m.rawBytes,gzipBytes:m.gzipBytes,codeBytes:m.sections.find(s=>s.id===10)?.payloadBytes??0,dataBytes:m.sections.find(s=>s.id===11)?.payloadBytes??0,functions:m.definedFunctions,named:m.namedFunctions})));
  for(const m of modules) {
    console.log("\n"+m.family+" — largest function bodies (raw bytes)");
    console.table(m.largestFunctions.slice(0,10).map(b=>({index:b.index,bytes:b.bodyBytes,name:b.name??"(anonymous)",group:b.group})));
  }
  console.log("Evidence: "+output);
}
