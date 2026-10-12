// What search engines measure of a page's first load (Core Web Vitals): the
// landing must not move while its demo starts, and its largest picture must
// be in the page from the start rather than made by a script.
import { expect, test } from "@playwright/test";

test("the landing does not move while the demo starts", async ({ page }) => {
  test.setTimeout(90_000);
  // A slow phone on a slow network, as the search engines' own test is: the
  // demo then starts well after the first paint, where a shift would show.
  const cdp = await page.context().newCDPSession(page);
  await cdp.send("Network.enable");
  await cdp.send("Network.emulateNetworkConditions", { offline: false, latency: 150, downloadThroughput: 200_000, uploadThroughput: 90_000 });
  await cdp.send("Emulation.setCPUThrottlingRate", { rate: 4 });
  await page.addInitScript(() => {
    localStorage.setItem("hexscope.tour", "done");
    const w = window as unknown as { shifted: number };
    w.shifted = 0;
    new PerformanceObserver((list) => {
      for (const entry of list.getEntries() as (PerformanceEntry & { value: number; hadRecentInput: boolean })[]) {
        if (!entry.hadRecentInput) w.shifted += entry.value;
      }
    }).observe({ type: "layout-shift", buffered: true });
  });
  await page.goto("./");
  await expect(page.locator(".demo-facts.is-clean")).toBeVisible({ timeout: 50_000 });
  const shifted = await page.evaluate(() => (window as unknown as { shifted: number }).shifted);
  expect(shifted).toBeLessThan(0.1);
});

test("the demo's photo is in the page before any script runs", async ({ request }) => {
  const html = await (await request.get("./")).text();
  expect(html).toMatch(/<img class="demo-image" src="\.\/samples\/photo\.jpg"[^>]*width="640" height="480"/);
  // The archive frames shown small are small files; the originals stay samples.
  expect(html).not.toMatch(/<img src="\.\/samples\/apollo11\//);
});
