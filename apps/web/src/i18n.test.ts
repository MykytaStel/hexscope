import { describe, expect, it } from "vitest";
import { GUIDE_UK } from "./guide-i18n";
import { registerLazyTranslations, resolveLocale, translateText } from "./i18n";

registerLazyTranslations({ ...GUIDE_UK, "A different value remains in the copy.": "У копії залишилося інше значення." });

describe("the interface language", () => {
  it("keeps an explicit language choice ahead of the browser language", () => {
    expect(resolveLocale("uk", ["en-US"])).toBe("uk");
    expect(resolveLocale("en", ["uk-UA"])).toBe("en");
  });

  it("starts in Ukrainian for Ukrainian browsers and English otherwise", () => {
    expect(resolveLocale(null, ["uk-UA", "en-US"])).toBe("uk");
    expect(resolveLocale(null, ["en-US"])).toBe("en");
  });

  it("translates the grouped metadata headings", () => {
    expect(translateText("General information", "uk")).toBe("Основна інформація");
    expect(translateText("Camera & device", "uk")).toBe("Камера й пристрій");
    expect(translateText("Dimensions", "uk")).toBe("Розміри");
    expect(translateText("Dates", "uk")).toBe("Дати");
    expect(translateText("Author & software", "uk")).toBe("Автор і програма");
    expect(translateText("Additional fields", "uk")).toBe("Додаткові поля");
  });

  it("translates the comparison workspace labels", () => {
    expect(translateText("Compare", "uk")).toBe("Порівняння");
    expect(translateText("File A", "uk")).toBe("Файл A");
    expect(translateText("Metadata fields", "uk")).toBe("Поля метаданих");
    expect(translateText("Only in A", "uk")).toBe("Лише у файлі A");
    expect(translateText("Inspect byte differences", "uk")).toBe("Переглянути відмінності байтів");
  });

  it("keeps technical names in English when no Ukrainian equivalent exists", () => {
    for (const term of ["JSON", "JPEG", "APP1 · EXIF", "PDF"]) {
      expect(translateText(term, "uk")).toBe(term);
    }
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

  it("translates the concise landing page direction", () => {
    expect(translateText("See what your file reveals about you", "uk")).toBe("Дізнайтеся, що файл розкриває про вас");
    expect(translateText("Check metadata, hidden content and file structure right in your browser.", "uk")).toBe(
      "Перевірте метадані, прихований вміст і структуру файла прямо у браузері.",
    );
    expect(translateText("See examples", "uk")).toBe("Переглянути приклади");
    expect(translateText("Examples", "uk")).toBe("Приклади");
    expect(translateText("Formats", "uk")).toBe("Формати");
    expect(translateText("Guides", "uk")).toBe("Посібники");
    expect(translateText("Menu", "uk")).toBe("Меню");
    expect(translateText("Different files, different findings", "uk")).toBe("Різні файли — різні знахідки");
    expect(translateText("Local processing.", "uk")).toBe("Обробка локально.");
    expect(translateText("No account.", "uk")).toBe("Без облікового запису.");
    expect(translateText("Open source.", "uk")).toBe("Відкритий код.");
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
    expect(translateText("Findings remain in the copy", "uk")).toBe("У копії залишилися дані");
    expect(translateText("Copy not fully checked", "uk")).toBe("Копію перевірено не повністю");
    expect(translateText("Removed", "uk")).toBe("Видалено");
    expect(translateText("Still present", "uk")).toBe("Залишилося в копії");
    expect(translateText("Not checked", "uk")).toBe("Не перевірено");
    expect(translateText("This finding is still present in the copy.", "uk")).toBe("Цей факт залишився в копії.");
    expect(translateText("A different value remains in the copy.", "uk")).toBe("У копії залишилося інше значення.");
    expect(translateText("This finding has no stable value to compare.", "uk")).toBe(
      "Значення цього факту не вдалося надійно порівняти.",
    );
    expect(translateText("Selected PDF text", "uk")).toBe("Вибраний текст PDF");
    expect(translateText("This finding appeared in the copy but was not found in the source file.", "uk")).toBe(
      "Цього факту не було у вихідному файлі, але він з’явився в копії.",
    );
    expect(translateText("Hexscope cannot yet confirm whether this JPEG finding was removed.", "uk")).toBe(
      "Hexscope поки не може підтвердити, чи видалено цей факт у форматі JPEG.",
    );
    expect(translateText("Hexscope could not completely read the copy, so its findings were not checked.", "uk")).toBe(
      "Hexscope не зміг повністю прочитати копію, тому факти в ній не перевірено.",
    );
    expect(translateText("No comparable findings were available to check.", "uk")).toBe(
      "Не було фактів, які можна було порівняти.",
    );
    expect(translateText("This finding appeared in the copy but was not found in the source file.", "uk")).toBe(
      "Цього факту не було у вихідному файлі, але він з’явився в копії.",
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
