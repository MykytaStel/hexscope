// Light, dark, or the system's: chosen with one button, remembered in this
// browser only. `public/theme.js` applies the choice before the page paints;
// this is how the page changes it, and how the canvases hear of it.

export type Theme = "auto" | "light" | "dark";

const KEY = "hexscope.theme";
const EVENT = "hexscope:theme";
const ORDER: Theme[] = ["auto", "light", "dark"];

export function currentTheme(): Theme {
  const t = document.documentElement.dataset.theme;
  return t === "light" || t === "dark" ? t : "auto";
}

export function setTheme(t: Theme): void {
  if (t === "auto") delete document.documentElement.dataset.theme;
  else document.documentElement.dataset.theme = t;
  try {
    if (t === "auto") localStorage.setItem(KEY, "auto");
    else localStorage.setItem(KEY, t);
  } catch {
    // Not remembered; it still applies now.
  }
  window.dispatchEvent(new Event(EVENT));
}

/** Calls back whenever the colours may have changed: a choice here, or the system's own switch. */
export function onThemeChange(cb: () => void): void {
  window.addEventListener(EVENT, cb);
  matchMedia("(prefers-color-scheme: dark)").addEventListener("change", cb);
}

const ICONS: Record<Theme, string> = {
  // Half a circle filled: whichever the system uses.
  auto: '<svg viewBox="0 0 16 16" aria-hidden="true"><circle cx="8" cy="8" r="6" fill="none" stroke="currentColor" stroke-width="1.5"/><path d="M8 2a6 6 0 0 1 0 12z" fill="currentColor"/></svg>',
  light:
    '<svg viewBox="0 0 16 16" aria-hidden="true"><circle cx="8" cy="8" r="3" fill="currentColor"/><path d="M8 1v2M8 13v2M1 8h2M13 8h2M3 3l1.4 1.4M11.6 11.6 13 13M3 13l1.4-1.4M11.6 4.4 13 3" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/></svg>',
  dark: '<svg viewBox="0 0 16 16" aria-hidden="true"><path d="M13.5 10.2A6 6 0 0 1 5.8 2.5a6 6 0 1 0 7.7 7.7z" fill="currentColor"/></svg>',
};
const NAMES: Record<Theme, string> = {
  auto: "Theme: as your system (click for light)",
  light: "Theme: light (click for dark)",
  dark: "Theme: dark (click to follow your system)",
};

/** A button that steps through the themes: the system's, light, dark. */
export function themeButton(): HTMLButtonElement {
  const b = document.createElement("button");
  b.type = "button";
  b.className = "btn btn-icon theme-button";
  const show = () => {
    const t = currentTheme();
    b.innerHTML = ICONS[t];
    b.title = NAMES[t];
    b.setAttribute("aria-label", NAMES[t]);
  };
  show();
  b.addEventListener("click", () => {
    setTheme(ORDER[(ORDER.indexOf(currentTheme()) + 1) % ORDER.length]);
    show();
  });
  window.addEventListener(EVENT, show);
  return b;
}
