const pngSignature = [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];

function starts(bytes: Uint8Array, signature: readonly number[]): boolean {
  return signature.every((value, index) => bytes[index] === value);
}

function be16(bytes: Uint8Array, at: number): number {
  return (bytes[at]! << 8) | bytes[at + 1]!;
}

function be32(bytes: Uint8Array, at: number): number {
  return ((bytes[at]! * 0x1000000) + (bytes[at + 1]! << 16) + (bytes[at + 2]! << 8) + bytes[at + 3]!) >>> 0;
}

function le16(bytes: Uint8Array, at: number): number {
  return bytes[at]! | (bytes[at + 1]! << 8);
}

function le24(bytes: Uint8Array, at: number): number {
  return bytes[at]! | (bytes[at + 1]! << 8) | (bytes[at + 2]! << 16);
}

function le32(bytes: Uint8Array, at: number): number {
  return (bytes[at]! | (bytes[at + 1]! << 8) | (bytes[at + 2]! << 16) | (bytes[at + 3]! << 24)) >>> 0;
}

function dimensions(width: number, height: number): [number, number] | undefined {
  return Number.isSafeInteger(width) && Number.isSafeInteger(height) && width > 0 && height > 0
    ? [width, height]
    : undefined;
}

function png(bytes: Uint8Array): [number, number] | undefined {
  if (bytes.length < 24 || !starts(bytes, pngSignature) || be32(bytes, 8) !== 13 || !starts(bytes.subarray(12, 16), [0x49, 0x48, 0x44, 0x52])) return undefined;
  return dimensions(be32(bytes, 16), be32(bytes, 20));
}

function isStartOfFrame(marker: number): boolean {
  return marker >= 0xc0 && marker <= 0xcf && marker !== 0xc4 && marker !== 0xc8 && marker !== 0xcc;
}

function jpeg(bytes: Uint8Array): [number, number] | undefined {
  if (!starts(bytes, [0xff, 0xd8])) return undefined;
  let at = 2;
  while (at < bytes.length) {
    if (bytes[at] !== 0xff) return undefined;
    while (bytes[at] === 0xff) at++;
    if (at >= bytes.length) return undefined;
    const marker = bytes[at++]!;
    if (marker === 0xd8 || marker === 0xd9 || marker === 0xda || marker === 0x00) return undefined;
    if (marker === 0x01 || (marker >= 0xd0 && marker <= 0xd7)) continue;
    if (at + 2 > bytes.length) return undefined;
    const length = be16(bytes, at);
    const end = at + length;
    if (length < 2 || end > bytes.length) return undefined;
    if (isStartOfFrame(marker)) {
      if (length < 8) return undefined;
      const height = be16(bytes, at + 3);
      const width = be16(bytes, at + 5);
      const components = bytes[at + 7]!;
      if (components === 0 || length < 8 + components * 3) return undefined;
      return dimensions(width, height);
    }
    at = end;
  }
  return undefined;
}

function gif(bytes: Uint8Array): [number, number] | undefined {
  if (bytes.length < 10 || !(starts(bytes, [0x47, 0x49, 0x46, 0x38, 0x37, 0x61]) || starts(bytes, [0x47, 0x49, 0x46, 0x38, 0x39, 0x61]))) return undefined;
  return dimensions(le16(bytes, 6), le16(bytes, 8));
}

function webp(bytes: Uint8Array): [number, number] | undefined {
  if (bytes.length < 20 || !starts(bytes, [0x52, 0x49, 0x46, 0x46]) || !starts(bytes.subarray(8, 12), [0x57, 0x45, 0x42, 0x50])) return undefined;
  const riffEnd = 8 + le32(bytes, 4);
  const chunkLength = le32(bytes, 16);
  const chunkEnd = 20 + chunkLength + (chunkLength & 1);
  if (riffEnd < 20 || riffEnd > bytes.length || chunkEnd > riffEnd) return undefined;

  if (starts(bytes.subarray(12, 16), [0x56, 0x50, 0x38, 0x58])) {
    if (chunkLength < 10) return undefined;
    return dimensions(le24(bytes, 24) + 1, le24(bytes, 27) + 1);
  }
  if (starts(bytes.subarray(12, 16), [0x56, 0x50, 0x38, 0x4c])) {
    if (chunkLength < 5 || bytes[20] !== 0x2f) return undefined;
    const width = 1 + bytes[21]! + ((bytes[22]! & 0x3f) << 8);
    const height = 1 + (bytes[22]! >>> 6) + (bytes[23]! << 2) + ((bytes[24]! & 0x0f) << 10);
    return dimensions(width, height);
  }
  if (starts(bytes.subarray(12, 16), [0x56, 0x50, 0x38, 0x20])) {
    if (chunkLength < 10 || (bytes[20]! & 1) !== 0 || !starts(bytes.subarray(23, 26), [0x9d, 0x01, 0x2a])) return undefined;
    return dimensions(le16(bytes, 26) & 0x3fff, le16(bytes, 28) & 0x3fff);
  }
  return undefined;
}

function bmp(bytes: Uint8Array): [number, number] | undefined {
  if (bytes.length < 26 || bytes[0] !== 0x42 || bytes[1] !== 0x4d) return undefined;
  const headerLength = le32(bytes, 14);
  if (headerLength === 12) return dimensions(le16(bytes, 18), le16(bytes, 20));
  if (headerLength < 40 || bytes.length < 54) return undefined;
  const width = le32(bytes, 18) | 0;
  const signedHeight = le32(bytes, 22) | 0;
  if (signedHeight === -0x80000000) return undefined;
  return dimensions(width, Math.abs(signedHeight));
}

/** Reads raster dimensions from headers only; unsupported or malformed inputs fall back to ordinary browser decoding. */
export function imageDimensions(bytes: Uint8Array): [number, number] | undefined {
  return png(bytes) ?? jpeg(bytes) ?? gif(bytes) ?? webp(bytes) ?? bmp(bytes);
}
