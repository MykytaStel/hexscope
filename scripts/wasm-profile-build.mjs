// Isolated diagnostic builds. Production dist and generated browser glue are
// never overwritten. Keep wasm-bindgen names if the compiler emits them.
import { execFileSync } from "node:child_process";
import { mkdirSync, rmSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root=fileURLToPath(new URL("../",import.meta.url));
const families={media:"hdr,svg,strings",pdf:"pdf",docs:"office,mail,module",audio:"audio",raster:"web"};
const requested=process.argv.slice(2);
if(!requested.length||requested.some(f=>!Object.hasOwn(families,f))||new Set(requested).size!==requested.length) throw new Error("Usage: pnpm perf:profile:named pdf [docs|media|audio|raster]");
const target=resolve(root,".perf/named-target");
const dir=resolve(root,".perf/named-modules");
rmSync(dir,{recursive:true,force:true});
mkdirSync(dir,{recursive:true});
for(const family of requested) {
  const crate=family==="raster"?"hexscope-raster":"hexscope-wasm";
  const artifact=crate.replaceAll("-","_");
  const env={...process.env,CARGO_TARGET_DIR:target,RUSTFLAGS:`--remap-path-prefix=${process.env.HOME??root}=~`};
  execFileSync("cargo",["build","--locked","-p",crate,"--target","wasm32-unknown-unknown","--profile","wasm","--no-default-features","--features",families[family]],{cwd:root,env,stdio:"inherit"});
  execFileSync("wasm-bindgen",[resolve(target,"wasm32-unknown-unknown/wasm",artifact+".wasm"),"--target","web","--remove-producers-section","--out-dir",dir,"--out-name","hexscope_"+family],{cwd:root,stdio:"inherit"});
}
execFileSync(process.execPath,[resolve(root,"scripts/wasm-profile.mjs"),"--dir="+dir,"--kind=diagnostic","--output=.perf/wasm-profile-diagnostic.json"],{cwd:root,stdio:"inherit"});
