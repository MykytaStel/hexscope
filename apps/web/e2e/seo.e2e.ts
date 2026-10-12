// What search engines read: every page in English and in Ukrainian, linked to
// each other, described as structured data, and listed in the sitemap.
import { expect, test } from "@playwright/test";

test("the sitemap lists both languages and robots.txt points to it", async ({ request }) => {
  const sitemap = await (await request.get("./sitemap.xml")).text();
  expect(sitemap).toContain("<loc>https://hexscope.org/remove-location-from-photo</loc>");
  expect(sitemap).toContain("<loc>https://hexscope.org/uk/remove-location-from-photo</loc>");
  expect(sitemap).toContain('hreflang="uk"');
  expect(await (await request.get("./robots.txt")).text()).toContain("Sitemap: https://hexscope.org/sitemap.xml");
});

test("a guide has a Ukrainian page of its own, linked both ways", async ({ page }) => {
  await page.goto("./uk/remove-location-from-photo");
  await expect(page.locator("html")).toHaveAttribute("lang", "uk");
  await expect(page.locator("h1")).toHaveText("Як прибрати геолокацію з фото");
  await expect(page.locator('link[rel="canonical"]')).toHaveAttribute("href", "https://hexscope.org/uk/remove-location-from-photo");
  await expect(page.locator('link[hreflang="en"]')).toHaveAttribute("href", "https://hexscope.org/remove-location-from-photo");
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

test("the home page names its own address", async ({ page }) => {
  await page.goto("./");
  await expect(page.locator('link[rel="canonical"]')).toHaveAttribute("href", "https://hexscope.org/");
});

test("guides link to pages by the address the host serves, not name.html", async ({ page }) => {
  for (const path of ["./remove-location-from-photo", "./uk/remove-location-from-photo"]) {
    await page.goto(path);
    await expect(page.locator(".guide-more a").first()).toBeVisible();
    expect(await page.locator('a[href$=".html"]').count()).toBe(0);
  }
});

test("the page for a wrong address works at any depth, in each language, and is never indexed", async ({ page, request }) => {
  await page.goto("./404");
  await expect(page.locator("h1")).toHaveText("This page is not here");
  await expect(page.locator('meta[name="robots"]')).toHaveAttribute("content", "noindex");
  await expect(page.locator(".guide-more a").first()).toHaveAttribute("href", "/remove-location-from-photo");
  for (const src of await page.locator("script[src], link[href]").evaluateAll((els) => els.map((e) => e.getAttribute("src") ?? e.getAttribute("href")))) {
    expect(src).toMatch(/^(\/|https:)/);
  }
  await page.goto("./uk/404");
  await expect(page.locator("h1")).toHaveText("Такої сторінки немає");
  await expect(page.locator(".brand")).toHaveAttribute("href", "/uk/");
  await expect(page.locator(".guide-more a").first()).toHaveAttribute("href", "/uk/remove-location-from-photo");
  expect(await (await request.get("./sitemap.xml")).text()).not.toContain("404");
});
