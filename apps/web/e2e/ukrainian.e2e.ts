// Every sample, opened by a reader of Ukrainian: no English sentence or word
// of hexscope's own reaches the page — only what the file itself says.
import { expect, test } from "@playwright/test";
import { openView } from "./views";

const SAMPLES = [
  "photo.jpg", "photo.heic", "holiday.jpg", "cropped.jpg", "progressive.jpg", "sample.png", "ai-picture.png",
  "wifi.png", "broken.png", "film-negative.png", "video.mov", "report.pdf", "redacted.pdf", "report.docx",
  "budget.xlsx", "lease.odt", "plan.doc", "message.eml", "phishing.eml", "phishing.msg", "hello.wasm",
];

/** Words hexscope writes in English that would give a page away as untranslated. */
const ENGLISH = new Set([
  "the", "and", "of", "to", "is", "it", "its", "in", "from", "with", "not", "for", "by", "was", "this", "that",
  "goes", "says", "has", "are", "be", "more", "thing", "things", "other", "another", "than", "what", "when",
  "where", "who", "nothing", "found", "file", "files", "photo", "picture", "email", "document", "archive",
  "page", "pages", "parsing", "reading", "removed", "hidden", "opens", "runs", "sent", "passed", "through",
  "replying", "link", "attached", "checked", "looked", "visited", "read", "own", "headers", "vouched", "sender",
]);

/** What a file holds is shown as it is; these parts carry its values, not hexscope's words. */
const VALUES = [
  "code", "kbd", "samp", "pre", "input", "textarea", "a[href^='http']", ".value", ".mono", ".hex", ".filename",
  ".fact-value", ".fact-val", "dd", ".ep-main", ".ep-sub", ".tree", ".hexview", "[data-value]", "[translate='no']",
  // A page of a PDF drawn as it is, and the path to a part by the names the format gives its parts.
  "svg", ".crumbs",
];

async function leftovers(page: import("@playwright/test").Page): Promise<string[]> {
  return page.evaluate(
    ({ english, values }) => {
      const found = new Set<string>();
      // The whole page as it is now, shown or folded away: the landing's own words are checked elsewhere.
      const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
      let node: Node | null;
      while ((node = walker.nextNode())) {
        const parent = node.parentElement;
        const text = node.textContent?.replace(/\s+/g, " ").trim() ?? "";
        if (!parent || !text || (/[А-Яа-яІіЇїЄєҐґ]/.test(text) && !/[a-z]{3,} [a-z]{3,}/.test(text))) continue;
        if (parent.closest(values.join(",")) || parent.closest(".empty, footer, script, style")) continue;
        // A file's own name, with its size, is the file's.
        const words = text.replace(/^[\w().-]+\.\w{2,5}(?= · |$)/, "").toLowerCase().match(/[a-z]+/g) ?? [];
        if (words.some((w) => english.includes(w))) found.add(text);
      }
      return [...found];
    },
    { english: [...ENGLISH], values: VALUES },
  );
}

const FIXTURES = [
  "android.mp4", "budget.ods", "chrome-highlight.pdf", "compact.pdf", "cwebp-photo.webp", "encrypted-aes-256.pdf",
  "ffmpeg.gif", "gif2webp-anim.webp", "iphone-hdr.jpg", "iphone.mov", "keynote.pptx", "lease.rtf", "loci.mp4",
  "mac-report.zip", "notes.png", "phishing-rtf.msg", "photo-grid.heic", "photo.avif", "photo.tif", "tall-interlaced.png",
  "tracked.docx", "voice.m4a", "windows.docx", "windows.xlsx",
].map((name) => `../../crates/hexscope-core/tests/fixtures/${name}`);

async function inUkrainian(page: import("@playwright/test").Page): Promise<void> {
  await page.addInitScript(() => {
    if (!localStorage.getItem("hexscope.language")) localStorage.setItem("hexscope.language", "uk");
    localStorage.setItem("hexscope.tour", "done");
  });
}

/** Each view of the open file, read through; what is left in English, by view. */
async function readViews(page: import("@playwright/test").Page): Promise<string[]> {
  await page.locator("body[data-state=ready]").waitFor();
  // The Ukrainian dictionary and the field explanations load beside the analyzer.
  await page.waitForFunction(() => document.documentElement.lang === "uk");
  await page.waitForTimeout(400);
  const seen = new Set<string>();
  for (const view of ["overview", "metadata", "content", "structure", "bytes"] as const) {
    const button = page.locator(`#app-nav [data-app-view='${view}']`);
    if (!(await button.count()) || (await button.isDisabled())) continue;
    await openView(page, view);
    await page.waitForTimeout(250);
    for (const line of await leftovers(page)) seen.add(line);
  }
  return [...seen];
}

test("every sample reads in Ukrainian, whichever view is open", async ({ page }) => {
  test.setTimeout(240_000);
  await inUkrainian(page);
  const report: Record<string, string[]> = {};
  for (const sample of SAMPLES) {
    await page.goto(`./?sample=${sample}`);
    const lines = await readViews(page);
    if (lines.length) report[sample] = lines;
  }
  expect(report).toEqual({});
});

test("every kind of file the tests hold reads in Ukrainian too", async ({ page }) => {
  test.setTimeout(240_000);
  await inUkrainian(page);
  const report: Record<string, string[]> = {};
  for (const fixture of FIXTURES) {
    await page.goto("./");
    await page.locator("#picker-empty").setInputFiles(fixture);
    const lines = await readViews(page);
    if (lines.length) report[fixture.split("/").pop()!] = lines;
  }
  expect(report).toEqual({});
});

test("what a reader does with a file reads in Ukrainian too: a clean copy, a comparison, the keys, several files", async ({ page }) => {
  test.setTimeout(180_000);
  await inUkrainian(page);
  const report: Record<string, string[]> = {};
  const note = async (step: string) => {
    const lines = await leftovers(page);
    if (lines.length) report[step] = lines;
  };
  for (const sample of ["photo.jpg", "report.pdf", "report.docx", "broken.png"]) {
    await page.goto(`./?sample=${sample}`);
    await page.locator("body[data-state=ready]").waitFor();
    await page.waitForFunction(() => document.documentElement.lang === "uk");
    const download = page.waitForEvent("download").catch(() => null);
    const cta = page.locator(".verdict-cta, .reveals .btn-clean").first();
    if (await cta.count()) {
      await cta.click();
      await download;
      await page.waitForTimeout(1500);
      await note(`${sample}: clean copy`);
    }
    const more = page.locator("#app-nav .app-nav-more summary");
    if (await more.isVisible()) await more.click();
    await page.locator("#app-nav [data-app-action=compare]").click();
    await page.locator("#compare-picker").setInputFiles(`public/samples/${sample === "photo.jpg" ? "cropped.jpg" : sample}`);
    await page.waitForTimeout(1500);
    await note(`${sample}: compare`);
  }
  await page.keyboard.press("Escape");
  await page.keyboard.press("?");
  await page.waitForTimeout(300);
  await note("shortcuts");
  await page.keyboard.press("Escape");
  await page.goto("./");
  await page.locator("#picker-empty").setInputFiles(["photo.jpg", "report.pdf", "phishing.eml", "sample.png"].map((f) => `public/samples/${f}`));
  await page.waitForTimeout(2500);
  await note("several files");
  expect(report).toEqual({});
});
