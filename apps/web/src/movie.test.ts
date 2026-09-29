// A large movie read in parts, against the same movie read whole: the same
// answer, the same clean copy, and the media never read. With the real
// parser, as `logic.test.ts` loads it.
import { readFileSync } from "node:fs";
import { beforeAll, describe as group, expect, it } from "vitest";
import { cleanCopy, cleanMovie, entropy, initSync, isMovie, parse, parseMovie } from "./wasm/hexscope_wasm.js";
import { describe } from "./describe";
import { assemble, movieEntropy, readMovie } from "./movie";
import { Missing } from "./missing";
import { StoredZip } from "./zipwrite";

beforeAll(() => {
  initSync({ module: readFileSync(new URL("./wasm/hexscope_wasm_bg.wasm", import.meta.url)) });
});

/** The sample video with its media grown to `size` bytes, as a file. */
function longVideo(size: number): { file: Blob; bytes: Uint8Array } {
  const src = new Uint8Array(readFileSync(new URL("../public/samples/video.mov", import.meta.url)));
  const view = new DataView(src.buffer);
  const parts: Uint8Array[] = [];
  for (let at = 0; at + 8 <= src.length; ) {
    const len = view.getUint32(at);
    if (String.fromCharCode(...src.subarray(at + 4, at + 8)) === "mdat") {
      const box = new Uint8Array(8 + size);
      new DataView(box.buffer).setUint32(0, 8 + size);
      box.set(src.subarray(at + 4, at + len), 4);
      for (let i = len; i < box.length; i += 1009) box[i] = i & 0xff;
      parts.push(box);
    } else parts.push(src.subarray(at, at + len));
    at += len;
  }
  const bytes = new Uint8Array(parts.reduce((n, p) => n + p.length, 0));
  let o = 0;
  for (const p of parts) {
    bytes.set(p, o);
    o += p.length;
  }
  return { file: new Blob([bytes as BlobPart]), bytes };
}

/** A Blob that fails a test if anything reads between `from` and `to`. */
function guarded(bytes: Uint8Array, from: number, to: number): Blob {
  const blob = new Blob([bytes as BlobPart]);
  const slice = blob.slice.bind(blob);
  return Object.assign(blob, {
    slice(start = 0, end = bytes.length) {
      if (start < to && end > from) throw new Error(`read ${start}–${end}, inside the media`);
      return slice(start, end);
    },
  });
}

group("a movie read in parts", () => {
  it("gives the answer the whole movie gives, without reading its media", async () => {
    const { bytes } = longVideo(4 << 20);
    const mediaStart = 36;
    const file = guarded(bytes, mediaStart + 64 * 1024, bytes.length - 1389);
    const movie = await readMovie(file, isMovie, 0);
    expect(movie).not.toBeNull();
    expect(movie!.given.length).toBeLessThan(2000);
    expect(Array.from(movie!.missing)).toEqual([mediaStart, 4 << 20]);

    const whole = describe(parse(bytes));
    const parts = describe(parseMovie(movie!.given, movie!.gaps));
    expect(parts.labels).toEqual(whole.labels);
    expect(parts.values).toEqual(whole.values);
    expect(Array.from(parts.starts)).toEqual(Array.from(whole.starts));
    expect(Array.from(parts.lens)).toEqual(Array.from(whole.lens));
    expect(parts.location).toEqual(whole.location);
  });

  it("makes the clean copy the whole movie makes", async () => {
    const { file, bytes } = longVideo(3 << 20);
    const movie = (await readMovie(file, isMovie, 0))!;
    const c = cleanMovie(movie.given, movie.gaps);
    expect(c.error).toBe("");
    const copy = new Uint8Array(await assemble(c.bytes, movie, file).arrayBuffer());
    const whole = cleanCopy(bytes, false).bytes;
    expect(copy.length).toBe(whole.length);
    expect(Buffer.compare(copy, whole)).toBe(0);
  });

  it("maps its media by samples of it", async () => {
    const { file } = longVideo(4 << 20);
    const movie = (await readMovie(file, isMovie, 0))!;
    const e = await movieEntropy(file, movie, 1024, (b) => entropy(b, 1)[0]);
    expect(e.values.length).toBe(1024);
    expect(e.window * 1024).toBeGreaterThanOrEqual(file.size);
    expect(e.values.every((v) => v >= 0 && v <= 8)).toBe(true);
  });

  it("leaves small movies and other files whole", async () => {
    const { file } = longVideo(4 << 20);
    expect(await readMovie(file, isMovie)).toBeNull();
    const photo = new Blob([readFileSync(new URL("../public/samples/photo.jpg", import.meta.url))]);
    expect(await readMovie(photo, isMovie, 0)).toBeNull();
  });
});

group("bytes not read yet", () => {
  it("are loaded a chunk at a time, and searched around", () => {
    const m = new Missing(Float64Array.from([100, 200_000]));
    expect(m.has(50)).toBe(true);
    expect(m.has(150)).toBe(false);
    const wanted = m.wanted(150, 70_000);
    expect(wanted).toEqual([
      [100, 65536],
      [65536, 131072],
    ]);
    m.filled(wanted);
    expect(m.has(150) && m.has(131_071)).toBe(true);
    expect(m.has(131_072)).toBe(false);
    expect(m.read(300_000)).toEqual([
      [0, 100],
      [200_100, 300_000],
    ]);
  });
});

group("an archive of clean copies", () => {
  it("is a ZIP whose entries check out", async () => {
    const zip = new StoredZip();
    await zip.add("a.txt", new Blob(["hello"]));
    await zip.add("a.txt", new Blob(["world"]));
    const bytes = new Uint8Array(await zip.finish().arrayBuffer());
    const view = new DataView(bytes.buffer);
    // CRC-32 of "hello", and the second name made unique.
    expect(view.getUint32(14, true)).toBe(0x3610a686);
    expect(new TextDecoder().decode(bytes)).toContain("a (2).txt");
  });
});
