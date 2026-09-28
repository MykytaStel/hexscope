"""Writes apps/web/src/land.ts: the world's land as one SVG path, for the
map that shows where a photo was taken without asking a server for tiles.

    curl -LO https://cdn.jsdelivr.net/npm/world-atlas@2/land-110m.json
    python3 scripts/make-land.py land-110m.json

Natural Earth's 1:110m land is public domain; world-atlas is ISC.
Equirectangular, two units a degree, integer points, relative moves.
"""

import json
import sys

HEADER = """// Where land is, for drawing a place without asking anyone for a map:
// Natural Earth's 1:110m land (public domain, via world-atlas, ISC),
// equirectangular at two units a degree — x = (longitude + 180) × 2,
// y = (90 − latitude) × 2 — as one SVG path. Generated; do not edit by hand.
"""


def main(path: str) -> None:
    t = json.load(open(path))
    sx, sy = t["transform"]["scale"]
    tx, ty = t["transform"]["translate"]
    arcs = []
    for a in t["arcs"]:
        x = y = 0
        pts = []
        for dx, dy in a:
            x += dx
            y += dy
            pts.append((x * sx + tx, y * sy + ty))
        arcs.append(pts)

    def arc(i: int):
        return arcs[i] if i >= 0 else arcs[~i][::-1]

    out = []
    for g in t["objects"]["land"]["geometries"]:
        polys = g["arcs"] if g["type"] == "MultiPolygon" else [g["arcs"]]
        for poly in polys:
            for ring in poly:
                pts = []
                for i in ring:
                    p = arc(i)
                    pts.extend(p if not pts else p[1:])
                q = []
                for lon, lat in pts:
                    p = (round((lon + 180) * 2), round((90 - lat) * 2))
                    if not q or q[-1] != p:
                        q.append(p)
                if len(q) < 4:
                    continue
                s = f"M{q[0][0]} {q[0][1]}"
                px, py = q[0]
                for x, y in q[1:]:
                    s += f"l{x - px}{' ' if y - py >= 0 else ''}{y - py}"
                    px, py = x, y
                out.append(s + "z")
    with open("apps/web/src/land.ts", "w") as f:
        f.write(HEADER + "\n" + f'export const LAND = "{"".join(out)}";\n')


if __name__ == "__main__":
    main(sys.argv[1])
