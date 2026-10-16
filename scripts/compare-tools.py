#!/usr/bin/env python3
"""
Three tools that remove metadata — hexscope, ExifTool and MAT2 — on the same
files, each copy then read by two readers: ExifTool (the reference for what a
file says, written by someone else) and hexscope's own check.

Every reader has blind spots, and hexscope reading its own copies would be no
test at all: that is why ExifTool reads every copy. hexscope reads every copy
too, because ExifTool reads only a PDF's latest version and no hidden text.

    python3 -I scripts/compare-tools.py [--hexscope target/release/hexscope] [--out docs/research]

Needs exiftool and mat2 on PATH and Pillow for the pixel check. Writes
results.json and metadata-removal.md into --out. Nothing is uploaded; copies
go to a temporary folder that is removed afterwards.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CORPUS = [ROOT / "crates/hexscope-core/tests/fixtures", ROOT / "apps/web/public/samples", ROOT / "apps/web/e2e/fixtures"]
# Files that are broken or locked on purpose, and folders of test suites, say little about removal.
SKIP = re.compile(r"^(broken\.png|encrypted-.*|tall-interlaced\.png|incomplete-form\.pdf|deflate-demo\.zip)$")

# What ExifTool reports about the file system and itself, not what the file says.
NOT_THE_FILE = {"System", "File", "ExifTool", "Composite"}

# What a tag is about, by its name. A tag in none of these — sizes, colour
# spaces, codecs — says how the file is made, not who made it, where or when.
CATEGORIES: list[tuple[str, re.Pattern[str]]] = [
    ("location", re.compile(r"^(GPS|Location|LocationInformation|City|Country|State|Province|Sub-?location|ContentLocation|Xyz|Coordinates)", re.I)),
    ("device", re.compile(r"(SerialNumber|^Make$|^Model$|^LensModel$|^LensMake$|^LensInfo$|^HostComputer$|^Lens$|^CameraModelName$|AndroidModel|AndroidMake|^Manufacturer$)", re.I)),
    ("person", re.compile(r"^(Artist|Author|Authors|Creator|By-line|OwnerName|CameraOwnerName|Owner|LastModifiedBy|Company|Manager|Contributor|Initials|Copyright|Rights|CreatorContactInfo|Publisher|LastSavedBy|AuthorEmail|UserName|Writer)$", re.I)),
    ("time", re.compile(r"^(DateTimeOriginal|CreateDate|ModifyDate|MetadataDate|DateCreated|DateTimeCreated|CreationDate|ModDate|TotalEditTime|RevisionNumber|LastPrinted|SubSecDateTimeOriginal|SubSecCreateDate|SubSecModifyDate|OffsetTime.*|DigitalCreationDate|TrackCreateDate|TrackModifyDate|MediaCreateDate|MediaModifyDate|ContentCreateDate|Date)$", re.I)),
    ("software", re.compile(r"^(Software|CreatorTool|Producer|Application|AppVersion|Generator|Encoder|ProcessingSoftware|HistorySoftwareAgent|Template|DocSecurity|AppVersion|PDFProducer)$", re.I)),
    ("words", re.compile(r"^(Comment|UserComment|ImageDescription|Description|Title|Subject|Keywords|Caption-Abstract|Headline|Parameters|Prompt|Workflow|Category|ContentStatus|Notes|Label|Instructions|Source)$", re.I)),
    ("thumbnail", re.compile(r"^(ThumbnailImage|PreviewImage|ThumbnailTIFF|OtherImage|PreviewPDF|JpgFromRaw)$", re.I)),
    ("history", re.compile(r"^(History.*|DocumentID|InstanceID|OriginalDocumentID|DerivedFrom.*|PreservedFileName|ImageUniqueID|RSID.*)$", re.I)),
    ("provenance", re.compile(r"^(JUMBF.*|C2PA.*|Claim.*|ActionsAction|DigitalSourceType|ActionsSoftwareAgent.*)$", re.I)),
]

# What hexscope calls each kind of finding, in the categories above.
HEXSCOPE = {
    "location": "location", "photoplace": "location", "qrplace": "location", "place": "location",
    "camera": "device", "lens": "device", "serial": "device", "photo": "device", "shutter": "device", "computer": "device",
    "owner": "person", "author": "person", "editor": "person", "company": "person", "manager": "person",
    "printed": "person", "label": "person", "comments": "person", "tracked": "person", "printer": "person",
    "taken": "time", "created": "time", "modified": "time", "editing": "time", "revisions": "time", "uptime": "time",
    "software": "software", "application": "software", "producer": "software", "toolchain": "software",
    "mailer": "software", "template": "software",
    "title": "words", "subject": "words", "keywords": "words", "description": "words", "caption": "words",
    "prompt": "words", "category": "words", "status": "words", "custom": "words", "notes": "words",
    "thumbnail": "thumbnail",
    "history": "history", "updates": "history", "earlier": "history", "original": "history", "linked": "history",
    "deleted": "history",
    "credentials": "provenance", "ai": "provenance",
    # What no metadata reader looks for: text and files the pages or sheets hide.
    "covered": "hidden", "hiddentext": "hidden", "attachments": "hidden", "embedded": "hidden", "form": "hidden",
    "hiddensheets": "hidden", "hiddencells": "hidden", "hiddenslides": "hidden",
}


def blank(value: object) -> bool:
    """A tag left in place but emptied — spaces, zeros, NULs — holds nothing any more."""
    if value is None:
        return True
    if isinstance(value, (int, float)):
        return value == 0
    text = str(value).strip().strip("\x00")
    return text == "" or re.fullmatch(r"[0:\s.\-+]*", text) is not None or text.startswith("(Binary data 0 bytes")


def category(name: str, group: str = "") -> str | None:
    # A PDF's Creator is the program that made the document it came from.
    if group == "PDF" and name == "Creator":
        return "software"
    for label, pattern in CATEGORIES:
        if pattern.search(name):
            return label
    return None


def exiftool(paths: list[Path]) -> dict[str, list[tuple[str, str]]]:
    """What ExifTool reads in each file: the identifying tags that hold something, as (category, group:name)."""
    if not paths:
        return {}
    out = subprocess.run(
        ["exiftool", "-j", "-a", "-u", "-G1", "-n", "-q", "-q", "-api", "largefilesupport=1", *map(str, paths)],
        capture_output=True, text=True, check=False,
    )
    found: dict[str, list[tuple[str, str]]] = {}
    for record in json.loads(out.stdout or "[]"):
        tags: list[tuple[str, str]] = []
        for key, value in record.items():
            if ":" not in key:
                continue
            group, name = key.split(":", 1)
            if group in NOT_THE_FILE:
                continue
            what = category(name, group)
            if what and not blank(value):
                tags.append((what, f"{group}:{name}"))
        found[str(Path(record["SourceFile"]).resolve())] = sorted(set(tags))
    return found


def hexscope_check(binary: Path, path: Path) -> dict:
    out = subprocess.run([str(binary), "check", "--json", "--fail-on", "none", str(path)], capture_output=True, text=True, check=False)
    line = next((line for line in out.stdout.splitlines() if line.startswith("{")), "{}")
    record = json.loads(line)
    kinds = sorted({r["kind"] for r in record.get("reveals", [])})
    hidden = sorted({p.get("label") or p.get("kind", "") for p in record.get("problems", []) if p.get("concern", p.get("kind")) in ("hidden",)})
    return {"format": record.get("format"), "reveals": kinds, "hidden": hidden, "problems": len(record.get("problems", []))}


def pixels(a: Path, b: Path) -> float | None:
    """Mean absolute difference of the decoded pictures, 0–255; None when either cannot be decoded."""
    try:
        from PIL import Image, ImageChops, ImageStat
    except ImportError:
        return None
    try:
        with Image.open(a) as x, Image.open(b) as y:
            x, y = x.convert("RGB"), y.convert("RGB")
            if x.size != y.size:
                return 255.0
            return round(sum(ImageStat.Stat(ImageChops.difference(x, y)).mean) / 3, 3)
    except Exception:
        return None


def run(cmd: list[str], cwd: Path) -> tuple[int, str]:
    p = subprocess.run(cmd, cwd=cwd, capture_output=True, text=True, check=False)
    # What a tool said, without the temporary folder it said it about.
    said = (p.stdout + p.stderr).replace(f"{cwd}/", "").replace(f"{ROOT}/", "")
    return p.returncode, " ".join(said.split())


def clean_copies(binary: Path, source: Path, work: Path) -> dict[str, dict]:
    """Each tool's copy of one file, or why there is none."""
    copies: dict[str, dict] = {}
    folder = work / f"{len(list(work.iterdir())):03d}-{source.name}"
    folder.mkdir()
    # hexscope: its default clean copy.
    out = folder / f"hexscope{source.suffix}"
    code, said = run([str(binary), "clean", "--out", str(out), str(source)], folder)
    copies["hexscope"] = {"path": out if out.exists() else None, "said": said[-300:] if not out.exists() else ""}
    # ExifTool: everything it can write removed.
    out = folder / f"exiftool{source.suffix}"
    code, said = run(["exiftool", "-all=", "-o", str(out), str(source)], folder)
    copies["exiftool"] = {"path": out if out.exists() else None, "said": said[-300:] if not out.exists() else ""}
    # MAT2: its default, which re-renders some formats.
    mat = folder / "mat2"
    mat.mkdir()
    shutil.copy(source, mat / source.name)
    code, said = run(["mat2", source.name], mat)
    made = [p for p in mat.iterdir() if ".cleaned." in p.name]
    copies["mat2"] = {"path": made[0] if made else None, "said": said[-300:] if not made else ""}
    return copies


TOOLS = ("hexscope", "exiftool", "mat2")


def tables(versions: dict[str, str], results: list[dict]) -> str:
    """The results as Markdown: one table by tool, one row per file."""
    lines = [
        "<!-- Written by scripts/compare-tools.py; do not edit by hand. -->",
        "",
        f"Tools: {versions['hexscope']}, ExifTool {versions['exiftool']}, {versions['mat2']}. "
        f"{len(results)} files.",
        "",
        "| | hexscope | ExifTool | MAT2 |",
        "|---|---|---|---|",
    ]
    def count(test) -> list[str]:
        return [str(sum(1 for r in results if test(r, t))) for t in TOOLS]
    made = lambda r, t: r["copies"][t]["made"]
    worth = lambda r: bool(r["original"]["exiftool"] or r["original"]["hexscope"]["reveals"])
    rows = [
        ("Files with something to remove", lambda r, t: worth(r)),
        ("…of which a copy was made", lambda r, t: worth(r) and made(r, t)),
        ("Copies where ExifTool still reads identifying tags", lambda r, t: made(r, t) and r["copies"][t]["exiftool"]),
        ("Copies where hexscope still finds something", lambda r, t: made(r, t) and (r["copies"][t]["hexscope"]["reveals"] or r["copies"][t]["hexscope"]["hidden"])),
        ("Pictures whose pixels changed", lambda r, t: made(r, t) and (r["copies"][t]["pixels"] or 0) > 0),
    ]
    for label, test in rows:
        lines.append(f"| {label} | " + " | ".join(count(test)) + " |")
    lines += ["", "| File | Found by ExifTool / hexscope | hexscope's copy | ExifTool's copy | MAT2's copy |", "|---|---|---|---|---|"]
    for r in results:
        cells = []
        for t in TOOLS:
            c = r["copies"][t]
            if not c["made"]:
                cells.append("no copy")
                continue
            left = sorted({name.split(":", 1)[1] for _, name in c["exiftool"]})
            found = c["hexscope"]["reveals"]
            bits = []
            if left:
                bits.append("ExifTool: " + ", ".join(left))
            if found:
                bits.append("hexscope: " + ", ".join(found))
            if (c["pixels"] or 0) > 0:
                bits.append(f"pixels differ by {c['pixels']}")
            cells.append("; ".join(bits) or "clean")
        o = r["original"]
        lines.append(f"| `{r['file']}` | {len(o['exiftool'])} / {len(o['hexscope']['reveals'])} | " + " | ".join(cells) + " |")
    return "\n".join(lines) + "\n"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--hexscope", type=Path, default=ROOT / "target/release/hexscope")
    parser.add_argument("--out", type=Path, default=ROOT / "docs/research")
    args = parser.parse_args()
    for tool in ("exiftool", "mat2"):
        if not shutil.which(tool):
            sys.exit(f"{tool} is not on PATH")
    versions = {
        "hexscope": run([str(args.hexscope), "--version"], ROOT)[1],
        "exiftool": run(["exiftool", "-ver"], ROOT)[1],
        "mat2": run(["mat2", "--version"], ROOT)[1],
    }
    # The same file kept in two places (a sample that is also a test file) counts once.
    unique: dict[str, Path] = {}
    for d in CORPUS:
        for p in sorted(d.iterdir()):
            if p.is_file() and not SKIP.match(p.name) and not p.name.startswith("."):
                unique.setdefault(hashlib.sha256(p.read_bytes()).hexdigest(), p.resolve())
    files = sorted(unique.values(), key=lambda p: p.name)
    results = []
    with tempfile.TemporaryDirectory() as tmp:
        work = Path(tmp)
        originals = exiftool(files)
        for source in files:
            copies = clean_copies(args.hexscope, source, work)
            made = [c["path"] for c in copies.values() if c["path"]]
            read = exiftool(made)
            entry = {
                "file": f"{source.parent.name}/{source.name}",
                "sha256": hashlib.sha256(source.read_bytes()).hexdigest()[:16],
                "bytes": source.stat().st_size,
                "original": {"exiftool": originals.get(str(source), []), "hexscope": hexscope_check(args.hexscope, source)},
                "copies": {},
            }
            for tool, copy in copies.items():
                path = copy["path"]
                if not path:
                    entry["copies"][tool] = {"made": False, "said": copy["said"]}
                    continue
                entry["copies"][tool] = {
                    "made": True,
                    "bytes": path.stat().st_size,
                    "exiftool": read.get(str(path.resolve()), []),
                    "hexscope": hexscope_check(args.hexscope, path),
                    "pixels": pixels(source, path),
                }
            results.append(entry)
    args.out.mkdir(parents=True, exist_ok=True)
    (args.out / "metadata-removal-results.json").write_text(json.dumps({"versions": versions, "files": results}, indent=1) + "\n")
    (args.out / "metadata-removal-tables.md").write_text(tables(versions, results))
    print(json.dumps(versions))
    print(f"{len(results)} files")
    return 0


if __name__ == "__main__":
    sys.exit(main())
