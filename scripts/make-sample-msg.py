#!/usr/bin/env python3
"""Writes phishing.msg: the sample fake bank email as Outlook saves a
message — a compound file (MS-CFB, version 3) with each property a stream
(MS-OXMSG) — for hexscope's tests. Its headers, text and attachment are
those of apps/web/public/samples/phishing.eml.

    python3 scripts/make-sample-msg.py crates/hexscope-core/tests/fixtures/phishing.msg
"""

import struct
import sys
from pathlib import Path

SECTOR, MINI, CUTOFF = 512, 64, 4096
FREE, END, FATSECT, NONE = 0xFFFFFFFF, 0xFFFFFFFE, 0xFFFFFFFD, 0xFFFFFFFF

EML = (Path(__file__).parent.parent / "apps/web/public/samples/phishing.eml").read_text()
HEADERS = EML.split("\n\n", 1)[0].replace("\n", "\r\n") + "\r\n"
HTML = (
    '<p>Confirm your details within 24 hours, or your account will be closed.</p>\r\n'
    '<p><a href="https://login.example.info/verify?id=7Q1">www.example-bank.com/verify</a></p>\r\n'
    '<p><a href="https://xn--exmple-bank-zij.com/">Help centre</a></p>\r\n'
)
TEXT = "Confirm your details within 24 hours, or your account will be closed.\r\nwww.example-bank.com/verify\r\n"


def utf16(s):
    return s.encode("utf-16-le") + b"\0\0"


def prop(tag, value):
    return (f"__substg1.0_{tag}", value)


# (name, bytes) for a stream, (name, [children]) for a storage.
TREE = [
    prop("0037001F", utf16("Your account is on hold")),
    prop("0C1A001F", utf16("Example Bank Security")),
    prop("0C1F001F", utf16("security@example-bank.com")),
    prop("0E04001F", utf16("taras@example.net")),
    prop("007D001F", utf16(HEADERS)),
    prop("1000001F", utf16(TEXT)),
    prop("10130102", HTML.encode()),
    ("__properties_version1.0", bytes(32)),
    ("__recip_version1.0_#00000000", [
        prop("3001001F", utf16("taras@example.net")),
        prop("39FE001F", utf16("taras@example.net")),
        ("__properties_version1.0", bytes(8)),
    ]),
    ("__attach_version1.0_#00000000", [
        prop("3707001F", utf16("invoice.pdf.html")),
        prop("370E001F", utf16("text/html")),
        prop("37010102", b"<html><body>Sign in to view your invoice</body></html>\r\n"),
        ("__properties_version1.0", bytes(8)),
    ]),
]


def key(name):
    """Siblings are ordered by name length, then by the name in capitals."""
    return (len(name), name.upper())


def build(tree):
    entries = [{"name": "Root Entry", "type": 5, "child": NONE, "right": NONE, "data": None}]
    mini = bytearray()

    def add(items):
        ids = []
        for name, value in sorted(items, key=lambda i: key(i[0])):
            i = len(entries)
            entries.append({"name": name, "type": 1 if isinstance(value, list) else 2, "child": NONE, "right": NONE})
            if isinstance(value, list):
                entries[i]["child"] = add(value)
            else:
                assert len(value) < CUTOFF
                entries[i]["start"] = len(mini) // MINI
                entries[i]["size"] = len(value)
                mini.extend(value + bytes(-len(value) % MINI))
            ids.append(i)
        # A chain of right siblings, all black: a tree that is valid to walk.
        for a, b in zip(ids, ids[1:]):
            entries[a]["right"] = b
        return ids[0] if ids else NONE

    entries[0]["child"] = add(tree)
    return entries, bytes(mini)


def main(out):
    entries, mini = build(TREE)
    minifat = []
    for e in entries[1:]:
        if e["type"] == 2:
            n = -(-e["size"] // MINI)
            minifat += [e["start"] + k + 1 for k in range(n - 1)] + [END]
    sectors = []  # (what, bytes)
    fat = []

    def chain(data):
        first = len(sectors)
        n = max(1, -(-len(data) // SECTOR))
        for k in range(n):
            sectors.append(data[k * SECTOR:(k + 1) * SECTOR].ljust(SECTOR, b"\0"))
            fat.append(first + k + 1 if k + 1 < n else END)
        return first

    mini_start = chain(mini)
    minifat_bytes = b"".join(struct.pack("<I", v) for v in minifat)
    minifat_bytes += struct.pack("<I", FREE) * (-len(minifat) % (SECTOR // 4))
    minifat_start = chain(minifat_bytes)
    minifat_count = len(minifat_bytes) // SECTOR

    entries[0]["start"], entries[0]["size"] = mini_start, len(mini)
    dirs = b""
    for e in entries:
        name = e["name"].encode("utf-16-le")
        d = name.ljust(64, b"\0") + struct.pack("<HBB", len(name) + 2, e["type"], 1)
        d += struct.pack("<III", NONE, e["right"], e["child"]) + bytes(16) + bytes(4) + bytes(16)
        d += struct.pack("<IQ", e.get("start", 0), e.get("size", 0))
        dirs += d
    dirs += (b"\0" * 64 + struct.pack("<HBB", 0, 0, 0) + struct.pack("<III", NONE, NONE, NONE) + bytes(48)) * (-len(entries) % 4)
    dir_start = chain(dirs)

    fat_sector = len(sectors)
    fat.append(FATSECT)
    assert len(fat) <= SECTOR // 4
    sectors.append(b"".join(struct.pack("<I", v) for v in fat).ljust(SECTOR, b"\xff"))

    header = bytes.fromhex("D0CF11E0A1B11AE1") + bytes(16)
    header += struct.pack("<HHHHH", 0x3E, 3, 0xFFFE, 9, 6) + bytes(6)
    header += struct.pack("<IIIIIIIII", 0, 1, dir_start, 0, CUTOFF, minifat_start, minifat_count, END, 0)
    header += struct.pack("<I", fat_sector) + struct.pack("<I", FREE) * 108
    assert len(header) == SECTOR
    Path(out).write_bytes(header + b"".join(sectors))


main(sys.argv[1])
