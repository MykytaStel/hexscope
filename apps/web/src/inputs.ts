// The ways a file comes in: the pickers, a drop anywhere on the page, a
// paste — of a copied picture, most often a screenshot — by key or by button.
// Each hands the files to `open`; what could not be had is said by `fail`.

export interface Inputs {
  /** One file opens; several are listed. */
  open(files: File[]): void;
  /** Says why nothing opened. */
  fail(message: string): void;
}

const $ = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;

/** Listens for files from every way in. */
export function wireInputs({ open, fail }: Inputs): void {
  for (const id of ["picker", "picker-empty", "picker-folder"]) {
    $<HTMLInputElement>(id).addEventListener("change", (e) => {
      const input = e.target as HTMLInputElement;
      const files = [...(input.files ?? [])];
      input.value = ""; // so choosing the same file again still fires
      open(files);
    });
  }
  let dragDepth = 0;
  window.addEventListener("dragenter", (e) => {
    e.preventDefault();
    if (++dragDepth === 1) document.body.classList.add("is-dragging");
  });
  window.addEventListener("dragleave", () => {
    if (--dragDepth === 0) document.body.classList.remove("is-dragging");
  });
  window.addEventListener("dragover", (e) => e.preventDefault());
  window.addEventListener("drop", (e) => {
    e.preventDefault();
    dragDepth = 0;
    document.body.classList.remove("is-dragging");
    open([...(e.dataTransfer?.files ?? [])]);
  });

  // A picture copied to the clipboard — a screenshot, most often — opens as
  // if it were chosen. Pasting into a field stays pasting.
  window.addEventListener("paste", (e) => {
    const t = e.target;
    if (t instanceof HTMLInputElement || t instanceof HTMLTextAreaElement || (t instanceof HTMLElement && t.isContentEditable)) return;
    const files = [...(e.clipboardData?.files ?? [])];
    if (files.length === 0) return;
    e.preventDefault();
    open(files);
  });

  // The same from a button, for a phone, which has no keys to paste with:
  // the browser asks, then hands over what was copied.
  const pasteKey = /Mac|iPhone|iPad/.test(navigator.platform) ? "⌘V" : "Ctrl+V";
  for (const btn of document.querySelectorAll<HTMLButtonElement>("[data-paste]")) btn.hidden = !navigator.clipboard?.read;
  document.addEventListener("click", (e) => {
    if ((e.target as Element | null)?.closest?.("[data-paste]")) void pasteFromClipboard();
  });

  async function pasteFromClipboard(): Promise<void> {
    let items: ClipboardItems;
    try {
      items = await navigator.clipboard.read();
    } catch {
      fail(`This browser did not let the page read what you copied. Press ${pasteKey} instead, or choose the file.`);
      return;
    }
    const files: File[] = [];
    for (const item of items) {
      const type = item.types.find((t) => t.startsWith("image/"));
      if (!type) continue;
      const blob = await item.getType(type);
      files.push(new File([blob], `pasted-picture.${type.split("/")[1].replace("jpeg", "jpg")}`, { type }));
    }
    if (files.length === 0) {
      fail("There is no picture among what you copied: copy a screenshot or a photo first, then paste it here.");
      return;
    }
    open(files);
  }
}
