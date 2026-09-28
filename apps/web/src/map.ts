// Where a photo was taken, drawn: the land around the place, a pin on it,
// and the whole world small in a corner. Drawn here from a coastline kept
// in the page (land.ts) — no map tiles are asked for, so the place never
// leaves the tab unless the person clicks through to a map site.
import { LAND } from "./land";

const NS = "http://www.w3.org/2000/svg";
/** Units a degree in LAND. */
const UNIT = 2;
/** The region shown: degrees of longitude across; the height follows. */
const SPAN = 36;
const ASPECT = 0.6;

function svg<K extends keyof SVGElementTagNameMap>(tag: K, attrs: Record<string, string | number>): SVGElementTagNameMap[K] {
  const e = document.createElementNS(NS, tag);
  for (const [k, v] of Object.entries(attrs)) e.setAttribute(k, String(v));
  return e;
}

/** The land drawn three times across, so a place near the date line has its neighbours on both sides. */
function land(cls: string): SVGGElement {
  const g = svg("g", { class: cls });
  for (const shift of [-360 * UNIT, 0, 360 * UNIT]) g.append(svg("path", { d: LAND, transform: `translate(${shift} 0)` }));
  return g;
}

/** A map of where `latitude`, `longitude` is, with a pin on it. */
export function placeMap(latitude: number, longitude: number, label: string): SVGSVGElement {
  const x = (longitude + 180) * UNIT;
  const y = (90 - latitude) * UNIT;
  const w = SPAN * UNIT;
  const h = w * ASPECT;
  // Kept within the poles: nothing to see beyond them.
  const top = Math.min(Math.max(y - h / 2, 0), 180 * UNIT - h);
  const left = x - w / 2;

  const map = svg("svg", { viewBox: `${left} ${top} ${w} ${h}`, class: "place-map-svg", role: "img", "aria-label": label });
  map.append(svg("rect", { x: left, y: top, width: w, height: h, class: "map-sea" }));
  // Every ten degrees, faint: enough to read how far north or east.
  const lines = svg("g", { class: "map-grid" });
  for (let lon = Math.ceil((left / UNIT - 180) / 10) * 10; lon * UNIT + 180 * UNIT <= left + w; lon += 10) {
    const gx = (lon + 180) * UNIT;
    lines.append(svg("line", { x1: gx, y1: top, x2: gx, y2: top + h }));
  }
  for (let lat = -80; lat <= 80; lat += 10) {
    const gy = (90 - lat) * UNIT;
    if (gy > top && gy < top + h) lines.append(svg("line", { x1: left, y1: gy, x2: left + w, y2: gy }));
  }
  map.append(lines, land("map-land"));

  const pin = svg("g", { class: "map-pin", transform: `translate(${x} ${y})` });
  pin.append(svg("circle", { r: 3.2, class: "map-pin-pulse" }), svg("circle", { r: 1.5, class: "map-pin-dot" }));
  map.append(pin);

  // The world, small, bottom left, with the region marked on it.
  const scale = w / (360 * UNIT) / 3.2;
  const inset = svg("g", { class: "map-inset", transform: `translate(${left + 2} ${top + h - 180 * UNIT * scale - 2}) scale(${scale})` });
  inset.append(svg("rect", { x: 0, y: 0, width: 360 * UNIT, height: 180 * UNIT, class: "map-inset-sea" }));
  const world = svg("path", { d: LAND, class: "map-inset-land" });
  const world360 = 360 * UNIT;
  const region = svg("rect", { x: ((left % world360) + world360) % world360, y: top, width: w, height: h, class: "map-inset-region" });
  inset.append(world, region);
  map.append(inset);
  return map;
}
