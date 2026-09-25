# Launch notes

Drafts for posting hexscope, and what to check first. Posting is done by
hand, from the author's own accounts.

## Before posting

- [ ] https://hexscope.pages.dev and /deflate load, on a phone too.
- [ ] Every sample works: photo, broken image, document, compression, HEIC,
      edited PDF.
- [ ] Pointing at a pixel of a PNG marks its bytes; pointing at an IDAT
      byte marks its pixels.
- [ ] The guides load: /remove-location-from-photo, /pdf-hidden-versions,
      /png-wont-open.
- [ ] The repo README opens on what it is and links the site.
- [ ] A few hours free after posting, to answer comments.

## Show HN

Post on a weekday, 8–10am US Eastern (15:00–17:00 in Kyiv). Link to the site,
not the repo; the site links the code.

**Title** (HN allows 80 characters):

> Show HN: Hexscope – point at a pixel, see the bits that made it

Or, if the DEFLATE story reads better on the day:

> Show HN: Hexscope – watch DEFLATE decompress, bit by bit, in the browser

**Text:**

> hexscope takes a file apart in your browser: every byte, what it means,
> where the file is broken, and what it says about you. PNG, JPEG, HEIC,
> AVIF, PDF and ZIP (so also .docx, .apk, .epub) for now.
>
> The part I most wanted to exist: point at a pixel of a PNG and see the
> DEFLATE step that wrote it — a literal, or a copy of 258 bytes from
> exactly one row up — with the bits that hold it marked in the file, and
> the reverse, from a byte to the pixels it became. Behind it is a
> step-by-step DEFLATE player: every decision, the bits it read, the
> Huffman code, which bytes a back-reference copies. There is a short
> visual explainer built on it: https://hexscope.pages.dev/deflate
>
> Other things it does: shows what a photo reveals (GPS, camera serial
> number, owner) and saves a copy without it, without re-encoding the
> picture; shows that an edited PDF usually still holds its earlier
> version, and writes one without it; names what a ZIP can hide (data
> before the archive, local headers that disagree with the central
> directory, overlapping entries); explains every field in one sentence,
> with a link to the spec section.
>
> It is Rust compiled to WebAssembly (under 200 KB gzipped), no runtime
> dependencies in the core — even the MD5, RC4, AES and SHA-2 for
> encrypted PDFs are written out — fuzzed, and it never uploads anything.
> The source is here: https://github.com/MykytaStel/hexscope
>
> I'd love to hear which formats you'd want next, and where it gets things
> wrong.

## Reddit

**r/rust** — *Show: a DEFLATE decoder you can step through, in Rust + WASM*

> I wrote a PNG/JPEG/ZIP parser and a pausable DEFLATE decoder in Rust
> (no runtime deps, `forbid(unsafe_code)`, fuzzed, never panics on any
> input) and put a browser UI on it. The decoder emits one step per
> decision and resumes from checkpoints, so the player can seek anywhere in
> a 10 MB stream. Explainer: https://hexscope.pages.dev/deflate — code:
> https://github.com/MykytaStel/hexscope. Feedback on the decoder design
> very welcome.

**r/programming** — link post to https://hexscope.pages.dev/deflate, titled
*How DEFLATE works, visually — step through a real decompression in the
browser*.

**r/privacy** — *Check what your photos reveal,
and remove it, without uploading them anywhere.* Link the guide rather than
the app: https://hexscope.pages.dev/remove-location-from-photo — or, for a
second post, https://hexscope.pages.dev/pdf-hidden-versions (*Deleted from a
PDF, but still inside it*).

## Answering

- Thank people, answer what was asked, and say plainly what it does not do
  yet.
- Bugs go straight into GitHub issues; link them in the reply.
- No argument about other tools. Say what hexscope does differently.
