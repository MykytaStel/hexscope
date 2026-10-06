# Hexscope analyzer shell — design QA

**Final result:** passed

## Comparison target and evidence

- **Source visual truth:** `/Users/mykyta/Downloads/hexscope-visual-direction/references/08-bytes-structure-compare-board.png`; app shell requirements are also in `/Users/mykyta/Downloads/hexscope-visual-direction/spec/page-map.md` and `spec/visual-system.md`.
- **Implementation screenshot:** `/tmp/hexscope-analyzer-bytes-clean.png`.
- **Combined comparison input:** `/tmp/hexscope-design-qa-comparison.png`.
- **Source dimensions:** 1536 × 1024 px. The bytes-workspace panel was cropped from `(x=774, y=0)` at 762 × 288 px and normalized to 1280 × 483 px for the comparison.
- **Implementation dimensions:** 1280 × 800 px, matching a 1280 × 800 CSS viewport at device scale factor 1.
- **Comparison input dimensions:** 2560 × 520 px, with the normalized reference and implementation panels side by side.
- **State:** desktop Bytes view, dark theme, Ukrainian UI, `photo.jpg` sample, GPS IFD selected; the same Bytes workspace state is shown in the reference panel.

## Visual review

- **Full-view comparison:** the combined input compares the reference's complete analyzer panel with the implementation's complete visible workspace. Both use the same left navigation → file structure → byte grid → inspector hierarchy. The implementation retains Hexscope's working header and byte toolbar.
- **Focused-region comparison:** reviewed the navigation and selected state, the structure tree, the hex/ASCII canvas, and the inspector fields. The byte canvas visibly renders real file bytes; the selected GPS node maps to its inspected offset and length.
- **Typography:** neutral sans-serif labels and monospaced technical values preserve the reference hierarchy. Ukrainian navigation labels fit at the tested desktop width.
- **Spacing and layout:** the four-column workspace remains within the viewport at the breakpoint sweep. Tree, bytes, and inspector have clear separators and usable widths.
- **Colors and tokens:** graphite surfaces, restrained borders, and existing semantic status colors follow the supplied palette. No blue glow or decorative color tiles were introduced.
- **Image quality and assets:** the analyzer shell uses the existing Hexscope logo and the actual byte renderer; no mock byte content or replacement imagery is used.
- **Copy and content:** desktop navigation is localized. Metadata and Content lead to real findings; Compression is unavailable for a JPEG without a playable stream; Compare uses the existing file comparison flow.
- **States, accessibility, and responsiveness:** navigation exposes the current page and disabled actions semantically. Keyboard focus and theme checks remain covered by the existing accessibility suite. On phones the existing compact Summary/Bytes switch remains usable; the page has no horizontal overflow at tested widths.

## Findings

No actionable P0, P1, or P2 differences remain for this analyzer-shell slice.

- **[P3] Add outline icons to the desktop navigation.** The reference uses small outline icons beside the labels; this implementation uses labels and a clear selected state. A consistent icon family would improve at-a-glance scanning. Text labels keep every action clear meanwhile.
- **[P3] Consider a full-width status footer in a later shell pass.** The reference page map places offset, selection, encoding, and file size in a bottom status row. The current app keeps its live byte status in the existing toolbar and file details in the header, so this is a layout refinement rather than missing file information.
- The mobile analyzer intentionally keeps its existing compact Summary/Bytes switch in this slice; the desktop section navigation is hidden below 901 CSS px.

## Comparison history

- An early screenshot was taken before the byte canvas completed its first paint and appeared black. After waiting for the canvas render, the captured grid displayed the file's hex and ASCII data. A browser assertion now checks that the visible canvas has rendered pixels; no product-code correction was needed.
- The full browser run exposed four existing desktop tests that still clicked the old Summary/Bytes switch. They were updated to use the new desktop navigation while preserving the phone switch behavior; the affected responsive and byte-inspector cases then passed.

## Verification

- `pnpm --filter web build` — passed. Vite reported its existing warning for classic `theme.js` scripts in HTML pages.
- `pnpm --filter web test` — 120 tests passed.
- `pnpm --filter web e2e` — 186 passed, 18 skipped.
- The full browser suite includes the desktop Compression navigation toggle; its phone variant is skipped as intended.
- Targeted navigation checks cover Ukrainian labels, Overview/Metadata/Content/Structure/Bytes, real compare selection, rendered byte pixels, and the phone Summary/Bytes control without horizontal overflow.

## Implementation checklist

- [x] Use a separate, localized analyzer navigation on desktop.
- [x] Connect each available navigation item to existing analyzer behavior.
- [x] Keep structure and bytes as distinct workspace states.
- [x] Keep the compact phone switch and verify responsive bounds.
- [x] Compare a rendered Bytes view against reference board 08.
- [x] Run production build, unit tests, and browser tests.

**Follow-up polish:** desktop navigation icons and the optional bottom status row.

# Hexscope Compare — design QA

**Final result:** passed

## Comparison target and evidence

- **Source visual truth:** `/Users/mykyta/Downloads/hexscope-visual-direction/references/08-bytes-structure-compare-board.png` and the Compare entry in `spec/page-map.md`.
- **Implementation screenshots:** `/tmp/hexscope-clean-compare-desktop.png` and `/tmp/hexscope-clean-compare-phone.png`.
- **Combined comparison input:** `/tmp/hexscope-compare-design-qa.png`.
- **Source dimensions:** 1536 × 1024 px. The Compare panel was cropped from `(774, 565)` to `(1520, 800)` and normalized to 980 × 309 px.
- **Desktop implementation:** 1280 × 800 CSS px at device scale factor 1. The dialog occupies 980 × 768 px and scrolls internally for lower sections.
- **Phone implementation:** 393 × 852 CSS px at device scale factor 1. File cards stack vertically; the dialog scrolls without horizontal page overflow.
- **Combined input dimensions:** 1978 × 447 px, with the normalized source and a focused implementation dialog crop side by side.
- **State:** dark theme, Ukrainian UI, original sample photo compared with its generated clean copy. The same image appears on both sides; metadata fields and findings decrease from 8 to 0.

## Visual review

- **Comparison hierarchy:** the two files are paired first, with previews, names, formats, byte sizes, and metadata-field counts. Three finding metrics make the change legible before optional detail.
- **Reference fit:** the implementation keeps the reference's paired-file concept and real photo evidence while adapting its original/clean labels to a general-purpose Compare action that accepts any two files. A/B labels avoid implying provenance the user did not select.
- **Typography and spacing:** file names and metric values lead; subdued labels and monospaced technical values support them. The dialog keeps the existing graphite surfaces, restrained borders, and rounded cards.
- **Images:** decodable supported images render from the selected local file. Preview work is bounded to 16 MiB and 40 million pixels; unsupported, missing, or undecodable images simply omit the preview.
- **Progressive detail:** finding categories, structure lists, and raw byte differences are disclosed on demand. The raw byte view remains available at the first difference.
- **Copy and localization:** the title, A/B labels, finding categories, metadata count, issues, structure metrics, and byte-difference labels have Ukrainian translations. File names and technical format names remain intact.
- **Accessibility and interaction:** the dialog is named by its heading; preview canvases have image labels; close has a localized accessible name. Browser checks open the category details and raw byte view, verify the close control, and close the dialog with Escape.
- **Responsive behavior:** desktop shows both file cards in one row. At phone widths they stack, metric cards wrap to two columns, and the dialog remains within the viewport.

## Findings

No actionable P0, P1, or P2 visual or interaction findings remain for this Compare slice.

## Verification

- `pnpm --filter web build` — passed. Existing Vite warnings remain for classic `theme.js` scripts in HTML pages.
- `pnpm --filter web test` — 122 tests passed.
- `pnpm --filter web e2e` — 196 passed, 18 skipped.
- `git diff --check` — passed.

## Implementation checklist

- [x] Present both files as a readable pair with actual image previews where safe.
- [x] Summarize findings, metadata-field counts, file issues, and structure changes.
- [x] Keep the raw byte diff available in a collapsed detail section.
- [x] Localize Compare UI and check desktop and phone layouts.
- [x] Compare the built dialog against reference board 08.
- [x] Finish the full browser run and final diff check.

# Hexscope guide navigation — design QA

**Final result:** passed

## Comparison target and evidence

- **Source visual truth:** `/Users/mykyta/Downloads/hexscope-visual-direction/references/05-metadata-guides-mobile-board.png`.
- **Implementation screenshots:** `/tmp/hexscope-guide-desktop-viewport.png`, `/tmp/hexscope-guide-phone-viewport.png`; full article captures are `/tmp/hexscope-guide-desktop.png` and `/tmp/hexscope-guide-phone.png`.
- **Combined comparison input:** `/tmp/hexscope-guide-design-qa.png`.
- **Source dimensions:** 1536 × 1024 px. The guide/article panel was cropped from `(825, 535)` to `(1510, 842)`, producing 685 × 307 px, then normalized to 1000 × 448 px.
- **Desktop implementation:** 1280 × 800 CSS px at device scale factor 1. The focused implementation crop is 1004 × 448 px, normalized to 1000 × 448 px. The article column measures 700 px; the contents rail measures 200 px with a 64 px gap.
- **Phone implementation:** 393 × 852 CSS px at device scale factor 1. The contents rail is hidden and the article uses the full single column. The document width remains 393 px.
- **Combined input dimensions:** 2020 × 480 px, with reference and implementation panels side by side.
- **State:** dark theme, Ukrainian UI, guide article route `remove-location-from-photo.html`, desktop contents visible; mobile article captured at its initial position.

The source board shows a different article, “Understanding photo metadata,” with an embedded sample preview. The implementation uses the existing location guide and its real content. This comparison evaluates the shared editorial layout and contents rail, not identical copy or article imagery; no missing sample artwork was fabricated.

## Visual review

- **Information hierarchy:** a readable 700 px article column sits beside a separate contents rail. Its six links map to the article's actual sections and jump to stable anchors; the current section has a clear marker.
- **Typography:** the existing neutral sans-serif system preserves the reference's heading/body distinction. Section names wrap inside the rail without colliding with the article; body copy retains comfortable line length and spacing.
- **Spacing and layout:** at 1280 px, the centered article and sticky 200 px rail have a measured 64 px gap. The top bar remains separate from the reading column, and anchor targets clear it when jumped to.
- **Colors and tokens:** graphite background, off-white headings, subdued secondary text, and low-contrast separators continue the supplied visual system. The rail uses existing border/text tokens.
- **Images and copy:** the reference's sample-preview card belongs to its metadata example. The location guide keeps its own copy, sample links, and existing article structure.
- **Localization and interaction:** the rail heading and labels follow the selected EN/UA locale. Each link moves to its corresponding section; focus can reach the target heading. The active link follows scrolling and uses `aria-current="location"`. Browser tests cover section changes, a real anchor jump, and both language states.
- **Responsive behavior:** the desktop rail is removed below 901 px rather than squeezed beside the copy. At 393 × 852 px the article remains one column without horizontal overflow.
- **Accessibility:** the contents navigation is named by its heading; links use real fragment targets and keyboard focus; target headings receive a visible focus outline.

## Findings

No actionable P0, P1, P2, or P3 findings remain for this guide-navigation slice. The contents rail marks the section at the current reading position and exposes it with `aria-current="location"`.

## Verification

- `pnpm --filter web build` — passed. Vite retains its existing warnings for classic `theme.js` scripts in HTML pages.
- `pnpm --filter web test` — 122 tests passed.
- `pnpm --filter web e2e --grep 'guide articles have a translated desktop contents list'` — 2 passed across desktop and phone.
- `pnpm --filter web e2e` — 198 passed, 18 skipped.
- `git diff --check` — passed.

## Implementation checklist

- [x] Generate a contents link for every top-level article section.
- [x] Keep anchors stable and focusable, with scroll offset for the sticky header.
- [x] Translate the rail and regenerate its labels when the page language changes.
- [x] Keep the mobile guide layout single-column without horizontal overflow.
- [x] Compare rendered article and contents layout against board 05.
- [x] Mark the current reading section in the contents rail.
- [x] Finish the full browser run and final diff check.
