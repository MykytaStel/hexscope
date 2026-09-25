#!/usr/bin/env python3
"""Writes the MP4 fixtures with ffmpeg, then shapes their location metadata
the way the devices that make them do. The values are samples.

  loci.mp4     as ffmpeg writes it: the location in a 3GPP 'loci' box
  android.mp4  as an Android phone writes it: '(c)xyz' in the user data,
               an ISO 6709 string, and nothing else

The iPhone-style fixture comes from make-sample-video.swift, with Apple's
own writer. Needs ffmpeg.

  python3 scripts/make-sample-video.py
"""
import os
import struct
import subprocess

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, "..", "crates", "hexscope-core", "tests", "fixtures")
ISO6709 = b"+48.8584+002.2945/"


def ffmpeg(path):
    subprocess.run(
        ["ffmpeg", "-v", "error", "-y", "-f", "lavfi", "-i", "testsrc=size=64x48:rate=5", "-t", "1",
         "-c:v", "mpeg4", "-q:v", "20", "-fflags", "+bitexact", "-map_metadata", "-1",
         "-metadata", "location=" + ISO6709.decode(), "-metadata", "creation_time=2026-06-14T16:32:07Z", path],
        check=True,
    )
    with open(path, "rb") as f:
        return f.read()


def boxes(data, start, end):
    """(type, start, size) of each box in [start, end)."""
    out = []
    while start + 8 <= end:
        size, kind = struct.unpack(">I4s", data[start:start + 8])
        out.append((kind, start, size))
        start += size
    return out


def box(kind, payload):
    return struct.pack(">I4s", 8 + len(payload), kind) + payload


loci = ffmpeg(os.path.join(OUT, "loci.mp4"))

# Android: udta holds one '(c)xyz' text item — a 2-byte length, a 2-byte
# language, the ISO 6709 string — in place of ffmpeg's udta.
top = boxes(loci, 0, len(loci))
kind, moov_at, moov_size = next(b for b in top if b[0] == b"moov")
assert all(b[1] < moov_at for b in top if b[0] == b"mdat"), "mdat must come first"
children = boxes(loci, moov_at + 8, moov_at + moov_size)
xyz = box(b"\xa9xyz", struct.pack(">HH", len(ISO6709), 0x15C7) + ISO6709)
body = b"".join(
    box(b"udta", xyz) if k == b"udta" else loci[s:s + n] for k, s, n in children
)
android = loci[:moov_at] + box(b"moov", body) + loci[moov_at + moov_size:]
with open(os.path.join(OUT, "android.mp4"), "wb") as f:
    f.write(android)
print("loci.mp4", len(loci), "android.mp4", len(android))
