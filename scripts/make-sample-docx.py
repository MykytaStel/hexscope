#!/usr/bin/env python3
"""Builds two Word documents:

- crates/hexscope-core/tests/fixtures/report.docx, written entirely by macOS
  `textutil` (Apple's Cocoa document writer), so the core is tested against
  an archive a real application produced;
- apps/web/public/samples/report.docx, the same document with what people
  forget a document carries: a comment, a tracked deletion that still holds
  the deleted text, and the sample photo added as word/media/photo.jpg,
  stored uncompressed the way Word stores images. Opening that image inside
  the document shows where it was taken.

Every name and date is invented.

    python3 scripts/make-sample-docx.py
"""
import os
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

W = 'xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"'
COMMENTS = f"""<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:comments {W}><w:comment w:id="0" w:author="Olena Koval" w:date="2025-11-03T09:40:00Z" w:initials="OK"><w:p><w:r><w:t>Numbers are still the old ones: do not send this to the client yet.</w:t></w:r></w:p></w:comment></w:comments>"""
SENTENCE = "Sales grew in every region this quarter. Sales grew in every region this quarter, and the photo from the launch event is attached below."
TRACKED = (
    '<w:t xml:space="preserve">Sales grew in every region this quarter</w:t></w:r>'
    '<w:del w:id="1" w:author="Petro Ivanenko" w:date="2025-11-03T10:02:00Z"><w:r>'
    '<w:delText xml:space="preserve">, except the east, where we lost the Lviv contract</w:delText></w:r></w:del>'
    '<w:r><w:t xml:space="preserve">. Sales grew in every region this quarter, and the photo from the launch event is attached below.</w:t>'
)


def edit(name, text):
    """The sample's changes to one part of the document."""
    if name == "word/document.xml":
        title = '<w:t xml:space="preserve">Quarterly report</w:t></w:r>'
        assert title in text and f'<w:t xml:space="preserve">{SENTENCE}</w:t>' in text
        text = text.replace(
            title,
            title + '<w:commentRangeEnd w:id="0"/><w:r><w:commentReference w:id="0"/></w:r>',
        )
        text = text.replace('<w:body><w:p><w:pPr><w:spacing w:after="321"/></w:pPr>',
                            '<w:body><w:p><w:pPr><w:spacing w:after="321"/></w:pPr><w:commentRangeStart w:id="0"/>')
        text = text.replace(f'<w:t xml:space="preserve">{SENTENCE}</w:t>', TRACKED)
    elif name == "word/_rels/document.xml.rels":
        text = text.replace("</Relationships>", '<Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments" Target="comments.xml"/></Relationships>')
    elif name == "[Content_Types].xml":
        text = text.replace("</Types>", '<Override PartName="/word/comments.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.comments+xml"/></Types>')
    return text


with zipfile.ZipFile(FIXTURE) as src, zipfile.ZipFile(SAMPLE, "w") as out:
    for info in src.infolist():
        data = edit(info.filename, src.read(info).decode()).encode()
        out.writestr(info, data, compress_type=info.compress_type)
    out.writestr("word/comments.xml", COMMENTS, compress_type=zipfile.ZIP_DEFLATED)
    out.write(PHOTO, "word/media/photo.jpg", compress_type=zipfile.ZIP_STORED)

print(f"wrote {FIXTURE} and {SAMPLE}")
