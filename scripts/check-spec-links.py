#!/usr/bin/env python3
"""Checks every specification link the explanations cite.

Each URL must answer 200, and each #anchor must exist in the page it points
into. Needs the network, so it is run by hand rather than in CI:

    python3 scripts/check-spec-links.py
"""
import glob
import os
import re
import subprocess
import sys
import tempfile

ROOT = os.path.join(os.path.dirname(__file__), "..")
SOURCES = glob.glob(os.path.join(ROOT, "crates/hexscope-core/src/**/*.rs"), recursive=True)

urls = set()
for path in SOURCES:
    with open(path, encoding="utf-8") as f:
        urls.update(re.findall(r'"(https://[^"\s]+)"', f.read()))

pages = {}
failed = []
for url in sorted(urls):
    page, _, anchor = url.partition("#")
    if page not in pages:
        # curl rather than urllib: some of these servers refuse Python's TLS
        # handshake while any browser gets through.
        with tempfile.NamedTemporaryFile() as out:
            status = subprocess.run(
                ["curl", "-sL", "-m", "120", "-A", "Mozilla/5.0 hexscope link check",
                 "-o", out.name, "-w", "%{http_code}", page],
                capture_output=True, text=True,
            ).stdout
            pages[page] = (int(status) if status.isdigit() else status, open(out.name, "rb").read())
    status, body = pages[page]
    ok = status == 200 and (not anchor or f'id="{anchor}"'.encode() in body)
    print(f"{'ok  ' if ok else 'FAIL'} {status} {url}")
    if not ok:
        failed.append(url)

print(f"\n{len(urls) - len(failed)} of {len(urls)} links ok")
sys.exit(1 if failed else 0)
