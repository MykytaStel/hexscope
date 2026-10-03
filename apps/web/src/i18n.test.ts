import { describe, expect, it } from "vitest";
import { GUIDE_UK } from "./guide-i18n";
import { registerGuideTranslations, resolveLocale, translateText } from "./i18n";

registerGuideTranslations(GUIDE_UK);

describe("the interface language", () => {
  it("keeps an explicit language choice ahead of the browser language", () => {
    expect(resolveLocale("uk", ["en-US"])).toBe("uk");
    expect(resolveLocale("en", ["uk-UA"])).toBe("en");
  });

  it("starts in Ukrainian for Ukrainian browsers and English otherwise", () => {
    expect(resolveLocale(null, ["uk-UA", "en-US"])).toBe("uk");
    expect(resolveLocale(null, ["en-US"])).toBe("en");
  });

  it("matches guide translations when HTML whitespace surrounds a text fragment", () => {
    expect(translateText(" is a ZIP of XML files, and several of them are about you rather than the text:", "uk")).toBe(
      " — це ZIP-архів із файлами XML. Деякі з них містять відомості про вас, а не про текст документа:",
    );
    expect(
      translateText(" — and the text each deletion removed, which is still in the file though no page shows it.", "uk"),
    ).toBe(" — разом із видаленим текстом, який і досі є у файлі, хоча його не видно на сторінках.");
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

  it("translates the clean-copy verification headings and concrete reasons", () => {
    expect(translateText("Checking the copy in this tab…", "uk")).toBe("Перевіряємо копію в цій вкладці…");
    expect(translateText("Removed", "uk")).toBe("Видалено");
    expect(translateText("Still present", "uk")).toBe("Залишилося в копії");
    expect(translateText("Not checked", "uk")).toBe("Не перевірено");
    expect(translateText("This finding is still present in the copy.", "uk")).toBe("Цей факт залишився в копії.");
    expect(translateText("Selected PDF text", "uk")).toBe("Вибраний текст PDF");
    expect(translateText("This finding appeared in the copy but was not found in the source file.", "uk")).toBe(
      "Цього факту не було у вихідному файлі, але він з’явився в копії.",
    );
    expect(translateText("Hexscope does not have complete JPEG coverage for this finding yet.", "uk")).toBe(
      "Hexscope ще не має повного покриття JPEG для цього факту.",
    );
    expect(translateText("This copy is larger than the 10 MiB verification limit.", "uk")).toBe(
      "Копія перевищує ліміт перевірки 10 МіБ.",
    );
    expect(translateText("Check unavailable.", "uk")).toBe("Перевірка недоступна.");
    expect(
      translateText(
        "The selected area was redacted by the operation, but text inside its pixels was not checked because Hexscope does not use OCR.",
        "uk",
      ),
    ).toBe("Вибрану ділянку зафарбовано, але текст у її пікселях не перевірено: Hexscope не розпізнає текст на зображеннях.");
    expect(
      translateText(
        "The selected area was redacted by the operation, but text inside its pixels was not checked because Hexscope does not use OCR. Check unavailable.",
        "uk",
      ),
    ).toBe(
      "Вибрану ділянку зафарбовано, але текст у її пікселях не перевірено: Hexscope не розпізнає текст на зображеннях. Перевірка недоступна.",
    );
  });

  it("translates the byte inspector, touch guidance, and copy feedback", () => {
    expect(translateText("Tap a byte", "uk")).toBe("Торкніться байта");
    expect(translateText("Hover a byte or row; click to pin", "uk")).toBe(
      "Наведіть на байт чи рядок; натисніть, щоб закріпити",
    );
    expect(translateText("The part selected", "uk")).toBe("Вибрана частина");
    expect(translateText("The file's bytes, in hex and as text", "uk")).toBe(
      "Байти файла у шістнадцятковому та текстовому вигляді",
    );
    expect(translateText("Field", "uk")).toBe("Поле");
    expect(translateText("Container", "uk")).toBe("Контейнер");
    expect(translateText("pinned", "uk")).toBe("закріплено");
    expect(translateText("Offset", "uk")).toBe("Зсув");
    expect(translateText("Length", "uk")).toBe("Довжина");
    expect(translateText("Kind", "uk")).toBe("Тип");
    expect(translateText("Value", "uk")).toBe("Значення");
    expect(translateText("Spec", "uk")).toBe("Специфікація");
    expect(translateText("Copy as", "uk")).toBe("Копіювати як");
    expect(translateText("a C array", "uk")).toBe("масив C");
    expect(translateText("1 byte", "uk")).toBe("1 байт");
    expect(translateText("90 bytes", "uk")).toBe("90 байтів");
    expect(translateText("Copied 90 bytes as a C array.", "uk")).toBe("Скопійовано 90 байтів у форматі масиву C.");
    expect(translateText("not allowed here", "uk")).toBe("Браузер заборонив копіювання");
    expect(translateText("Start of image: the two bytes every JPEG begins with.", "uk")).toBe(
      "Початок зображення: перші два байти кожного JPEG.",
    );
    expect(translateText("The location directory: where the picture was taken.", "uk")).toBe(
      "Каталог геоданих: де зроблено фото.",
    );
    expect(translateText("Copied 3 bytes as hex.", "uk")).toBe("Скопійовано 3 байти у форматі hex.");
    expect(translateText("Copied 11 bytes as base64.", "uk")).toBe("Скопійовано 11 байтів у форматі base64.");
    expect(translateText("Copied 1 byte as a C array.", "uk")).toBe("Скопійовано 1 байт у форматі масиву C.");
    expect(translateText("7 entries", "uk")).toBe("7 записів");
    expect(translateText("1 of 2", "uk")).toBe("Крок 1 із 2");
    expect(translateText("“Bytes” shows the structure, part by part: hover or tap anything and it says what it is and where the format defines it.", "uk")).toBe(
      "«Байти» показують структуру файла: торкніться байта або рядка, щоб побачити його призначення й посилання на формат.",
    );
    expect(translateText("In the bytes, once they have focus (Tab to them)", "uk")).toBe("У байтах, після переходу до них клавішею Tab");
    expect(translateText("Move one byte", "uk")).toBe("Перейти на один байт");
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
