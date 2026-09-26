// What the black boxes on a PDF page cover, drawn: the part of the page
// around them, the boxes where the page has them, and the text still under
// each one, in white on the black. A toggle shows it as a viewer does, with
// the text hidden, so the two can be compared. Positions come from the
// core's estimate of where each string sits, so each text is stretched to
// the width the core gave it.
import type { Blackout, PageArea } from "./model";

const SVG = "http://www.w3.org/2000/svg";
/** Points of page kept around the boxes. */
const MARGIN = 28;

function svg<K extends keyof SVGElementTagNameMap>(
  tag: K,
  attrs: Record<string, string | number>,
): SVGElementTagNameMap[K] {
  const e = document.createElementNS(SVG, tag);
  for (const [k, v] of Object.entries(attrs)) e.setAttribute(k, String(v));
  return e;
}

/** The smallest area holding all of these, grown by the margin and kept on the page. */
function around(areas: PageArea[], media: PageArea): PageArea {
  let [l, b, r, t] = [Infinity, Infinity, -Infinity, -Infinity];
  for (const [al, ab, ar, at] of areas) {
    l = Math.min(l, al);
    b = Math.min(b, ab);
    r = Math.max(r, ar);
    t = Math.max(t, at);
  }
  return [
    Math.max(media[0], l - MARGIN),
    Math.max(media[1], b - MARGIN),
    Math.min(media[2], r + MARGIN),
    Math.min(media[3], t + MARGIN),
  ];
}

/** One page's boxes and what they cover, with a toggle between the two views. */
export function blackoutFigure(b: Blackout): HTMLElement {
  const figure = document.createElement("figure");
  figure.className = "blackout";
  const [ml, , , mt] = b.media;
  const view = around([...b.boxes, ...b.texts.map((t) => t.area), ...b.context.map((t) => t.area)], b.media);
  const [vl, vb, vr, vt] = view;
  // PDF counts up from the bottom; SVG counts down from the top.
  const y = (v: number) => mt - v;
  const pic = svg("svg", {
    viewBox: `${vl - ml} ${y(vt)} ${vr - vl} ${vt - vb}`,
    role: "img",
    "aria-label": `Page ${b.page}: black boxes, and the text under them`,
  });
  pic.append(
    svg("rect", {
      class: "blackout-page",
      x: vl - ml,
      y: y(vt),
      width: vr - vl,
      height: vt - vb,
    }),
  );
  for (const [l, bottom, r, t] of b.boxes) {
    pic.append(
      svg("rect", {
        class: "blackout-box",
        x: l - ml,
        y: y(t),
        width: r - l,
        height: t - bottom,
      }),
    );
  }
  // What is left showing, as the page shows it.
  for (const { area, text } of b.context) pic.append(words(area, text, "blackout-context"));
  for (const { area, text } of b.texts) {
    const [l, bottom, r, t] = area;
    const h = t - bottom;
    if (text) {
      pic.append(words(area, text, "blackout-text"));
    } else {
      // Text is there, in codes that do not read as letters.
      pic.append(
        svg("rect", {
          class: "blackout-unread",
          x: l - ml,
          y: y(t),
          width: r - l,
          height: h,
        }),
      );
    }
  }
  function words([l, bottom, r, t]: PageArea, text: string, cls: string): SVGTextElement {
    const h = t - bottom;
    const e = svg("text", {
      class: cls,
      x: l - ml,
      // The core's area runs from a fifth of the size below the baseline.
      y: y(bottom + 0.2 * h),
      "font-size": h * 0.8,
      textLength: Math.max(1, r - l),
      lengthAdjust: "spacingAndGlyphs",
    });
    e.textContent = text;
    return e;
  }

  const caption = document.createElement("figcaption");
  const label = document.createElement("span");
  label.textContent = `Page ${b.page}: what anyone can still read under the boxes`;
  const toggle = document.createElement("button");
  toggle.className = "link blackout-toggle";
  toggle.type = "button";
  toggle.setAttribute("aria-pressed", "false");
  toggle.textContent = "Show as a viewer does";
  toggle.addEventListener("click", () => {
    const hidden = figure.classList.toggle("is-viewer");
    toggle.setAttribute("aria-pressed", String(hidden));
    toggle.textContent = hidden ? "Show what is under the boxes" : "Show as a viewer does";
    label.textContent = hidden
      ? `Page ${b.page}: as a viewer shows it`
      : `Page ${b.page}: what anyone can still read under the boxes`;
  });
  caption.append(label, toggle);
  figure.append(pic, caption);
  return figure;
}
