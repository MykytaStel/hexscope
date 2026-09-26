#!/usr/bin/env python3
"""Builds apps/web/public/samples/photo.jpg: a JPEG carrying the EXIF a phone
would write — camera, serial number, time, a thumbnail, and GPS.

The location is the Eiffel Tower, a public place. Every value is invented.
Needs macOS `sips` to encode the JPEGs; everything else is written by hand so
the exact bytes are known.

    python3 scripts/make-sample-photo.py

With `cropped`, it builds cropped.jpg instead: the same photo cropped to its
left part, the tower gone, with the thumbnail still showing the whole scene
— what an editor that does not update the thumbnail leaves behind — and the
XMP record such an editor adds: the place typed in, the original file's
name, and each step of the edit.

    python3 scripts/make-sample-photo.py cropped
"""
import os
import struct
import subprocess
import tempfile
import zlib

OUT = os.path.join(os.path.dirname(__file__), "..", "apps", "web", "public", "samples", "photo.jpg")


def png(w, h, pixel):
    raw = bytearray()
    for y in range(h):
        raw.append(0)
        for x in range(w):
            raw += bytes(pixel(x, y))
    def chunk(t, d):
        return struct.pack(">I", len(d)) + t + d + struct.pack(">I", zlib.crc32(t + d) & 0xFFFFFFFF)
    return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0))
            + chunk(b"IDAT", zlib.compress(bytes(raw), 9)) + chunk(b"IEND", b""))


def dusk(w, h):
    """A dusk sky over a dark horizon, with a tower-ish silhouette."""
    def pixel(x, y):
        t = y / h
        sky = (int(250 - 170 * t), int(150 - 110 * t), int(110 + 60 * t))
        ground = y > h * 0.78
        cx = abs(x - w * 0.62) / w
        tower = cx < 0.004 + (y / h - 0.18) * 0.07 and y > h * 0.18
        if ground or tower:
            return (22, 18, 34)
        return sky
    return pixel


def jpeg(w, h, quality, pixel=None):
    with tempfile.TemporaryDirectory() as d:
        src, dst = os.path.join(d, "in.png"), os.path.join(d, "out.jpg")
        open(src, "wb").write(png(w, h, pixel or dusk(w, h)))
        subprocess.run(["sips", "-s", "format", "jpeg", "-s", "formatOptions", str(quality), src, "--out", dst],
                       check=True, capture_output=True)
        return open(dst, "rb").read()


# --- a big-endian TIFF block, laid out by hand -------------------------------

def ascii_(s): return (2, len(s) + 1, s.encode() + b"\0")
def short(*v): return (3, len(v), b"".join(struct.pack(">H", x) for x in v))
def long_(*v): return (4, len(v), b"".join(struct.pack(">I", x) for x in v))
def rational(*v): return (5, len(v), b"".join(struct.pack(">II", n, d) for n, d in v))
def byte(*v): return (1, len(v), bytes(v))
def undefined(b): return (7, len(b), b)


def tiff(thumbnail):
    ifd0 = [
        (0x010F, ascii_("hexscope")),
        (0x0110, ascii_("Sample Camera X1")),
        (0x0112, short(1)),
        (0x0131, ascii_("hexscope sample generator")),
        (0x0132, ascii_("2026:06:14 18:32:07")),
        (0x8769, long_(0)),  # Exif IFD, patched below
        (0x8825, long_(0)),  # GPS IFD, patched below
    ]
    exif = [
        (0x829A, rational((1, 250))),
        (0x829D, rational((18, 10))),
        (0x8827, short(100)),
        (0x9000, undefined(b"0232")),
        (0x9003, ascii_("2026:06:14 18:32:07")),
        (0x9011, ascii_("+02:00")),
        (0x920A, rational((26, 1))),
        (0xA430, ascii_("Sample Owner")),
        (0xA431, ascii_("HX-000042")),
        (0xA434, ascii_("Sample Lens 26mm f/1.8")),
    ]
    gps = [
        (0x00, byte(2, 2, 0, 0)),
        (0x01, ascii_("N")),
        (0x02, rational((48, 1), (51, 1), (3024, 100))),
        (0x03, ascii_("E")),
        (0x04, rational((2, 1), (17, 1), (402, 10))),
        (0x05, byte(0)),
        (0x06, rational((35, 1))),
    ]
    ifd1 = [
        (0x0103, short(6)),     # JPEG compression
        (0x0201, long_(0)),     # thumbnail offset, patched below
        (0x0202, long_(len(thumbnail))),
    ]

    size = lambda entries: 2 + 12 * len(entries) + 4
    off0 = 8
    off_exif = off0 + size(ifd0)
    off_gps = off_exif + size(exif)
    off_ifd1 = off_gps + size(gps)
    values_at = off_ifd1 + size(ifd1)

    # Values first, to know where the thumbnail lands after them.
    blobs = [v for _, (_, _, v) in ifd0 + exif + gps + ifd1 if len(v) > 4]
    off_thumb = values_at + sum(len(b) for b in blobs)

    def patch(entries, tag, value):
        return [(t, long_(value) if t == tag else v) for t, v in entries]
    ifd0 = patch(patch(ifd0, 0x8769, off_exif), 0x8825, off_gps)
    ifd1 = patch(ifd1, 0x0201, off_thumb)

    out = bytearray(b"MM" + struct.pack(">HI", 42, off0))
    values = bytearray()

    def write(entries, next_ifd):
        nonlocal values_at
        out.extend(struct.pack(">H", len(entries)))
        for tag, (typ, count, data) in entries:
            out.extend(struct.pack(">HHI", tag, typ, count))
            if len(data) <= 4:
                out.extend(data.ljust(4, b"\0"))
            else:
                out.extend(struct.pack(">I", values_at))
                values_at += len(data)
                values.extend(data)
        out.extend(struct.pack(">I", next_ifd))

    write(ifd0, off_ifd1)
    write(exif, 0)
    write(gps, 0)
    write(ifd1, 0)
    return bytes(out + values + thumbnail)


# What an editor writes when it exports: invented, but shaped like
# Lightroom's and Photoshop's.
XMP = b"""<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
<rdf:Description rdf:about=""
  xmlns:xmp="http://ns.adobe.com/xap/1.0/"
  xmlns:xmpMM="http://ns.adobe.com/xap/1.0/mm/"
  xmlns:stEvt="http://ns.adobe.com/xap/1.0/sType/ResourceEvent#"
  xmlns:photoshop="http://ns.adobe.com/photoshop/1.0/"
  xmlns:Iptc4xmpCore="http://iptc.org/std/Iptc4xmpCore/1.0/xmlns/"
  xmp:CreatorTool="Sample Editor 2.1"
  xmpMM:PreservedFileName="IMG_0042.HEIC"
  Iptc4xmpCore:Location="Champ de Mars" photoshop:City="Paris" photoshop:Country="France">
 <xmpMM:History><rdf:Seq>
  <rdf:li stEvt:action="derived" stEvt:softwareAgent="Sample Editor 2.1" stEvt:when="2026-06-14T21:05:00+02:00"/>
  <rdf:li stEvt:action="saved" stEvt:softwareAgent="Sample Editor 2.1" stEvt:when="2026-06-14T21:07:30+02:00"/>
 </rdf:Seq></xmpMM:History>
</rdf:Description></rdf:RDF></x:xmpmeta>"""


def main():
    import sys
    cropped = sys.argv[1:] == ["cropped"]
    if cropped:
        # The left 360 pixels of the 640-wide scene: the tower is at 62%.
        scene = dusk(640, 480)
        photo = jpeg(360, 480, "normal", lambda x, y: scene(x, y))
    else:
        photo = jpeg(640, 480, "normal")
    thumb = jpeg(160, 120, "low")
    block = b"Exif\0\0" + tiff(thumb)
    app1 = b"\xFF\xE1" + struct.pack(">H", len(block) + 2) + block
    assert photo[:2] == b"\xFF\xD8"
    # EXIF goes straight after SOI, where cameras put it; an editor's XMP
    # right after.
    if cropped:
        packet = b"http://ns.adobe.com/xap/1.0/\0" + XMP
        app1 += b"\xFF\xE1" + struct.pack(">H", len(packet) + 2) + packet
    out = photo[:2] + app1 + photo[2:]
    path = OUT.replace("photo.jpg", "cropped.jpg") if cropped else OUT
    os.makedirs(os.path.dirname(path), exist_ok=True)
    open(path, "wb").write(out)
    print(f"wrote {os.path.normpath(path)}: {len(out)} bytes, EXIF {len(block)} bytes, thumbnail {len(thumb)} bytes")


if __name__ == "__main__":
    main()
