import { describe, expect, it } from "vitest";
import { resolveLocale, translateText } from "./i18n";

describe("the interface language", () => {
  it("keeps an explicit language choice ahead of the browser language", () => {
    expect(resolveLocale("uk", ["en-US"])).toBe("uk");
    expect(resolveLocale("en", ["uk-UA"])).toBe("en");
  });

  it("starts in Ukrainian for Ukrainian browsers and English otherwise", () => {
    expect(resolveLocale(null, ["uk-UA", "en-US"])).toBe("uk");
    expect(resolveLocale(null, ["en-US"])).toBe("en");
  });

  it("translates the primary file action and a counted photo verdict", () => {
    expect(translateText("Choose a file", "uk")).toBe("Вибрати файл");
    expect(translateText("This photo gives away 8 things", "uk")).toBe("Фото розкриває 8 фактів");
    expect(translateText("Reveals where it was taken, the camera and the camera's serial number.", "uk")).toBe(
      "Розкриває місце зйомки, фотоапарат, серійний номер фотоапарата.",
    );
    expect(translateText("Reveals a link that says one site and goes to another.", "uk")).toBe(
      "Розкриває посилання, що веде не туди, куди обіцяє.",
    );
    expect(translateText("Save a clean copy", "uk")).toBe("Зберегти очищену копію");
    expect(translateText(" The file never leaves this tab: no account, no tracking. ", "uk")).toBe(
      " Файл не залишає цю вкладку: без облікового запису й стеження. ",
    );
    expect(translateText("All 10 scans: every detail. hexscope shows which bytes draw which block, in each scan.", "uk")).toContain(
      "Усі скани (10)",
    );
    expect(translateText("photo.jpg is open. Reveals where it was taken.", "uk")).toBe("photo.jpg відкрито. Розкриває місце зйомки.");
  });

  it("translates both clean-copy captions in the photo demonstration", () => {
    expect(translateText("Same photo. Camera details removed in your browser.", "uk")).toBe(
      "Те саме фото. Дані камери видалено у вашому браузері.",
    );
    expect(translateText("Same photo. 1 bytes removed in your browser.", "uk")).toBe(
      "Те саме фото. У вашому браузері видалено 1 байт.",
    );
    expect(translateText("Same photo. 24 bytes removed in your browser.", "uk")).toBe(
      "Те саме фото. У вашому браузері видалено 24 байти.",
    );
    expect(translateText("Same photo. 25 bytes removed in your browser.", "uk")).toBe(
      "Те саме фото. У вашому браузері видалено 25 байтів.",
    );
  });

  it("translates every copy distance in the pixel explanation", () => {
    expect(translateText("Pixel 9, 27: part of a copy of 258 bytes from exactly one row up — 9 bits say so.", "uk")).toBe(
      "Піксель 9, 27: частина копії з 258 байтів, узятої рівно на один рядок вище; це закодовано у 9 бітах.",
    );
    expect(translateText("Pixel 9, 27: part of a copy of 258 bytes from exactly 3 rows up — 9 bits say so.", "uk")).toContain(
      "рівно на 3 рядки вище",
    );
    expect(translateText("Pixel 9, 27: part of a copy of 258 bytes from 193 bytes back — 9 bits say so.", "uk")).toContain(
      "на 193 байти раніше",
    );
  });
});
