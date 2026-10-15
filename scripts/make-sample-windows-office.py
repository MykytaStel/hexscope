#!/usr/bin/env python3
"""Writes windows.docx and windows.xlsx: Word and Excel files with what
Office on Windows leaves beside the properties — the folder a workbook was
last saved in (x15ac:absPath), the full path of a document's template, a
printer's settings naming the printer, SharePoint's properties kept as
custom XML, a picture of the first page, and the editing-session numbers
(w:rsid) on every paragraph. The parts are written as Office writes them;
the people, folders and printers are made up.

    python3 scripts/make-sample-windows-office.py crates/hexscope-core/tests/fixtures
"""

import struct
import sys
import zipfile
from pathlib import Path

OUT = Path(sys.argv[1] if len(sys.argv) > 1 else ".")

CONTENT_TYPES = """<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Default Extension="bin" ContentType="application/vnd.openxmlformats-officedocument.{kind}.printerSettings"/><Default Extension="jpeg" ContentType="image/jpeg"/>{overrides}</Types>"""

CORE = """<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:dcterms="http://purl.org/dc/terms/" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"><dc:creator>Olena Koval</dc:creator><cp:lastModifiedBy>Olena Koval</cp:lastModifiedBy><dcterms:created xsi:type="dcterms:W3CDTF">2026-09-14T09:12:00Z</dcterms:created></cp:coreProperties>"""

SHAREPOINT = """<?xml version="1.0"?>
<p:properties xmlns:p="http://schemas.microsoft.com/office/2006/metadata/properties" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns:pc="http://schemas.microsoft.com/office/infopath/2007/PartnerControls"><documentManagement><Reviewer xmlns="5f1c2b3a"><UserInfo><DisplayName>Petro Shevchuk</DisplayName><AccountId>12</AccountId></UserInfo></Reviewer><ContactEmail xmlns="5f1c2b3a">olena.koval@northwind.example</ContactEmail></documentManagement></p:properties>"""

ITEM_PROPS = """<?xml version="1.0" encoding="UTF-8" standalone="no"?>
<ds:datastoreItem ds:itemID="{3A1B2C3D-0000-4000-8000-000000000001}" xmlns:ds="http://schemas.openxmlformats.org/officeDocument/2006/customXml"><ds:schemaRefs><ds:schemaRef ds:uri="http://schemas.microsoft.com/office/2006/metadata/properties"/></ds:schemaRefs></ds:datastoreItem>"""


def devmode(printer: str) -> bytes:
    """A Windows DEVMODE as a printer-settings part holds it: the device
    name in 32 UTF-16 code units, the spec version, sizes, then the fields."""
    name = printer.encode("utf-16-le")[:62].ljust(64, b"\0")
    fields = struct.pack("<HHHHI", 0x0401, 0x0001, 220, 0, 0x0001_0F03)
    return (name + fields).ljust(220, b"\0")


def picture() -> bytes:
    """The first page as Office draws it: a small grey JPEG."""
    return bytes.fromhex(
        "ffd8ffdb004300100b0c0e0c0a100e0d0e1211101318281a18161618312325"
        "1d283a333d3c3933383740485c4e404457453738506d51575f626768673e4d"
        "71797064785c656763ffc0000b080001000101011100ffc4001400010000000000"
        "0000000000000000000007ffc40014100100000000000000000000000000000000"
        "ffda0008010100003f00207fffd9"
    )


def docx(path: Path) -> None:
    body = (
        '<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>'
        '<w:p w:rsidR="00A1B2C3" w:rsidRDefault="00A1B2C3" w:rsidP="00D4E5F6"><w:r w:rsidRPr="00112233"><w:t>Lease agreement</w:t></w:r></w:p>'
        '<w:p w:rsidR="00D4E5F6" w:rsidRDefault="00D4E5F6"><w:r><w:t>The rent is due on the first of each month.</w:t></w:r></w:p>'
        "</w:body></w:document>"
    )
    settings = (
        '<w:settings xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" '
        'xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">'
        '<w:attachedTemplate r:id="rId1"/><w:defaultTabStop w:val="720"/>'
        '<w:rsids><w:rsidRoot w:val="00A1B2C3"/><w:rsid w:val="00A1B2C3"/><w:rsid w:val="00D4E5F6"/><w:rsid w:val="00112233"/></w:rsids>'
        "</w:settings>"
    )
    settings_rels = (
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>\n'
        '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">'
        '<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/attachedTemplate" '
        'Target="file:///C:\\Users\\olena.koval\\AppData\\Roaming\\Microsoft\\Templates\\Lease.dotx" TargetMode="External"/>'
        "</Relationships>"
    )
    overrides = (
        '<Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>'
        '<Override PartName="/word/settings.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml"/>'
        '<Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/>'
    )
    rels = (
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>\n'
        '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">'
        '<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>'
        '<Relationship Id="rId2" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml"/>'
        '<Relationship Id="rId3" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/thumbnail" Target="docProps/thumbnail.jpeg"/>'
        "</Relationships>"
    )
    doc_rels = (
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>\n'
        '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">'
        '<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/settings" Target="settings.xml"/>'
        '<Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/customXml" Target="../customXml/item1.xml"/>'
        "</Relationships>"
    )
    with zipfile.ZipFile(path, "w", zipfile.ZIP_DEFLATED) as z:
        z.writestr("[Content_Types].xml", CONTENT_TYPES.format(kind="wordprocessingml", overrides=overrides))
        z.writestr("_rels/.rels", rels)
        z.writestr("docProps/core.xml", CORE)
        z.writestr("docProps/thumbnail.jpeg", picture())
        z.writestr("word/document.xml", body)
        z.writestr("word/_rels/document.xml.rels", doc_rels)
        z.writestr("word/settings.xml", settings)
        z.writestr("word/_rels/settings.xml.rels", settings_rels)
        z.writestr("word/printerSettings/printerSettings1.bin", devmode("HP LaserJet Pro M404 - Office 2nd floor"))
        z.writestr("customXml/item1.xml", SHAREPOINT)
        z.writestr("customXml/itemProps1.xml", ITEM_PROPS)


def xlsx(path: Path) -> None:
    workbook = (
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>\n'
        '<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" '
        'xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" '
        'xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" mc:Ignorable="x15" '
        'xmlns:x15="http://schemas.microsoft.com/office/spreadsheetml/2010/11/main">'
        '<fileVersion appName="xl" lastEdited="7" lowestEdited="7" rupBuild="27425"/>'
        '<mc:AlternateContent xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006">'
        '<mc:Choice Requires="x15"><x15ac:absPath url="C:\\Users\\olena.koval\\Documents\\Salaries 2026\\" '
        'xmlns:x15ac="http://schemas.microsoft.com/office/spreadsheetml/2010/11/ac"/></mc:Choice></mc:AlternateContent>'
        '<sheets><sheet name="Budget" sheetId="1" r:id="rId1"/></sheets></workbook>'
    )
    sheet = (
        '<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">'
        '<sheetData><row r="1"><c r="A1" t="inlineStr"><is><t>Rent</t></is></c></row></sheetData></worksheet>'
    )
    overrides = (
        '<Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>'
        '<Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>'
        '<Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/>'
    )
    rels = (
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>\n'
        '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">'
        '<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/>'
        '<Relationship Id="rId2" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml"/>'
        "</Relationships>"
    )
    wb_rels = (
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>\n'
        '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">'
        '<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/>'
        "</Relationships>"
    )
    with zipfile.ZipFile(path, "w", zipfile.ZIP_DEFLATED) as z:
        z.writestr("[Content_Types].xml", CONTENT_TYPES.format(kind="spreadsheetml", overrides=overrides))
        z.writestr("_rels/.rels", rels)
        z.writestr("docProps/core.xml", CORE)
        z.writestr("xl/workbook.xml", workbook)
        z.writestr("xl/_rels/workbook.xml.rels", wb_rels)
        z.writestr("xl/worksheets/sheet1.xml", sheet)
        z.writestr("xl/printerSettings/printerSettings1.bin", devmode("Canon iR-ADV C5535 Accounting"))


docx(OUT / "windows.docx")
xlsx(OUT / "windows.xlsx")
print(f"wrote {OUT / 'windows.docx'} and {OUT / 'windows.xlsx'}")
