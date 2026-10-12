// The parser is built per family of files: a file loads the build that
// reads it and none of the others.
import { expect, test, type Page } from "@playwright/test";

async function wasmLoadedFor(page: Page, sample: string): Promise<string[]> {
  const loaded: string[] = [];
  page.on("request", (r) => {
    const name = /\/(hexscope_[a-z]+)_bg-[^/]+\.wasm$/.exec(r.url())?.[1];
    if (name) loaded.push(name);
  });
  await page.addInitScript(() => localStorage.setItem("hexscope.tour", "done"));
  await page.goto(`./?sample=${sample}`);
  await expect(page.locator(".verdict-title")).toBeVisible();
  return [...new Set(loaded)].sort();
}

for (const [sample, build] of [
  ["photo.jpg", "hexscope_media"],
  ["report.pdf", "hexscope_pdf"],
  ["report.docx", "hexscope_docs"],
  ["message.eml", "hexscope_docs"],
] as const) {
  test(`${sample} loads ${build} alone`, async ({ page }) => {
    expect(await wasmLoadedFor(page, sample)).toEqual([build]);
  });
}
