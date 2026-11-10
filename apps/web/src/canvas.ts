/** Shared by the canvases: the hex grid and the DEFLATE player's strip. */

export const MONO = 'ui-monospace, "SF Mono", "JetBrains Mono", Menlo, Consolas, monospace';

/** Two-digit upper-case hex for every byte value, built once. */
export const HEX = Array.from({ length: 256 }, (_, i) => i.toString(16).padStart(2, "0").toUpperCase());

/** Reads the theme's CSS custom properties, which canvas drawing cannot use directly. */
export function themeColors() {
  const css = getComputedStyle(document.documentElement);
  const v = (name: string) => css.getPropertyValue(name).trim();
  /**
   * Custom properties keep their original tokens when read from the root,
   * including light-dark(...). Resolve a colour through a real CSS property
   * before passing it to Canvas2D, which accepts colours but not CSS vars.
   */
  const color = (name: string) => {
    const probe = document.createElement("span");
    probe.style.cssText = "position:fixed;visibility:hidden;pointer-events:none";
    probe.style.color = `var(${name})`;
    document.documentElement.append(probe);
    const resolved = getComputedStyle(probe).color;
    probe.remove();
    const channels = resolved.match(/[\d.]+/g)?.slice(0, 3).map(Number);
    if (!channels || channels.length < 3) return "#000000";
    return `#${channels.map((n) => Math.round(n).toString(16).padStart(2, "0")).join("")}`;
  };
  /** `#rrggbb` at the given opacity. */
  const rgba = (hex: string, a: number) => {
    const n = parseInt(hex.slice(1), 16);
    return `rgba(${(n >> 16) & 255},${(n >> 8) & 255},${n & 255},${a})`;
  };
  return { v, color, rgba };
}
