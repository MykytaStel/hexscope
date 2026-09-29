# Launch notes

Drafts for posting hexscope, and what to check first. Posting is done by
hand, from the author's own accounts.

## Before posting

- [ ] https://hexscope.pages.dev loads on a phone and a computer, in both
      themes, and the landing demo plays: a photo's facts, then struck out.
- [ ] Every door works: a photo, a blacked-out PDF, a fake bank email, a
      sample email, a broken image, compression.
- [ ] Each opens on its one-line answer with "Remove it — save a clean
      copy" under it; a photo shows its map, an email its way to you, and a
      clean copy ends on before and after.
- [ ] A photo from the phone's own library: its location shows, and the
      clean copy is shared or saved without it.
- [ ] A long video from the phone (a gigabyte or more): it opens in a
      moment, the bytes view scrolls into its picture and sound, and the
      clean copy saves at its full size without the place.
- [ ] Installed on a computer, hexscope is offered in the file manager's
      "Open with", and opens the file.
- [ ] A real PDF blacked out by drawing boxes (Preview, Word → PDF, a scan):
      hexscope finds what is under them, and its copy no longer holds it.
- [ ] Black out text yourself, on a text page and on a scan; the copy
      opened again says nothing is hidden.
- [ ] Several photos at once: the list, then clean copies (shared on a
      phone, a ZIP on a computer).
- [ ] The guides load: /black-out-a-pdf, /is-this-email-real,
      /what-a-screenshot-gives-away, /remove-location-from-photo,
      /check-document-before-sending, /hidden-text-in-pdf,
      /pdf-hidden-versions, /png-wont-open, /deflate,
      /your-files-stay-private.
- [ ] The repo README opens on what it is and links the site.
- [ ] A few hours free after posting, to answer comments.

## Show HN

Post on a weekday, 8–10am US Eastern (15:00–17:00 in Kyiv). Link to the site,
not the repo; the site links the code.

**Title** (HN allows 80 characters):

> Show HN: Hexscope – see what a file gives away before you send it

Or, for the redaction side:

> Show HN: Hexscope – find the text under a PDF's black boxes, and remove it

Or, for the curious:

> Show HN: Hexscope – point at a pixel, see the bits that made it

**Text:**

> hexscope reads a file in your browser and tells you what it says about
> you before you send it: where a photo was taken and the camera's serial
> number, the text still under a PDF's black boxes, the comments and
> tracked changes in a Word file, the hidden sheets in a workbook, the
> address and computer name in an email's headers. Then it saves a copy
> without them. Nothing is uploaded; there is no account and no server.
>
> The part I cared most about is blacking out. A box drawn over a name in a
> PDF hides it on screen and leaves it in the file, and a box over a
> scanned page leaves the picture whole. hexscope finds both, shows what
> anyone can still read or see under them, and its copy takes out the
> letters — and the picture's pixels — rather than covering them. You can
> also black out by search (names, emails, phone numbers, card numbers),
> or draw boxes on the page, scans included.
>
> For emails it reads the saved .eml: where replies really go, whether the
> sender's domain vouched for it, and where it was sent from.
>
> Under it is a parser that explains every byte: pick any part and it says
> what it is in one sentence, with the spec section. Point at a pixel of a
> PNG and see the DEFLATE step that wrote it; point at part of a JPEG and
> see the bytes that draw it. There is a step-by-step DEFLATE player and an
> explainer built on it: https://hexscope.pages.dev/deflate
>
> Formats: JPEG, PNG, HEIC/AVIF, WebP, GIF, MP4/MOV, PDF, Word/Excel/
> PowerPoint and other ZIPs, email, WebAssembly. It is Rust compiled to
> WebAssembly — about 130 KB gzipped for pictures and movies, the rest
> loaded when a document is opened — with no runtime dependencies in the
> core, fuzzed, and never panics on any input. The same core is a command
> line tool, a GitHub Action and a pre-commit hook, for checking what goes
> into a repository or onto a site.
>
> Source: https://github.com/MykytaStel/hexscope — I'd love to hear where
> it gets things wrong, and which files you'd check with it.

## Reddit

**r/privacy** — *Blacked out a PDF by drawing boxes? The text is still
there — how to check, and remove it, without uploading the file.* Link the
guide: https://hexscope.pages.dev/black-out-a-pdf. A second post, another
week: *Check what your photos reveal before you send them* —
https://hexscope.pages.dev/remove-location-from-photo.

**r/scams** or **r/phishing** — *How to tell from its headers whether an
email is from who it says, without clicking anything:*
https://hexscope.pages.dev/is-this-email-real. Ask the moderators first if
the rules want it.

**r/rust** — *Show: a file inspector in Rust + WASM that never panics, and
redacts PDFs properly*

> The core parses JPEG, PNG, HEIF, MP4, PDF, ZIP/Office, email and WASM with
> no runtime dependencies (even the deflate encoder, MD5, RC4, AES and
> SHA-2 are written out), `forbid(unsafe_code)`, fuzzed in CI. The browser
> gets two builds — pictures only, and everything — chosen by the file's
> first bytes. Redaction rewrites page content glyph by glyph and zeroes
> the pixels of pictures under a box. Code:
> https://github.com/MykytaStel/hexscope — feedback on the design very
> welcome.

**r/programming** — link post to https://hexscope.pages.dev/deflate, titled
*How DEFLATE works, visually — step through a real decompression in the
browser*.

## What to record

A short clip or GIF, for the posts that allow one:

1. The sample blacked-out PDF: the verdict, then the page with the names
   showing under the boxes, then "Show as a viewer does".
2. A scan with a box drawn over a name: the name visible through the box's
   outline; the clean copy; the copy opened again, nothing hidden.
3. The sample photo: its location, then "Remove it — save a clean copy",
   and the facts struck out.

## Answering

- Thank people, answer what was asked, and say plainly what it does not do
  yet (docs/known-issues.md).
- Bugs go straight into GitHub issues; link them in the reply.
- No argument about other tools. Say what hexscope does differently.
