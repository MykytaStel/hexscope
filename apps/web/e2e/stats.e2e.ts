// The visit counter's open numbers, drawn from a stand-in for /api/stats, and
// the promise behind it: nothing is counted from anywhere but the real site.
import { expect, test, type Page } from "@playwright/test";

const day = (offset: number) => new Date(Date.UTC(2026, 9, 10 - offset)).toISOString().slice(0, 10);
const FIGURES = {
  updated: "2026-10-10T12:00:00.000Z",
  today: { day: day(0), people: 12, new: 9 },
  month: { month: "2026-10", people: 140 },
  days: Array.from({ length: 30 }, (_, i) => ({ day: day(29 - i), people: i === 29 ? 12 : 20 + (i % 7), new: 9 })),
  months: Array.from({ length: 12 }, (_, i) => ({ month: new Date(Date.UTC(2025, 10 + i, 1)).toISOString().slice(0, 7), people: i === 11 ? 140 : 0 })),
  top: {
    country: [["UA", 80], ["PL", 20], ["unknown", 2]],
    from: [["reddit.com", 30]],
    page: [["/", 90], ["/remove-location-from-photo", 12]],
    device: [["phone", 60], ["computer", 40]],
    lang: [["uk", 70], ["en", 30]],
  },
};

async function stats(page: Page, answer: object | null): Promise<void> {
  await page.addInitScript(() => localStorage.setItem("hexscope.language", "en"));
  await page.route("**/api/stats", (route) =>
    answer ? route.fulfill({ json: answer }) : route.fulfill({ status: 503, json: { error: "unavailable" } }),
  );
  await page.goto("./stats");
}

test("shows every number the counter keeps", async ({ page }) => {
  await stats(page, FIGURES);
  const tiles = page.locator(".stats-tile");
  await expect(tiles.nth(0)).toContainText("Today");
  await expect(tiles.nth(0)).toContainText("12");
  await expect(tiles.nth(0)).toContainText("9 new");
  await expect(tiles.nth(1)).toContainText("140");
  await expect(page.locator(".stats-chart").first().locator(".stats-slot")).toHaveCount(30);
  await expect(page.locator(".stats-ranked").first()).toContainText("Ukraine");
  await expect(page.locator(".stats-ranked").first()).toContainText("Unknown");

  const table = page.locator(".stats-table").first();
  await table.locator("summary").click();
  await expect(table.locator("tr")).toHaveCount(30);
  await expect(table.locator("tr").last()).toContainText("12 people, 9 new");
});

test("says so when the numbers cannot be read", async ({ page }) => {
  await stats(page, null);
  await expect(page.locator("#stats")).toHaveText("The numbers are not available right now.");
});

test("sends no visit from anywhere but the real site", async ({ page }) => {
  const seen: string[] = [];
  page.on("request", (r) => {
    if (r.url().includes("/api/seen")) seen.push(r.url());
  });
  await page.goto("./");
  await page.waitForFunction(() => document.body.dataset.state === "empty");
  await page.goto("./your-files-stay-private");
  await page.waitForTimeout(2500);
  expect(seen).toEqual([]);
});
