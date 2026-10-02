// "Copy as": a part's bytes as hex, base64 or a C array.
import { announce } from "./announce";
import { el } from "./dom";

/** Bytes of a part copied at most: past this, it is a file, not a snippet. */
export const MAX_COPIED = 1024 * 1024;

/** A part's bytes as text to paste elsewhere: hex, base64, or a C array. */
export function bytesAs(bytes: Uint8Array, form: "hex" | "base64" | "c", label: string): string {
  const hex = (b: number) => b.toString(16).padStart(2, "0");
  if (form === "base64") {
    let bin = "";
    for (let i = 0; i < bytes.length; i += 0x8000) bin += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
    return btoa(bin);
  }
  const lines: string[] = [];
  const per = form === "hex" ? 16 : 12;
  for (let i = 0; i < bytes.length; i += per) {
    const row = [...bytes.subarray(i, i + per)];
    lines.push(form === "hex" ? row.map(hex).join(" ") : `  ${row.map((b) => `0x${hex(b)}`).join(", ")},`);
  }
  if (form === "hex") return lines.join("\n");
  const name = label.toLowerCase().replace(/[^a-z0-9]+/g, "_").replace(/^_|_$/g, "") || "part";
  return `const unsigned char ${/^[a-z_]/.test(name) ? name : `part_${name}`}[${bytes.length}] = {\n${lines.join("\n")}\n};`;
}

/** Buttons that copy a part's bytes, each said when done. */
export function copyBytes(bytes: Uint8Array, label: string): HTMLElement {
  const row = el("div", "copy-bytes");
  row.append(el("span", "hint", "Copy as"));
  for (const [form, name] of [["hex", "hex"], ["base64", "base64"], ["c", "a C array"]] as const) {
    const b = el("button", "link", name);
    b.addEventListener("click", async () => {
      try {
        await navigator.clipboard.writeText(bytesAs(bytes, form, label));
        announce(`Copied ${bytes.length === 1 ? "1 byte" : `${bytes.length.toLocaleString("en")} bytes`} as ${name}.`);
        b.textContent = "copied";
        setTimeout(() => (b.textContent = name), 1500);
      } catch {
        b.textContent = "not allowed here";
        setTimeout(() => (b.textContent = name), 2000);
      }
    });
    row.append(b);
  }
  return row;
}
