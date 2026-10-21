// The home page: the landing, its demo, and every way a file comes in. The
// analyzer that shows an open file (analyzer.ts) is a module of its own,
// loaded when a file or a sample is asked for, or once the page is idle —
// the first page waits for none of it.
import "./style.css";
import { currentLocale, installLocale, languageButton, withUkrainian } from "./i18n";
import { themeButton } from "./theme";
import { wireInstall } from "./install";
import { wireInputs } from "./inputs";
import { openShortcuts } from "./shortcuts";
import { startDemo } from "./demo";
import { siteUrl } from "./files";
import { countVisit } from "./count";
import { announce } from "./announce";

// The advice and clean-copy notes in Ukrainian: only an opened file needs them,
// and only a reader in Ukrainian, so they come with the dictionary.
withUkrainian(() => import("./analysis-i18n"));
installLocale();
document.querySelector(".topbar .actions")?.prepend(languageButton());
// Light, dark or the system's, beside Open.
document.querySelector('[data-opens="picker"]')?.before(themeButton());

function syncAnalyzerPreviewLocale(): void {
  const image = document.querySelector<HTMLImageElement>(".deeper-image");
  if (!image) return;
  const suffix = currentLocale() === "uk" ? "Uk" : "En";
  const imageSource = image.dataset[`src${suffix}`];
  const mobileSource = document.querySelector<HTMLSourceElement>(".deeper-image-mobile")?.dataset[`src${suffix}`];
  if (imageSource && image.getAttribute("src") !== imageSource) image.src = imageSource;
  const mobile = document.querySelector<HTMLSourceElement>(".deeper-image-mobile");
  if (mobile && mobileSource && mobile.getAttribute("srcset") !== mobileSource) mobile.srcset = mobileSource;
}

syncAnalyzerPreviewLocale();
window.addEventListener("hexscope:locale", syncAnalyzerPreviewLocale);

type Analyzer = typeof import("./analyzer");
let analyzer: Promise<Analyzer> | undefined;

/** The analyzer, loaded once; a load that failed is tried again next time. */
function app(): Promise<Analyzer> {
  analyzer ??= import("./analyzer").catch((error: unknown) => {
    analyzer = undefined;
    const message = navigator.onLine
      ? "hexscope could not load its file viewer. Reload the page and try again."
      : "hexscope could not load its file viewer: you are offline, and it has not been saved for offline use yet.";
    const landing = document.getElementById("load-error");
    if (landing) {
      landing.textContent = message;
      landing.hidden = false;
    }
    document.body.dataset.state = "empty";
    throw error;
  });
  return analyzer;
}

/** Does `use` with the analyzer, once it is here. */
function withApp(use: (a: Analyzer) => unknown): void {
  void app().then(use, () => undefined);
}

for (const b of document.querySelectorAll<HTMLButtonElement>("[data-shortcuts]")) b.addEventListener("click", openShortcuts);
for (const b of document.querySelectorAll<HTMLButtonElement>("[data-feedback]")) {
  b.addEventListener("click", () => void import("./feedback").then((m) => m.openFeedback()));
}
wireInstall();
for (const b of document.querySelectorAll<HTMLButtonElement>("[data-tour]")) b.addEventListener("click", () => withApp((a) => a.startTour()));
for (const b of document.querySelectorAll<HTMLButtonElement>("[data-sample]")) b.addEventListener("click", () => withApp((a) => a.openSample(b)));
// The film lab is a tool of its own, not the analyzer.
document.getElementById("film-roll-open")?.addEventListener("click", () => {
  void import("./film-roll-ui").then((m) => m.showFilmRoll()).catch((e: unknown) => announce(String(e)));
});

// The file pickers: buttons that open hidden inputs — wherever they are,
// the summary's own included, drawn after the page loaded.
document.addEventListener("click", (e) => {
  if ((e.target as Element).closest(".landing-menu a")) document.querySelector(".landing-menu")?.removeAttribute("open");
  const button = (e.target as Element | null)?.closest?.<HTMLButtonElement>("button[data-opens]");
  if (button) document.getElementById(button.dataset.opens ?? "")?.click();
});

// The landing's own button, on screen or scrolled away: on a phone the header's Open shows only then.
const heroButton = document.querySelector(".hero-actions .btn-primary");
if (heroButton && "IntersectionObserver" in window) {
  new IntersectionObserver(([seen]) => {
    document.body.dataset.heroButton = seen.isIntersecting ? "shown" : "away";
  }).observe(heroButton);
}

// Every way a file comes in.
wireInputs({ open: (files) => withApp((a) => a.openFiles(files)), fail: (message) => withApp((a) => a.loadFailed(message)) });

// Files opened with the installed app from the computer's own file manager.
interface LaunchParams {
  files: FileSystemFileHandle[];
}
const launchQueue = (window as Window & { launchQueue?: { setConsumer(f: (p: LaunchParams) => void): void } }).launchQueue;
launchQueue?.setConsumer(async ({ files }) => {
  if (files.length > 0) {
    const opened = await Promise.all(files.map((f) => f.getFile()));
    withApp((a) => a.openFiles(opened));
  }
});

window.addEventListener("keydown", (e) => {
  if (e.target instanceof HTMLInputElement && e.target.type !== "range") return;
  if (document.querySelector("dialog[open]")) return;
  if (e.key === "?" && !e.metaKey && !e.ctrlKey) openShortcuts();
  // Letters only bare: Cmd-O belongs to the browser.
  if (!e.metaKey && !e.ctrlKey && !e.altKey && (e.key === "o" || e.key === "O")) {
    document.getElementById(document.body.dataset.state === "empty" ? "picker-empty" : "picker")?.click();
  }
});

// Offline after the first visit, and installable. Only in production: in
// development the cache would serve stale modules.
if (import.meta.env.PROD && "serviceWorker" in navigator) {
  void navigator.serviceWorker.register(siteUrl("sw.js"));
}

document.body.dataset.state = "empty";
const asked = new URLSearchParams(location.search);
// What a link asked to open goes first: a sample by name, or a file shared to the app.
const linked = asked.get("sample");
if (linked && [...document.querySelectorAll<HTMLElement>("[data-sample]")].some((b) => b.dataset.name === linked)) {
  // Loading from now, so the demo does not start under it while the analyzer comes.
  document.body.dataset.state = "loading";
  withApp((a) => a.openLinkedSample());
} else if (linked) history.replaceState(null, "", location.pathname);
if (asked.has("shared") || asked.has("share-error")) withApp((a) => void a.openShared());
// Only a page still empty shows the demo.
setTimeout(() => void startDemo(document.getElementById("demo")!), 0);
// The analyzer, ready before the first file: once the page has nothing else to do.
const idle = (window as Window & { requestIdleCallback?: (f: () => void, o?: { timeout: number }) => void }).requestIdleCallback;
if (idle) idle(() => void app().catch(() => undefined), { timeout: 3000 });
else setTimeout(() => void app().catch(() => undefined), 1500);
countVisit();
