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
  const [article, crumbs] = data["@graph"];
  expect(article).toMatchObject({ "@type": "Article", inLanguage: "uk", headline: "Як прибрати геолокацію з фото" });
  // Dated by the page's own commits, not by the day of the build.
  expect(article.datePublished).toMatch(/^\d{4}-\d{2}-\d{2}$/);
  expect(article.dateModified >= article.datePublished).toBe(true);
  expect(crumbs).toMatchObject({
    "@type": "BreadcrumbList",
    itemListElement: [
      { position: 1, item: "https://hexscope.org/uk/" },
      { position: 2, name: "Як прибрати геолокацію з фото", item: "https://hexscope.org/uk/remove-location-from-photo" },
    ],
  });
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
  await expect(page.locator(".guide-more a").first()).toHaveAttribute("href", "/exif-viewer");
  for (const src of await page.locator("script[src], link[href]").evaluateAll((els) => els.map((e) => e.getAttribute("src") ?? e.getAttribute("href")))) {
    expect(src).toMatch(/^(\/|https:)/);
  }
  await page.goto("./uk/404");
  await expect(page.locator("h1")).toHaveText("Такої сторінки немає");
  await expect(page.locator(".brand")).toHaveAttribute("href", "/uk/");
  await expect(page.locator(".guide-more a").first()).toHaveAttribute("href", "/uk/exif-viewer");
  expect(await (await request.get("./sitemap.xml")).text()).not.toContain("404");
});

test("the pages people search for are written in both languages and listed in the sitemap", async ({ page, request }) => {
  const sitemap = await (await request.get("./sitemap.xml")).text();
  for (const [path, en, uk] of [
    ["exif-viewer", "EXIF viewer: what a photo's metadata says", "Перегляд EXIF: що розповідають метадані фото"],
    ["ai-image-metadata", "Was this picture made by AI? What its file says", "Чи створив цю картинку ШІ? Що каже сам файл"],
    ["remove-metadata-from-pdf", "How to remove metadata from a PDF", "Як видалити метадані з PDF"],
    ["remove-metadata-from-word", "How to remove metadata from a Word document", "Як видалити метадані з документа Word"],
    ["remove-metadata-from-photo", "How to remove EXIF and metadata from a photo", "Як видалити EXIF і метадані з фото"],
    ["remove-metadata-from-video", "How to remove metadata from a video", "Як видалити метадані з відео"],
    ["remove-metadata-from-libreoffice", "How to remove metadata from LibreOffice and RTF files", "Як видалити метадані з файлів LibreOffice і RTF"],
    ["what-file-type-is-this", "What type of file is this?", "Що це за файл?"],
    ["metadata-removers-compared", "ExifTool, MAT2 or hexscope: what each leaves in a file", "ExifTool, MAT2 чи hexscope: що кожен лишає у файлі"],
    ["photos-from-sensitive-places", "A photo from a sensitive place: what gives it away", "Фото з чутливого місця: що його видає"],
  ]) {
    expect(sitemap).toContain(`<loc>https://hexscope.org/${path}</loc>`);
    expect(sitemap).toContain(`<loc>https://hexscope.org/uk/${path}</loc>`);
    await page.goto(`./${path}`);
    await expect(page.locator("h1")).toHaveText(en);
    await page.goto(`./uk/${path}`);
    await expect(page.locator("h1")).toHaveText(uk);
  }
});

test("the home page says what it is in the words people search with", async ({ page }) => {
  await page.goto("./");
  await expect(page).toHaveTitle(/metadata/i);
  await expect(page.locator('meta[name="description"]')).toHaveAttribute("content", /EXIF/);
});

test("an AI picture says what made it and the prompt, and its clean copy is confirmed without them", async ({ page }) => {
  await page.addInitScript(() => localStorage.setItem("hexscope.language", "en"));
  await page.goto("./?sample=ai-picture.png");
  await expect(page.locator(".verdict-title")).toHaveText("This picture gives away 2 things");
  await expect(page.getByText("Reveals that AI made it and the prompt it was made from.").first()).toBeVisible();
  await page.goto("./?sample=ai-picture.png&view=metadata");
  await expect(page.locator(":visible", { hasText: /made with Stable Diffusion, model sdxl_base_1\.0/ }).last()).toBeVisible();
  await expect(page.locator(":visible", { hasText: /a quiet sunset over a calm sea/ }).last()).toBeVisible();
});

test("every guide links to every other one", async ({ page }) => {
  await page.goto("./what-file-type-is-this");
  const links = await page.locator(".guide-more a").evaluateAll((as) => as.map((a) => a.getAttribute("href")));
  expect(links).not.toContain("what-file-type-is-this");
  expect(links).toEqual(expect.arrayContaining(["remove-metadata-from-video", "remove-metadata-from-libreoffice", "exif-viewer", "png-wont-open"]));
});

test("the samples a guide offers open: a photo with the wrong name, and a LibreOffice document", async ({ page }) => {
  await page.addInitScript(() => {
    localStorage.setItem("hexscope.language", "en");
    localStorage.setItem("hexscope.tour", "done");
  });
  await page.goto("./?sample=holiday.jpg");
  await expect(page.locator(".verdict")).toContainText("Named .jpg, but it is a HEIC image.");
  await page.goto("./?sample=lease.odt&view=metadata");
  await expect(page.locator(":visible", { hasText: /HP LaserJet Pro M404/ }).last()).toBeVisible();
});

test("an English reader downloads no Ukrainian, and a Ukrainian page needs no articles' copy", async ({ page }) => {
  const dictionaries: string[] = [];
  page.on("request", (r) => {
    const name = new URL(r.url()).pathname.split("/").pop() ?? "";
    if (/i18n/.test(name)) dictionaries.push(name.replace(/-[\w-]{8}\.js$/, ""));
  });
  await page.addInitScript(() => {
    if (!sessionStorage.getItem("chosen")) localStorage.setItem("hexscope.language", "en");
  });
  await page.goto("./remove-location-from-photo");
  await expect(page.locator(".guide-toc h2, .story-text h1").first()).toBeVisible();
  await page.goto("./?sample=photo.jpg");
  await expect(page.locator(".verdict-title")).toHaveText("This photo gives away 8 things");
  expect(dictionaries).toEqual([]);

  // A /uk/ page read in Ukrainian is Ukrainian already: only the words the page adds come.
  await page.evaluate(() => {
    sessionStorage.setItem("chosen", "1");
    localStorage.setItem("hexscope.language", "uk");
  });
  await page.goto("./uk/remove-location-from-photo");
  await expect(page.locator(".guide-toc h2")).toHaveText("На цій сторінці");
  expect(dictionaries).toEqual(["i18n-uk"]);

  // Turned to English, it opens the English page: nothing is translated, nothing more comes.
  await page.locator(".topbar .language-button").click();
  await expect(page).toHaveURL(/\/remove-location-from-photo$/);
  await expect(page.locator("h1")).toHaveText("How to remove the location from a photo");
  expect(dictionaries).toEqual(["i18n-uk"]);
});

test("an article opens in the reader's language before it is drawn, keeping where it was", async ({ page }) => {
  await page.addInitScript(() => localStorage.setItem("hexscope.language", "uk"));
  await page.goto("./exif-viewer#guide-section-2");
  await expect(page).toHaveURL(/\/uk\/exif-viewer#guide-section-2$/);
  await expect(page.locator("h1")).toHaveText("Перегляд EXIF: що розповідають метадані фото");
});
