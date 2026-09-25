#!/usr/bin/env python3
"""Writes a progressive JPEG: a sky, a sun and hills with grass, so that some
blocks are flat and cheap and others busy and costly. Saved as libjpeg's
standard progressive script — DC first, then AC bands refined bit by bit —
with no metadata. Needs Pillow.

  python3 scripts/make-sample-progressive.py
"""
import os
import random

from PIL import Image, ImageDraw

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = [
    os.path.join(HERE, "..", "crates", "hexscope-core", "tests", "fixtures", "progressive.jpg"),
    os.path.join(HERE, "..", "apps", "web", "public", "samples", "progressive.jpg"),
]
W, H = 480, 320

random.seed(6709)
im = Image.new("RGB", (W, H))
px = im.load()
for y in range(H):
    t = y / H
    for x in range(W):
        px[x, y] = (int(40 + 180 * t), int(90 + 110 * t), int(200 - 40 * t))
d = ImageDraw.Draw(im)
d.ellipse((330, 50, 400, 120), fill=(255, 226, 150))
# Hills, far and near; the near one covered in grass.
far = [(0, 210)] + [(x, 200 - 30 * ((x / 70) % 2 - 0.5) ** 2 * 4) for x in range(0, W + 1, 35)] + [(W, H), (0, H)]
d.polygon(far, fill=(70, 110, 90))
near = [(0, 250)] + [(x, 245 - 18 * ((x / 120) % 2 - 1) ** 2) for x in range(0, W + 1, 20)] + [(W, H), (0, H)]
d.polygon(near, fill=(60, 130, 50))
for _ in range(9000):
    x, y = random.randrange(W), random.randrange(235, H)
    if im.getpixel((x, y))[1] > 100:
        g = random.randrange(90, 200)
        d.line((x, y, x + random.randrange(-2, 3), y - random.randrange(3, 9)), fill=(g // 3, g, g // 4))

for path in OUT:
    im.save(path, "JPEG", quality=85, progressive=True, optimize=True)
    print(path, os.path.getsize(path))
