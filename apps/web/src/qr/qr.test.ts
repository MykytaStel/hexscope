// The QR reader against codes made by Core Image, the system's own encoder
// (scripts/make-qr-fixtures.swift): every level, versions 1 to 40, as grids
// and as pictures — scaled, turned, tilted, blurred.
import { readdirSync, readFileSync } from "node:fs";
import { inflateSync } from "node:zlib";
import { describe, expect, it } from "vitest";
import { readGrid } from "./grid";
import { findCodes } from "./index";
import { alignment, blocks, LEVELS, rawModules } from "./version";
import { meaning } from "./meaning";

const dir = new URL("./fixtures/", import.meta.url);

/** An 8-bit greyscale PNG's pixels. */
function png(name: string): { width: number; height: number; pixels: Uint8Array } {
  const b = readFileSync(new URL(name, dir));
  const width = b.readUInt32BE(16);
  const height = b.readUInt32BE(20);
  const idat: Buffer[] = [];
  for (let at = 8; at < b.length; ) {
    const len = b.readUInt32BE(at);
    if (b.toString("latin1", at + 4, at + 8) === "IDAT") idat.push(b.subarray(at + 8, at + 8 + len));
    at += 12 + len;
  }
  const raw = inflateSync(Buffer.concat(idat));
  const pixels = new Uint8Array(width * height);
  for (let y = 0; y < height; y++) {
    const filter = raw[y * (width + 1)];
    for (let x = 0; x < width; x++) {
      const v = raw[y * (width + 1) + 1 + x];
      const a = x > 0 ? pixels[y * width + x - 1] : 0;
      const up = y > 0 ? pixels[(y - 1) * width + x] : 0;
      const c = x > 0 && y > 0 ? pixels[(y - 1) * width + x - 1] : 0;
      const p = a + up - c;
      const paeth = Math.abs(p - a) <= Math.abs(p - up) && Math.abs(p - a) <= Math.abs(p - c) ? a : Math.abs(p - up) <= Math.abs(p - c) ? up : c;
      const pred = [0, a, up, (a + up) >> 1, paeth][filter];
      pixels[y * width + x] = (v + pred) & 0xff;
    }
  }
  return { width, height, pixels };
}

/** What make-qr-fixtures.swift wrote into a code of `n` characters. */
function expected(n: number): string {
  const words = "hexscope reads the file you give it, in your browser, and nothing leaves the tab. ";
  let s = `https://hexscope.pages.dev/?n=${n}&t=`;
  while (s.length < n) s += words;
  return s.slice(0, n);
}

const codes = readdirSync(dir).filter((f) => /^[LMQH]-\d+\.png$/.test(f));

describe("the tables", () => {
  it("hold every version's codewords, at every level", () => {
    for (let v = 1; v <= 40; v++) {
      for (let level = 0; level < 4; level++) {
        const total = blocks(v, level).reduce((n, b) => n + b.data + b.ecc, 0);
        expect(total).toBe(Math.floor(rawModules(v) / 8));
      }
    }
    expect(alignment(7)).toEqual([6, 22, 38]);
    expect(alignment(32)).toEqual([6, 34, 60, 86, 112, 138]);
    expect(alignment(40)).toEqual([6, 30, 58, 86, 114, 142, 170]);
  });
});

describe("a grid of modules", () => {
  it("reads every code Core Image made, one module a pixel", () => {
    const versions = new Set<string>();
    for (const name of codes) {
      const { width, pixels } = png(name);
      // One module of quiet zone around it.
      const size = width - 2;
      const dark = new Uint8Array(size * size);
      for (let y = 0; y < size; y++) for (let x = 0; x < size; x++) dark[y * size + x] = pixels[(y + 1) * width + x + 1] < 128 ? 1 : 0;
      const read = readGrid({ size, dark });
      expect(read?.text, name).toBe(expected(Number(name.slice(2, -4))));
      expect(LEVELS[read!.level], name).toBe(name[0]);
      versions.add(`${name[0]}${read!.version}`);
    }
    // Every version at L, to 40.
    for (let v = 1; v <= 40; v++) expect(versions.has(`L${v}`), `L${v}`).toBe(true);
  });

  it("mends as many errors as its level allows", () => {
    const { width, pixels } = png("H-78.png");
    const size = width - 2;
    const dark = new Uint8Array(size * size);
    for (let y = 0; y < size; y++) for (let x = 0; x < size; x++) dark[y * size + x] = pixels[(y + 1) * width + x + 1] < 128 ? 1 : 0;
    // A stain over the middle: a sixth of the code.
    const stain = Math.floor(size / 2.5);
    const from = Math.floor((size - stain) / 2);
    for (let y = from; y < from + stain; y++) for (let x = from; x < from + stain; x++) dark[y * size + x] ^= 1;
    expect(readGrid({ size, dark })?.text).toBe(expected(78));
  });
});

/** A code from the fixtures drawn into a picture: `scale` pixels a module, turned by `turn`, on a light margin. */
function drawn(name: string, scale: number, turn: number): { lum: Uint8Array; width: number; height: number } {
  const { width: n, pixels } = png(name);
  const side = Math.ceil(n * scale * 1.5) + 40;
  const lum = new Uint8Array(side * side).fill(235);
  const [cos, sin] = [Math.cos(turn), Math.sin(turn)];
  for (let y = 0; y < side; y++) {
    for (let x = 0; x < side; x++) {
      const dx = x - side / 2;
      const dy = y - side / 2;
      const mx = Math.floor((cos * dx + sin * dy) / scale + n / 2);
      const my = Math.floor((-sin * dx + cos * dy) / scale + n / 2);
      if (mx >= 0 && my >= 0 && mx < n && my < n && pixels[my * n + mx] < 128) lum[y * side + x] = 30;
    }
  }
  return { lum, width: side, height: side };
}

describe("a picture", () => {
  it("finds codes of every size, upright and turned", () => {
    for (const name of ["L-17.png", "M-78.png", "L-271.png", "Q-425.png", "L-1003.png", "H-1273.png"]) {
      const n = Number(name.slice(2, -4));
      for (const turn of [0, 0.3, 1.2, Math.PI]) {
        const { lum, width, height } = drawn(name, 4, turn);
        const found = findCodes(lum, width, height);
        expect(found.map((c) => c.text), `${name} turned ${turn}`).toEqual([expected(n)]);
      }
    }
  });

  it("reads codes as a camera sees them: tilted, blurred, on a background", () => {
    const text = (name: string) => {
      const { width, height, pixels } = png(name);
      return findCodes(pixels, width, height).map((c) => c.text);
    };
    expect(text("scene-wifi.png")).toEqual(["WIFI:S:Home Network;T:WPA;P:correct horse battery staple;;"]);
    expect(text("scene-link.png")).toEqual(["https://xn--exmple-bank-zij.com/verify?id=7Q1"]);
    expect(text("scene-v10.png")).toEqual([expected(300)]);
  });

  it("finds nothing where there is nothing", () => {
    const lum = new Uint8Array(300 * 200);
    for (let i = 0; i < lum.length; i++) lum[i] = (i * 2654435761) >>> 24;
    expect(findCodes(lum, 300, 200)).toEqual([]);
  });
});

describe("what a code says", () => {
  it("names what it gives away, and a link's tricks", () => {
    expect(meaning("WIFI:T:WPA;S:Home \\;Net;P:pa\\:ss;;")).toEqual({ kind: "qrwifi", text: "the Wi-Fi network “Home ;Net” and its password “pa:ss”" });
    expect(meaning("WIFI:S:Cafe;T:nopass;;").text).toBe("the open Wi-Fi network “Cafe”");
    expect(meaning("otpauth://totp/Example:olena@example.com?secret=JBSWY3DPEHPK3PXP&issuer=Example")).toEqual({
      kind: "qrsecret",
      text: "the two-factor secret for Example, “olena@example.com”: whoever has it can make the sign-in codes",
    });
    expect(meaning("geo:48.8584,2.2945")).toEqual({ kind: "qrplace", text: "the place 48.8584, 2.2945" });
    expect(meaning("BEGIN:VCARD\nVERSION:3.0\nFN:Olena Koval\nTEL;TYPE=cell:+380501234567\nEND:VCARD")).toEqual({ kind: "qrcontact", text: "a contact card: Olena Koval, +380501234567" });
    expect(meaning("https://xn--exmple-bank-zij.com/verify").kind).toBe("qrtrick");
    expect(meaning("https://xn--exmple-bank-zij.com/verify").text).toContain("shown as “exаmple-bank.com”");
    expect(meaning("https://bank.example@evil.example/login").text).toContain("is not where it goes");
    expect(meaning("https://bit.ly/3xyz").text).toContain("a short link");
    expect(meaning("https://example.com/menu")).toEqual({ kind: "qrlink", text: "goes to example.com: “https://example.com/menu”" });
    expect(meaning("SMSTO:+380501234567:Hi").text).toBe("starts a text message to +380501234567");
    expect(meaning("Table 12").text).toBe("the text “Table 12”");
  });
});
