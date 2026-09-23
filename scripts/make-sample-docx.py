#!/usr/bin/env python3
"""Builds two Word documents:

- crates/hexscope-core/tests/fixtures/report.docx, written entirely by macOS
  `textutil` (Apple's Cocoa document writer), so the core is tested against
  an archive a real application produced;
- apps/web/public/samples/report.docx, the same document with the sample
  photo added as word/media/photo.jpg, stored uncompressed the way Word
  stores images. Opening that image inside the document shows where it was
  taken.

Every name and date is invented.

    python3 scripts/make-sample-docx.py
"""
import os
import shutil
import subprocess
import tempfile
import zipfile

ROOT = os.path.join(os.path.dirname(__file__), "..")
FIXTURE = os.path.join(ROOT, "crates", "hexscope-core", "tests", "fixtures", "report.docx")
SAMPLE = os.path.join(ROOT, "apps", "web", "public", "samples", "report.docx")
PHOTO = os.path.join(ROOT, "apps", "web", "public", "samples", "photo.jpg")

HTML = """<html><head><title>Quarterly report</title></head><body>
<h1>Quarterly report</h1>
<p>Sales grew in every region this quarter. Sales grew in every region this
quarter, and the photo from the launch event is attached below.</p>
</body></html>"""

with tempfile.TemporaryDirectory() as tmp:
    src = os.path.join(tmp, "report.html")
    with open(src, "w") as f:
        f.write(HTML)
    subprocess.run(
        ["textutil", "-convert", "docx", src, "-output", FIXTURE,
         "-title", "Quarterly report", "-author", "Olena Koval", "-editor", "o.koval",
         "-company", "Hexscope Test Co", "-creationtime", "2025-11-02T09:14:00Z"],
        check=True,
    )

shutil.copyfile(FIXTURE, SAMPLE)
with zipfile.ZipFile(SAMPLE, "a") as z:
    z.write(PHOTO, "word/media/photo.jpg", compress_type=zipfile.ZIP_STORED)

print(f"wrote {FIXTURE} and {SAMPLE}")
