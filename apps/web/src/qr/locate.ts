// Finding QR codes in a picture: dark and light told apart block by block,
// the three finder squares found by their 1:1:3:1:1 stripes, the code's
// fourth corner by its alignment pattern, and every module sampled through
// the perspective that maps the square code onto the picture.

import { versionCopies, type Grid } from "./grid";
import { versionOf } from "./version";

export interface Point {
  x: number;
  y: number;
}

/** A light-and-dark picture: 1 for dark. */
export interface Bitmap {
  width: number;
  height: number;
  dark: Uint8Array;
}

const BLOCK = 8;

/**
 * Each pixel against the average of the 5×5 blocks of 8 around it, so a
 * shadow or glare across the picture does not decide what is dark. A flat
 * block, with nothing to tell apart, takes its neighbours' level.
 */
export function binarize(lum: Uint8Array, width: number, height: number): Bitmap {
  const dark = new Uint8Array(width * height);
  if (width < BLOCK * 5 || height < BLOCK * 5) {
    let sum = 0;
    for (const v of lum) sum += v;
    const mean = sum / lum.length;
    for (let i = 0; i < lum.length; i++) dark[i] = lum[i] < mean ? 1 : 0;
    return { width, height, dark };
  }
  const bw = Math.ceil(width / BLOCK);
  const bh = Math.ceil(height / BLOCK);
  const level = new Float32Array(bw * bh);
  for (let by = 0; by < bh; by++) {
    const y0 = Math.min(by * BLOCK, height - BLOCK);
    for (let bx = 0; bx < bw; bx++) {
      const x0 = Math.min(bx * BLOCK, width - BLOCK);
      let sum = 0;
      let min = 255;
      let max = 0;
      for (let y = y0; y < y0 + BLOCK; y++) {
        for (let x = x0; x < x0 + BLOCK; x++) {
          const v = lum[y * width + x];
          sum += v;
          if (v < min) min = v;
          if (v > max) max = v;
        }
      }
      let avg = sum / (BLOCK * BLOCK);
      if (max - min <= 24) {
        avg = min / 2;
        if (by > 0 && bx > 0) {
          const around = (level[(by - 1) * bw + bx] + 2 * level[by * bw + bx - 1] + level[(by - 1) * bw + bx - 1]) / 4;
          if (min < around) avg = around;
        }
      }
      level[by * bw + bx] = avg;
    }
  }
  for (let by = 0; by < bh; by++) {
    const cy = Math.min(Math.max(by, 2), bh - 3);
    const y0 = Math.min(by * BLOCK, height - BLOCK);
    for (let bx = 0; bx < bw; bx++) {
      const cx = Math.min(Math.max(bx, 2), bw - 3);
      let sum = 0;
      for (let dy = -2; dy <= 2; dy++) for (let dx = -2; dx <= 2; dx++) sum += level[(cy + dy) * bw + cx + dx];
      const threshold = sum / 25;
      const x0 = Math.min(bx * BLOCK, width - BLOCK);
      for (let y = y0; y < y0 + BLOCK; y++) {
        for (let x = x0; x < x0 + BLOCK; x++) dark[y * width + x] = lum[y * width + x] <= threshold ? 1 : 0;
      }
    }
  }
  return { width, height, dark };
}

interface Finder extends Point {
  module: number;
  seen: number;
}

/** Five runs, dark-light-dark-light-dark, in the proportions 1:1:3:1:1. */
function finderRuns(c: number[]): boolean {
  const total = c[0] + c[1] + c[2] + c[3] + c[4];
  if (total < 7) return false;
  const m = total / 7;
  const v = m / 2;
  return Math.abs(m - c[0]) < v && Math.abs(m - c[1]) < v && Math.abs(3 * m - c[2]) < 3 * v && Math.abs(m - c[3]) < v && Math.abs(m - c[4]) < v;
}

/**
 * The stripes through a point along one axis: out from the centre each way,
 * dark, light, dark. The centre along that axis and the stripes' total, or
 * null when they are not a finder's.
 */
function across(b: Bitmap, cx: number, cy: number, dx: number, dy: number, max: number, expected: number): { centre: number; total: number } | null {
  const dark = (i: number) => {
    const x = cx + dx * i;
    const y = cy + dy * i;
    return x >= 0 && y >= 0 && x < b.width && y < b.height ? b.dark[y * b.width + x] === 1 : null;
  };
  const c = [0, 0, 0, 0, 0];
  let i = 0;
  while (dark(i) === true) (c[2]++, i--);
  while (dark(i) === false && c[1] <= max) (c[1]++, i--);
  if (dark(i) === null || c[1] > max) return null;
  while (dark(i) === true && c[0] <= max) (c[0]++, i--);
  if (c[0] > max) return null;
  i = 1;
  while (dark(i) === true) (c[2]++, i++);
  while (dark(i) === false && c[3] <= max) (c[3]++, i++);
  if (dark(i) === null || c[3] > max) return null;
  while (dark(i) === true && c[4] <= max) (c[4]++, i++);
  if (c[4] > max) return null;
  const total = c[0] + c[1] + c[2] + c[3] + c[4];
  if (5 * Math.abs(total - expected) >= 2 * expected || !finderRuns(c)) return null;
  return { centre: i - c[4] - c[3] - c[2] / 2, total };
}

/** Finder patterns: stripes found along rows, confirmed down the column and along the row again. */
export function finders(b: Bitmap): Finder[] {
  const found: Finder[] = [];
  const note = (x: number, y: number, module: number) => {
    for (const f of found) {
      if (Math.abs(f.x - x) <= f.module && Math.abs(f.y - y) <= f.module && Math.abs(module - f.module) <= Math.max(1, f.module)) {
        const n = f.seen + 1;
        f.x = (f.x * f.seen + x) / n;
        f.y = (f.y * f.seen + y) / n;
        f.module = (f.module * f.seen + module) / n;
        f.seen = n;
        return;
      }
    }
    found.push({ x, y, module, seen: 1 });
  };
  const check = (c: number[], y: number, end: number) => {
    const total = c[0] + c[1] + c[2] + c[3] + c[4];
    const cx = Math.round(end - c[4] - c[3] - c[2] / 2);
    const v = across(b, cx, y, 0, 1, c[2], total);
    if (!v) return;
    const cy = Math.floor(y + v.centre);
    const h = across(b, cx, cy, 1, 0, c[2], total);
    if (!h) return;
    note(cx + h.centre, y + v.centre, (v.total + h.total) / 14);
  };
  const skip = Math.max(2, Math.floor((3 * b.height) / (4 * 97)));
  for (let y = skip - 1; y < b.height; y += skip) {
    const c = [0, 0, 0, 0, 0];
    let s = 0;
    const row = y * b.width;
    for (let x = 0; x < b.width; x++) {
      const dark = b.dark[row + x] === 1;
      if (dark) {
        if (s % 2 === 1) s++;
        c[s]++;
      } else if (s % 2 === 0) {
        if (s === 0 && c[0] === 0) continue;
        if (s === 4) {
          if (finderRuns(c)) check(c, y, x);
          c.splice(0, 5, c[2], c[3], c[4], 1, 0);
          s = 3;
          continue;
        }
        s++;
        c[s]++;
      } else {
        c[s]++;
      }
    }
    if (s === 4 && finderRuns(c)) check(c, y, b.width);
  }
  return found;
}

const distance = (a: Point, b: Point) => Math.hypot(a.x - b.x, a.y - b.y);

interface Corners {
  topLeft: Finder;
  topRight: Finder;
  bottomLeft: Finder;
}

/** Three finders that can be one code's corners: a right angle at the top left, legs about equal, modules alike. */
function arrange(a: Finder, b: Finder, c: Finder): { corners: Corners; score: number } | null {
  const [ab, ac, bc] = [distance(a, b), distance(a, c), distance(b, c)];
  let [tl, p, q] = [a, b, c];
  if (bc >= ab && bc >= ac) [tl, p, q] = [a, b, c];
  else if (ac >= ab && ac >= bc) [tl, p, q] = [b, a, c];
  else [tl, p, q] = [c, a, b];
  const l1 = distance(tl, p);
  const l2 = distance(tl, q);
  const cos = ((p.x - tl.x) * (q.x - tl.x) + (p.y - tl.y) * (q.y - tl.y)) / (l1 * l2);
  const modules = [a.module, b.module, c.module];
  const spread = Math.max(...modules) / Math.min(...modules);
  const legs = Math.max(l1, l2) / Math.min(l1, l2);
  const size = (l1 + l2) / 2 / ((a.module + b.module + c.module) / 3);
  if (Math.abs(cos) > 0.45 || legs > 2 || spread > 2 || size < 10 || size > 190) return null;
  // With y growing downwards, turning from the top right to the bottom left is clockwise.
  const clockwise = (p.x - tl.x) * (q.y - tl.y) - (p.y - tl.y) * (q.x - tl.x) > 0;
  const [topRight, bottomLeft] = clockwise ? [p, q] : [q, p];
  return { corners: { topLeft: tl, topRight, bottomLeft }, score: Math.abs(cos) + (legs - 1) + (spread - 1) };
}

/** A projective map of one quadrilateral onto another. */
class Perspective {
  constructor(private readonly m: number[]) {}

  static square(x0: number, y0: number, x1: number, y1: number, x2: number, y2: number, x3: number, y3: number): Perspective {
    const dx3 = x0 - x1 + x2 - x3;
    const dy3 = y0 - y1 + y2 - y3;
    if (dx3 === 0 && dy3 === 0) return new Perspective([x1 - x0, x2 - x1, x0, y1 - y0, y2 - y1, y0, 0, 0, 1]);
    const dx1 = x1 - x2;
    const dx2 = x3 - x2;
    const dy1 = y1 - y2;
    const dy2 = y3 - y2;
    const den = dx1 * dy2 - dx2 * dy1;
    const a13 = (dx3 * dy2 - dx2 * dy3) / den;
    const a23 = (dx1 * dy3 - dx3 * dy1) / den;
    return new Perspective([x1 - x0 + a13 * x1, x3 - x0 + a23 * x3, x0, y1 - y0 + a13 * y1, y3 - y0 + a23 * y3, y0, a13, a23, 1]);
  }

  adjoint(): Perspective {
    const [a11, a21, a31, a12, a22, a32, a13, a23, a33] = this.m;
    return new Perspective([
      a22 * a33 - a23 * a32, a23 * a31 - a21 * a33, a21 * a32 - a22 * a31,
      a13 * a32 - a12 * a33, a11 * a33 - a13 * a31, a12 * a31 - a11 * a32,
      a12 * a23 - a13 * a22, a13 * a21 - a11 * a23, a11 * a22 - a12 * a21,
    ]);
  }

  times(o: Perspective): Perspective {
    const [a11, a21, a31, a12, a22, a32, a13, a23, a33] = this.m;
    const [b11, b21, b31, b12, b22, b32, b13, b23, b33] = o.m;
    return new Perspective([
      a11 * b11 + a21 * b12 + a31 * b13, a11 * b21 + a21 * b22 + a31 * b23, a11 * b31 + a21 * b32 + a31 * b33,
      a12 * b11 + a22 * b12 + a32 * b13, a12 * b21 + a22 * b22 + a32 * b23, a12 * b31 + a22 * b32 + a32 * b33,
      a13 * b11 + a23 * b12 + a33 * b13, a13 * b21 + a23 * b22 + a33 * b23, a13 * b31 + a23 * b32 + a33 * b33,
    ]);
  }

  map(x: number, y: number): Point {
    const [a11, a21, a31, a12, a22, a32, a13, a23, a33] = this.m;
    const d = a13 * x + a23 * y + a33;
    return { x: (a11 * x + a21 * y + a31) / d, y: (a12 * x + a22 * y + a32) / d };
  }

  /** The map from module coordinates `from` to picture points `to`, each in order top left, top right, bottom right, bottom left. */
  static between(from: Point[], to: Point[]): Perspective {
    const f = Perspective.square(from[0].x, from[0].y, from[1].x, from[1].y, from[2].x, from[2].y, from[3].x, from[3].y).adjoint();
    const t = Perspective.square(to[0].x, to[0].y, to[1].x, to[1].y, to[2].x, to[2].y, to[3].x, to[3].y);
    return t.times(f);
  }
}

/** Whether an alignment pattern — dark centre, light ring, dark ring — is centred on `p`. */
function alignmentAt(b: Bitmap, p: Point, module: number): number {
  let score = 0;
  for (let dy = -2; dy <= 2; dy++) {
    for (let dx = -2; dx <= 2; dx++) {
      const x = Math.round(p.x + dx * module);
      const y = Math.round(p.y + dy * module);
      if (x < 0 || y < 0 || x >= b.width || y >= b.height) return 0;
      const ring = Math.max(Math.abs(dx), Math.abs(dy));
      const want = ring === 1 ? 0 : 1;
      if (b.dark[y * b.width + x] === want) score++;
    }
  }
  return score;
}

/** The alignment pattern nearest where the corners put it, if one is there. */
function findAlignment(b: Bitmap, estimate: Point, module: number, reach: number): Point | null {
  let best: Point | null = null;
  let bestScore = 22;
  const r = Math.ceil(reach * module);
  const step = Math.max(1, Math.floor(module / 3));
  for (let y = Math.round(estimate.y - r); y <= estimate.y + r; y += step) {
    for (let x = Math.round(estimate.x - r); x <= estimate.x + r; x += step) {
      const score = alignmentAt(b, { x, y }, module);
      const closer = best && score === bestScore && distance({ x, y }, estimate) < distance(best, estimate);
      if (score > bestScore || closer) [best, bestScore] = [{ x, y }, score];
    }
  }
  return best;
}

/** The modules of a code of `size` modules, sampled through `map`. */
function sample(b: Bitmap, map: Perspective, size: number): Grid | null {
  const dark = new Uint8Array(size * size);
  for (let y = 0; y < size; y++) {
    for (let x = 0; x < size; x++) {
      const p = map.map(x + 0.5, y + 0.5);
      const px = Math.floor(p.x);
      const py = Math.floor(p.y);
      if (px < -1 || py < -1 || px > b.width || py > b.height) return null;
      const cx = Math.min(b.width - 1, Math.max(0, px));
      const cy = Math.min(b.height - 1, Math.max(0, py));
      dark[y * size + x] = b.dark[cy * b.width + cx];
    }
  }
  return { size, dark };
}

export interface Candidate {
  /** The three finder patterns it stands on. */
  finders: object[];
  /** Grids to try, most likely first: a size estimated from the corners, and its neighbours. */
  grids: () => Generator<Grid>;
  /** The code's corners in the picture: top left, top right, bottom right, bottom left. */
  corners: (size: number) => Point[];
}

/** Sets of three finders tried at most. */
const MAX_SETS = 40;

/** Every set of three finders that could be a code, the likeliest first. */
export function locate(b: Bitmap): Candidate[] {
  const all = finders(b);
  const strong = all.filter((f) => f.seen >= 2);
  const pool = (strong.length >= 3 ? strong : all).sort((p, q) => q.seen - p.seen).slice(0, 15);
  const sets: { corners: Corners; score: number }[] = [];
  for (let i = 0; i < pool.length; i++) {
    for (let j = i + 1; j < pool.length; j++) {
      for (let k = j + 1; k < pool.length; k++) {
        const s = arrange(pool[i], pool[j], pool[k]);
        if (s) sets.push(s);
      }
    }
  }
  sets.sort((p, q) => p.score - q.score);
  return sets.slice(0, MAX_SETS).map(({ corners }) => candidate(b, corners));
}

function candidate(b: Bitmap, { topLeft: tl, topRight: tr, bottomLeft: bl }: Corners): Candidate {
  const module = (tl.module + tr.module + bl.module) / 3;
  const estimate = Math.round((distance(tl, tr) + distance(tl, bl)) / 2 / module) + 7;
  const version = Math.min(40, Math.max(1, Math.round((estimate - 17) / 4)));
  const mapFor = (size: number): Perspective => {
    const far = size - 3.5;
    const br = { x: tr.x - tl.x + bl.x, y: tr.y - tl.y + bl.y };
    if (size > 21) {
      // The bottom-right alignment pattern, three modules in from the corner's finder-to-be.
      const k = 1 - 3 / (size - 7);
      const guess = { x: tl.x + k * (br.x - tl.x), y: tl.y + k * (br.y - tl.y) };
      const found = findAlignment(b, guess, module, Math.min(8, 2 + size / 20));
      if (found) {
        return Perspective.between(
          [{ x: 3.5, y: 3.5 }, { x: far, y: 3.5 }, { x: far - 3, y: far - 3 }, { x: 3.5, y: far }],
          [tl, tr, found, bl],
        );
      }
    }
    return Perspective.between([{ x: 3.5, y: 3.5 }, { x: far, y: 3.5 }, { x: far, y: far }, { x: 3.5, y: far }], [tl, tr, br, bl]);
  };
  return {
    finders: [tl, tr, bl],
    *grids() {
      const tried = new Set<number>();
      const at = (v: number) => {
        tried.add(v);
        const size = 17 + 4 * v;
        return sample(b, mapFor(size), size);
      };
      const first = at(version);
      if (first) yield first;
      // From version 7 the code says its version beside its top-right corner.
      const said = first && version >= 6 ? versionOf(versionCopies(first)) : null;
      if (said && !tried.has(said)) {
        const grid = at(said);
        if (grid) yield grid;
      }
      for (const v of [version + 1, version - 1, version + 2, version - 2, version + 3, version - 3]) {
        if (v < 1 || v > 40 || tried.has(v)) continue;
        const grid = at(v);
        if (grid) yield grid;
      }
    },
    corners(size: number) {
      const map = mapFor(size);
      return [map.map(0, 0), map.map(size, 0), map.map(size, size), map.map(0, size)];
    },
  };
}

/** The same picture, light for dark: a code printed light on a dark ground. */
export function inverted(b: Bitmap): Bitmap {
  const dark = new Uint8Array(b.dark.length);
  for (let i = 0; i < dark.length; i++) dark[i] = b.dark[i] ^ 1;
  return { ...b, dark };
}
