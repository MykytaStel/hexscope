// Ways to thank the developer: shown on /support, and once, gently, after a
// few clean copies. Plain links, opened in a new tab: no payment script runs
// on this site, and nothing about the visitor goes with them.
import { currentLocale, type Locale } from "./i18n";

export interface Way {
  label: string;
  href: string;
  /** Who it suits, in a few words. */
  note: string;
  /** Whose readers see it first: the way that suits them best. */
  first: Locale;
}

/** With none, the page shows only the other ways to help. */
export const WAYS: Way[] = [
  {
    label: "Ko-fi",
    href: "https://ko-fi.com/mykytacherven",
    note: "From anywhere: a card or PayPal, once or every month.",
    first: "en",
  },
  {
    label: "Monobank jar",
    href: "https://send.monobank.ua/jar/8onzpLEBC",
    note: "A card from any Ukrainian bank, Apple Pay or Google Pay.",
    first: "uk",
  },
];

/** The ways in the order that suits a reader of `locale`: theirs first. */
export function waysFor(locale: Locale): Way[] {
  return [...WAYS.filter((w) => w.first === locale), ...WAYS.filter((w) => w.first !== locale)];
}

const KEY = "hexscope.cleans";
const QUIET = "hexscope.thanks-hidden";
/** The clean copies after which the thanks line shows, once each. */
const AT = [3, 25];

function stored(key: string): string | null {
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}

function store(key: string, value: string): void {
  try {
    localStorage.setItem(key, value);
  } catch {
    // A private window: the line simply shows again, or not at all.
  }
}

/**
 * Counts a clean copy made in this browser — the number only, kept here —
 * and, at the third and the twenty-fifth, a quiet line to thank the
 * developer; never again once closed.
 */
export function thanksLine(): HTMLElement | null {
  const n = Number(stored(KEY) ?? 0) + 1;
  store(KEY, String(n));
  if (!AT.includes(n) || stored(QUIET)) return null;
  const line = document.createElement("p");
  line.className = "thanks-line";
  const text = document.createElement("span");
  text.textContent = "hexscope is free, with no ads and no tracking, and made by one person. If it helped, ";
  const link = document.createElement("a");
  link.href = location.pathname.startsWith("/uk/") ? "/uk/support" : "/support";
  link.textContent = "you can thank them";
  const close = document.createElement("button");
  close.type = "button";
  close.className = "link thanks-close";
  close.setAttribute("aria-label", "Do not show this again");
  close.textContent = "×";
  close.addEventListener("click", () => {
    store(QUIET, "1");
    line.remove();
  });
  line.append(text, link, document.createTextNode("."), close);
  return line;
}

/** Fills the support page's list of ways to give, or hides it while there are none. */
export function showWays(list: HTMLElement): void {
  const section = list.closest("section");
  if (WAYS.length === 0) {
    if (section) section.hidden = true;
    // The sections left are numbered from one again.
    const shown = document.querySelectorAll<HTMLElement>(".story-text > section:not([hidden]) .num");
    shown.forEach((num, i) => (num.textContent = String(i + 1)));
    return;
  }
  const fill = () =>
    list.replaceChildren(
      ...waysFor(currentLocale()).map((way) => {
        const item = document.createElement("li");
        const a = document.createElement("a");
        a.className = "btn";
        a.href = way.href;
        a.target = "_blank";
        a.rel = "noopener noreferrer";
        a.textContent = way.label;
        item.append(a, document.createTextNode(` ${way.note}`));
        return item;
      }),
    );
  fill();
  window.addEventListener("hexscope:locale", fill);
}
