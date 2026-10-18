// Tools for reading bytes by hand, as a hex editor has them: what the bytes
// at one place mean as numbers, a date and text; the SHA-256 of the file and
// of the part chosen; and how random each stretch of the file is — low where
// a byte repeats, high where data is compressed or encrypted. Loaded when the
// panel under the bytes is first opened, so a page that never opens it does
// not pay for it.
import { el } from "./dom";
import { currentLocale, translateText } from "./i18n";
import type { FileModel } from "./model";

/** Rows of what the bytes at an offset read as: a name, then little- and big-endian. */
export type ValueRow = [name: string, little: string, big: string];

const hex = (n: number, digits: number) => `0x${n.toString(16).toUpperCase().padStart(digits, "0")}`;

/** A float as a person would write it: no long tail of digits, NaN and infinity named. */
function float(x: number): string {
  if (Number.isNaN(x)) return "NaN";
  if (!Number.isFinite(x)) return x > 0 ? "∞" : "−∞";
  return Math.abs(x) >= 1e-6 && Math.abs(x) < 1e15 ? String(Number(x.toPrecision(9))) : x.toExponential(6);
}

/** Seconds since 1970 as a date, when it falls where a file's date could: 1980 to 2100. */
function unixTime(seconds: number): string {
  if (seconds < 315_532_800 || seconds > 4_102_444_800) return "—";
  return new Date(seconds * 1000).toISOString().replace("T", " ").replace(".000Z", " UTC");
}

/** What the bytes from `offset` read as, as far as there are bytes for each width. */
export function valuesAt(bytes: Uint8Array, offset: number): ValueRow[] {
  const left = bytes.length - offset;
  if (offset < 0 || left <= 0) return [];
  const view = new DataView(bytes.buffer, bytes.byteOffset + offset, Math.min(left, 8));
  const both = (read: (le: boolean) => string): [string, string] => [read(true), read(false)];
  const rows: ValueRow[] = [];
  const b = view.getUint8(0);
  rows.push(["8-bit", `${b} · ${view.getInt8(0)} · ${hex(b, 2)}`, ""]);
  if (left >= 2) rows.push(["16-bit", ...both((le) => `${view.getUint16(0, le)} · ${view.getInt16(0, le)}`)]);
  if (left >= 4) {
    rows.push(["32-bit", ...both((le) => `${view.getUint32(0, le)} · ${view.getInt32(0, le)}`)]);
    rows.push(["32-bit float", ...both((le) => float(view.getFloat32(0, le)))]);
    rows.push(["Unix time", ...both((le) => unixTime(view.getUint32(0, le)))]);
  }
  if (left >= 8) {
    rows.push(["64-bit", ...both((le) => `${view.getBigUint64(0, le)}`)]);
    rows.push(["64-bit float", ...both((le) => float(view.getFloat64(0, le)))]);
  }
  rows.push(["Bits", b.toString(2).padStart(8, "0"), ""]);
  // The character these bytes start, if they start one in UTF-8.
  const text = new TextDecoder("utf-8", { fatal: false }).decode(bytes.subarray(offset, offset + 4));
  const first = [...text][0] ?? "";
  rows.push(["UTF-8", first && first !== "�" && first >= " " ? `“${first}” U+${first.codePointAt(0)!.toString(16).toUpperCase().padStart(4, "0")}` : "—", ""]);
  return rows;
}

/** Bytes read in each stretch, at most: past this a stretch is judged by its start. */
const SAMPLE = 64 * 1024;

/** Shannon entropy, 0 to 8 bits a byte, of `blocks` equal stretches of the file. */
export function entropy(bytes: Uint8Array, blocks: number): Float32Array {
  const n = Math.max(1, Math.min(blocks, bytes.length));
  const out = new Float32Array(n);
  const size = Math.ceil(bytes.length / n);
  const counts = new Uint32Array(256);
  for (let k = 0; k < n; k++) {
    const start = k * size;
    const end = Math.min(bytes.length, start + size, start + SAMPLE);
    if (end <= start) break;
    counts.fill(0);
    for (let i = start; i < end; i++) counts[bytes[i]]++;
    let h = 0;
    const total = end - start;
    for (const c of counts) {
      if (c === 0) continue;
      const p = c / total;
      h -= p * Math.log2(p);
    }
    out[k] = h;
  }
  return out;
}

const t = (s: string) => translateText(s, currentLocale());

/** SHA-256 as hex, worked out by the browser. */
async function sha256(bytes: Uint8Array): Promise<string> {
  const digest = new Uint8Array(await crypto.subtle.digest("SHA-256", bytes as Uint8Array<ArrayBuffer>));
  return [...digest].map((b) => b.toString(16).padStart(2, "0")).join("");
}

export interface ByteToolHooks {
  /** Goes to an offset in the bytes. */
  jump(offset: number): void;
}

/** The panel's content: values at the cursor, hashes, entropy along the file. */
export class ByteTools {
  private model: FileModel | null = null;
  private offset = -1;
  private part: { start: number; len: number; label: string } | null = null;
  private view: [number, number] = [0, 0];
  private levels: Float32Array | null = null;
  private readonly values = el("div", "bytetools-values");
  private readonly hashes = el("div", "bytetools-hashes");
  private readonly strip = el("canvas", "bytetools-entropy");

  constructor(host: HTMLElement, private readonly hooks: ByteToolHooks) {
    const entropyBlock = el("div", "bytetools-entropy-block");
    entropyBlock.append(
      el("h3", undefined, "Entropy along the file"),
      this.strip,
      el("p", "hint", "Low where bytes repeat, high where data is compressed or encrypted. Click to go there."),
    );
    this.strip.setAttribute("role", "img");
    this.strip.addEventListener("click", (e) => {
      if (!this.model) return;
      const r = this.strip.getBoundingClientRect();
      this.hooks.jump(Math.floor(((e.clientX - r.left) / r.width) * this.model.bytes.length));
    });
    new ResizeObserver(() => this.drawStrip()).observe(this.strip);
    host.append(this.values, this.hashes, entropyBlock);
  }

  setModel(model: FileModel | null): void {
    if (model === this.model) return;
    this.model = model;
    this.offset = model ? 0 : -1;
    this.part = null;
    this.levels = null;
    this.render();
  }

  setOffset(offset: number): void {
    if (offset === this.offset || offset < 0) return;
    this.offset = offset;
    this.renderValues();
  }

  setPart(start: number, len: number, label: string): void {
    this.part = len > 0 ? { start, len, label } : null;
    this.renderHashes();
  }

  setView(start: number, end: number): void {
    this.view = [start, end];
    this.drawStrip();
  }

  private render(): void {
    this.renderValues();
    this.renderHashes();
    this.drawStrip();
  }

  private renderValues(): void {
    const m = this.model;
    this.values.replaceChildren();
    if (!m || this.offset < 0) return;
    this.values.append(el("h3", undefined, `${t("At")} ${hex(this.offset, 8)}`));
    const table = el("table", "bytetools-table");
    const head = el("tr");
    head.append(el("th"), el("th", undefined, t("little-endian")), el("th", undefined, t("big-endian")));
    table.append(head);
    for (const [name, little, big] of valuesAt(m.bytes, this.offset)) {
      const row = el("tr");
      const value = el("td", "mono", little);
      row.append(el("th", undefined, t(name)), value);
      if (big) row.append(el("td", "mono", big));
      else value.colSpan = 2;
      table.append(row);
    }
    this.values.append(table, el("p", "hint", "Whole numbers: unsigned · signed."));
  }

  private renderHashes(): void {
    const m = this.model;
    this.hashes.replaceChildren();
    if (!m) return;
    this.hashes.append(el("h3", undefined, "SHA-256"));
    const missing = Boolean(m.missing);
    const row = (label: string, bytes: () => Uint8Array) => {
      const line = el("div", "bytetools-hash");
      const button = el("button", "btn btn-small", t("Work it out"));
      button.type = "button";
      button.disabled = missing;
      const out = el("code", "mono");
      button.addEventListener("click", async () => {
        button.disabled = true;
        out.textContent = t("Working…");
        out.textContent = await sha256(bytes());
        button.replaceWith(copy(out));
      });
      line.append(el("span", undefined, label), button, out);
      this.hashes.append(line);
    };
    row(t("The whole file"), () => m.bytes);
    const part = this.part;
    if (part && !(part.start === 0 && part.len === m.bytes.length)) {
      row(`${t(part.label)} · ${t(`${part.len.toLocaleString("en")} bytes`)}`, () => m.bytes.subarray(part.start, part.start + part.len));
    }
    if (missing) this.hashes.append(el("p", "hint", "A large movie's media is read only where it is shown, so it has no hash here."));
  }

  private drawStrip(): void {
    const m = this.model;
    const canvas = this.strip;
    const ctx = canvas.getContext("2d");
    const w = canvas.clientWidth;
    const h = canvas.clientHeight;
    if (!ctx || !m || w === 0 || m.missing) return;
    const ratio = devicePixelRatio || 1;
    canvas.width = Math.round(w * ratio);
    canvas.height = Math.round(h * ratio);
    ctx.scale(ratio, ratio);
    this.levels ??= entropy(m.bytes, 400);
    const css = getComputedStyle(document.documentElement);
    const accent = css.getPropertyValue("--accent").trim() || "#2563eb";
    const muted = css.getPropertyValue("--border").trim() || "#888";
    ctx.clearRect(0, 0, w, h);
    const levels = this.levels;
    const bar = w / levels.length;
    for (let i = 0; i < levels.length; i++) {
      const level = levels[i] / 8;
      ctx.fillStyle = level > 0.9 ? accent : muted;
      ctx.globalAlpha = 0.35 + 0.65 * level;
      const bh = Math.max(1, level * h);
      ctx.fillRect(i * bar, h - bh, Math.max(1, bar), bh);
    }
    ctx.globalAlpha = 1;
    // Where the bytes on screen are.
    const [start, end] = this.view;
    if (end > start) {
      ctx.strokeStyle = css.getPropertyValue("--text").trim() || "#000";
      const x = (start / m.bytes.length) * w;
      ctx.strokeRect(x + 0.5, 0.5, Math.max(2, ((end - start) / m.bytes.length) * w) - 1, h - 1);
    }
    const mean = levels.reduce((a, b) => a + b, 0) / levels.length;
    canvas.setAttribute("aria-label", `${t("Entropy along the file")}: ${mean.toFixed(1)} / 8`);
  }
}

/** A button that copies the text beside it. */
function copy(of: HTMLElement): HTMLButtonElement {
  const button = el("button", "btn btn-small", t("Copy"));
  button.type = "button";
  button.addEventListener("click", async () => {
    await navigator.clipboard.writeText(of.textContent ?? "");
    button.textContent = t("Copied");
  });
  return button;
}
