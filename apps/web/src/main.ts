// The home page: the landing and every way a file comes in. The
// analyzer that shows an open file (analyzer.ts) is a module of its own,
// loaded when a file or a sample is asked for; browsing the landing does not
// parse the file viewer before it is needed.
import "./style.css";
import { currentLocale, installLocale, languageButton, withUkrainian } from "./i18n";
import { themeButton } from "./theme";
import { wireInstall } from "./install";
import { wireInputs } from "./inputs";
import { openShortcuts } from "./shortcuts";
import { siteUrl } from "./files";
import { countVisit } from "./count";
import { announce } from "./announce";

// The advice and clean-copy notes in Ukrainian: only an opened file needs them,
// and only a reader in Ukrainian, so they come with the dictionary.
withUkrainian(() => import("./analysis-i18n"));
installLocale();
const languageControl = languageButton();
document.querySelector(".topbar .actions")?.prepend(languageControl);
const themeControl = themeButton();
document.querySelector('[data-opens="picker"]')?.before(themeControl);

// On a phone, keep the landing header to the brand and one menu. Preferences
// stay available in that menu; an open workspace brings them back beside Open.
const landingPreferences = document.querySelector<HTMLElement>(".landing-menu-controls");
const phoneLanding = matchMedia("(max-width: 480px)");
function placeLandingPreferences(): void {
  landingPreferences?.setAttribute("aria-label", currentLocale() === "uk" ? "Налаштування" : "Preferences");
  const inMenu = phoneLanding.matches && document.body.dataset.state === "empty";
  const menu = document.querySelector<HTMLElement>(".landing-menu");
  const actions = document.querySelector<HTMLElement>(".topbar .actions");
  const picker = document.querySelector('[data-opens="picker"]');
  if (inMenu && landingPreferences) {
    landingPreferences.append(languageControl, themeControl);
    landingPreferences.hidden = false;
    return;
  }
  landingPreferences?.setAttribute("hidden", "");
  actions?.prepend(languageControl);
  picker?.before(themeControl);
  menu?.removeAttribute("open");
}
window.addEventListener("hexscope:locale", () => {
  landingPreferences?.setAttribute("aria-label", currentLocale() === "uk" ? "Налаштування" : "Preferences");
});
for (const control of [languageControl, themeControl]) {
  control.addEventListener("click", () => document.querySelector(".landing-menu")?.removeAttribute("open"));
}
phoneLanding.addEventListener("change", placeLandingPreferences);
new MutationObserver(placeLandingPreferences).observe(document.body, {
  attributes: true,
  attributeFilter: ["data-state"],
});

// The expanded guide/sample library is a quick index on desktop; on a phone,
// open only the topic the reader wants to scan.
if (window.matchMedia("(max-width: 560px)").matches) {
  document.querySelectorAll<HTMLDetailsElement>(".guide-group-details, .sample-category").forEach((group) => (group.open = false));
}

document.querySelector<HTMLAnchorElement>("[data-home-link]")?.addEventListener("click", (event) => {
  if (document.body.dataset.state === "empty") return;
  event.preventDefault();
  // Use a distinct entry so the logo always returns to a fresh landing page,
  // even when the analyzer itself is already at `/`.
  location.assign(`${location.pathname}?home=1`);
});

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
  const target = e.target as Element;
  const anchor = target.closest<HTMLAnchorElement>('a[href^="#"]');
  if (anchor) {
    const id = decodeURIComponent(anchor.hash.slice(1));
    const destination = id ? document.getElementById(id) : null;
    if (destination instanceof HTMLDetailsElement) destination.open = true;
    for (let parent = destination?.parentElement; parent; parent = parent.parentElement) {
      if (parent instanceof HTMLDetailsElement) parent.open = true;
    }
    document.querySelector(".landing-menu")?.removeAttribute("open");
  }
  const button = (e.target as Element | null)?.closest?.<HTMLButtonElement>("button[data-opens]");
  if (button) {
    if (button.dataset.opens?.startsWith("picker")) void app().catch(() => undefined);
    document.getElementById(button.dataset.opens ?? "")?.click();
  }
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
    void app().catch(() => undefined);
    document.getElementById(document.body.dataset.state === "empty" ? "picker-empty" : "picker")?.click();
  }
});

// Offline after the first visit, and installable. Only in production: in
// development the cache would serve stale modules.
if (import.meta.env.PROD && "serviceWorker" in navigator) {
  void navigator.serviceWorker.register(siteUrl("sw.js"));
}

document.body.dataset.state = "empty";
document.body.dataset.heroButton = "shown";
placeLandingPreferences();
const asked = new URLSearchParams(location.search);
if (asked.has("home")) history.replaceState(null, "", location.pathname);
// What a link asked to open goes first: a sample by name, or a file shared to the app.
const linked = asked.get("sample");
if (linked && [...document.querySelectorAll<HTMLElement>("[data-sample]")].some((b) => b.dataset.name === linked)) {
  // Load only the file the link asked for.
  document.body.dataset.state = "loading";
  withApp((a) => a.openLinkedSample());
} else if (linked) history.replaceState(null, "", location.pathname);
if (asked.has("shared") || asked.has("share-error")) withApp((a) => void a.openShared());
countVisit();
