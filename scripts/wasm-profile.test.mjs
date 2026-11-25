import { test } from "node:test";
import assert from "node:assert/strict";
import { profileWasm } from "./wasm-profile-core.mjs";
const head=[0,97,115,109,1,0,0,0];
const sec=(id,body)=>[id,body.length,...body];
const str=s=>[s.length,...Buffer.from(s)];
const moduleOf=(...sections)=>Uint8Array.from([...head,...sections.flat()]);
const basic=()=>moduleOf(sec(1,[1,0x60,0,0]),sec(3,[1,0]),sec(7,[1,...str("f"),0,0]),sec(10,[1,2,0,0x0b]));
test("accounts for all bytes and names exported functions",()=>{
  const bytes=basic();
  assert.equal(WebAssembly.validate(bytes),true);
  const p=profileWasm(bytes);
  assert.equal(p.rawBytes,bytes.length);
  assert.equal(p.sections.reduce((n,s)=>n+s.encodedBytes,8),bytes.length);
  assert.equal(p.definedFunctions,1);
  assert.equal(p.largestFunctions[0].name,"f");
  assert.equal(p.bodyBytes,2);
  assert.equal(p.encodedBodyBytes,3);
  assert.equal(p.codeVectorOverheadBytes,1);
});
test("function imports shift defined indices; optional names override exports",()=>{
  const nameMap=[1,1,...str("hexscope_core::pdf")];
  const bytes=moduleOf(sec(1,[1,0x60,0,0]),sec(2,[1,...str("env"),...str("log"),0,0]),sec(3,[1,0]),sec(7,[1,...str("f"),0,1]),sec(10,[1,2,0,0x0b]),sec(0,[...str("name"),1,nameMap.length,...nameMap]));
  const p=profileWasm(bytes);
  assert.equal(p.importedFunctions,1);
  assert.equal(p.largestFunctions[0].index,1);
  assert.equal(p.largestFunctions[0].name,"hexscope_core::pdf");
  assert.equal(p.groups[0].group,"hexscope_core");
  assert.deepEqual(p.largestFunctions[0].exports,["f"]);
});
test("anonymous bodies remain anonymous; reports initial defined memory limits",()=>{
  const p=profileWasm(moduleOf(sec(1,[1,0x60,0,0]),sec(3,[1,0]),sec(5,[1,1,2,4]),sec(10,[1,2,0,0x0b])));
  assert.equal(p.namedFunctions,0);
  assert.equal(p.groups[0].group,"anonymous");
  assert.deepEqual(p.definedMemory,[{min:2,max:4,shared:false}]);
});
test("rejects truncated sections, overflowing LEB, inconsistent counts and invalid names",()=>{
  assert.throws(()=>profileWasm(Uint8Array.of(0,1)),/WASM v1/);
  assert.throws(()=>profileWasm(Uint8Array.from([...head,10,20,1])),/range/);
  assert.throws(()=>profileWasm(Uint8Array.from([...head,10,255,255,255,255,16])),/LEB128/);
  assert.throws(()=>profileWasm(moduleOf(sec(3,[1,0]),sec(10,[0]))),/counts/);
  assert.throws(()=>profileWasm(moduleOf(sec(0,[1,255]))));
  assert.throws(()=>profileWasm(moduleOf(sec(3,[0]),sec(3,[0]))),/Duplicate/);
  assert.throws(()=>profileWasm(moduleOf(sec(3,[1,0]),sec(10,[1,10,0]))),/range/);
});
