// Structural attribution only. No instruction decoding or reachability claims.
// Every length is bounded by its enclosing section before advancing.
const SECTION_NAMES = ["custom","type","import","function","table","memory","global","export","start","element","code","data","data-count","tag"];
const MAGIC = [0,97,115,109,1,0,0,0];
const decoder = new TextDecoder("utf-8", { fatal: true });

class Reader {
  constructor(bytes, start=0, end=bytes.length) { this.bytes=bytes; this.pos=start; this.end=end; }
  byte() {
    if (this.pos >= this.end) throw new Error("Unexpected end of WASM section");
    return this.bytes[this.pos++];
  }
  u32() {
    let value=0;
    for (let i=0;i<5;i++) {
      const b=this.byte();
      if (i===4 && (b & 0xf0)) throw new Error("Invalid WASM u32 LEB128");
      value += (b & 127) * 2 ** (7*i);
      if (!(b & 128)) return value;
    }
    throw new Error("Invalid WASM u32 LEB128");
  }
  take(length) {
    if (!Number.isSafeInteger(length) || length<0 || length>this.end-this.pos) throw new Error("WASM range exceeds enclosing section");
    const start=this.pos;
    this.pos+=length;
    return new Reader(this.bytes,start,this.pos);
  }
  name() {
    const r=this.take(this.u32());
    return decoder.decode(r.bytes.subarray(r.pos,r.end));
  }
  finish() { if (this.pos!==this.end) throw new Error("Unexpected trailing WASM section bytes"); }
}

function limits(r) {
  const flags=r.u32();
  if (flags>3) throw new Error("Unsupported WASM limits encoding (memory64 or newer proposal)");
  const min=r.u32();
  const max=(flags&1)?r.u32():null;
  return { min,max,shared:!!(flags&2) };
}
function imports(r) {
  const count=r.u32();
  let functions=0;
  for(let i=0;i<count;i++) {
    r.name(); r.name();
    const kind=r.byte();
    if(kind===0) { r.u32(); functions++; }
    else if(kind===1) { const type=r.byte(); if(type!==0x70 && type!==0x6f) throw new Error("Unsupported table reference type"); limits(r); }
    else if(kind===2) limits(r);
    else if(kind===3) { const type=r.byte(); if(![0x7f,0x7e,0x7d,0x7c,0x7b,0x70,0x6f].includes(type)) throw new Error("Unsupported global type"); r.byte(); }
    else if(kind===4) { r.byte(); r.u32(); }
    else throw new Error("Unknown WASM import kind");
  }
  r.finish();
  return functions;
}
function readNames(r,names) {
  while(r.pos<r.end) {
    const kind=r.byte();
    const body=r.take(r.u32());
    if(kind!==1) continue;
    const count=body.u32();
    for(let i=0;i<count;i++) names.set(body.u32(),body.name());
    body.finish();
  }
}
function groupOf(name) {
  // Compiler-generated names can be mangled or absent. Only label explicit
  // crate tokens, never assign anonymous functions to a parser by guesswork.
  if(!name) return "anonymous";
  if(name.includes("hexscope_inflate")) return "hexscope_inflate";
  if(name.includes("hexscope_core")) return "hexscope_core";
  if(name.includes("hexscope_wasm")) return "hexscope_wasm";
  if(name.includes("hexscope_raster")) return "hexscope_raster";
  if(name.includes("wasm_bindgen") || name.includes("__wbindgen")) return "wasm_bindgen";
  return "other named";
}

export function profileWasm(bytes,{top=30}={}) {
  if (!Number.isInteger(top)||top<1||top>1000) throw new Error("Top count must be 1–1000");
  if (!MAGIC.every((v,i)=>bytes[i]===v)) throw new Error("Expected a WASM v1 core module");
  const r=new Reader(bytes,8);
  const sections=[], bodies=[], exports=[], names=new Map(), seen=new Set();
  let importedFunctions=0, declaredFunctions=0, definedMemory=[];
  while(r.pos<r.end) {
    const start=r.pos, id=r.byte(), length=r.u32(), payload=r.take(length);
    if(id!==0) {
      if(seen.has(id)) throw new Error("Duplicate WASM section "+id);
      seen.add(id);
    }
    let name=SECTION_NAMES[id]??"section-"+id;
    if(id===0) {
      name="custom:"+payload.name();
      if(name==="custom:name") readNames(payload,names);
    } else if(id===2) importedFunctions=imports(payload);
    else if(id===3) {
      declaredFunctions=payload.u32();
      for(let i=0;i<declaredFunctions;i++) payload.u32();
      payload.finish();
    } else if(id===5) {
      const count=payload.u32();
      for(let i=0;i<count;i++) definedMemory.push(limits(payload));
      payload.finish();
    } else if(id===7) {
      const count=payload.u32();
      for(let i=0;i<count;i++) exports.push({name:payload.name(),kind:payload.byte(),index:payload.u32()});
      payload.finish();
    } else if(id===10) {
      const count=payload.u32();
      for(let i=0;i<count;i++) {
        const from=payload.pos, size=payload.u32();
        payload.take(size);
        bodies.push({index:importedFunctions+i,bodyBytes:size,encodedBytes:payload.pos-from});
      }
      payload.finish();
    }
    sections.push({id,name,offset:start,payloadBytes:length,encodedBytes:r.pos-start});
  }
  if(declaredFunctions!==bodies.length) throw new Error("Function and code section counts differ");
  const aliases=new Map();
  for(const e of exports) if(e.kind===0) {
    const list=aliases.get(e.index)??[];
    list.push(e.name); aliases.set(e.index,list);
  }
  const groups=new Map();
  for(const body of bodies) {
    body.name=names.get(body.index)??aliases.get(body.index)?.[0]??null;
    body.exports=aliases.get(body.index)??[];
    body.group=groupOf(body.name);
    const group=groups.get(body.group)??{group:body.group,functions:0,bodyBytes:0};
    group.functions++; group.bodyBytes+=body.bodyBytes; groups.set(body.group,group);
  }
  const bodyBytes=bodies.reduce((n,b)=>n+b.bodyBytes,0);
  const encodedBodyBytes=bodies.reduce((n,b)=>n+b.encodedBytes,0);
  const code=sections.find(s=>s.id===10);
  const accounted=8+sections.reduce((n,s)=>n+s.encodedBytes,0);
  if(accounted!==bytes.length) throw new Error("Section accounting mismatch");
  return {rawBytes:bytes.length,headerBytes:8,sections,importedFunctions,definedFunctions:bodies.length,
    definedMemory,bodyBytes,encodedBodyBytes,codeVectorOverheadBytes:(code?.payloadBytes??0)-encodedBodyBytes,
    namedFunctions:bodies.filter(b=>b.name!==null).length,groups:[...groups.values()].sort((a,b)=>b.bodyBytes-a.bodyBytes),
    largestFunctions:bodies.sort((a,b)=>b.bodyBytes-a.bodyBytes||a.index-b.index).slice(0,top),exports};
}
