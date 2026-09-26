// "Share what it revealed": a card and a sentence that say what kinds of
// thing a file gave away — never the things themselves. No coordinates, no
// names, no serial numbers, not even the file's name: only categories, so
// sharing the card cannot leak what the file did.
import type { FileModel } from "./model";
import { list } from "./verdict";

/** Each kind of fact as a category, most telling first. */
const CATEGORIES: [string, string][] = [
  ["covered", "text that was blacked out but not removed"],
  ["location", "where it was taken"],
  ["updates", "earlier versions of itself"],
  ["deleted", "text that was deleted"],
  ["earlier", "text an edit took off the page"],
  ["photoplace", "where its photos were taken"],
  ["serial", "the camera's serial number"],
  ["owner", "the owner's name"],
  ["author", "who wrote it"],
  ["editor", "who saved it last"],
  ["company", "the company"],
  ["comments", "who commented"],
  ["tracked", "who changed what"],
  ["photo", "the camera behind its photos"],
  ["camera", "the camera"],
  ["shutter", "how many photos the camera has taken"],
  ["linked", "IDs that link it to other shots"],
  ["uptime", "how long the phone had been on"],
  ["place", "the place it names"],
  ["original", "the file it was made from"],
  ["caption", "its caption"],
  ["lens", "the lens"],
  ["taken", "when it was taken"],
  ["created", "when it was written"],
  ["modified", "when it was last changed"],
  ["editing", "how long it was worked on"],
  ["revisions", "how many times it was saved"],
  ["history", "its editing history"],
  ["software", "the software used"],
  ["application", "the program it was written in"],
  ["producer", "the program that made the PDF"],
  ["template", "the template it came from"],
  ["title", "its title"],
  ["subject", "its subject"],
  ["keywords", "its keywords"],
  ["thumbnail", "a hidden preview of the picture"],
  ["paths", "the user name of the computer that built it"],
  ["names", "its functions' names"],
  ["toolchain", "the compiler that built it"],
  ["sourcemap", "where its source map is"],
  ["debug", "debug info from its source"],
];

/** The categories a file gives away, in order. */
export function categories(m: FileModel): string[] {
  const kinds = new Set([...(m.file.location ? ["location"] : []), ...m.file.facts.map((f) => f.kind)]);
  return CATEGORIES.filter(([k]) => kinds.has(k)).map(([, words]) => words);
}

function noun(m: FileModel): string {
  switch (m.file.format) {
    case "jpeg":
    case "heif":
      return "photo";
    case "png":
      return "image";
    case "video":
      return "video";
    default:
      return "document";
  }
}

const SITE = "https://hexscope.pages.dev";

/** One sentence, ready to paste. */
export function sentence(m: FileModel): string {
  const found = categories(m);
  const said =
    found.length > 4 ? `${found.slice(0, 4).join(", ")} and ${found.length - 4} more things` : list(found);
  return `My ${noun(m)} was giving away ${said}. I checked it in my browser with hexscope, and nothing was uploaded: ${SITE}`;
}

/** The card, 1200 × 630 like a link preview. */
async function card(m: FileModel): Promise<Blob | null> {
  const found = categories(m);
  const [w, h] = [1200, 630];
  const canvas = document.createElement("canvas");
  canvas.width = w;
  canvas.height = h;
  const ctx = canvas.getContext("2d");
  if (!ctx) return null;
  const font = (weight: number, size: number) =>
    `${weight} ${size}px system-ui, -apple-system, "Segoe UI", Roboto, sans-serif`;

  ctx.fillStyle = "#101114";
  ctx.fillRect(0, 0, w, h);
  ctx.fillStyle = "#e8527a";
  ctx.fillRect(0, 0, 14, h);

  ctx.fillStyle = "#9aa4b8";
  ctx.font = font(600, 26);
  ctx.fillText("CHECKED WITH HEXSCOPE", 80, 96);

  ctx.fillStyle = "#f4f4f2";
  ctx.font = font(700, 58);
  ctx.fillText(`My ${noun(m)} was giving away`, 80, 172);

  // Up to five, then "and N more".
  const shown = found.slice(0, 5);
  ctx.font = font(500, 38);
  shown.forEach((text, i) => {
    const y = 244 + i * 54;
    ctx.fillStyle = "#e8527a";
    ctx.beginPath();
    ctx.arc(92, y - 12, 8, 0, Math.PI * 2);
    ctx.fill();
    ctx.fillStyle = "#f4f4f2";
    ctx.fillText(text, 120, y);
  });
  if (found.length > shown.length) {
    ctx.fillStyle = "#9aa4b8";
    ctx.fillText(`and ${found.length - shown.length} more`, 120, 244 + shown.length * 54);
  }

  ctx.fillStyle = "#9aa4b8";
  ctx.font = font(500, 26);
  ctx.fillText("Checked in the browser — nothing uploaded.  hexscope.pages.dev", 80, h - 56);

  return new Promise((resolve) => canvas.toBlob(resolve, "image/png"));
}

/**
 * Shares the card and the sentence where the device can (phones), and
 * otherwise saves the card and copies the sentence. Says what it did.
 */
export async function share(m: FileModel): Promise<string> {
  const text = sentence(m);
  const blob = await card(m);
  const file = blob ? new File([blob], "hexscope-found.png", { type: "image/png" }) : null;
  const data: ShareData = file ? { files: [file], text } : { text };
  if (navigator.canShare?.(data)) {
    try {
      await navigator.share(data);
      return "Shared.";
    } catch (e) {
      if ((e as DOMException).name === "AbortError") return "";
    }
  }
  let copied = false;
  try {
    await navigator.clipboard.writeText(text);
    copied = true;
  } catch {
    // Clipboard refused: the card alone still goes.
  }
  if (blob) {
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = "hexscope-found.png";
    a.click();
    setTimeout(() => URL.revokeObjectURL(url), 1000);
  }
  return copied ? "Saved the card, and copied the sentence to paste with it." : "Saved the card.";
}
