// What ISO/IEC 18004 fixes for each version: where its function patterns
// are, how many codewords it holds, and how they split into blocks at each
// error correction level; and the format and version information around the
// finder patterns, each read against every valid code.

/** Error correction levels, in the tables' order. */
export const LEVELS = ["L", "M", "Q", "H"] as const;

// Check codewords per block, and blocks, by level then version (index 0 unused).
const ECC_PER_BLOCK = [
  [0, 7, 10, 15, 20, 26, 18, 20, 24, 30, 18, 20, 24, 26, 30, 22, 24, 28, 30, 28, 28, 28, 28, 30, 30, 26, 28, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30],
  [0, 10, 16, 26, 18, 24, 16, 18, 22, 22, 26, 30, 22, 22, 24, 24, 28, 28, 26, 26, 26, 26, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28],
  [0, 13, 22, 18, 26, 18, 24, 18, 22, 20, 24, 28, 26, 24, 20, 30, 24, 28, 28, 26, 30, 28, 30, 30, 30, 30, 28, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30],
  [0, 17, 28, 22, 16, 22, 28, 26, 26, 24, 28, 24, 28, 22, 24, 24, 30, 28, 28, 26, 28, 30, 24, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30],
];
const BLOCKS = [
  [0, 1, 1, 1, 1, 1, 2, 2, 2, 2, 4, 4, 4, 4, 4, 6, 6, 6, 6, 7, 8, 8, 9, 9, 10, 12, 12, 12, 13, 14, 15, 16, 17, 18, 19, 19, 20, 21, 22, 24, 25],
  [0, 1, 1, 1, 2, 2, 4, 4, 4, 5, 5, 5, 8, 9, 9, 10, 10, 11, 13, 14, 16, 17, 17, 18, 20, 21, 23, 25, 26, 28, 29, 31, 33, 35, 37, 38, 40, 43, 45, 47, 49],
  [0, 1, 1, 2, 2, 4, 4, 6, 6, 8, 8, 8, 10, 12, 16, 12, 17, 16, 18, 21, 20, 23, 23, 25, 27, 29, 34, 34, 35, 38, 40, 43, 45, 48, 51, 53, 56, 59, 62, 65, 68],
  [0, 1, 1, 2, 4, 4, 4, 5, 6, 8, 8, 11, 11, 16, 16, 18, 16, 19, 21, 25, 25, 25, 34, 30, 32, 35, 37, 40, 42, 45, 48, 51, 54, 57, 60, 63, 66, 70, 74, 77, 81],
];

export const sizeOf = (version: number) => 17 + 4 * version;

/** Centres of the alignment patterns along each axis. */
export function alignment(version: number): number[] {
  if (version === 1) return [];
  const count = Math.floor(version / 7) + 2;
  const step = Math.floor((version * 8 + count * 3 + 5) / (count * 4 - 4)) * 2;
  const out = [6];
  for (let pos = sizeOf(version) - 7; out.length < count; pos -= step) out.splice(1, 0, pos);
  return out;
}

/** Modules that carry data or its check bits, not a pattern or format information. */
export function rawModules(version: number): number {
  let n = (16 * version + 128) * version + 64;
  if (version >= 2) {
    const count = Math.floor(version / 7) + 2;
    n -= (25 * count - 10) * count - 55;
    if (version >= 7) n -= 36;
  }
  return n;
}

/** How a version's codewords split at a level: each block's data and check lengths. */
export function blocks(version: number, level: number): { data: number; ecc: number }[] {
  const count = BLOCKS[level][version];
  const ecc = ECC_PER_BLOCK[level][version];
  const total = Math.floor(rawModules(version) / 8);
  const short = count - (total % count);
  const shortLength = Math.floor(total / count);
  const out = [];
  for (let i = 0; i < count; i++) out.push({ data: shortLength - ecc + (i < short ? 0 : 1), ecc });
  return out;
}

/** Which modules of a version are function patterns and format or version information. */
export function functionModules(version: number): Uint8Array {
  const size = sizeOf(version);
  const f = new Uint8Array(size * size);
  const fill = (x0: number, y0: number, w: number, h: number) => {
    for (let y = Math.max(0, y0); y < Math.min(size, y0 + h); y++) for (let x = Math.max(0, x0); x < Math.min(size, x0 + w); x++) f[y * size + x] = 1;
  };
  fill(0, 0, 9, 9);
  fill(size - 8, 0, 8, 9);
  fill(0, size - 8, 9, 8);
  fill(0, 6, size, 1);
  fill(6, 0, 1, size);
  const centres = alignment(version);
  const last = centres.length - 1;
  for (let i = 0; i <= last; i++) {
    for (let j = 0; j <= last; j++) {
      if ((i === 0 && j === 0) || (i === 0 && j === last) || (i === last && j === 0)) continue;
      fill(centres[i] - 2, centres[j] - 2, 5, 5);
    }
  }
  if (version >= 7) {
    fill(size - 11, 0, 3, 6);
    fill(0, size - 11, 6, 3);
  }
  return f;
}

function bits(n: number): number {
  let c = 0;
  for (; n; n &= n - 1) c++;
  return c;
}

/** The valid code nearest `read`, if within three bits: its data. */
function nearest(read: number, codes: number[], dataOf: (i: number) => number): number | null {
  let best = -1;
  let distance = 4;
  for (let i = 0; i < codes.length; i++) {
    const d = bits(read ^ codes[i]);
    if (d < distance) [best, distance] = [i, d];
  }
  return best < 0 ? null : dataOf(best);
}

/** The 32 format codes: level bits and mask, BCH-coded and masked. */
const FORMAT_CODES = Array.from({ length: 32 }, (_, data) => {
  let rem = data;
  for (let i = 0; i < 10; i++) rem = (rem << 1) ^ ((rem >>> 9) * 0x537);
  return ((data << 10) | rem) ^ 0x5412;
});
/** Level bits in the format information: 01 is L, 00 M, 11 Q, 10 H. */
const LEVEL_OF_BITS = [1, 0, 3, 2];

/** Level (index into `LEVELS`) and mask from either copy of the format information. */
export function format(copies: number[]): { level: number; mask: number } | null {
  for (const read of copies) {
    const data = nearest(read, FORMAT_CODES, (i) => i);
    if (data !== null) return { level: LEVEL_OF_BITS[data >> 3], mask: data & 7 };
  }
  return null;
}

const VERSION_CODES = Array.from({ length: 34 }, (_, i) => {
  const v = i + 7;
  let rem = v;
  for (let k = 0; k < 12; k++) rem = (rem << 1) ^ ((rem >>> 11) * 0x1f25);
  return (v << 12) | rem;
});

/** A version from 7 up, from either copy of its version information. */
export function versionOf(copies: number[]): number | null {
  for (const read of copies) {
    const v = nearest(read, VERSION_CODES, (i) => i + 7);
    if (v !== null) return v;
  }
  return null;
}
