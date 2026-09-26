// Find in the file: text, or bytes written in hex. Each match is marked in
// the bytes, and the part of the file it falls in is selected, so a string
// found is also a string explained.

/** Matches found, at most: past this the count says "10,000+". */
const MAX_MATCHES = 10_000;

export interface SearchHooks {
  bytes(): Uint8Array | null;
  /** Shows one match; `-1` clears. */
  show(start: number, end: number): void;
}

/**
 * The bytes a query stands for. `FF D8`, `ff:d8` or `0xFFD8` are bytes;
 * anything else is text, as UTF-8.
 */
export function parseQuery(q: string): { bytes: Uint8Array; hex: boolean } | null {
  const t = q.trim();
  if (!t) return null;
  const spaced = /^[0-9a-f]{2}([\s:][0-9a-f]{2})+$/i.test(t);
  const prefixed = /^0x([0-9a-f]{2})+$/i.test(t);
  if (spaced || prefixed) {
    const digits = t.replace(/^0x/i, "").replace(/[\s:]/g, "");
    const out = new Uint8Array(digits.length / 2);
    for (let i = 0; i < out.length; i++) out[i] = parseInt(digits.slice(i * 2, i * 2 + 2), 16);
    return { bytes: out, hex: true };
  }
  return { bytes: new TextEncoder().encode(q), hex: false };
}

const lower = (b: number) => (b >= 0x41 && b <= 0x5a ? b + 32 : b);

/** Where the needle occurs, up to the limit. Text matches ignore ASCII case. */
export function findAll(hay: Uint8Array, needle: Uint8Array, foldCase: boolean): number[] {
  const out: number[] = [];
  const n = needle.length;
  if (n === 0 || n > hay.length) return out;
  const first = foldCase ? lower(needle[0]) : needle[0];
  outer: for (let i = 0; i + n <= hay.length && out.length < MAX_MATCHES; i++) {
    if ((foldCase ? lower(hay[i]) : hay[i]) !== first) continue;
    for (let k = 1; k < n; k++) {
      if ((foldCase ? lower(hay[i + k]) : hay[i + k]) !== (foldCase ? lower(needle[k]) : needle[k])) continue outer;
    }
    out.push(i);
  }
  return out;
}

export class SearchBar {
  private readonly root = document.createElement("form");
  private readonly input = document.createElement("input");
  private readonly count = document.createElement("span");
  private matches: number[] = [];
  private length = 0;
  private at = -1;

  constructor(host: HTMLElement, private readonly hooks: SearchHooks) {
    this.root.className = "search";
    this.root.hidden = true;
    this.root.setAttribute("role", "search");
    this.input.type = "search";
    this.input.placeholder = "Find text, or hex: FF D8";
    this.input.setAttribute("aria-label", "Find in the file: text, or bytes in hex");
    this.input.spellcheck = false;
    this.count.className = "search-count";
    this.count.setAttribute("aria-live", "polite");
    const button = (label: string, title: string, fn: () => void) => {
      const b = document.createElement("button");
      b.type = "button";
      b.className = "search-step";
      b.textContent = label;
      b.title = title;
      b.addEventListener("click", fn);
      return b;
    };
    this.root.append(
      this.input,
      this.count,
      button("↑", "Previous (Shift+Enter)", () => this.step(-1)),
      button("↓", "Next (Enter)", () => this.step(1)),
      button("✕", "Close (Esc)", () => this.close()),
    );
    this.root.addEventListener("submit", (e) => e.preventDefault());
    this.input.addEventListener("input", () => this.run());
    this.input.addEventListener("keydown", (e) => {
      if (e.key === "Enter") {
        e.preventDefault();
        this.step(e.shiftKey ? -1 : 1);
      } else if (e.key === "Escape") {
        e.preventDefault();
        this.close();
      }
    });
    host.append(this.root);
  }

  get isOpen(): boolean {
    return !this.root.hidden;
  }

  open(): void {
    this.root.hidden = false;
    this.input.focus();
    this.input.select();
    if (this.input.value) this.run();
  }

  close(): void {
    this.root.hidden = true;
    this.hooks.show(-1, -1);
  }

  /** A new file: the old matches mean nothing. */
  reset(): void {
    this.matches = [];
    this.at = -1;
    this.count.textContent = "";
    if (this.isOpen) this.run();
  }

  private run(): void {
    const bytes = this.hooks.bytes();
    const q = parseQuery(this.input.value);
    if (!bytes || !q) {
      this.matches = [];
      this.count.textContent = "";
      this.hooks.show(-1, -1);
      return;
    }
    this.matches = findAll(bytes, q.bytes, !q.hex);
    this.length = q.bytes.length;
    this.at = -1;
    if (this.matches.length === 0) {
      this.count.textContent = "not found";
      this.hooks.show(-1, -1);
      return;
    }
    this.step(1);
  }

  private step(by: number): void {
    const n = this.matches.length;
    if (n === 0) return;
    this.at = (this.at + by + n) % n;
    const start = this.matches[this.at];
    const more = n >= MAX_MATCHES ? "+" : "";
    this.count.textContent = `${(this.at + 1).toLocaleString("en")} of ${n.toLocaleString("en")}${more}`;
    this.hooks.show(start, start + this.length);
  }
}
