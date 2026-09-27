// A ZIP of files stored as they are, for saving many clean copies as one
// download. Pictures, videos and PDFs are compressed already, so nothing
// is deflated. Every entry gets the same date, 1 January 1980: the archive
// says nothing about when it was made, or by whom.

const CRC_TABLE = (() => {
  const t = new Uint32Array(256);
  for (let n = 0; n < 256; n++) {
    let c = n;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    t[n] = c >>> 0;
  }
  return t;
})();

const DOS_DATE = (0 << 9) | (1 << 5) | 1; // 1980-01-01

export function crc32(bytes: Uint8Array): number {
  let c = 0xffffffff;
  for (let i = 0; i < bytes.length; i++) c = CRC_TABLE[(c ^ bytes[i]) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}

/**
 * A ZIP built one file at a time. Each file's bytes go into a Blob as soon
 * as they are added, so a folder of clean copies never has to fit in memory
 * at once: the browser may keep a large Blob on disk.
 */
export class StoredZip {
  private readonly parts: BlobPart[] = [];
  private readonly central: Uint8Array[] = [];
  private readonly seen = new Set<string>();
  private offset = 0;
  private count = 0;

  /** Adds a file. A second `photo.jpg` becomes `photo (2).jpg`. Throws past what a plain ZIP can say (4 GB, 65,535 entries). */
  add(fileName: string, bytes: Uint8Array): void {
    if (this.count >= 0xffff) throw new Error("too many files for one ZIP");
    let unique = fileName;
    const dot = fileName.lastIndexOf(".");
    const [stem, ext] = dot > 0 ? [fileName.slice(0, dot), fileName.slice(dot)] : [fileName, ""];
    for (let n = 2; this.seen.has(unique.toLowerCase()); n++) unique = `${stem} (${n})${ext}`;
    this.seen.add(unique.toLowerCase());

    const name = new TextEncoder().encode(unique);
    const crc = crc32(bytes);
    const size = bytes.length;
    if (this.offset + 30 + name.length + size > 0xffffffff) throw new Error("too large for one ZIP");
    const local = new DataView(new ArrayBuffer(30));
    local.setUint32(0, 0x04034b50, true);
    local.setUint16(4, 20, true);
    local.setUint16(6, 1 << 11, true); // names are UTF-8
    local.setUint16(8, 0, true); // stored
    local.setUint16(10, 0, true);
    local.setUint16(12, DOS_DATE, true);
    local.setUint32(14, crc, true);
    local.setUint32(18, size, true);
    local.setUint32(22, size, true);
    local.setUint16(26, name.length, true);
    local.setUint16(28, 0, true);
    this.parts.push(local.buffer, name as BlobPart, new Blob([bytes as BlobPart]));

    const cd = new DataView(new ArrayBuffer(46));
    cd.setUint32(0, 0x02014b50, true);
    cd.setUint16(4, 20, true);
    cd.setUint16(6, 20, true);
    cd.setUint16(8, 1 << 11, true);
    cd.setUint16(10, 0, true);
    cd.setUint16(12, 0, true);
    cd.setUint16(14, DOS_DATE, true);
    cd.setUint32(16, crc, true);
    cd.setUint32(20, size, true);
    cd.setUint32(24, size, true);
    cd.setUint16(28, name.length, true);
    cd.setUint32(42, this.offset, true);
    this.central.push(new Uint8Array(cd.buffer), name);
    this.offset += 30 + name.length + size;
    this.count++;
  }

  get size(): number {
    return this.count;
  }

  /** The archive, with its central directory at the end. */
  finish(): Blob {
    const cdSize = this.central.reduce((n, p) => n + p.length, 0);
    const end = new DataView(new ArrayBuffer(22));
    end.setUint32(0, 0x06054b50, true);
    end.setUint16(8, this.count, true);
    end.setUint16(10, this.count, true);
    end.setUint32(12, cdSize, true);
    end.setUint32(16, this.offset, true);
    return new Blob([...this.parts, ...(this.central as BlobPart[]), end.buffer], { type: "application/zip" });
  }
}
