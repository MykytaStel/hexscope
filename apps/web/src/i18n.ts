// The page's language: which one it reads in, the button that switches it,
// and the translation of whatever the page shows. The Ukrainian words live
// in i18n-uk.ts, loaded only when a page is to read in Ukrainian or was
// written in it: an English reader never downloads them.
export type Locale = "en" | "uk";

const LANGUAGE_KEY = "hexscope.language";
/** Ukrainian copy that features and pages register for themselves, beside the dictionary's. */
export const lazyTranslations: Record<string, string> = {};
/** The same, from the Ukrainian back to the English. */
export const lazyReverse = new Map<string, string>();

/** Adds copy for an optional, dynamically loaded feature without bloating the first page bundle. */
export function registerLazyTranslations(translations: Readonly<Record<string, string>>): void {
  Object.assign(lazyTranslations, translations);
  // A page written in Ukrainian (the /uk/ pages) turns back to English with these too.
  for (const [english, ukrainian] of Object.entries(translations)) lazyReverse.set(ukrainian, english);
}

/** What the Ukrainian dictionary does, once it has loaded. */
interface Dictionary {
  translate(value: string): string;
  untranslate(value: string): string | undefined;
}
let dictionary: Dictionary | undefined;

/** Called by i18n-uk.ts as it loads, however it was loaded. */
export function useDictionary(loaded: Dictionary): void {
  dictionary = loaded;
}

/** The English a Ukrainian text was translated from, keeping the spaces around it. */
export function untranslate(value: string): string | undefined {
  return dictionary?.untranslate(value);
}

/** `value` in `locale`; English until the Ukrainian dictionary has loaded. */
export function translateText(value: string, locale: Locale): string {
  return locale === "en" || !dictionary ? value : dictionary.translate(value);
}

export function resolveLocale(saved: string | null, browserLanguages: readonly string[]): Locale {
  if (saved === "en" || saved === "uk") return saved;
  return browserLanguages.some((language) => language.toLowerCase().startsWith("uk")) ? "uk" : "en";
}

let activeLocale: Locale | undefined;
let installed = false;
const sourceText = new WeakMap<Text, { source: string; rendered: string }>();
const sourceAttributes = new WeakMap<Element, Map<string, { source: string; rendered: string }>>();
/** Copy each kind of page needs in Ukrainian, loaded with the dictionary. */
const companions: (() => Promise<unknown>)[] = [];
let loading: Promise<void> | undefined;

/**
 * Loads the Ukrainian dictionary and every page's own copy, once, then
 * translates the page again and tells the parts that listen.
 */
export function loadUkrainian(): Promise<void> {
  loading ??= Promise.all([import("./i18n-uk"), ...companions.map((load) => load())])
    .then(() => {
      if (installed) localizeDocument();
      window.dispatchEvent(new Event("hexscope:locale"));
    })
    .catch((error: unknown) => {
      // Offline at the first visit, say: the next switch tries again.
      loading = undefined;
      throw error;
    });
  return loading;
}

/** Copy a page needs in Ukrainian, loaded beside the dictionary rather than with the page. */
export function withUkrainian(load: () => Promise<unknown>): void {
  companions.push(load);
  if (loading) loading = loading.then(load).then(() => localizeDocument());
}

function storageValue(): string | null {
  try {
    return localStorage.getItem(LANGUAGE_KEY);
  } catch {
    return null;
  }
}

export function currentLocale(): Locale {
  if (activeLocale) return activeLocale;
  return resolveLocale(storageValue(), typeof navigator === "undefined" ? [] : [...navigator.languages, navigator.language]);
}

function localizeText(node: Text): void {
  const parent = node.parentElement;
  if (!parent || parent.closest("script, style, textarea, [data-language-control]")) return;
  let state = sourceText.get(node);
  // Text never translated may be Ukrainian a dictionary loaded since can turn back.
  if (!state || node.data !== state.rendered || state.source === state.rendered) {
    const source = untranslate(node.data) ?? node.data;
    state = { source, rendered: node.data };
  }
  const translated = translateText(state.source, currentLocale());
  state.rendered = translated;
  sourceText.set(node, state);
  if (translated !== node.data) node.data = translated;
}

function localizeAttribute(element: Element, name: string): void {
  if (element.closest("[data-language-control]")) return;
  let values = sourceAttributes.get(element);
  if (!values) sourceAttributes.set(element, (values = new Map()));
  let state = values.get(name);
  const current = element.getAttribute(name) ?? "";
  if (!state || current !== state.rendered || state.source === state.rendered) {
    const source = untranslate(current) ?? current;
    state = { source, rendered: current };
  }
  const translated = translateText(state.source, currentLocale());
  state.rendered = translated;
  values.set(name, state);
  if (translated !== element.getAttribute(name)) element.setAttribute(name, translated);
}

function localizeNode(root: Node): void {
  if (root.nodeType === Node.TEXT_NODE) {
    localizeText(root as Text);
    return;
  }
  if (!(root instanceof Element) || root.matches("script, style, textarea")) return;
  for (const element of [root, ...root.querySelectorAll("*")]) {
    if (element.matches("script, style, textarea")) continue;
    for (const name of ["aria-label", "title", "alt", "placeholder", "content"]) {
      if (element.hasAttribute(name)) localizeAttribute(element, name);
    }
  }
  const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
  let text: Text | null;
  while ((text = walker.nextNode() as Text | null)) localizeText(text);
}

function localizeDocument(): void {
  document.documentElement.lang = currentLocale();
  localizeNode(document.body);
  const title = document.querySelector("head title");
  if (title) localizeNode(title);
  for (const meta of document.querySelectorAll<HTMLMetaElement>('meta[name="description"], meta[property^="og:"]')) {
    localizeAttribute(meta, "content");
  }
}

export function setLocale(locale: Locale): void {
  activeLocale = locale;
  try {
    localStorage.setItem(LANGUAGE_KEY, locale);
  } catch {
    // The choice still applies for this page even when storage is disabled.
  }
  localizeDocument();
  window.dispatchEvent(new Event("hexscope:locale"));
  if (locale === "uk") void loadUkrainian().catch(() => undefined);
}

/** The language the page was written in: Ukrainian for the /uk/ pages, translated at build. */
export function writtenIn(): Locale {
  // theme.js keeps it before it sets `lang` to the language the page will read in.
  const root = document.documentElement;
  return (root.dataset.written ?? root.lang) === "uk" ? "uk" : "en";
}

/** Installs the saved/browser language and translates both static and later UI text. */
export function installLocale(): void {
  if (installed || typeof document === "undefined") return;
  installed = true;
  // A page written in Ukrainian (/uk/) reads in Ukrainian unless a language was chosen.
  const written = writtenIn() === "uk" ? ["uk"] : [];
  activeLocale = resolveLocale(storageValue(), [...written, ...(typeof navigator === "undefined" ? [] : [...navigator.languages, navigator.language])]);
  localizeDocument();
  // Read in Ukrainian, or written in it and to turn back to English.
  if (activeLocale === "uk" || written.length > 0) void loadUkrainian().catch(() => undefined);
  new MutationObserver((records) => {
    for (const record of records) {
      if (record.type === "characterData") localizeText(record.target as Text);
      if (record.type === "attributes" && record.attributeName) localizeAttribute(record.target as Element, record.attributeName);
      if (record.type === "childList") for (const node of record.addedNodes) localizeNode(node);
    }
  }).observe(document.documentElement, {
    subtree: true,
    childList: true,
    characterData: true,
    attributes: true,
    attributeFilter: ["aria-label", "title", "alt", "placeholder"],
  });
}

export function languageButton(): HTMLButtonElement {
  const button = document.createElement("button");
  button.type = "button";
  button.className = "btn language-button";
  button.dataset.languageControl = "true";
  const refresh = () => {
    const target = currentLocale() === "en" ? "uk" : "en";
    button.textContent = target === "uk" ? "UA" : "EN";
    button.setAttribute("aria-label", target === "uk" ? "Перемкнути мову на українську" : "Switch language to English");
    button.title = target === "uk" ? "Перемкнути мову на українську" : "Switch language to English";
  };
  refresh();
  button.addEventListener("click", () => setLocale(currentLocale() === "en" ? "uk" : "en"));
  window.addEventListener("hexscope:locale", refresh);
  return button;
}
