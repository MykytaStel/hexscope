#!/usr/bin/env python3
"""Bounded independent metadata/pixel regression gate; never prints field values."""
import argparse
import json
from pathlib import Path
import subprocess
import tempfile
from PIL import Image

ROOT = Path(__file__).resolve().parents[1]

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--hexscope", type=Path, required=True)
    args = parser.parse_args()
    fields = ["GPSLatitude", "GPSLongitude", "SerialNumber", "CameraSerialNumber", "OwnerName", "Artist", "Author", "Comment", "UserComment"]
    with tempfile.TemporaryDirectory(prefix="hexscope-independent-") as folder:
        for index, name in enumerate(["photo.jpg", "ai-picture.png"]):
            source = ROOT / "apps/web/public/samples" / name
            copy = Path(folder) / f"copy{index}{source.suffix}"
            result = subprocess.run([str(args.hexscope.resolve()), "clean", "--verify", "--json", "--out", str(copy), str(source)], capture_output=True, timeout=30)
            if result.returncode not in (0, 1) or not copy.is_file():
                raise AssertionError(f"fixture {index}: copy not created")
            read = subprocess.run(["exiftool", "-j", "-n", *[f"-{field}" for field in fields], str(copy)], capture_output=True, check=True, timeout=30)
            tags = json.loads(read.stdout)[0]
            remaining = [key for key in fields if tags.get(key) not in (None, "", 0)]
            if remaining: raise AssertionError(f"fixture {index}: independent reader found {len(remaining)} identifying fields")
            with Image.open(source) as before, Image.open(copy) as after:
                if before.size != after.size or before.convert("RGBA").tobytes() != after.convert("RGBA").tobytes():
                    raise AssertionError(f"fixture {index}: decoded pixels changed")
    print("Independent ExifTool + Pillow gate: 2 fixtures passed; no values exported")

if __name__ == "__main__":
    main()
