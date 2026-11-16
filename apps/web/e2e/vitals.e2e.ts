// What search engines measure of a page's first load (Core Web Vitals): the
// compact sample story must not shift as the landing page starts.
import { expect, test } from "@playwright/test";

test("the compact landing sample does not cause layout shift", async ({ page }) => {
  test.setTimeout(90_000);
  // A slow phone on a slow network catches delayed page content and fonts.
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
  await expect(page.locator("body")).toHaveAttribute("data-state", "empty", { timeout: 50_000 });
  await expect(page.locator("#examples [data-example-panel=photo] .example-facts")).toBeVisible();
  await page.waitForTimeout(1_000);
  const shifted = await page.evaluate(() => (window as unknown as { shifted: number }).shifted);
  expect(shifted).toBeLessThan(0.1);
});

test("the compact sample story is in the page before any script runs", async ({ request }) => {
  const html = await (await request.get("./")).text();
  expect(html).toMatch(/class="[^"]*\bexample-photo-art\b/);
  expect(html).toContain('class="example-facts"');
  expect(html).not.toContain('class="demo-image"');
  // Archive frames remain lazy loaded inside the collapsed specialist section.
  expect(html).not.toMatch(/<img src="\.\/samples\/apollo11\//);
});
