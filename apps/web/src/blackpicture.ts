// Black out part of a picture: drag boxes over faces, number plates, an
// address, a screen; the copy is a new picture with the boxes in its pixels,
// and — drawn afresh — no camera data, place or thumbnail at all. What no
// clean copy can remove, removed by hand.
import type { FileModel } from "./model";
import { decodePicture, drawn } from "./thumbnail";

/** The largest side the picture is shown at while boxes are drawn; the copy is made at full size. */
const SHOWN = 1600;
/** Boxes smaller than this share of the picture's side are a stray tap, not a box. */
const MIN_SIDE = 0.01;

type Box = [number, number, number, number];

function el<K extends keyof HTMLElementTagNameMap>(tag: K, cls?: string, text?: string): HTMLElementTagNameMap[K] {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (text !== undefined) e.textContent = text;
  return e;
}

/** What the page does with the copy: save it, or offer to share it. */
export interface PictureCopy {
  deliver(name: string, bytes: Uint8Array, into: HTMLElement): void;
}

/** Whether a picture can be blacked out here: one the browser can decode. */
export function canBlackOut(m: FileModel): boolean {
  return ["jpeg", "png", "heif", "webp", "gif"].includes(m.file.format);
}

export async function openBlackPicture(m: FileModel, copy: PictureCopy): Promise<void> {
  const bitmap = await decodePicture(m);
  const dialog = el("dialog", "report blackpicture");
  const close = el("button", "btn", "Close");
  close.addEventListener("click", () => dialog.close());
  dialog.addEventListener("close", () => dialog.remove());
  dialog.append(el("h2", undefined, "Black out part of the picture"));
  if (!bitmap) {
    dialog.append(el("p", "hint", "This browser cannot draw this picture, so it cannot be blacked out here. Safari reads HEIC photos; other browsers may not."), close);
    document.body.append(dialog);
    dialog.showModal();
    return;
  }
  const orientation = m.file.format === "jpeg" ? m.file.orientation : 1;
  const base = drawn(bitmap, orientation, SHOWN);
  const canvas = el("canvas", "blackpicture-canvas");
  canvas.width = base.width;
  canvas.height = base.height;
  canvas.setAttribute("aria-label", "The picture: drag across it to draw a black box");
  const ctx = canvas.getContext("2d");
  const boxes: Box[] = [];
  let drawing: Box | null = null;
  const draw = () => {
    if (!ctx) return;
    ctx.drawImage(base, 0, 0);
    ctx.fillStyle = "#000";
    for (const [x0, y0, x1, y1] of drawing ? [...boxes, drawing] : boxes) {
      ctx.fillRect(x0 * canvas.width, y0 * canvas.height, (x1 - x0) * canvas.width, (y1 - y0) * canvas.height);
    }
  };
  draw();
  // Where the pointer is, as a share of the picture's width and height.
  const at = (e: PointerEvent): [number, number] => {
    const r = canvas.getBoundingClientRect();
    return [Math.min(1, Math.max(0, (e.clientX - r.left) / r.width)), Math.min(1, Math.max(0, (e.clientY - r.top) / r.height))];
  };
  let start: [number, number] | null = null;
  canvas.addEventListener("pointerdown", (e) => {
    start = at(e);
    canvas.setPointerCapture(e.pointerId);
  });
  canvas.addEventListener("pointermove", (e) => {
    if (!start) return;
    const [x, y] = at(e);
    drawing = [Math.min(start[0], x), Math.min(start[1], y), Math.max(start[0], x), Math.max(start[1], y)];
    draw();
  });
  const finish = () => {
    if (drawing && drawing[2] - drawing[0] > MIN_SIDE && drawing[3] - drawing[1] > MIN_SIDE) boxes.push(drawing);
    drawing = null;
    start = null;
    draw();
    refresh();
  };
  canvas.addEventListener("pointerup", finish);
  canvas.addEventListener("pointercancel", finish);

  const undo = el("button", "btn", "Undo");
  undo.addEventListener("click", () => {
    boxes.pop();
    draw();
    refresh();
  });
  const save = el("button", "btn btn-primary", "Make the copy");
  const result = el("div", "cleaner");
  const refresh = () => {
    undo.disabled = boxes.length === 0;
    save.disabled = boxes.length === 0;
  };
  refresh();
  save.addEventListener("click", async () => {
    save.disabled = true;
    save.textContent = "Making the copy…";
    // At full size, from the decoded picture: a new file with nothing but pixels.
    const full = drawn(bitmap, orientation, Infinity);
    const fctx = full.getContext("2d");
    if (fctx) {
      fctx.fillStyle = "#000";
      for (const [x0, y0, x1, y1] of boxes) {
        // Whole pixels, rounded outward: no edge of what is covered shows.
        const l = Math.floor(x0 * full.width);
        const t = Math.floor(y0 * full.height);
        fctx.fillRect(l, t, Math.ceil(x1 * full.width) - l, Math.ceil(y1 * full.height) - t);
      }
    }
    // Pictures drawn in few colours stay sharp as PNG; photographs go as JPEG.
    const png = m.file.format === "png" || m.file.format === "gif";
    const blob = await new Promise<Blob | null>((resolve) => full.toBlob(resolve, png ? "image/png" : "image/jpeg", 0.92));
    save.textContent = "Make the copy";
    refresh();
    result.replaceChildren();
    if (!blob) {
      result.append(el("p", "problem is-warning", "No copy was made: the browser could not write the picture."));
      return;
    }
    const stem = (m.name.split("/").pop() ?? m.name).replace(/\.[^.]*$/, "");
    const name = `${stem}-blacked-out.${png ? "png" : "jpg"}`;
    result.append(
      el("p", "clean-done", `Made ${name}: ${boxes.length === 1 ? "1 box" : `${boxes.length} boxes`} in its pixels, and no camera data, place or thumbnail.`),
    );
    copy.deliver(name, new Uint8Array(await blob.arrayBuffer()), result);
  });

  const hint = el(
    "p",
    "hint",
    "Drag across faces, number plates, an address, a screen. The copy is a new picture: the boxes are part of its pixels, and it carries nothing about the camera or the place.",
  );
  const bar = el("div", "blackpicture-bar");
  bar.append(save, undo, close);
  dialog.append(hint, canvas, bar, result);
  document.body.append(dialog);
  dialog.showModal();
}
