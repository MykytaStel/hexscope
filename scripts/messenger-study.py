#!/usr/bin/env python3
"""
What messengers and other ways of sending take out of a file: the received
copies of the kit's files, each read by ExifTool and hexscope and set beside
the original. See docs/research/messenger-study.md for how to make them.

    python3 -I scripts/messenger-study.py RECEIVED_DIR [--hexscope target/release/hexscope] [--out docs/research]

Name each received file CHANNEL__MODE__ORIGINAL, for example
telegram__photo__photo.jpg or gmail__attachment__report.pdf: the original is
found in the kit by its name, whatever extension the channel gave the copy.
Writes messenger-study.md and messenger-study.json into --out.
"""

from __future__ import annotations

import argparse
import importlib.util
import json
import sys
from pathlib import Path

# The comparison's readers, loaded from its file: its name is not a module's.
spec = importlib.util.spec_from_file_location("compare", Path(__file__).resolve().parent / "compare-tools.py")
if spec is None or spec.loader is None:
    sys.exit("scripts/compare-tools.py is missing")
compare = importlib.util.module_from_spec(spec)
spec.loader.exec_module(compare)

ROOT = compare.ROOT
KIT = ROOT / "apps/web/public/samples"
FILES = ["photo.jpg", "photo.heic", "ai-picture.png", "video.mov", "report.pdf", "report.docx"]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("received", type=Path)
    parser.add_argument("--hexscope", type=Path, default=ROOT / "target/release/hexscope")
    parser.add_argument("--out", type=Path, default=ROOT / "docs/research")
    args = parser.parse_args()
    if not args.received.is_dir():
        sys.exit(f"{args.received} is not a folder")
    received = sorted(p for p in args.received.iterdir() if p.is_file() and p.name.count("__") >= 2)
    if not received:
        sys.exit(f"no files named CHANNEL__MODE__ORIGINAL in {args.received}")
    originals = {name: KIT / name for name in FILES}
    tags = compare.exiftool([*originals.values(), *received])
    rows = []
    for copy in received:
        channel, mode, name = copy.name.split("__", 2)
        stem = Path(name).stem
        original = next((p for n, p in originals.items() if Path(n).stem == stem), None)
        if original is None:
            print(f"skipped {copy.name}: no kit file named {stem}", file=sys.stderr)
            continue
        before = {c for c, _ in tags.get(str(original.resolve()), [])}
        after = {c for c, _ in tags.get(str(copy.resolve()), [])}
        found = compare.hexscope_check(args.hexscope, copy)
        rows.append({
            "channel": channel,
            "mode": mode,
            "file": original.name,
            "bytes": [original.stat().st_size, copy.stat().st_size],
            "kept": sorted(after),
            "removed": sorted(before - after),
            "hexscope": found["reveals"],
            "pixels": compare.pixels(original, copy),
        })
    args.out.mkdir(parents=True, exist_ok=True)
    (args.out / "messenger-study.json").write_text(json.dumps(rows, indent=1) + "\n")
    lines = [
        "<!-- Written by scripts/messenger-study.py; do not edit by hand. -->",
        "",
        "| Channel | Sent as | File | Kept, by ExifTool | hexscope still finds | Size before → after | Pixels |",
        "|---|---|---|---|---|---|---|",
    ]
    for r in rows:
        px = "same" if r["pixels"] == 0 else "—" if r["pixels"] is None else f"differ by {r['pixels']}"
        lines.append(
            f"| {r['channel']} | {r['mode']} | `{r['file']}` | {', '.join(r['kept']) or 'nothing'} | "
            f"{', '.join(r['hexscope']) or 'nothing'} | {r['bytes'][0]:,} → {r['bytes'][1]:,} | {px} |"
        )
    (args.out / "messenger-study.md").write_text("\n".join(lines) + "\n")
    print(f"{len(rows)} copies read")
    return 0


if __name__ == "__main__":
    sys.exit(main())
