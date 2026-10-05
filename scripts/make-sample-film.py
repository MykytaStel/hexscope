#!/usr/bin/env python3
"""Original synthetic negative for the film lab; no user photographs or data."""
import math
from pathlib import Path
import struct
import zlib

WIDTH, HEIGHT = 640, 440
BASE = (230, 170, 100)


def linear(v):
    s = v / 255
    return s / 12.92 if s <= 0.04045 else ((s + 0.055) / 1.055) ** 2.4


def encoded(v):
    s = 12.92 * v if v <= 0.0031308 else 1.055 * v ** (1 / 2.4) - 0.055
    return round(255 * max(0, min(1, s)))


pixels = bytearray(BASE * (WIDTH * HEIGHT))


def put(x, y, color):
    at = (y * WIDTH + x) * 3
    pixels[at:at + 3] = bytes(color)


for y in range(52, HEIGHT - 52):
    for x in range(56, WIDTH - 56):
        u, v = (x - 56) / (WIDTH - 112), (y - 52) / (HEIGHT - 104)
        if v < 0.47:
            scene = (0.35 + v * 0.5, 0.6 + v * 0.3, 0.94)
            if (u - 0.76) ** 2 + (v - 0.18) ** 2 < 0.008:
                scene = (0.98, 0.87, 0.5)
        elif v < 0.61 + 0.07 * math.sin(u * 12):
            scene = (0.16, 0.28, 0.31)
        elif v < 0.78 + 0.02 * math.sin(u * 37):
            scene = (0.3, 0.6 + 0.05 * math.sin(v * 180), 0.79)
        else:
            scene = (0.3 + u * 0.1, 0.44 + u * 0.1, 0.17)
        for trunk in (0.08, 0.16, 0.9):
            if 0.39 < v < 0.94 and abs(u - trunk) < 0.008 + max(0, 0.72 - v) * 0.085:
                scene = (0.025, 0.08, 0.045)
        put(x, y, tuple(encoded(linear(base) * math.exp(-(0.15 + tone * 1.6))) for base, tone in zip(BASE, scene)))
for x in range(20, WIDTH - 20, 46):
    for y0 in (9, HEIGHT - 26):
        for y in range(y0, y0 + 17):
            for xx in range(x, x + 19):
                put(xx, y, (247, 245, 240))


def chunk(kind, data):
    return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))


rows = b"".join(b"\0" + pixels[y * WIDTH * 3:(y + 1) * WIDTH * 3] for y in range(HEIGHT))
png = (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", WIDTH, HEIGHT, 8, 2, 0, 0, 0))
       + chunk(b"tEXt", b"Comment\0Synthetic demonstration; not a photograph or detector validation case.")
       + chunk(b"IDAT", zlib.compress(rows, 9)) + chunk(b"IEND", b""))
target = Path(__file__).resolve().parents[1] / "apps/web/public/samples/film-negative.png"
target.write_bytes(png)
print(f"Generated synthetic negative: {WIDTH} x {HEIGHT}, {len(png)} bytes")
