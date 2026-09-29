// Reed–Solomon error correction over GF(256) with QR's field polynomial,
// x⁸ + x⁴ + x³ + x² + 1, and generator roots α⁰ upward. Polynomials are
// arrays of coefficients, the highest degree first.

const EXP = new Uint8Array(512);
const LOG = new Uint8Array(256);
{
  let x = 1;
  for (let i = 0; i < 255; i++) {
    EXP[i] = x;
    LOG[x] = i;
    x <<= 1;
    if (x & 0x100) x ^= 0x11d;
  }
  for (let i = 255; i < 512; i++) EXP[i] = EXP[i - 255];
}

const mul = (a: number, b: number) => (a === 0 || b === 0 ? 0 : EXP[LOG[a] + LOG[b]]);
const inv = (a: number) => EXP[255 - LOG[a]];

type Poly = number[];

function trim(p: Poly): Poly {
  let i = 0;
  while (i < p.length - 1 && p[i] === 0) i++;
  return i === 0 ? p : p.slice(i);
}

const degree = (p: Poly) => p.length - 1;
const lead = (p: Poly) => p[0];
const isZero = (p: Poly) => p[0] === 0;

function evaluate(p: Poly, x: number): number {
  if (x === 0) return p[p.length - 1];
  let r = 0;
  for (const c of p) r = mul(r, x) ^ c;
  return r;
}

function add(a: Poly, b: Poly): Poly {
  if (a.length < b.length) [a, b] = [b, a];
  const out = a.slice();
  const shift = a.length - b.length;
  for (let i = 0; i < b.length; i++) out[i + shift] ^= b[i];
  return trim(out);
}

function times(a: Poly, b: Poly): Poly {
  if (isZero(a) || isZero(b)) return [0];
  const out = new Array<number>(a.length + b.length - 1).fill(0);
  for (let i = 0; i < a.length; i++) for (let j = 0; j < b.length; j++) out[i + j] ^= mul(a[i], b[j]);
  return trim(out);
}

function scaled(p: Poly, by: number, shift = 0): Poly {
  if (by === 0) return [0];
  const out = new Array<number>(p.length + shift).fill(0);
  for (let i = 0; i < p.length; i++) out[i] = mul(p[i], by);
  return trim(out);
}

/**
 * Corrects `block`, data then `ecc` check bytes, in place. False when it
 * has more errors than its check bytes can mend.
 */
export function correct(block: Uint8Array, ecc: number): boolean {
  const received = Array.from(block);
  const syndrome = new Array<number>(ecc);
  let clean = true;
  for (let i = 0; i < ecc; i++) {
    const s = evaluate(received, EXP[i]);
    syndrome[ecc - 1 - i] = s;
    if (s !== 0) clean = false;
  }
  if (clean) return true;

  // The Euclidean algorithm gives the error locator and evaluator.
  let rLast: Poly = [1, ...new Array<number>(ecc).fill(0)];
  let r: Poly = trim(syndrome);
  let tLast: Poly = [0];
  let t: Poly = [1];
  if (degree(rLast) < degree(r)) [rLast, r] = [r, rLast];
  while (2 * degree(r) >= ecc) {
    const rLastLast = rLast;
    const tLastLast = tLast;
    rLast = r;
    tLast = t;
    if (isZero(rLast)) return false;
    r = rLastLast;
    let q: Poly = [0];
    const leadInverse = inv(lead(rLast));
    while (degree(r) >= degree(rLast) && !isZero(r)) {
      const shift = degree(r) - degree(rLast);
      const scale = mul(lead(r), leadInverse);
      q = add(q, scaled([1], scale, shift));
      r = add(r, scaled(rLast, scale, shift));
    }
    t = add(times(q, tLast), tLastLast);
    if (degree(r) >= degree(rLast)) return false;
  }
  const atZero = t[t.length - 1];
  if (atZero === 0) return false;
  const sigma = scaled(t, inv(atZero));
  const omega = scaled(r, inv(atZero));

  // Where the errors are: the locator's roots, found by trying every element.
  const errors = degree(sigma);
  const locations: number[] = [];
  if (errors === 1) locations.push(sigma[0]);
  else for (let i = 1; i < 256 && locations.length < errors; i++) if (evaluate(sigma, i) === 0) locations.push(inv(i));
  if (locations.length !== errors) return false;

  // And how large: Forney's formula.
  for (let i = 0; i < locations.length; i++) {
    const xInverse = inv(locations[i]);
    let denominator = 1;
    for (let j = 0; j < locations.length; j++) {
      if (i !== j) denominator = mul(denominator, 1 ^ mul(locations[j], xInverse));
    }
    const magnitude = mul(evaluate(omega, xInverse), inv(denominator));
    const position = block.length - 1 - LOG[locations[i]];
    if (position < 0) return false;
    block[position] ^= magnitude;
  }
  return true;
}
