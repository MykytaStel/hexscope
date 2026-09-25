# Launch notes

Drafts for posting hexscope, and what to check first. Posting is done by
hand, from the author's own accounts.

## Before posting

- [ ] https://hexscope.pages.dev and /deflate load, on a phone too.
- [ ] Every sample works: photo, broken image, document, compression.
- [ ] The repo README opens on what it is and links the site.
- [ ] A few hours free after posting, to answer comments.

## Show HN

Post on a weekday, 8–10am US Eastern (15:00–17:00 in Kyiv). Link to the site,
not the repo; the site links the code.

**Title** (HN allows 80 characters):

> Show HN: Hexscope – watch DEFLATE decompress, bit by bit, in the browser

**Text:**

> hexscope takes a file apart in your browser: every byte, what it means,
> where the file is broken, and what it says about you. PNG, JPEG, HEIC,
> AVIF, PDF and ZIP (so also .docx, .apk, .epub) for now.
>
> The part I most wanted to exist: a step-by-step DEFLATE player. For any
> PNG or ZIP entry you can step through the decompression one decision at
> a time — the bits it read, the Huffman code, which bytes a back-reference
> copies — with a reading head on the file's own bytes. There is a short
> visual explainer built on it: https://hexscope.pages.dev/deflate
>
> Other things it does: shows what a photo reveals (GPS, camera serial
> number, owner) and saves a copy without it, without re-encoding the
> picture; names what a ZIP can hide (data before the archive, local
> headers that disagree with the central directory, overlapping entries);
> explains every field in one sentence, with a link to the spec section.
>
> It is Rust compiled to WebAssembly (about 120 KB gzipped), no runtime
> dependencies in the core, fuzzed, and it never uploads anything — the
> source is here: https://github.com/MykytaStel/hexscope
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
and remove it, without uploading them anywhere.*

## Answering

- Thank people, answer what was asked, and say plainly what it does not do
  yet.
- Bugs go straight into GitHub issues; link them in the reply.
- No argument about other tools. Say what hexscope does differently.
