#!/usr/bin/env python3
"""Builds apps/web/public/samples/budget.xlsx: a small workbook that looks
like one sheet of totals, and keeps what its author meant to take out — a
sheet of salaries hidden from the tabs, and a row of bonuses hidden in the
sheet everyone sees. Every name and number is invented.

    python3 scripts/make-sample-xlsx.py
"""
import os
import zipfile

OUT = os.path.join(os.path.dirname(__file__), "..", "apps", "web", "public", "samples", "budget.xlsx")

def cell(ref, value):
    if isinstance(value, (int, float)):
        return f'<c r="{ref}"><v>{value}</v></c>'
    return f'<c r="{ref}" t="inlineStr"><is><t>{value}</t></is></c>'

def sheet(rows, hidden=()):
    out = []
    for i, row in enumerate(rows, 1):
        attrs = ' hidden="1"' if i in hidden else ''
        cells = "".join(cell(f"{chr(65 + k)}{i}", v) for k, v in enumerate(row))
        out.append(f'<row r="{i}"{attrs}>{cells}</row>')
    return ('<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
            '<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">'
            f'<sheetData>{"".join(out)}</sheetData></worksheet>')

FILES = {
    "[Content_Types].xml": '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
        '<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">'
        '<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>'
        '<Default Extension="xml" ContentType="application/xml"/>'
        '<Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>'
        '<Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>'
        '<Override PartName="/xl/worksheets/sheet2.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>'
        '<Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/>'
        '</Types>',
    "_rels/.rels": '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
        '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">'
        '<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/>'
        '<Relationship Id="rId2" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml"/>'
        '</Relationships>',
    "docProps/core.xml": '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
        '<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:dcterms="http://purl.org/dc/terms/" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">'
        '<dc:title>Team budget 2026</dc:title><dc:creator>Olena Koval</dc:creator><cp:lastModifiedBy>Petro Ivanenko</cp:lastModifiedBy>'
        '<dcterms:created xsi:type="dcterms:W3CDTF">2026-02-10T09:00:00Z</dcterms:created></cp:coreProperties>',
    "xl/workbook.xml": '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
        '<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">'
        '<sheets><sheet name="Budget" sheetId="1" r:id="rId1"/><sheet name="Salaries" sheetId="2" state="hidden" r:id="rId2"/></sheets></workbook>',
    "xl/_rels/workbook.xml.rels": '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
        '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">'
        '<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/>'
        '<Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet2.xml"/>'
        '</Relationships>',
    "xl/worksheets/sheet1.xml": sheet([
        ["Item", "EUR"], ["Travel", 12000], ["Equipment", 8500],
        ["Bonuses (do not share)", 30000], ["Total", 20500]], hidden=(4,)),
    "xl/worksheets/sheet2.xml": sheet([
        ["Name", "Salary, EUR"], ["Olena Koval", 4200], ["Petro Ivanenko", 3900]]),
}

with zipfile.ZipFile(OUT, "w", zipfile.ZIP_DEFLATED) as z:
    for name, text in FILES.items():
        info = zipfile.ZipInfo(name, date_time=(2026, 2, 10, 9, 0, 0))
        info.compress_type = zipfile.ZIP_DEFLATED
        z.writestr(info, text)
print(f"wrote {os.path.normpath(OUT)}: {os.path.getsize(OUT)} bytes")
