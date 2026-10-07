// Every dialog closes the ways people expect: Escape, the × in its corner,
// and — where nothing would be lost — a click outside it.

/**
 * Adds the × and, unless `keepsWork`, closing on a click outside. A click
 * counts as outside only when it both starts and ends there: selecting text
 * inside and letting go outside keeps the dialog open.
 */
export function dismissable(dialog: HTMLDialogElement, keepsWork = false, onDismiss?: () => void): void {
  dialog.addEventListener("cancel", () => onDismiss?.());
  const close = () => {
    onDismiss?.();
    dialog.close();
  };
  const x = document.createElement("button");
  x.type = "button";
  x.className = "dialog-x";
  x.setAttribute("aria-label", "Close");
  x.title = "Close (Esc)";
  x.textContent = "×";
  x.addEventListener("click", close);
  dialog.prepend(x);
  if (keepsWork) return;
  const outside = (e: MouseEvent) => {
    if (e.target !== dialog) return false;
    const r = dialog.getBoundingClientRect();
    return e.clientX < r.left || e.clientX > r.right || e.clientY < r.top || e.clientY > r.bottom;
  };
  let startedOutside = false;
  dialog.addEventListener("pointerdown", (e) => (startedOutside = outside(e)));
  dialog.addEventListener("click", (e) => {
    if (startedOutside && outside(e)) close();
    startedOutside = false;
  });
}
