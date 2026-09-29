// A grid of modules, as sampled from a picture, to the text it holds: the
// format information says the level and the mask, the modules outside the
// function patterns give the codewords, and each block is corrected.

import { decodeData } from "./data";
import { correct } from "./rs";
import { blocks, format, functionModules, rawModules, sizeOf, versionOf } from "./version";

/** A square of modules, 1 for dark, row by row. */
export interface Grid {
  size: number;
  dark: Uint8Array;
}

export interface Read {
  text: string;
  version: number;
  level: number;
}

const at = (g: Grid, x: number, y: number) => g.dark[y * g.size + x];

/** Reads bits at `points`, most significant first. */
function word(g: Grid, points: [number, number][]): number {
  let v = 0;
  for (const [x, y] of points) v = (v << 1) | at(g, x, y);
  return v;
}

function formatCopies(g: Grid): number[] {
  const s = g.size;
  const first: [number, number][] = [];
  for (let x = 0; x <= 5; x++) first.push([x, 8]);
  first.push([7, 8], [8, 8], [8, 7]);
  for (let y = 5; y >= 0; y--) first.push([8, y]);
  const second: [number, number][] = [];
  for (let y = s - 1; y >= s - 7; y--) second.push([8, y]);
  for (let x = s - 8; x < s; x++) second.push([x, 8]);
  return [word(g, first), word(g, second)];
}

/** The version information beside the top-right and bottom-left finders, read both ways. */
export function versionCopies(g: Grid): number[] {
  const s = g.size;
  const topRight: [number, number][] = [];
  const bottomLeft: [number, number][] = [];
  for (let a = 5; a >= 0; a--) {
    for (let b = s - 9; b >= s - 11; b--) {
      topRight.push([b, a]);
      bottomLeft.push([a, b]);
    }
  }
  return [word(g, topRight), word(g, bottomLeft)];
}

const MASKS: ((x: number, y: number) => boolean)[] = [
  (x, y) => (x + y) % 2 === 0,
  (_, y) => y % 2 === 0,
  (x) => x % 3 === 0,
  (x, y) => (x + y) % 3 === 0,
  (x, y) => (Math.floor(x / 3) + Math.floor(y / 2)) % 2 === 0,
  (x, y) => ((x * y) % 2) + ((x * y) % 3) === 0,
  (x, y) => (((x * y) % 2) + ((x * y) % 3)) % 2 === 0,
  (x, y) => (((x + y) % 2) + ((x * y) % 3)) % 2 === 0,
];

/** The text in `g`, or null when it is not a readable QR code of its size. */
export function readGrid(g: Grid): Read | null {
  const version = (g.size - 17) / 4;
  if (!Number.isInteger(version) || version < 1 || version > 40) return null;
  if (version >= 7 && versionOf(versionCopies(g)) !== version) return null;
  const f = format(formatCopies(g));
  if (!f) return null;

  // The codewords, two columns at a time from the right, up then down.
  const functions = functionModules(version);
  const mask = MASKS[f.mask];
  const total = Math.floor(rawModules(version) / 8);
  const codewords = new Uint8Array(total);
  let bit = 0;
  const s = g.size;
  for (let right = s - 1; right >= 1; right -= 2) {
    if (right === 6) right = 5;
    const upward = ((right + 1) & 2) === 0;
    for (let v = 0; v < s; v++) {
      const y = upward ? s - 1 - v : v;
      for (let j = 0; j < 2; j++) {
        const x = right - j;
        if (functions[y * s + x] || bit >= total * 8) continue;
        const dark = at(g, x, y) ^ (mask(x, y) ? 1 : 0);
        if (dark) codewords[bit >> 3] |= 0x80 >> (bit & 7);
        bit++;
      }
    }
  }

  // Blocks are interleaved codeword by codeword: data, then check bytes.
  const layout = blocks(version, f.level);
  const parts = layout.map((b) => new Uint8Array(b.data + b.ecc));
  const longest = Math.max(...layout.map((b) => b.data));
  let k = 0;
  for (let i = 0; i < longest; i++) layout.forEach((b, j) => i < b.data && (parts[j][i] = codewords[k++]));
  const ecc = layout[0].ecc;
  for (let i = 0; i < ecc; i++) layout.forEach((b, j) => (parts[j][b.data + i] = codewords[k++]));

  const data: number[] = [];
  for (let j = 0; j < parts.length; j++) {
    if (!correct(parts[j], ecc)) return null;
    for (let i = 0; i < layout[j].data; i++) data.push(parts[j][i]);
  }
  const text = decodeData(Uint8Array.from(data), version);
  return text === null ? null : { text, version, level: f.level };
}

/** The grid read the other way round: a code seen in a mirror, or whose sampling swapped the axes. */
export function transposed(g: Grid): Grid {
  const s = g.size;
  const dark = new Uint8Array(s * s);
  for (let y = 0; y < s; y++) for (let x = 0; x < s; x++) dark[x * s + y] = g.dark[y * s + x];
  return { size: s, dark };
}

export { sizeOf };
