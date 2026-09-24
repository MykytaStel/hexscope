#!/usr/bin/env python3
"""Builds apps/web/public/samples/deflate-demo.zip: one short text, deflated,
for the "How DEFLATE works" page. Text shows compression best: every copy
from earlier is readable, and the repetition is plain to see.

    python3 scripts/make-deflate-demo.py
"""
import os
import zipfile

OUT = os.path.join(os.path.dirname(__file__), "..", "apps", "web", "public", "samples", "deflate-demo.zip")

# Written for this demo: plain words that repeat, so the copies are easy to
# follow, and enough variety that DEFLATE builds its own code tables.
# apps/web/deflate.html quotes this text's code lengths (a space, e, a, o, t:
# 4 bits; S: 8) and the distance in its diagram: check them if it changes.
TEXT = (
    "A byte is a byte, and a bit is a bit.\n"
    "Eight bits make a byte, and bytes make a file.\n"
    "A file that says the same thing twice\n"
    "need not say the same thing twice:\n"
    "it can say it once, then point back to it.\n"
    "\n"
    "Point back four bytes, copy four bytes.\n"
    "Point back forty bytes, copy forty bytes.\n"
    "Point back to a word you have seen before,\n"
    "and the word you have seen before costs almost nothing.\n"
    "\n"
    "Letters you use often get short codes;\n"
    "letters you use rarely get long codes.\n"
    "Short codes for often, long codes for rarely:\n"
    "that is how a file that says the same thing twice\n"
    "becomes a file that says it once.\n"
)

with zipfile.ZipFile(OUT, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=9) as z:
    info = zipfile.ZipInfo("rain.txt", date_time=(2026, 9, 24, 12, 0, 0))
    info.compress_type = zipfile.ZIP_DEFLATED
    z.writestr(info, TEXT)
print(f"wrote {OUT}: {len(TEXT)} bytes of text")
