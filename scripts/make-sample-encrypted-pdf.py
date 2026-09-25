#!/usr/bin/env python3
"""Writes encrypted PDF fixtures: one small document under each of the
standard security handler's schemes (ISO 32000-1 §7.6.3, and revision 6
from ISO 32000-2), all opening without a password, plus one that needs one.

  encrypted-rc4-40.pdf    V1 R2, RC4 with a 40-bit key
  encrypted-rc4-128.pdf   V2 R3, RC4 with a 128-bit key
  encrypted-aes-128.pdf   V4 R4, AESV2 crypt filters
  encrypted-aes-256.pdf   V5 R6, AESV3 crypt filters
  encrypted-password.pdf  V5 R6, and a password to open it: "open-sesame"

Needs the `cryptography` package. The salts and IVs are fixed, so the
output does not change between runs.

  python3 scripts/make-sample-encrypted-pdf.py
"""
import hashlib
import os
import zlib

from cryptography.hazmat.decrepit.ciphers.algorithms import ARC4
from cryptography.hazmat.primitives.ciphers import Cipher, algorithms, modes

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, "..", "crates", "hexscope-core", "tests", "fixtures")

PAD = bytes.fromhex("28BF4E5E4E758A4164004E56FFFA01082E2E00B6D0683E802F0CA9FE6453697A")
FILE_ID = bytes.fromhex("5e6a1c0d9b2f4e7a8c3d1f0e2b4a6c8d")
OWNER = b"owner-secret"
P = -1340  # print and copy allowed, some bits off; any value works


def rc4(key, data):
    c = Cipher(ARC4(key), mode=None).encryptor()
    return c.update(data) + c.finalize()


def aes_cbc(key, iv, data, encrypt=True):
    c = Cipher(algorithms.AES(key), modes.CBC(iv))
    op = c.encryptor() if encrypt else c.decryptor()
    return op.update(data) + op.finalize()


def pkcs7(data):
    n = 16 - len(data) % 16
    return data + bytes([n]) * n


def p_bytes():
    return (P & 0xFFFFFFFF).to_bytes(4, "little")


# --- revisions 2 to 4 (Algorithms 2, 3, 4, 5) ---

def padded(pw):
    return (pw + PAD)[:32]


def owner_entry(rev, n):
    h = hashlib.md5(padded(OWNER)).digest()
    if rev >= 3:
        for _ in range(50):
            h = hashlib.md5(h[:n]).digest()
    key = h[:n]
    o = rc4(key, padded(b""))
    if rev >= 3:
        for i in range(1, 20):
            o = rc4(bytes(b ^ i for b in key), o)
    return o


def file_key(rev, n, o, user=b""):
    h = hashlib.md5(padded(user) + o + p_bytes() + FILE_ID).digest()
    if rev >= 3:
        for _ in range(50):
            h = hashlib.md5(h[:n]).digest()
    return h[:n]


def user_entry(rev, key):
    if rev == 2:
        return rc4(key, PAD)
    u = rc4(key, hashlib.md5(PAD + FILE_ID).digest())
    for i in range(1, 20):
        u = rc4(bytes(b ^ i for b in key), u)
    return u + b"\x00" * 16


# --- revision 6 (ISO 32000-2, Algorithms 2.A, 2.B, 8, 9, 10) ---

def hash_2b(pw, salt, udata):
    k = hashlib.sha256(pw + salt + udata).digest()
    i = 0
    e = b""
    while i < 64 or e[-1] > i - 32:
        k1 = (pw + k + udata) * 64
        e = aes_cbc(k[:16], k[16:32], k1)
        h = [hashlib.sha256, hashlib.sha384, hashlib.sha512][sum(e[:16]) % 3]
        k = h(e).digest()
        i += 1
    return k[:32]


def r6_entries(key, user):
    vs, ks = b"uservals", b"userkeys"
    u = hash_2b(user, vs, b"") + vs + ks
    ue = aes_cbc(hash_2b(user, ks, b""), b"\x00" * 16, key)
    ovs, oks = b"ownrvals", b"ownrkeys"
    o = hash_2b(OWNER, ovs, u) + ovs + oks
    oe = aes_cbc(hash_2b(OWNER, oks, u), b"\x00" * 16, key)
    perms_plain = p_bytes() + b"\xff\xff\xff\xff" + b"Tadb" + b"perm"
    c = Cipher(algorithms.AES(key), modes.ECB()).encryptor()
    perms = c.update(perms_plain) + c.finalize()
    return o, u, oe, ue, perms


# --- the document ---

XMP = b"""<?xpacket begin="\xef\xbb\xbf" id="W5M0MpCehiHzreSzNTczkc9d"?>
<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
<rdf:Description rdf:about="" xmlns:xmp="http://ns.adobe.com/xap/1.0/">
<xmp:CreatorTool>Sample Writer 3.1</xmp:CreatorTool></rdf:Description></rdf:RDF></x:xmpmeta>
<?xpacket end="w"?>"""


def build(scheme, user=b""):
    rev, v, n = {"rc4-40": (2, 1, 5), "rc4-128": (3, 2, 16), "aes-128": (4, 4, 16), "aes-256": (6, 5, 32)}[scheme]
    if rev == 6:
        key = hashlib.sha256(b"hexscope fixture key " + scheme.encode()).digest()
        o, u, oe, ue, perms = r6_entries(key, user)
    else:
        o = owner_entry(rev, n)
        key = file_key(rev, n, o, user)
        u = user_entry(rev, key)

    counter = [0]

    def seal(num, data):
        """Encrypts a string or stream of object `num` (generation 0)."""
        if rev == 6:
            counter[0] += 1
            iv = hashlib.md5(b"iv%d" % counter[0]).digest()
            return iv + aes_cbc(key, iv, pkcs7(data))
        salt = b"sAlT" if rev == 4 else b""
        k = hashlib.md5(key + num.to_bytes(3, "little") + b"\x00\x00" + salt).digest()[: min(n + 5, 16)]
        if rev == 4:
            counter[0] += 1
            iv = hashlib.md5(b"iv%d" % counter[0]).digest()
            return iv + aes_cbc(k, iv, pkcs7(data))
        return rc4(k, data)

    def s(num, text):
        return b"<" + seal(num, text).hex().encode() + b">"

    out = bytearray(b"%PDF-1.7\n%\xe2\xe3\xcf\xd3\n")
    offsets = {}

    def obj(num, body, stream=None):
        offsets[num] = len(out)
        out.extend(b"%d 0 obj\n" % num)
        if stream is None:
            out.extend(body + b"\nendobj\n")
        else:
            data = seal(num, stream)
            out.extend(body % len(data) + b"\nstream\n" + data + b"\nendstream\nendobj\n")

    obj(1, b"<< /Type /Catalog /Pages 2 0 R /Metadata 6 0 R >>")
    obj(2, b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>")
    obj(3, b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R >>")
    obj(4, b"<< /Length %d /Filter /FlateDecode >>",
        zlib.compress(b"BT /F1 14 Tf 72 720 Td (An encrypted sample) Tj ET"))
    obj(5, b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>")
    obj(6, b"<< /Type /Metadata /Subtype /XML /Length %d >>", XMP)
    obj(7, b"<< /Title " + s(7, b"Payroll 2026") + b" /Author " + s(7, b"Mariia Bondar")
        + b" /Producer " + s(7, b"hexscope sample generator")
        + b" /CreationDate " + s(7, b"D:20260512100000Z") + b" >>")

    enc = [b"/Filter /Standard", b"/V %d" % v, b"/R %d" % rev, b"/P %d" % P,
           b"/O <" + o.hex().encode() + b">", b"/U <" + u.hex().encode() + b">"]
    if v == 2:
        enc.append(b"/Length 128")
    if v >= 4:
        cfm, length = (b"/AESV2", 16) if v == 4 else (b"/AESV3", 32)
        enc += [b"/Length %d" % (length * 8),
                b"/CF << /StdCF << /CFM " + cfm + b" /AuthEvent /DocOpen /Length %d >> >>" % length,
                b"/StmF /StdCF /StrF /StdCF"]
    if v == 5:
        enc += [b"/OE <" + oe.hex().encode() + b">", b"/UE <" + ue.hex().encode() + b">",
                b"/Perms <" + perms.hex().encode() + b">"]
    obj(8, b"<< " + b"\n   ".join(enc) + b" >>")

    xref = len(out)
    out.extend(b"xref\n0 9\n0000000000 65535 f\r\n")
    for num in range(1, 9):
        out.extend(b"%010d 00000 n\r\n" % offsets[num])
    ident = b"<" + FILE_ID.hex().encode() + b">"
    out.extend(b"trailer\n<< /Size 9 /Root 1 0 R /Info 7 0 R /Encrypt 8 0 R /ID [" + ident + b" " + ident
               + b"] >>\nstartxref\n%d\n%%%%EOF\n" % xref)
    return bytes(out)


for scheme in ["rc4-40", "rc4-128", "aes-128", "aes-256"]:
    name = f"encrypted-{scheme}.pdf"
    data = build(scheme)
    with open(os.path.join(OUT, name), "wb") as f:
        f.write(data)
    print(name, len(data))
with open(os.path.join(OUT, "encrypted-password.pdf"), "wb") as f:
    f.write(build("aes-256", user=b"open-sesame"))
print("encrypted-password.pdf")
