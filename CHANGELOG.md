# Changelog

What changed in each version of hexscope — the website, the command-line
tool and the crates share one version number. Newest first. The format is
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/): until 1.0, a minor version may
change what the CLI's JSON says. How a version is made: [docs/releasing.md](docs/releasing.md).

## [Unreleased]

### Changed

- **The analyzer in Ukrainian, all of it**: the parts of a file in the bytes'
  colours, what a file is made of, the details pane and its tools, the way an
  email came, blacking out text and pictures, the keys, finding in a file,
  several files at once, the DEFLATE player and the pictures' captions —
  about 250 lines that stayed in English. Sizes read "105,2 КБ", a verdict's
  list is Ukrainian to its end ("… і ще 4"), and a list's heading names a
  finding in its own case. A new end-to-end test opens every sample and test
  file in every view, makes a clean copy, compares, lists several files, and
  fails on any English word of hexscope's own.
- The photo's facts are on the home page from its first paint; the clean
  copy strikes them once it is checked.
- The home page's top bar and sections share one gutter at every width.
- The film-scan clues show only where a picture has some: a camera's photo
  no longer carries a section saying it has none.
- The row under "Location" is called "Coordinates".
- The home page says "nothing is uploaded" once, and in its other places what
  else is true: it works offline, and the developer tools show nothing sent.
- The README lists every guide, and the crates by the site's version.

### Fixed

- Ukrainian guides closed every quote they open and keep the space after a
  name in italics ("EXIF — блоку…"); a test reads each built page for it.
- What a document is made of no longer says "the the document".
- The visitor counter does not count a browser a program drives.

## [0.5.0] — 2026-10-15

### Added

- **Four guides**, in English and Ukrainian: removing EXIF and metadata from
  a photo, removing metadata and the location from a video, removing
  metadata from LibreOffice and RTF files, and telling what type a file
  really is when its name says another. Two new samples go with them: a
  LibreOffice lease and a HEIC photo named `.jpg`.
- **What Office on Windows leaves** is found and taken out of a clean copy:
  the folder Excel last saved a workbook in (`x15ac:absPath`, a path with the
  account's name), the folder of a Word document's template, the printer a
  printer-settings part names, SharePoint's properties kept as custom XML
  (with the names and addresses in them), a picture of the first page, and
  Word's editing-session numbers (`w:rsid`), which tie documents written in
  the same sittings to one another. The test files `windows.docx` and
  `windows.xlsx` are written as Office writes these parts
  (`scripts/make-sample-windows-office.py`).

### Fixed

- **A link to a site under `.com.ua`, `.org.ua`, `.co.uk` and other shared
  suffixes** was compared by its last two labels, so `rozetka.com.ua` and
  `rozetka-sale.com.ua` counted as one site and a lying link went unsaid.
  The registered name now counts the suffixes anyone registers under (106
  of them, Ukraine's regional ones among them) — in
  emails, Word documents and PDFs, and for replies and bounces sent to
  another domain.
- **A clean copy with any hidden finding left in it** — an attachment, text
  under a black box — was called not completely read, and every finding
  went unchecked. Only bytes that were not read make a copy incomplete now:
  damage, and data nothing in the file accounts for.
- **A PDF's clean copy kept its notes, who wrote every comment, and the files
  it carried.** Now notes and their pop-ups go, every other annotation loses
  its author and dates, a highlight the words of its note, and attached files
  go — as in Word's copy. What a person typed or signed on the page stays,
  and so do the files an e-invoice or a PDF/A-3 file names as part of itself
  (`/AF`). Comments, attachments, text under boxes and hidden text join the
  PDF row of the removal contract, with `redacted.pdf` as evidence.
- **An email's "SPF passed"** was believed from any `Authentication-Results`
  header. A result naming a server the message never came through — as
  anyone can type into a message — is now said to be one, as a sign of
  forgery.
- A ZIP with bytes past its end, or in entries nothing points at, was taken
  for completely read; and a PNG's caption, which every copy loses, was said
  to be unchecked.
- A Ukrainian support page drew its ways to give in English: the words it
  draws for itself were only in the articles' dictionary, which a Ukrainian
  page no longer loads.
- The clean copy of an iPhone photo said it looks the same without its HDR
  gain map; on an HDR screen its highlights are less bright, and it says so.

### Changed

- **An article loads a fifth of the stylesheet:** guides, the support page,
  the privacy page and the 404 page wear the tokens, the top bar, buttons
  and links, the dialog "Write to the developer" opens and their own type —
  3.8 KB gzipped in one file instead of the app's 21 KB in three. What every
  page shares is in one place (`chrome.css`, `dialog.css`); about 25 rules
  that never won — the redesign's story type under the story's own, a
  narrow top bar's spacing under the wide one's — are gone. A script
  (`apps/web/scripts/style-diff.mjs`) reads the computed style of every
  element in 940 states of two builds and names what differs: only the
  feedback field's corners changed, which keep their own rounding in
  keyboard focus now. CI holds an article's stylesheet to 6 KB.
- **On a phone the workspace's bar keeps the views of the answer** —
  Overview, Metadata, Content — and the structure and the bytes wait under
  More, which says when one of them is open. On the landing the header's
  Open shows only once the landing's own button has scrolled away.
- **The Structure and Bytes views explain each part in Ukrainian:** the 512
  explanations a reader meets on real files — what a field, a box, a chunk
  or an object is — have their Ukrainian, loaded with the analyzer for a
  reader in Ukrainian only (27 KB gzipped). The corpus test now holds every
  explanation shown for a test file to its translation.
- The WebAssembly builds are 1.4–2.8 KB smaller: three stable sorts of
  their own item types gave way to the one key sort the builds share.
- **What the core says about a file reads in Ukrainian:** about 30 kinds of
  finding written as sentences — tracked changes, attachments, links, signs
  of forgery, hidden text, hidden rows, editing history, what a module asks
  for — and 13 lines of what a clean copy took out were shown in English.
  A test now reads every test file and sample and fails on any sentence a
  reader meets without its Ukrainian; two more hold the page's table of
  file names to the core's, and every public page's head to its own address.
- **An article opens in the reader's language as its own page:** a guide,
  the support or the privacy page in the other language than the reader's
  opens its `/uk/` or English page before anything is drawn, with where it
  was on the page, and the language button opens the other page instead of
  translating this one. Nothing flashes in the wrong language, and no
  article loads the articles' dictionary any more. The app itself still
  switches in place, so an open file stays open.
- The landing says by its button that nothing is uploaded, where it is seen
  first, not only below the demo.
- **Pages load less for a reader in English:** the Ukrainian words are a file
  of their own, loaded only when a page is read in Ukrainian or was written
  in it. A guide now loads about 7 KB of script instead of about 78 KB, and
  the home page about 25 KB less. A Ukrainian guide read in Ukrainian, which
  is translated at build, no longer loads the articles' dictionary either.
- **The home page starts with 18 KB of script instead of about 85 KB:** the
  analyzer that shows an open file is a module of its own, loaded once the
  page is idle or as soon as a file or a sample is asked for. The size budget
  in CI now counts the home page's script and what it preloads, and the
  analyzer on its own.
- `hexscope-raster`, the film lab's codecs, is published with the other
  crates, so `cargo install hexscope-cli` builds the `film` command too.
- Every guide lists all the others, made from one list at build, so a new
  guide is linked from each page at once.
- Search engines get each page's real dates — from its commits, not the day
  of the build — in the sitemap and in the page's structured data, with a
  breadcrumb back to the home page; after a deploy, IndexNow hears only of
  the pages that changed.

## [0.4.0] — 2026-10-13

### Added

- **A file named as another format** — a Word document called `.pdf`, a HEIC
  photo called `.jpg` — is named by the core, so the CLI says it too: what
  the file is and the name that fits, in its output, JSON (`misnamed`) and
  SARIF, and `--fail-on misnamed` fails on it. A document in a ZIP is named
  as what it is: a Word file called `.xlsx`, or a LibreOffice spreadsheet
  called `.xlsx`, is misnamed, and the name that fits is `.docx` or `.ods`,
  not `.zip` — on the site as well, where the line is now in Ukrainian too.

- **RTF documents**: their title, author, who saved them last, company,
  manager, dates, printing date, versions and editing time, the program and
  the template with its path, custom properties, who made the tracked changes
  and wrote the comments, and the text a deletion still holds — Ukrainian
  names in Windows code page 1251 read right. The clean copy leaves out the
  information and the rest, accepts the tracked changes and deletes the
  comments, and is confirmed to have lost them.
- A document's custom properties count among the things it gives away.

- **LibreOffice documents** (.odt, .ods, .odp): who made it and who saved it
  last, when, how long it was edited, the program and the system it ran on,
  who printed it and the printer's name, the template's path, its comments
  and tracked changes — with the text a deletion still holds — the picture of
  its first page kept inside, and the photos in it. The clean copy empties
  its properties and settings, accepts the changes, deletes the comments,
  blanks the thumbnail and cleans the photos, and is confirmed to have lost
  them. Word's tracked changes and the photos in a document are now
  confirmed removed too.

- **Install the app**, from the menu or the footer: where the browser offers
  it (Chrome, Edge, Android) the button asks it; an iPhone or iPad, which has
  no such offer, is shown the three taps to Add to Home Screen. Nothing shows
  where hexscope is installed already.

- **Write to the developer, without an account.** A problem, an idea or
  thanks goes from the page itself — from the footer, the end of every guide,
  or a file's “Report a problem”, which can attach the file's structure
  report (its layout, none of its content) when ticked. What is sent is what
  the form shows; never the file, never an address. Read by the developer
  with `pnpm feedback`.

- **A way to thank the developer**: a support page with ways to help — Ko-fi,
  from anywhere by card or PayPal, once or every month, and a Monobank jar,
  which takes a card from any Ukrainian bank, Apple Pay or Google Pay, shown
  first to Ukrainian readers — and, after the third clean copy, one quiet line
  saying how, closed for good with ×. The repository's Sponsor button leads
  there too.

- Installed as an app, hexscope also opens TIFF files from the computer or a
  phone's share sheet.

- **TIFF.** Scans and camera TIFFs say where they were taken, the camera and
  its serial number, the owner, the time and the software, read by the same
  code as a JPEG's EXIF. The clean copy empties those tags and the EXIF and
  GPS tables where they lie, so every strip of the picture stays where it
  was, and confirmed removal covers it.

- **Guides for what people search.** An EXIF viewer page, how to remove
  metadata from a PDF and from Word, Excel and PowerPoint files, and whether
  a picture says AI made it — each in English and Ukrainian, linked from every
  guide, the home page and the sitemap.

- **An AI-made picture among the samples**: a PNG with the prompt, settings
  and model a Stable Diffusion web UI writes.

- A PNG's clean copy is confirmed to have lost what made it and the prompt
  (`ai`, `prompt` in the removal contract), with a real file as evidence.

- **hexscope.org.** The site moves to its own address; hexscope.pages.dev
  forwards to it, and the canonical links, sitemap and link preview name it.

- **The site in Ukrainian for search engines.** Every page has a Ukrainian
  copy of its own at `/uk/…`, written at build time, so a search in Ukrainian
  finds the guides; the two are linked to each other (hreflang), described as
  structured data (a free web application, articles) and listed in a sitemap
  of both. The site's address is set once (`SITE` in `apps/web/seo.ts`).

- **A page for a wrong address.** A mistyped or old address now gets a real
  404 with a way back to the app and the guides, in English or Ukrainian,
  instead of the app's home page answering for every address.

- Search engines that take IndexNow (Bing, Yandex and others) hear of the
  site's pages after each deploy.

### Changed

- The visit numbers are read only by the site's author, straight from the
  database with `pnpm stats`; the `/stats` page and its key are gone.

- Pages link to each other by the address the host serves them at, the home
  page and the DEFLATE story name their own address, and link previews say
  which language they are in.

- The link preview is a JPEG half the size, so apps that skip large images
  (WhatsApp) show it.

- Browsers that have seen the site come back to it over HTTPS only.

- The home page's title and description say what it is in the words people
  search with — metadata, EXIF — and its structured data lists what it does.

- **A document loads a quarter less.** The parser is built per family of
  files: a PDF fetches 221 KB of WebAssembly instead of 295 KB, a Word file
  or an email 212 KB, a photo 131 KB as before. A new format now grows only
  its own family.

- The home page loads faster on phones: the demo's photo and its facts are in
  the page from the start, so nothing below it jumps when the demo begins
  (layout shift 0.23 → under 0.1), and the archive frames are shown from small
  copies (≈190 KB instead of ≈950 KB) while the originals stay samples.

- In Ukrainian, a list of what a file reveals ends with “і”, and a button's
  name in quotes has no space inside them.

### Fixed

- **Several photos on a phone**: one full-width button makes the clean
  copies and the next shares them all; making them again steps back once
  they are ready. The photo tools fold under “More tools”, the names option
  is only offered for a ZIP, and the receipt is only promised when there is a
  ZIP. In Ukrainian, the whole batch view is translated, numbers agreeing.
- After a clean copy, its check shows only what has something under it: no
  empty “Still present” or “Not checked”.

- Every dialog's × sits in the corner beside its heading, not on a line of
  its own at the left.
- **A photo from an iPhone is no longer said to hide something.** After the
  picture, an iPhone keeps a second one — the HDR gain map — and lists it in
  the file's MPF index. hexscope now reads that index and shows the picture as
  what it is, so the verdict no longer says “Hidden in it”, and the clean
  copy, which still leaves it out, is confirmed clean instead of “not fully
  checked”. Bytes after the picture that no index lists are still flagged.
- A page left open while a new version came out checks its clean copy again:
  the previous version's files stay until the one after, instead of going the
  moment the new one arrives.
- On a phone: no tour over the first result; the clean copy's actions stand
  one under another, all the same size, sharing first; “Find in file” reads
  as a link, leaving one button on the card; the location badge shows the
  place in the summary rather than the bytes; the photo is drawn as sharp as
  the screen; and the keys and tour links leave the footer.
- On a computer, the tour rings the part it is about with a rounded, softer
  outline that its text never touches, and its note stands clear of it.
- In Ukrainian: what a clean copy removed, the files found inside a file, “Save
  it”, the kept orientation, the bytes view's colours and how fast a file was
  read are translated.

- On an English page, a full stop after a link or a bold word no longer
  turns into “it is.”.

- After a clean copy, its card says what its number means: “8 removed —
  nothing is left in the copy”, not a bare 8 over “nothing found”.

- What a file reveals adds up to the headline: four named, “and 4 more”.

- In Ukrainian, the clean copy has one name, and its button fits a phone:
  «Зберегти копію без цих даних».

- Visits to hexscope.org are counted: the counter still waited for
  hexscope.pages.dev, which now forwards there.

- The share card and its sentence name hexscope.org.

- The guides' section numbers are readable in the dark theme.

## [0.3.0] — 2026-10-12

### Added

- **Confirmed clean copies for more files.** Word, Excel and PowerPoint, old
  and new, HEIC photos and videos now say *Removed* for each property they
  lose instead of *Not checked*; photos also confirm the lens, the date, the
  software and the thumbnail. The CLI's `clean --verify` exits 0 when every
  finding is confirmed.
- The coverage table in `docs/verification-coverage.md` is written from the
  core's contract, and a test fails when they differ.
- **Links that open a sample where it matters.** `?sample=photo.jpg&view=metadata`
  opens a view, `&part=GPS IFD` selects a part; the guides' samples use them.

### Changed

- **Metadata is a view of its own:** every field the file has, grouped, the
  extra ones open — the answer and the actions stay on the overview.
- **A status bar under the bytes:** the byte under the pointer on the left,
  the part chosen and the file's size on the right.
- A new link preview in the site's look, and a demo at the top of the README
  (`scripts/make-demo.mjs` records it again).

### Fixed

- The note under the short answer no longer sits on the findings below it.

## [0.2.0] — 2026-10-11

### Added

- **Clean copies are checked.** The copy is read back and compared with the
  original: what was removed, what remains, and what could not be checked,
  in the Web, the CLI (`clean --verify`) and batch receipts. Confirmed for
  JPEG, PNG and WebP photo data and for PDF Info and XMP.
- **Film photo lab.** Scans of film: frame borders and perforations spotted,
  negatives converted with a calibrated film base, rolls processed alike,
  16-bit TIFF output, and a JPEG for sharing that is checked like any copy.
- **Photo privacy mosaic.** Across several photos: the same serial number,
  places near each other, dates in order — what a set gives away that one
  photo does not.
- **Zips made on a Mac** say where each file was downloaded from, with which
  app and when, and its Finder tags (the hidden `__MACOSX` notes). The clean
  copy leaves the notes out.
- **More of what a document says.** Word, Excel and PowerPoint, old and new:
  subject, keywords, description, category, status, manager, custom
  properties, and Microsoft sensitivity labels, which name the organisation.
- **Comments in pictures** (JPEG, GIF, PNG) are shown: a program's stamp as
  the software, anything else as a caption.
- **Ukrainian interface**, with every verdict, piece of advice and clean-copy
  note translated; tests fail when one is missing.
- **Visit counter without tracking.** One empty request a day at most, no
  cookies, ids or addresses, nothing about files; Do Not Track and Global
  Privacy Control are honoured. Daily totals only, read only by the site's
  author with a key.
- The version is shown at the foot of the site and in structure reports.

### Changed

- **New look.** Graphite design, an analyzer with its own navigation on
  computers, tablets and phones, grouped metadata, a clearer comparison, and
  contents beside each guide.
- The landing page shows a real analyzer and real Apollo 11 film scans.

### Fixed

- A Mac's screen recording had its camera, software and date shifted by one
  field.
- Drawings were reported as hidden text: white labels on rounded or gradient
  panels, boxes over a drawing program's shadows, see-through boxes, and
  letters of a word cut off by the page's edge.
- A PDF from macOS with references to impossible object numbers was called
  damaged.
- Clean-copy results no longer claim more than was checked.

## [0.1.0] — 2026-10-02

First published version: the website at hexscope.pages.dev, and
`hexscope-cli`, `hexscope-core` and `hexscope-inflate` on crates.io. Reads
photos, videos, PDFs, Office documents, emails, archives and WebAssembly in
the browser; says what each gives away; makes clean, redacted and repaired
copies; and a CLI and GitHub Action for the same checks in CI.

[Unreleased]: https://github.com/MykytaStel/hexscope/compare/v0.5.0...HEAD
[0.5.0]: https://github.com/MykytaStel/hexscope/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/MykytaStel/hexscope/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/MykytaStel/hexscope/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/MykytaStel/hexscope/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/MykytaStel/hexscope/releases/tag/v0.1.0
