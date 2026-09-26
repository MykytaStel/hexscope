#!/usr/bin/env python3
"""Writes the PDF fixtures, byte for byte, so every offset is known.

  report.pdf   classic cross-reference tables, an XMP packet, and one
               incremental update: the salary line was changed and the
               title and date updated, but the first version is still inside.
  compact.pdf  PDF 1.5: the document information sits inside a compressed
               object stream, found through a cross-reference stream, with
               compressed XMP.
  redacted.pdf "redacted" the way that does not work: black boxes painted
               over a name, a phone number and a sum, which are all still in
               the page; a note in white on white; a comment; a spreadsheet
               of payments attached; and on page two an area marked for
               redaction that was never applied.

The text is original. Run from anywhere:

  python3 scripts/make-sample-pdf.py
"""
import os
import zlib

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, "..", "crates", "hexscope-core", "tests", "fixtures")

XMP = b"""<?xpacket begin="\xef\xbb\xbf" id="W5M0MpCehiHzreSzNTczkc9d"?>
<x:xmpmeta xmlns:x="adobe:ns:meta/">
 <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
  <rdf:Description rdf:about=""
    xmlns:dc="http://purl.org/dc/elements/1.1/"
    xmlns:xmp="http://ns.adobe.com/xap/1.0/"
    xmlns:pdf="http://ns.adobe.com/pdf/1.3/"
    xmlns:xmpMM="http://ns.adobe.com/xap/1.0/mm/"
    xmlns:stEvt="http://ns.adobe.com/xap/1.0/sType/ResourceEvent#">
   <dc:creator><rdf:Seq><rdf:li>Olena Koval</rdf:li></rdf:Seq></dc:creator>
   <dc:title><rdf:Alt><rdf:li xml:lang="x-default">Quarterly plan</rdf:li></rdf:Alt></dc:title>
   <xmp:CreatorTool>Sample Writer 3.1</xmp:CreatorTool>
   <xmp:CreateDate>2026-03-02T09:14:00+02:00</xmp:CreateDate>
   <pdf:Producer>hexscope sample generator</pdf:Producer>
   <xmpMM:History><rdf:Seq>
    <rdf:li stEvt:action="created" stEvt:softwareAgent="Sample Writer 3.1"/>
    <rdf:li stEvt:action="saved" stEvt:softwareAgent="Sample Writer 3.1"/>
    <rdf:li stEvt:action="converted" stEvt:softwareAgent="hexscope sample generator"/>
   </rdf:Seq></xmpMM:History>
  </rdf:Description>
 </rdf:RDF>
</x:xmpmeta>
<?xpacket end="w"?>"""


class Writer:
    def __init__(self, header):
        self.out = bytearray(header)
        self.offsets = {}

    def obj(self, num, body, stream=None):
        self.offsets[num] = len(self.out)
        self.out += b"%d 0 obj\n" % num
        if stream is None:
            self.out += body + b"\nendobj\n"
        else:
            self.out += body + b"\nstream\n" + stream + b"\nendstream\nendobj\n"

    def xref(self, nums, trailer):
        start = len(self.out)
        self.out += b"xref\n"
        runs = []
        for n in sorted(nums):
            if runs and runs[-1][-1] == n - 1:
                runs[-1].append(n)
            else:
                runs.append([n])
        for run in runs:
            self.out += b"%d %d\n" % (run[0], len(run))
            for n in run:
                if n == 0:
                    self.out += b"0000000000 65535 f\r\n"
                else:
                    self.out += b"%010d 00000 n\r\n" % self.offsets[n]
        self.out += b"trailer\n" + trailer + b"\nstartxref\n%d\n%%%%EOF\n" % start
        return start


def page_text(lines):
    ops = b"BT /F1 14 Tf 72 720 Td 18 TL\n"
    for line in lines:
        ops += b"(" + line + b") Tj T*\n"
    return ops + b"ET"


def report():
    w = Writer(b"%PDF-1.7\n%\xe2\xe3\xcf\xd3\n")
    w.obj(1, b"<< /Type /Catalog /Pages 2 0 R /Metadata 6 0 R /Lang (en-GB) >>")
    w.obj(2, b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>")
    w.obj(3, b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842]\n"
             b"   /Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R >>")
    content = zlib.compress(page_text([b"Quarterly plan", b"Salary: 4,200 EUR a month"]))
    w.obj(4, b"<< /Length %d /Filter /FlateDecode >>" % len(content), content)
    w.obj(5, b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>")
    w.obj(6, b"<< /Type /Metadata /Subtype /XML /Length %d >>" % len(XMP), XMP)
    w.obj(7, b"<< /Title (Quarterly plan) /Author (Olena Koval)\n"
             b"   /Creator (Sample Writer 3.1) /Producer (hexscope sample generator)\n"
             b"   /CreationDate (D:20260302091400+02'00') >>")
    first = w.xref(range(0, 8),
                   b"<< /Size 8 /Root 1 0 R /Info 7 0 R\n"
                   b"   /ID [<8a3f0c1e5b7d49a2a1c4e6f809b2d3c4> <8a3f0c1e5b7d49a2a1c4e6f809b2d3c4>] >>")

    # The update: new page content and new information, appended.
    content = zlib.compress(page_text([b"Quarterly plan", b"Salary: see the attached sheet"]))
    w.obj(4, b"<< /Length %d /Filter /FlateDecode >>" % len(content), content)
    w.obj(7, b"<< /Title <FEFF0051007500610072007400650072006C007900200070006C0061006E0020002D00200076003200>\n"
             b"   /Author (Olena Koval) /Creator (Sample Writer 3.1)\n"
             b"   /Producer (hexscope sample generator)\n"
             b"   /CreationDate (D:20260302091400+02'00') /ModDate (D:20260305174210+02'00') >>")
    w.xref([0, 4, 7],
           b"<< /Size 8 /Root 1 0 R /Info 7 0 R /Prev %d\n"
           b"   /ID [<8a3f0c1e5b7d49a2a1c4e6f809b2d3c4> <c0ffee00c0ffee00c0ffee00c0ffee00>] >>" % first)
    return bytes(w.out)


def compact():
    w = Writer(b"%PDF-1.5\n%\xe2\xe3\xcf\xd3\n")
    xmp = zlib.compress(XMP)
    content = zlib.compress(page_text([b"A compact sample"]))
    w.obj(4, b"<< /Length %d /Filter /FlateDecode >>" % len(content), content)
    w.obj(6, b"<< /Type /Metadata /Subtype /XML /Length %d /Filter /FlateDecode >>" % len(xmp), xmp)

    inner = [
        (1, b"<< /Type /Catalog /Pages 2 0 R /Metadata 6 0 R >>"),
        (2, b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>"),
        (3, b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842]"
            b" /Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R >>"),
        (5, b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>"),
        (7, b"<< /Author (Taras Melnyk) /Creator (Sample Writer 3.1)"
            b" /Producer (hexscope sample generator) /CreationDate (D:20260411120000Z) >>"),
    ]
    head, body = b"", b""
    for num, obj in inner:
        head += b"%d %d " % (num, len(body))
        body += obj + b"\n"
    packed = zlib.compress(head + body)
    w.obj(8, b"<< /Type /ObjStm /N %d /First %d /Length %d /Filter /FlateDecode >>"
          % (len(inner), len(head), len(packed)), packed)

    # The cross-reference stream: type, field 2, field 3 for objects 0-9.
    xref_at = len(w.out)
    rows = [(0, 0, 65535)]
    where = {n: i for i, (n, _) in enumerate(inner)}
    for n in range(1, 10):
        if n in where:
            rows.append((2, 8, where[n]))
        elif n == 9:
            rows.append((1, xref_at, 0))
        elif n in w.offsets:
            rows.append((1, w.offsets[n], 0))
        else:
            rows.append((0, 0, 0))
    table = zlib.compress(b"".join(bytes([t]) + f2.to_bytes(4, "big") + f3.to_bytes(2, "big")
                                   for t, f2, f3 in rows))
    w.obj(9, b"<< /Type /XRef /Size 10 /W [1 4 2] /Root 1 0 R /Info 7 0 R\n"
             b"   /Length %d /Filter /FlateDecode >>" % len(table), table)
    w.out += b"startxref\n%d\n%%%%EOF\n" % xref_at
    return bytes(w.out)


def redacted():
    w = Writer(b"%PDF-1.7\n%\xe2\xe3\xcf\xd3\n")
    w.obj(1, b"<< /Type /Catalog /Pages 2 0 R\n"
             b"   /Names << /EmbeddedFiles << /Names [(payments.csv) 11 0 R] >> >> >>")
    w.obj(2, b"<< /Type /Pages /Kids [3 0 R 6 0 R] /Count 2 >>")
    w.obj(3, b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842]\n"
             b"   /Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R /Annots [10 0 R] >>")
    rows = [(b"Claimant:", b"Olena Koval"), (b"Phone:", b"+380 67 123 4567"),
            (b"Settlement:", b"EUR 48,000")]
    ops = b"BT /F1 16 Tf 72 780 Td (Settlement agreement) Tj ET\n"
    for i, (label, value) in enumerate(rows):
        y = 740 - 22 * i
        ops += b"BT /F1 12 Tf 72 %d Td (%s) Tj ET\n" % (y, label)
        ops += b"BT /F1 12 Tf 160 %d Td (%s) Tj ET\n" % (y, value)
    # The "redaction": a black box over each value, drawn after it.
    ops += b"0 g\n"
    for i, (_, value) in enumerate(rows):
        ops += b"156 %d %d 17 re f\n" % (735 - 22 * i, 7 * len(value) + 10)
    # A note left in white: invisible on the page, there for anyone who searches.
    ops += b"1 g BT /F1 9 Tf 72 620 Td (Internal: the client would accept EUR 60,000 if pushed.) Tj ET\n"
    content = zlib.compress(ops)
    w.obj(4, b"<< /Length %d /Filter /FlateDecode >>" % len(content), content)
    w.obj(5, b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>")
    w.obj(6, b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842]\n"
             b"   /Resources << /Font << /F1 5 0 R >> >> /Contents 7 0 R /Annots [8 0 R] >>")
    content = zlib.compress(b"BT /F1 12 Tf 72 780 Td (Signed in Kyiv on 3 March 2026 by Petro Ivanenko.) Tj ET")
    w.obj(7, b"<< /Length %d /Filter /FlateDecode >>" % len(content), content)
    w.obj(8, b"<< /Type /Annot /Subtype /Redact /Rect [258 775 342 794]\n"
             b"   /OverlayText (REDACTED) /IC [0 0 0] >>")
    w.obj(9, b"<< /Title (Settlement agreement - redacted) /Producer (hexscope sample generator) >>")
    w.obj(10, b"<< /Type /Annot /Subtype /Text /Rect [400 730 420 750] /T (Olena Koval)\n"
              b"   /Contents (Check the sum with Petro before this goes out.) >>")
    w.obj(11, b"<< /Type /Filespec /F (payments.csv) /UF (payments.csv) /EF << /F 12 0 R >> >>")
    csv = zlib.compress(b"payee,amount,date\nOlena Koval,48000,2026-03-20\nPetro Ivanenko,12000,2026-03-20\n")
    w.obj(12, b"<< /Type /EmbeddedFile /Subtype /text#2Fcsv /Length %d /Filter /FlateDecode >>" % len(csv), csv)
    w.xref(range(0, 13), b"<< /Size 13 /Root 1 0 R /Info 9 0 R >>")
    return bytes(w.out)


for name, data in [("report.pdf", report()), ("compact.pdf", compact()), ("redacted.pdf", redacted())]:
    with open(os.path.join(OUT, name), "wb") as f:
        f.write(data)
    print(name, len(data))

# The edited one and the badly redacted one are also the web app's samples.
for name, data in [("report.pdf", report()), ("redacted.pdf", redacted())]:
    with open(os.path.join(HERE, "..", "apps", "web", "public", "samples", name), "wb") as f:
        f.write(data)
