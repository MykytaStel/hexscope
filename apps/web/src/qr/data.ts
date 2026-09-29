// A QR code's data codewords as text: segments of digits, letters, bytes
// or kanji, each after a mode and a count, until a terminator.

const ALNUM = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ $%*+-./:";

/** Character sets an ECI designator can name, as TextDecoder labels. */
const ECI: Record<number, string> = {
  1: "iso-8859-1", 3: "iso-8859-1", 4: "iso-8859-2", 5: "iso-8859-3", 6: "iso-8859-4", 7: "iso-8859-5",
  9: "iso-8859-7", 13: "iso-8859-11", 15: "iso-8859-13", 17: "iso-8859-15", 20: "shift_jis",
  21: "windows-1250", 22: "windows-1251", 23: "windows-1252", 24: "windows-1256", 25: "utf-16be", 26: "utf-8", 29: "gbk", 30: "euc-kr",
};

class Bits {
  private at = 0;
  constructor(private readonly bytes: Uint8Array) {}

  get left(): number {
    return this.bytes.length * 8 - this.at;
  }

  read(n: number): number {
    let v = 0;
    for (let i = 0; i < n; i++, this.at++) v = (v << 1) | ((this.bytes[this.at >> 3] >> (7 - (this.at & 7))) & 1);
    return v;
  }
}

/** Bits of a segment's character count, by mode and version. */
function countBits(mode: number, version: number): number {
  const tier = version <= 9 ? 0 : version <= 26 ? 1 : 2;
  const table: Record<number, number[]> = { 1: [10, 12, 14], 2: [9, 11, 13], 4: [8, 16, 16], 8: [8, 10, 12] };
  return table[mode][tier];
}

/** Bytes as text: in the character set named, else UTF-8 when they are, else Latin-1. */
function text(bytes: Uint8Array, charset: string | null): string {
  if (charset) {
    try {
      return new TextDecoder(charset).decode(bytes);
    } catch {
      // A character set this browser does not know: guessed below.
    }
  }
  try {
    return new TextDecoder("utf-8", { fatal: true }).decode(bytes);
  } catch {
    return new TextDecoder("iso-8859-1").decode(bytes);
  }
}

/** The text the data codewords hold, or null when they do not parse. */
export function decodeData(data: Uint8Array, version: number): string | null {
  const bits = new Bits(data);
  let out = "";
  let charset: string | null = null;
  while (bits.left >= 4) {
    const mode = bits.read(4);
    if (mode === 0) break;
    if (mode === 7) {
      const first = bits.read(8);
      const value = (first & 0x80) === 0 ? first : (first & 0xc0) === 0x80 ? ((first & 0x3f) << 8) | bits.read(8) : ((first & 0x1f) << 16) | bits.read(16);
      charset = ECI[value] ?? null;
      continue;
    }
    // Structured append: which part of a set this is, then its parity.
    if (mode === 3) {
      bits.read(16);
      continue;
    }
    // FNC1: marks GS1 or an application's format, and holds no text.
    if (mode === 5) continue;
    if (mode === 9) {
      bits.read(8);
      continue;
    }
    if (![1, 2, 4, 8].includes(mode)) return null;
    const count = bits.read(countBits(mode, version));
    if (mode === 1) {
      let n = count;
      for (; n >= 3; n -= 3) {
        if (bits.left < 10) return null;
        const v = bits.read(10);
        if (v > 999) return null;
        out += v.toString().padStart(3, "0");
      }
      if (n > 0) {
        const width = n === 2 ? 7 : 4;
        if (bits.left < width) return null;
        const v = bits.read(width);
        if (v >= 10 ** n) return null;
        out += v.toString().padStart(n, "0");
      }
    } else if (mode === 2) {
      let n = count;
      for (; n >= 2; n -= 2) {
        if (bits.left < 11) return null;
        const v = bits.read(11);
        if (v >= 45 * 45) return null;
        out += ALNUM[Math.floor(v / 45)] + ALNUM[v % 45];
      }
      if (n === 1) {
        if (bits.left < 6) return null;
        const v = bits.read(6);
        if (v >= 45) return null;
        out += ALNUM[v];
      }
    } else if (mode === 4) {
      if (bits.left < count * 8) return null;
      const bytes = new Uint8Array(count);
      for (let i = 0; i < count; i++) bytes[i] = bits.read(8);
      out += text(bytes, charset);
    } else {
      // Kanji: 13 bits each, back to their two Shift JIS bytes.
      if (bits.left < count * 13) return null;
      const bytes = new Uint8Array(count * 2);
      for (let i = 0; i < count; i++) {
        const v = bits.read(13);
        let sjis = ((Math.floor(v / 0xc0) << 8) | v % 0xc0) >>> 0;
        sjis += sjis < 0x1f00 ? 0x8140 : 0xc140;
        bytes[2 * i] = sjis >> 8;
        bytes[2 * i + 1] = sjis & 0xff;
      }
      out += text(bytes, "shift_jis");
    }
  }
  return out;
}
