// A view of the open file chosen as a reader does: on a phone the structure
// and the bytes are under More, so More opens first.
import type { Page } from "@playwright/test";

export async function openView(page: Page, view: "overview" | "metadata" | "content" | "files" | "structure" | "bytes"): Promise<void> {
  const button = page.locator(`#app-nav [data-app-view='${view}']`);
  await page.locator("#app-nav").waitFor();
  if (await page.locator(`#app-nav .app-nav-more-actions [data-app-view='${view}']`).count()) {
    const more = page.locator("#app-nav .app-nav-more");
    if (!(await more.evaluate((d) => (d as HTMLDetailsElement).open))) await more.locator("summary").click();
  }
  await button.click();
}
