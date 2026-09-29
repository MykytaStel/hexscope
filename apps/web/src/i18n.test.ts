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
});
