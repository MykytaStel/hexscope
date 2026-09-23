/** Shared by the canvases: the hex grid and the DEFLATE player's strip. */

export const MONO = 'ui-monospace, "SF Mono", "JetBrains Mono", Menlo, Consolas, monospace';

/** Two-digit upper-case hex for every byte value, built once. */
export const HEX = Array.from({ length: 256 }, (_, i) => i.toString(16).padStart(2, "0").toUpperCase());

/** Reads the theme's CSS custom properties, which canvas drawing cannot use directly. */
export function themeColors() {
  const css = getComputedStyle(document.documentElement);
  const v = (name: string) => css.getPropertyValue(name).trim();
  /** `#rrggbb` at the given opacity. */
  const rgba = (hex: string, a: number) => {
    const n = parseInt(hex.slice(1), 16);
    return `rgba(${(n >> 16) & 255},${(n >> 8) & 255},${n & 255},${a})`;
  };
  return { v, rgba };
}
