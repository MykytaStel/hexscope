// Black out part of a picture: drag boxes over faces, number plates, an
// address, a screen; the copy is a new picture with the boxes in its pixels,
// and — drawn afresh — no camera data, place or thumbnail at all. What no
// clean copy can remove, removed by hand.
import type { FileModel } from "./model";
import { decodePicture, drawn } from "./thumbnail";
import { done } from "./announce";
import { dismissable } from "./dialogs";
import { currentLocale, translateText } from "./i18n";

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

/** Opens the editor on `m`, with the boxes of any QR codes found in it already drawn. */
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
    dismissable(dialog, true);
    dialog.showModal();
    return;
  }
  const orientation = m.file.format === "jpeg" ? m.file.orientation : 1;
  const base = drawn(bitmap, orientation, SHOWN);
  const canvas = el("canvas", "blackpicture-canvas");
  canvas.width = base.width;
  canvas.height = base.height;
  const ctx = canvas.getContext("2d");
  const boxes: Box[] = m.codeBoxes.map((b) => [...b] as Box);
  const labels = m.codeBoxes.map(() => "Detected QR code area");
  const history: { boxes: Box[]; labels: string[] }[] = [];
  let drawing: Box | null = null;
  let selectedBox: number | null = null;
  let moving: { index: number; start: [number, number]; original: Box; preview: Box } | null = null;
  let makingCopy = false;
  const draw = () => {
    if (!ctx) return;
    ctx.drawImage(base, 0, 0);
    ctx.fillStyle = "#000";
    for (let i = 0; i < boxes.length; i++) {
      const [x0, y0, x1, y1] = moving?.index === i ? moving.preview : boxes[i];
      ctx.fillRect(x0 * canvas.width, y0 * canvas.height, (x1 - x0) * canvas.width, (y1 - y0) * canvas.height);
    }
    if (selectedBox !== null && boxes[selectedBox]) {
      const [x0, y0, x1, y1] = moving?.index === selectedBox ? moving.preview : boxes[selectedBox];
      ctx.setLineDash([6, 4]);
      ctx.lineWidth = Math.max(2, canvas.width / 400);
      ctx.strokeStyle = "#fff";
      ctx.strokeRect(x0 * canvas.width, y0 * canvas.height, (x1 - x0) * canvas.width, (y1 - y0) * canvas.height);
      ctx.setLineDash([]);
    }
    // The box being drawn or moved: see-through, outlined, until it is made.
    if (drawing) {
      const [x0, y0, x1, y1] = drawing;
      const r: [number, number, number, number] = [x0 * canvas.width, y0 * canvas.height, (x1 - x0) * canvas.width, (y1 - y0) * canvas.height];
      ctx.fillStyle = "rgb(0 0 0 / 0.45)";
      ctx.fillRect(...r);
      ctx.setLineDash([6, 4]);
      ctx.lineWidth = Math.max(1.5, canvas.width / 500);
      ctx.strokeStyle = "#fff";
      ctx.strokeRect(...r);
      ctx.setLineDash([]);
    }
  };
  const commitBoxes = (change: () => void) => {
    history.push({ boxes: boxes.map((box) => [...box] as Box), labels: [...labels] });
    change();
    invalidateCopy();
    draw();
    refresh();
  };
  draw();
  // Where the pointer is, as a share of the picture's width and height.
  const at = (e: PointerEvent): [number, number] => {
    const r = canvas.getBoundingClientRect();
    return [Math.min(1, Math.max(0, (e.clientX - r.left) / r.width)), Math.min(1, Math.max(0, (e.clientY - r.top) / r.height))];
  };
  let start: [number, number] | null = null;
  canvas.addEventListener("pointerdown", (e) => {
    const point = at(e);
    drawing = null;
    let hit = -1;
    for (let i = boxes.length - 1; i >= 0; i--) {
      const [x0, y0, x1, y1] = boxes[i];
      if (point[0] >= x0 && point[0] <= x1 && point[1] >= y0 && point[1] <= y1) {
        hit = i;
        break;
      }
    }
    if (hit >= 0) {
      selectedBox = hit;
      moving = { index: hit, start: point, original: [...boxes[hit]] as Box, preview: [...boxes[hit]] as Box };
    } else {
      selectedBox = null;
      start = point;
    }
    canvas.setPointerCapture(e.pointerId);
    refresh();
  });
  canvas.addEventListener("pointermove", (e) => {
    const [x, y] = at(e);
    if (moving) {
      const [x0, y0, x1, y1] = moving.original;
      const w = x1 - x0;
      const h = y1 - y0;
      const left = Math.max(0, Math.min(1 - w, x0 + x - moving.start[0]));
      const top = Math.max(0, Math.min(1 - h, y0 + y - moving.start[1]));
      moving.preview = [left, top, left + w, top + h];
      draw();
      return;
    }
    if (!start) return;
    drawing = [Math.min(start[0], x), Math.min(start[1], y), Math.max(start[0], x), Math.max(start[1], y)];
    draw();
  });
  const finish = () => {
    if (moving) {
      const moved = moving;
      moving = null;
      start = null;
      if (moved.preview.some((v, i) => Math.abs(v - moved.original[i]) > 0.0001)) {
        commitBoxes(() => {
          boxes[moved.index] = moved.preview;
        });
      } else {
        draw();
      }
      return;
    }
    if (drawing && drawing[2] - drawing[0] > MIN_SIDE && drawing[3] - drawing[1] > MIN_SIDE) {
      const box = [...drawing] as Box;
      commitBoxes(() => {
        boxes.push(box);
        labels.push("Added black area");
        selectedBox = boxes.length - 1;
      });
    }
    drawing = null;
    start = null;
    draw();
  };
  canvas.addEventListener("pointerup", finish);
  canvas.addEventListener("pointercancel", () => {
    moving = null;
    drawing = null;
    start = null;
    draw();
  });

  // The keyboard adds a fresh box, or edits the listed selection in place.
  // Shift plus an arrow resizes; Alt makes either operation smaller.
  const kb = [0.4, 0.45, 0.2, 0.1];
  const showKb = () => {
    if (selectedBox !== null) {
      drawing = null;
      draw();
      return;
    }
    drawing = [kb[0], kb[1], kb[0] + kb[2], kb[1] + kb[3]];
    draw();
  };
  canvas.tabIndex = 0;
  canvas.setAttribute(
    "aria-label",
    "The picture: drag on a clear area to add a black box or drag an existing box to move it. Select a listed box to edit it with the arrow keys; Shift and the arrows resize it. Press Enter to leave box editing, then Enter again to add the keyboard box.",
  );
  canvas.addEventListener("focus", showKb);
  canvas.addEventListener("blur", () => {
    if (start) return;
    drawing = null;
    draw();
  });
  canvas.addEventListener("keydown", (e) => {
    const step = e.altKey ? 0.005 : 0.02;
    const moves: Record<string, [number, number]> = { ArrowLeft: [-step, 0], ArrowRight: [step, 0], ArrowUp: [0, -step], ArrowDown: [0, step] };
    const move = moves[e.key];
    if (move) {
      e.preventDefault();
      if (selectedBox !== null && boxes[selectedBox]) {
        const index = selectedBox;
        commitBoxes(() => {
          const [x0, y0, x1, y1] = boxes[index];
          if (e.shiftKey) {
            boxes[index] = [
              x0,
              y0,
              Math.max(Math.min(1, x0 + MIN_SIDE), Math.min(1, x1 + move[0])),
              Math.max(Math.min(1, y0 + MIN_SIDE), Math.min(1, y1 + move[1])),
            ];
          } else {
            const w = x1 - x0;
            const h = y1 - y0;
            const left = Math.max(0, Math.min(1 - w, x0 + move[0]));
            const top = Math.max(0, Math.min(1 - h, y0 + move[1]));
            boxes[index] = [left, top, left + w, top + h];
          }
        });
        return;
      }
      if (e.shiftKey) {
        kb[2] = Math.max(MIN_SIDE * 2, Math.min(1 - kb[0], kb[2] + move[0]));
        kb[3] = Math.max(MIN_SIDE * 2, Math.min(1 - kb[1], kb[3] + move[1]));
      } else {
        kb[0] = Math.max(0, Math.min(1 - kb[2], kb[0] + move[0]));
        kb[1] = Math.max(0, Math.min(1 - kb[3], kb[1] + move[1]));
      }
      showKb();
    } else if ((e.key === "Enter" || e.key === " ") && selectedBox !== null) {
      e.preventDefault();
      selectedBox = null;
      showKb();
    } else if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      commitBoxes(() => {
        boxes.push([kb[0], kb[1], kb[0] + kb[2], kb[1] + kb[3]]);
        labels.push("Added black area");
        selectedBox = boxes.length - 1;
      });
      showKb();
    }
  });

  const undo = el("button", "btn", "Undo");
  undo.addEventListener("click", () => {
    const previous = history.pop();
    if (!previous) return;
    boxes.splice(0, boxes.length, ...previous.boxes);
    labels.splice(0, labels.length, ...previous.labels);
    selectedBox = boxes.length > 0 ? Math.min(selectedBox ?? boxes.length - 1, boxes.length - 1) : null;
    invalidateCopy();
    draw();
    refresh();
  });
  const clear = el("button", "btn", "Remove all boxes");
  clear.addEventListener("click", () => commitBoxes(() => {
    boxes.splice(0, boxes.length);
    labels.splice(0, labels.length);
    selectedBox = null;
  }));
  const save = el("button", "btn btn-primary", "Make the copy");
  const result = el("div", "cleaner");
  const boxList = el("ol", "blackpicture-boxes");
  const invalidateCopy = () => {
    result.replaceChildren();
    save.textContent = "Make the copy";
  };
  const refresh = () => {
    undo.disabled = makingCopy || history.length === 0;
    clear.disabled = makingCopy || boxes.length === 0;
    save.disabled = makingCopy || boxes.length === 0;
    canvas.style.pointerEvents = makingCopy ? "none" : "";
    canvas.tabIndex = makingCopy ? -1 : 0;
    boxList.replaceChildren();
    boxes.forEach((_, index) => {
      const item = el("li", "blackpicture-box");
      const label = el("span", undefined, `${translateText(labels[index] ?? "Added black area", currentLocale())} ${index + 1}`);
      const select = el("button", "link", translateText("Edit", currentLocale()));
      select.disabled = makingCopy;
      select.setAttribute("aria-label", `${translateText("Edit black box", currentLocale())} ${index + 1}`);
      select.setAttribute("aria-pressed", String(selectedBox === index));
      select.addEventListener("click", () => {
        selectedBox = index;
        refresh();
        canvas.focus();
      });
      const remove = el("button", "link", translateText("Remove", currentLocale()));
      remove.disabled = makingCopy;
      remove.setAttribute("aria-label", `${translateText("Remove black box", currentLocale())} ${index + 1}`);
      remove.addEventListener("click", () => {
        commitBoxes(() => {
          boxes.splice(index, 1);
          labels.splice(index, 1);
          if (selectedBox === index) selectedBox = null;
          else if (selectedBox !== null && selectedBox > index) selectedBox--;
        });
        const remaining = boxList.querySelectorAll<HTMLButtonElement>("button");
        (remaining[Math.min(index, remaining.length - 1)] ?? canvas).focus();
      });
      item.append(label, select, remove);
      boxList.append(item);
    });
    boxList.hidden = boxes.length === 0;
  };
  refresh();
  save.addEventListener("click", async () => {
    makingCopy = true;
    save.textContent = "Making the copy…";
    const selectedBoxes = boxes.map((box) => [...box] as Box);
    refresh();
    const png = m.file.format === "png" || m.file.format === "gif";
    let blob: Blob | null = null;
    try {
      // At full size, from the decoded picture: a new file with nothing but pixels.
      const full = drawn(bitmap, orientation, Infinity);
      const fctx = full.getContext("2d");
      if (!fctx) throw new Error("Could not draw the redactions.");
      fctx.fillStyle = "#000";
      for (const [x0, y0, x1, y1] of selectedBoxes) {
        // Whole pixels, rounded outward: no edge of what is covered shows.
        const l = Math.floor(x0 * full.width);
        const t = Math.floor(y0 * full.height);
        fctx.fillRect(l, t, Math.ceil(x1 * full.width) - l, Math.ceil(y1 * full.height) - t);
      }
      // Pictures drawn in few colours stay sharp as PNG; photographs go as JPEG.
      blob = await new Promise<Blob | null>((resolve) => full.toBlob(resolve, png ? "image/png" : "image/jpeg", 0.92));
    } catch {
      // Large canvases and browser encoders can fail; never offer a copy whose
      // black boxes may not have been drawn.
    }
    makingCopy = false;
    save.textContent = "Make the copy";
    refresh();
    result.replaceChildren();
    if (!blob) {
      result.append(el("p", "problem is-warning", "No copy was made: the browser could not write the picture."));
      return;
    }
    const stem = (m.name.split("/").pop() ?? m.name).replace(/\.[^.]*$/, "");
    const outputPng = png || blob.type === "image/png";
    const name = `${stem}-blacked-out.${outputPng ? "png" : "jpg"}`;
    result.append(
      done(`Made ${name}: ${selectedBoxes.length === 1 ? "1 box" : `${selectedBoxes.length} boxes`} in its pixels, and no camera data, place or thumbnail.`),
    );
    try {
      copy.deliver(name, new Uint8Array(await blob.arrayBuffer()), result);
    } catch {
      result.replaceChildren(el("p", "problem is-warning", "The browser could not offer the picture for saving."));
      return;
    }
    // Copy actions appear below the canvas and can extend a phone-sized dialog.
    // Bring the new action and its instructions into view as soon as the copy is ready.
    if (dialog.scrollHeight > dialog.clientHeight) dialog.scrollTop = dialog.scrollHeight;
  });

  const hint = el(
    "p",
    "hint",
    "Drag across faces, number plates, an address or a screen. Drag on a clear area to add a box; drag a box to move it. Edit a listed box with the arrow keys; Shift and the arrows resize it. Press Enter to leave box editing, then Enter again to add the keyboard box. Check each box still covers the right area before making a copy. The new picture has the boxes in its pixels and carries nothing about the camera or place.",
  );
  const conversion = el(
    "p",
    "hint",
    "The copy is re-encoded: PNG stays PNG; animated images become a still image; JPEG, HEIF and WebP become JPEG at 92% quality. HDR and colour-profile details may change. The original stays untouched.",
  );
  const bar = el("div", "blackpicture-bar");
  bar.append(save, undo, clear, close);
  dialog.append(hint, conversion, canvas, boxList, bar, result);
  document.body.append(dialog);
  dismissable(dialog, true);
  dialog.showModal();
}
