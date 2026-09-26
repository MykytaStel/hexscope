// The keyboard, in one list: "?" opens it anywhere, and the landing page
// links to it.

const KEYS: [string[], string][] = [
  [["O"], "Open a file"],
  [["N"], "Go to the next problem"],
  [["P"], "Watch the file decompress, byte by byte (PNG and ZIP entries)"],
  [["Backspace"], "Back up: out of an archive entry, or to the list of files"],
  [["Esc"], "Close the player, or clear the selection"],
  [["?"], "Show these keys"],
];

const PLAYER: [string[], string][] = [
  [["Space"], "Play or pause"],
  [["←", "→"], "One step back or on"],
  [["Shift", "←/→"], "Ten steps"],
  [["Home", "End"], "The first or the last step"],
];

function el<K extends keyof HTMLElementTagNameMap>(tag: K, cls?: string, text?: string): HTMLElementTagNameMap[K] {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (text !== undefined) e.textContent = text;
  return e;
}

function table(rows: [string[], string][]): HTMLElement {
  const dl = el("dl", "keys");
  for (const [keys, what] of rows) {
    const dt = el("dt");
    keys.forEach((k, i) => {
      if (i > 0) dt.append(" ");
      dt.append(el("kbd", undefined, k));
    });
    dl.append(dt, el("dd", undefined, what));
  }
  return dl;
}

/** Shows the keys; a second call while open does nothing. */
export function openShortcuts(): void {
  if (document.querySelector("dialog.shortcuts")) return;
  const dialog = el("dialog", "report shortcuts");
  dialog.setAttribute("aria-label", "Keyboard shortcuts");
  const close = el("button", "btn", "Close");
  close.addEventListener("click", () => dialog.close());
  dialog.addEventListener("close", () => dialog.remove());
  dialog.append(
    el("h2", undefined, "Keyboard shortcuts"),
    table(KEYS),
    el("h3", undefined, "In the DEFLATE player"),
    table(PLAYER),
    close,
  );
  document.body.append(dialog);
  dialog.showModal();
}
