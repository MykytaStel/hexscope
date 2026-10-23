/** Shared translation state; keeping it below the loader avoids an import cycle. */
export type Locale = "en" | "uk";

export interface Dictionary {
  translate(value: string): string;
  untranslate(value: string): string | undefined;
}

/** Ukrainian copy that features and pages register beside the main dictionary. */
export const lazyTranslations: Record<string, string> = {};
/** Reverse lookup for optional copy on Ukrainian pages. */
export const lazyReverse = new Map<string, string>();

let dictionary: Dictionary | undefined;

/** Called by the optional Ukrainian dictionary when it loads. */
export function useDictionary(loaded: Dictionary): void {
  dictionary = loaded;
}

export function loadedDictionary(): Dictionary | undefined {
  return dictionary;
}
