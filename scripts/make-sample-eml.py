#!/usr/bin/env python3
"""Builds two email files:

- apps/web/public/samples/message.eml (and the same as a test fixture): an
  ordinary message as a mail server delivers it, with what people do not
  know an email carries — the address the sender connected from in the
  first server's Received line, the laptop's name in the message ID, the
  mail app and its version, the sender's time zone — and the sample photo
  attached, which says where it was taken.
- crates/hexscope-core/tests/fixtures/phishing.eml: a message pretending to
  be a bank, whose replies go elsewhere and whose domain did not vouch for it.

Every name, address and domain is invented; the IP addresses are from the
ranges set aside for documentation (RFC 5737).

    python3 scripts/make-sample-eml.py
"""
import base64
import os
import shutil

ROOT = os.path.join(os.path.dirname(__file__), "..")
SAMPLE = os.path.join(ROOT, "apps", "web", "public", "samples", "message.eml")
FIXTURES = os.path.join(ROOT, "crates", "hexscope-core", "tests", "fixtures")
PHOTO = os.path.join(ROOT, "apps", "web", "public", "samples", "photo.jpg")


def wrap(b: bytes) -> str:
    s = base64.b64encode(b).decode()
    return "\r\n".join(s[i : i + 76] for i in range(0, len(s), 76))


photo = open(PHOTO, "rb").read()

message = f"""Return-Path: <olena.koval@example.org>
Delivered-To: taras@example.net
Received: from mx.example.net (mx.example.net [198.51.100.20])
\tby inbox.example.net with LMTP; Mon, 14 Sep 2026 18:33:02 +0000
Received: from smtp.example.org (smtp.example.org [198.51.100.7])
\tby mx.example.net (Postfix) with ESMTPS id 4Hx2;
\tMon, 14 Sep 2026 18:33:01 +0000 (UTC)
Received: from [192.168.1.23] (MacBook-Pro-Olena.local [203.0.113.54])
\tby smtp.example.org (Postfix) with ESMTPSA id 91AF;
\tMon, 14 Sep 2026 18:32:59 +0000 (UTC)
Authentication-Results: mx.example.net;
\tspf=pass smtp.mailfrom=example.org;
\tdkim=pass header.d=example.org;
\tdmarc=pass header.from=example.org
From: Olena Koval <olena.koval@example.org>
To: Taras <taras@example.net>
Subject: =?utf-8?Q?Photos_from_Saturday_=E2=80=94_the_view?=
Date: Mon, 14 Sep 2026 21:32:57 +0300
Message-ID: <5E2A1C77-3B0F-4D9E-9A61-7F4C2B9D8E10@MacBook-Pro-Olena.local>
X-Mailer: Apple Mail (2.3826.300.87)
MIME-Version: 1.0
Content-Type: multipart/mixed; boundary="Apple-Mail=_B8D2"

--Apple-Mail=_B8D2
Content-Type: text/plain; charset=utf-8
Content-Transfer-Encoding: quoted-printable

Hi Taras,

Here is the photo from Saturday =E2=80=94 what a view.

Olena

--Apple-Mail=_B8D2
Content-Type: image/jpeg; name="IMG_2041.jpg"
Content-Disposition: attachment; filename="IMG_2041.jpg"
Content-Transfer-Encoding: base64

{wrap(photo)}
--Apple-Mail=_B8D2--
""".replace("\n", "\r\n")

phishing = """Return-Path: <bounce@mailer.example.com>
Received: from mailer.example.com (unknown [192.0.2.66])
\tby mx.example.net (Postfix) with ESMTP id 7Q1;
\tTue, 15 Sep 2026 07:12:44 +0000 (UTC)
Authentication-Results: mx.example.net;
\tspf=fail smtp.mailfrom=example-bank.com;
\tdkim=none;
\tdmarc=fail header.from=example-bank.com
From: Example Bank Security <security@example-bank.com>
Reply-To: verify-account@example.info
To: taras@example.net
Subject: Your account is on hold
Date: Tue, 15 Sep 2026 07:12:40 +0000
Message-ID: <a81f@mailer.example.com>
MIME-Version: 1.0
Content-Type: text/plain; charset=utf-8

Confirm your details within 24 hours, or your account will be closed.
""".replace("\n", "\r\n")

with open(SAMPLE, "w", newline="") as f:
    f.write(message)
shutil.copy(SAMPLE, os.path.join(FIXTURES, "message.eml"))
with open(os.path.join(FIXTURES, "phishing.eml"), "w", newline="") as f:
    f.write(phishing)
print("wrote", SAMPLE, "and the fixtures")
