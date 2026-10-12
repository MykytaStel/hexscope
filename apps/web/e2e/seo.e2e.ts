// What search engines read: every page in English and in Ukrainian, linked to
// each other, described as structured data, and listed in the sitemap.
import { expect, test } from "@playwright/test";

test("the sitemap lists both languages and robots.txt points to it", async ({ request }) => {
  const sitemap = await (await request.get("./sitemap.xml")).text();
  expect(sitemap).toContain("<loc>https://hexscope.pages.dev/remove-location-from-photo</loc>");
  expect(sitemap).toContain("<loc>https://hexscope.pages.dev/uk/remove-location-from-photo</loc>");
  expect(sitemap).toContain('hreflang="uk"');
  expect(await (await request.get("./robots.txt")).text()).toContain("Sitemap: https://hexscope.pages.dev/sitemap.xml");
});

test("a guide has a Ukrainian page of its own, linked both ways", async ({ page }) => {
  await page.goto("./uk/remove-location-from-photo");
  await expect(page.locator("html")).toHaveAttribute("lang", "uk");
  await expect(page.locator("h1")).toHaveText("Як прибрати геолокацію з фото");
  await expect(page.locator('link[rel="canonical"]')).toHaveAttribute("href", "https://hexscope.pages.dev/uk/remove-location-from-photo");
  await expect(page.locator('link[hreflang="en"]')).toHaveAttribute("href", "https://hexscope.pages.dev/remove-location-from-photo");
  const data = JSON.parse((await page.locator('script[type="application/ld+json"]').textContent()) ?? "{}");
  expect(data).toMatchObject({ "@type": "Article", inLanguage: "uk", headline: "Як прибрати геолокацію з фото" });
});

test("the app is described as a free web application, in each language", async ({ page }) => {
  for (const [path, lang] of [["./", "en"], ["./uk/", "uk"]] as const) {
    await page.goto(path);
    const data = JSON.parse((await page.locator('script[type="application/ld+json"]').textContent()) ?? "{}");
    expect(data).toMatchObject({ "@type": "WebApplication", inLanguage: lang, isAccessibleForFree: true });
  }
});

test("a sample opens from the Ukrainian app", async ({ page }) => {
  await page.goto("./uk/?sample=photo.jpg");
  await expect(page.locator(".verdict-title")).toHaveText("Фото розкриває 8 фактів");
});
