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
import * as audioBuild from "./wasm-audio/hexscope_audio.js";
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
  audioBuild.initSync({ module: readFileSync(here("./wasm-audio/hexscope_audio_bg.wasm")) });
  // What each part is, as the page fetches it: one file per build.
  await Promise.all((["png", "pdf", "zip", "mp3"] as const).map(explain));
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
  out.push(...audioExamples());
  return out;
}

/** Small metadata-only streams exercise the audio build and its explanations. */
function audioExamples(): [string, Uint8Array][] {
  const le32 = (value: number) => [value & 255, (value >>> 8) & 255, (value >>> 16) & 255, value >>> 24];
  const be32 = (value: number) => [(value >>> 24) & 255, (value >>> 16) & 255, (value >>> 8) & 255, value & 255];
  const text = new TextEncoder();
  const field = text.encode("TITLE=Sample");
  const comments = new Uint8Array([...le32(0), ...le32(1), ...le32(field.length), ...field]);

  const mp3Frame = new Uint8Array([
    ...text.encode("TIT2"), 0, 0, 0, 3, 0, 0, 3, 83, 111,
  ]);
  const mp3 = new Uint8Array([
    ...text.encode("ID3"), 4, 0, 0, 0, 0, 0, mp3Frame.length, ...mp3Frame,
  ]);

  const picture = new Uint8Array([
    ...be32(3), ...be32(3), 80, 78, 71, ...be32(0),
    ...be32(1), ...be32(1), ...be32(24), ...be32(0), ...be32(1), 0x42,
  ]);
  const block = (type: number, body: Uint8Array, last: boolean) => [
    (last ? 0x80 : 0) | type,
    (body.length >>> 16) & 255,
    (body.length >>> 8) & 255,
    body.length & 255,
    ...body,
  ];
  const streamInfo = new Uint8Array([
    ...text.encode("fLaC"), ...block(0, new Uint8Array(34), false),
  ]);
  const flacFrames = new Uint8Array([0xff, 0xf8, 0x69, 0x08, 1, 2, 3, 4]);
  const flac = new Uint8Array([
    ...streamInfo,
    ...block(4, comments, false),
    ...block(6, picture, true),
    ...flacFrames,
  ]);
  const flacPictureOnly = new Uint8Array([
    ...streamInfo,
    ...block(6, picture, true),
    ...flacFrames,
  ]);

  const oggPage = (sequence: number, flags: number, packet: Uint8Array) => {
    const page = new Uint8Array(28 + packet.length);
    page.set(text.encode("OggS"));
    page[5] = flags;
    page.set([7, 0, 0, 0], 14);
    page.set(le32(sequence), 18);
    page[26] = 1;
    page[27] = packet.length;
    page.set(packet, 28);
    let crc = 0;
    for (let i = 0; i < page.length; i++) {
      crc ^= (i >= 22 && i < 26 ? 0 : page[i]) << 24;
      for (let bit = 0; bit < 8; bit++) {
        crc = ((crc << 1) ^ (crc & 0x80000000 ? 0x04c11db7 : 0)) >>> 0;
      }
    }
    page.set([crc & 255, (crc >>> 8) & 255, (crc >>> 16) & 255, crc >>> 24], 22);
    return page;
  };
  const identification = new Uint8Array([
    1, ...text.encode("vorbis"), ...le32(0), 2, ...le32(44_100),
    ...new Uint8Array(12), 0x86, 1,
  ]);
  const oggComments = new Uint8Array([
    ...text.encode("\x03vorbis"), ...comments, 1,
  ]);
  const pages = [oggPage(0, 2, identification), oggPage(1, 0, oggComments)];
  const ogg = new Uint8Array(pages.reduce((size, page) => size + page.length, 0));
  let cursor = 0;
  for (const page of pages) {
    ogg.set(page, cursor);
    cursor += page.length;
  }
  const oggBadChecksum = ogg.slice();
  oggBadChecksum[oggBadChecksum.length - 1] ^= 1;

  return [
    ["sample.mp3", mp3],
    ["sample.flac", flac],
    ["cover-only.flac", flacPictureOnly],
    ["sample.ogg", ogg],
    ["bad-checksum.ogg", oggBadChecksum],
  ];
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
      const build = { media: mediaBuild, pdf: pdfBuild, docs: docsBuild, audio: audioBuild }[familyOf(bytes)];
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
