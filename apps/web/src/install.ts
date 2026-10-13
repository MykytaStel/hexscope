// "Install the app": hexscope then opens from the home screen or the dock
// like an app, without the browser's bars, and works offline once opened.
// Chrome, Edge and Android ask through a prompt of their own; an iPhone or
// an iPad has none, so it is told the three taps. Nothing shows where it is
// installed already, or where the browser cannot install it.
import { dismissable } from "./dialogs";

/** What Chrome, Edge and Android hand the page when it may be installed. */
interface InstallPrompt extends Event {
  prompt(): Promise<void>;
  userChoice: Promise<{ outcome: "accepted" | "dismissed" }>;
}

/** An iPhone or iPad, the iPad asking for the Mac site as it does by default. */
export function isIos(ua: string, touchPoints: number): boolean {
  return /iPhone|iPad|iPod/.test(ua) || (/Macintosh/.test(ua) && touchPoints > 1);
}

function installed(): boolean {
  return matchMedia("(display-mode: standalone)").matches || (navigator as Navigator & { standalone?: boolean }).standalone === true;
}

function el<K extends keyof HTMLElementTagNameMap>(tag: K, cls?: string, text?: string): HTMLElementTagNameMap[K] {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (text !== undefined) e.textContent = text;
  return e;
}

/** The three taps, for a browser with no prompt to show. */
function showSteps(device: string): void {
  const dialog = el("dialog", "report install-steps");
  const steps = el("ol", "install-list");
  steps.append(
    el("li", undefined, "Tap Share — the square with an arrow. If it is not in view, tap ••• or ≡ beside the address first."),
    el("li", undefined, "Choose Add to Home Screen. Scroll down the list if it is not in view."),
    el("li", undefined, "Tap Add."),
  );
  const done = el("button", "btn btn-primary", "Got it");
  done.type = "button";
  done.addEventListener("click", () => dialog.close());
  const actions = el("div", "report-actions");
  actions.append(done);
  dialog.append(
    el("h2", undefined, device === "iPad" ? "Install hexscope on this iPad" : "Install hexscope on this iPhone"),
    el("p", "hint", "It opens from the home screen like an app, without the browser's bars, and works offline once opened."),
    steps,
    actions,
  );
  dialog.addEventListener("close", () => dialog.remove());
  document.body.append(dialog);
  dismissable(dialog);
  dialog.showModal();
}

/** Shows every `[data-install]` button where hexscope can be installed, and makes it work. */
export function wireInstall(): void {
  const buttons = [...document.querySelectorAll<HTMLButtonElement>("[data-install]")];
  if (buttons.length === 0 || installed()) return;
  const show = (on: boolean) => buttons.forEach((b) => (b.hidden = !on));
  let prompt: InstallPrompt | null = null;
  const ios = isIos(navigator.userAgent, navigator.maxTouchPoints);
  if (ios) show(true);
  // The browser's own small banner gives way to the button, asked for when wanted.
  addEventListener("beforeinstallprompt", (e) => {
    e.preventDefault();
    prompt = e as InstallPrompt;
    show(true);
  });
  addEventListener("appinstalled", () => {
    prompt = null;
    show(false);
  });
  for (const b of buttons) {
    b.addEventListener("click", async () => {
      if (prompt) {
        const asked = prompt;
        prompt = null;
        await asked.prompt();
        // The prompt can be shown once: after a "not now", the button waits for the browser to offer again.
        if ((await asked.userChoice).outcome === "accepted" || !ios) show(false);
        return;
      }
      // The menu it was chosen from closes behind the steps.
      b.closest("details")?.removeAttribute("open");
      showSteps(/iPad|Macintosh/.test(navigator.userAgent) ? "iPad" : "iPhone");
    });
  }
}
