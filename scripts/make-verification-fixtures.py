#!/usr/bin/env python3
"""Writes deterministic Office metadata fixtures used by clean-copy coverage.

The ZIP fixture uses Python's standard library. The Office 97 fixture uses
macOS `textutil` to create a real Compound File with its supported properties.

    python3 scripts/make-verification-fixtures.py
"""
from pathlib import Path
import subprocess
import tempfile
import zipfile


ROOT = Path(__file__).resolve().parent.parent
FIXTURES = ROOT / "crates/hexscope-core/tests/fixtures"

CORE = """<?xml version="1.0" encoding="UTF-8"?>
<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties"
 xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:dcterms="http://purl.org/dc/terms/">
 <dc:title>Coverage plan</dc:title><dc:subject>Board</dc:subject>
 <dc:creator>Olena</dc:creator><cp:keywords>plan, budget</cp:keywords>
 <dc:description>Draft for review</dc:description><cp:lastModifiedBy>Petro</cp:lastModifiedBy>
 <cp:revision>3</cp:revision><cp:category>Finance</cp:category>
 <cp:contentStatus>Draft</cp:contentStatus>
 <dcterms:created>2025-11-02T09:14:00Z</dcterms:created>
 <dcterms:modified>2025-11-03T09:14:00Z</dcterms:modified>
</cp:coreProperties>"""

APP = """<?xml version="1.0" encoding="UTF-8"?>
<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties">
 <Template>Normal.dotm</Template><TotalTime>192</TotalTime>
 <Application>Microsoft Office Word</Application><Manager>Petro Shevchenko</Manager>
 <Company>Hexscope Test Co</Company>
</Properties>"""

CUSTOM = """<?xml version="1.0" encoding="UTF-8"?>
<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/custom-properties"
 xmlns:vt="http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes">
 <property name="MSIP_Label_1_Name"><vt:lpwstr>Confidential</vt:lpwstr></property>
 <property name="MSIP_Label_1_SiteId"><vt:lpwstr>72f988bf-86f1-41af-91ab-2d7cd011db47</vt:lpwstr></property>
 <property name="Client"><vt:lpwstr>Acme</vt:lpwstr></property>
</Properties>"""


def write_zip_fixture():
    path = FIXTURES / "all-office-properties.docx"
    photo = FIXTURES / "photo-no-location.jpg"
    with zipfile.ZipFile(path, "w", compression=zipfile.ZIP_DEFLATED) as archive:
        def add(name, data):
            info = zipfile.ZipInfo(name, date_time=(2026, 11, 8, 0, 0, 0))
            info.compress_type = zipfile.ZIP_DEFLATED
            archive.writestr(info, data)

        add("[Content_Types].xml", "<Types/>")
        add("word/document.xml", "<w:document/>")
        add("docProps/core.xml", CORE)
        add("docProps/app.xml", APP)
        add("docProps/custom.xml", CUSTOM)
        add("word/media/photo.jpg", photo.read_bytes())
    print(f"wrote {path.relative_to(ROOT)}")


def write_office97_fixture():
    path = FIXTURES / "office97-properties.doc"
    with tempfile.TemporaryDirectory() as directory:
        source = Path(directory) / "coverage.rtf"
        source.write_text(r"{\rtf1\ansi Office 97 verification fixture}")
        subprocess.run(
            [
                "textutil", "-convert", "doc", str(source), "-output", str(path),
                "-title", "Coverage plan", "-author", "Olena Koval",
                "-subject", "Board", "-keywords", "(plan,budget)",
                "-comment", "Draft for review", "-editor", "Petro Shevchenko",
                "-company", "Hexscope Test Co",
                "-creationtime", "2025-11-02T09:14:00Z",
                "-modificationtime", "2025-11-03T09:14:00Z",
            ],
            check=True,
        )
    print(f"wrote {path.relative_to(ROOT)}")


if __name__ == "__main__":
    FIXTURES.mkdir(parents=True, exist_ok=True)
    subprocess.run(["python3", str(ROOT / "scripts/make-sample-photo.py")], check=True)
    write_zip_fixture()
    write_office97_fixture()
