// The keyboard, in one list: "?" opens it anywhere, and the landing page
// links to it.

/** The modifier for pasting on this computer. */
const MOD = /Mac|iPhone|iPad/.test(navigator.platform) ? "⌘" : "Ctrl";

const KEYS: [string[], string][] = [
  [["O"], "Open a file"],
  [[`${MOD} V`], "Check a picture you copied, such as a screenshot"],
  [["N"], "Go to the next problem"],
  [["/"], "Find text, or bytes in hex, in the file"],
  [["P"], "Watch the file decompress, byte by byte (PNG and ZIP entries)"],
  [["Backspace"], "Back up: out of an archive entry, or to the list of files"],
  [["Esc"], "Close the player, or clear the selection"],
  [["?"], "Show these keys"],
];

/** On a page or a picture to black out, once it has the focus. */
const BOXES: [string[], string][] = [
  [["←", "→", "↑", "↓"], "Move the box"],
  [["Shift", "arrows"], "Make the box wider, narrower, taller or shorter"],
  [["Alt", "arrows"], "The same, a point at a time"],
  [["Enter"], "Black out what is under the box"],
];

const TREE: [string[], string][] = [
  [["↑", "↓"], "The part above or below"],
  [["→"], "Open a part, or go to its first part"],
  [["←"], "Close a part, or go to the one it is in"],
  [["Home", "End"], "The first or the last part"],
  [["Enter"], "Open or close the part"],
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
    el("h3", undefined, "In the structure, once it has focus (Tab to it)"),
    table(TREE),
    el("h3", undefined, "Blacking out, on a page or a picture (Tab to it)"),
    table(BOXES),
    el("h3", undefined, "In the DEFLATE player"),
    table(PLAYER),
    close,
  );
  document.body.append(dialog);
  dialog.showModal();
}
