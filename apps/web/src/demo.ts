// The landing page demonstrates the everyday question first: what does a
// photo reveal, and what does its clean copy remove? Technical examples live
// in the collapsed "Look inside" section below.
import { call } from "./rpc";

const DWELL_MS = 2400;
const FACT_MS = 900;
const PHOTO = "samples/photo.jpg";
const TOLD: [string, string][] = [
  ["location", "Location"],
  ["owner", "Owner"],
  ["serial", "Camera serial"],
  ["taken", "Taken"],
];

const el = <K extends keyof HTMLElementTagNameMap>(tag: K, cls?: string, text?: string) => {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (text !== undefined) e.textContent = text;
  return e;
};

const idle = () => document.body.dataset.state === "empty";
const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

export async function startDemo(host: HTMLElement): Promise<void> {
  if (!idle()) return;

  const said = await photoScene();
  if (!idle() || !said) return;

  const frame = el("div", "demo-frame");
  const image = el("img", "demo-image") as HTMLImageElement;
  image.alt = "";
  image.setAttribute("aria-hidden", "true");
  frame.append(image);
  const caption = el("p", "demo-caption");
  const side = el("div", "demo-side");
  const kicker = el("p", "demo-kicker");
  const told = el("dl", "demo-facts");
  told.hidden = true;
  side.append(kicker, told, caption);

  // Revealing the facts must not move the sections below the hero.
  let width = 0;
  new ResizeObserver(() => {
    if (side.offsetWidth !== width) {
      width = side.offsetWidth;
      side.style.minHeight = "";
    }
    const height = side.offsetHeight;
    if (height > (parseFloat(side.style.minHeight) || 0)) side.style.minHeight = `${height}px`;
  }).observe(side);

  host.replaceChildren(frame, side);
  host.hidden = false;
  await tell(said, image, kicker, told, caption, matchMedia("(prefers-reduced-motion: reduce)").matches);
}

interface Said {
  picture: Blob;
  facts: [string, string][];
  removedBytes: number;
  /** What a clean copy still reveals among the facts we showed. */
  left: number;
}

/** Parse one ordinary photo and verify the clean copy before showing its result. */
async function photoScene(): Promise<Said | null> {
  try {
    const bytes = new Uint8Array(await (await fetch(PHOTO)).arrayBuffer());
    if (!idle()) return null;
    const picture = new Blob([bytes as BlobPart], { type: "image/jpeg" });
    const decoded = await createImageBitmap(picture);
    decoded.close();
    const parsed = await call({ type: "parse", file: new File([bytes], "photo.jpg") });
    if (parsed.type !== "parsed" || !idle()) return null;

    const facts: [string, string][] = [];
    for (const [kind, name] of TOLD) {
      if (kind === "location" && parsed.result.location) {
        const { latitude, longitude } = parsed.result.location;
        facts.push([
          name,
          `${Math.abs(latitude).toFixed(4)}° ${latitude < 0 ? "S" : "N"}, ${Math.abs(longitude).toFixed(4)}° ${longitude < 0 ? "W" : "E"}`,
        ]);
        continue;
      }
      const fact = parsed.result.facts.find((item) => item.kind === kind);
      // Seconds and the time zone make the row needlessly long.
      if (fact) facts.push([name, kind === "taken" ? fact.text.slice(0, 16) : fact.text]);
    }

    const cleaned = await call({ type: "clean", source: new Blob([bytes as BlobPart]) });
    if (cleaned.type !== "cleaned" || cleaned.error || !idle()) return null;
    const verified = await call({ type: "parse", file: new File([cleaned.copy], "photo.jpg") });
    if (verified.type !== "parsed" || !idle()) return null;
    const kinds = TOLD.map(([kind]) => kind);
    const left = verified.result.facts.filter((fact) => kinds.includes(fact.kind)).length + (verified.result.location ? 1 : 0);
    const removedBytes = cleaned.removed.reduce((total, range) => total + range.bytes, 0);
    return { picture, facts, removedBytes, left };
  } catch {
    return null;
  }
}

/** Show the finding first, then leave the verified clean-copy result in view. */
async function tell(
  said: Said,
  image: HTMLImageElement,
  kicker: HTMLElement,
  told: HTMLElement,
  caption: HTMLElement,
  reduceMotion: boolean,
): Promise<void> {
  kicker.textContent = "Sample file · example metadata";
  const imageUrl = URL.createObjectURL(said.picture);
  image.src = imageUrl;
  try {
    await image.decode();
  } finally {
    URL.revokeObjectURL(imageUrl);
  }
  told.replaceChildren();
  told.classList.remove("is-clean");
  told.hidden = false;
  caption.textContent = "A photo, as a camera or phone saves it. Inside it:";
  const rows = said.facts.map(([name, value]) => {
    const row = el("div", "demo-fact");
    row.setAttribute("aria-hidden", "true");
    row.append(el("dt", undefined, name), el("dd", undefined, value));
    return row;
  });
  told.replaceChildren(...rows);

  const cleanCaption =
    said.left === 0
      ? `Same photo. ${said.removedBytes.toLocaleString("en")} bytes removed in your browser.`
      : "Same photo. Camera details removed in your browser.";

  if (reduceMotion) {
    for (const row of rows) {
      row.removeAttribute("aria-hidden");
      row.classList.add("is-shown");
    }
    told.classList.add("is-clean");
    caption.textContent = cleanCaption;
    return;
  }

  await sleep(FACT_MS);
  for (const row of rows) {
    if (!idle()) return;
    row.removeAttribute("aria-hidden");
    row.classList.add("is-shown");
    await sleep(FACT_MS);
  }
  if (!idle()) return;
  caption.textContent = "Anyone you send it to can read all of that.";
  await sleep(DWELL_MS);
  if (!idle()) return;
  told.classList.add("is-clean");
  caption.textContent = cleanCaption;
}
