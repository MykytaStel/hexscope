// What the core says, in English sentences, reaches a Ukrainian reader only
// through the dictionary: a sentence reworded in Rust, or a new one, is shown
// in English until someone adds it. This reads every test file and sample
// with the real parser and names each sentence a reader meets — the verdict,
// the headline, the tips, the findings that are sentences, what a clean copy took out, what each
// part of the file is — that has no Ukrainian.
import { readdirSync, readFileSync, statSync } from "node:fs";
import { beforeAll, describe, expect, it } from "vitest";
import * as mediaBuild from "./wasm-media/hexscope_media.js";
import * as pdfBuild from "./wasm-pdf/hexscope_pdf.js";
import * as docsBuild from "./wasm-docs/hexscope_docs.js";
import { familyOf } from "./family";
import { describe as describeParsed, SEPARATOR } from "./describe";
import { FileModel } from "./model";
import { explain } from "./explanations";
import { headline } from "./headline";
import { verdict } from "./verdict";
import { advice } from "./advice";
import { translateText } from "./i18n";
import "./i18n-uk";
import "./analysis-i18n";
import "./docs-i18n";

const here = (path: string) => new URL(path, import.meta.url);

beforeAll(async () => {
  mediaBuild.initSync({ module: readFileSync(here("./wasm-media/hexscope_media_bg.wasm")) });
  pdfBuild.initSync({ module: readFileSync(here("./wasm-pdf/hexscope_pdf_bg.wasm")) });
  docsBuild.initSync({ module: readFileSync(here("./wasm-docs/hexscope_docs_bg.wasm")) });
  // What each part is, as the page fetches it: one file per build.
  await Promise.all((["png", "pdf", "zip"] as const).map(explain));
});

/** Every file the tests and the site have, by name. */
function corpus(): [string, Uint8Array][] {
  const out: [string, Uint8Array][] = [];
  for (const dir of ["../../../crates/hexscope-core/tests/fixtures/", "../public/samples/"]) {
    const root = here(dir);
    for (const name of readdirSync(root)) {
      const at = new URL(name, root);
      if (statSync(at).isFile()) out.push([name, new Uint8Array(readFileSync(at))]);
    }
  }
  return out;
}

/** English words left in a line, outside what it quotes from the file: a sentence half translated is not translated. */
const english = (s: string) =>
  /\b(the|and|of|to|is|it|its|from|with|not|for|by|was|this|that|goes|more|things?|another|than|what|when|where|who)\b/.test(
    s.replace(/«[^»]*»|“[^”]*”|"[^"]*"|\S+@\S+|https?:\/\/\S+/g, ""),
  );

/** A sentence in words, not a name, a number or an address. */
const prose = (s: string) => /\b[a-z]{3,}\b/.test(s.replace(/\bhexscope\b/g, "")) && !/^[\w.-]+@|https?:\/\//.test(s);

/** What a clean copy took out, as the page shows it: a part's name kept, what it held said in pieces. */
function removedLines(removed: string): string[] {
  const items = removed ? removed.split(SEPARATOR) : [];
  const out: string[] = [];
  for (let i = 0; i < items.length; i += 2) {
    const what = items[i];
    const part = /^([^\s:]+\.[A-Za-z0-9]+): (.+)$/.exec(what);
    if (part) out.push(...part[2].split("; "));
    else if (!/^[^\s:]+\//.test(what)) out.push(what.charAt(0).toUpperCase() + what.slice(1));
  }
  return out;
}

/** Kinds of finding whose text is the file's own words — a title, a name, a path, a quote — and stays as it is. */
const DATA = new Set([
  "title", "subject", "keywords", "description", "category", "status", "author", "editor", "company", "manager",
  "template", "folder", "printer", "application", "software", "producer", "camera", "lens", "serial", "owner",
  "computer", "toolchain", "caption", "prompt", "deleted", "earlier", "downloaded", "tags", "label", "custom",
  "taken", "language", "sdk", "sourcemap", "debug", "debuginfo", "mailer",
]);

describe("every sentence a reader meets about a real file", () => {
  it("is said in Ukrainian", () => {
    const missing = new Map<string, string>();
    let explained = 0;
    for (const [name, bytes] of corpus()) {
      const build = { media: mediaBuild, pdf: pdfBuild, docs: docsBuild }[familyOf(bytes)];
      let m: FileModel;
      try {
        m = new FileModel(describeParsed(build.parse(bytes)), bytes, name);
      } catch {
        continue;
      }
      const said = [
        ...verdict(m).map((l) => l.text),
        headline(m).text,
        ...advice(m).map((t) => t.text),
      ];
      said.push(...m.file.facts.filter((f) => !DATA.has(f.kind)).map((f) => f.text));
      // What each part is, as the Structure and Bytes views explain it.
      for (let i = 0; i < m.file.labels.length; i++) {
        const doc = m.doc(i);
        if (doc?.text) {
          said.push(doc.text);
          explained++;
        }
      }
      const copy = build.cleanCopy(bytes, false);
      if (!copy.error) said.push(...removedLines(copy.removed));
      for (const s of said) {
        if (prose(s) && english(translateText(s, "uk")) && !missing.has(s)) missing.set(s, name);
      }
    }
    expect([...missing].map(([s, name]) => `${name}: ${s}`)).toEqual([]);
    // The sentences came: a part without one would pass unread.
    expect(explained).toBeGreaterThan(1000);
  });
});
