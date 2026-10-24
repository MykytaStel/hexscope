import { describe, expect, it } from "vitest";
import "./analysis-i18n";
import { GUIDE_UK } from "./guide-i18n";
import { registerLazyTranslations, resolveLocale, translateText, untranslate } from "./i18n";
import "./i18n-uk";
import { OTHERS, READS } from "./verdict";

registerLazyTranslations({ ...GUIDE_UK, "A different value remains in the copy.": "У копії залишилося інше значення." });

describe("the interface language", () => {
  it("preserves whitespace-only text nodes when translating", () => {
    expect(translateText(" ", "uk")).toBe(" ");
    expect(translateText("\n  \t", "uk")).toBe("\n  \t");
  });

  it("names what was found inside a file, and how fast it was read, in Ukrainian", () => {
    expect(translateText("Hidden in it: 1 file", "uk")).toBe("Приховано у файлі: 1 файл");
    expect(translateText("Files inside: 3 files", "uk")).toBe("Файли всередині: 3 файли");
    expect(translateText("Hidden in it: data after the end of the image.", "uk")).toBe("Приховано у файлі: дані після кінця зображення.");
    expect(translateText("parsed in 5.5 ms", "uk")).toBe("розібрано за 5,5 мс");
    expect(translateText("Save it", "uk")).toBe("Зберегти");
    expect(translateText("A JPEG picture", "uk")).toBe("Зображення JPEG");
  });

  it("explains when ZIP contents disagree with the recorded size", () => {
    expect(
      translateText("the extracted size does not match what the archive records", "uk"),
    ).toBe("Розмір розпакованих даних не збігається з тим, який записано в архіві.");
  });

  it("names what a document's clean copy took out of each part", () => {
    expect(translateText("2 tracked changes, accepted", "uk")).toBe("2 виправлення прийнято");
    expect(translateText("5 tracked changes, accepted", "uk")).toBe("5 виправлень прийнято");
    expect(translateText("3 comments and who wrote them", "uk")).toBe("3 коментарі і хто їх написав");
    expect(translateText("a picture of the first page", "uk")).toBe("зображення першої сторінки");
    expect(translateText("Printer", "uk")).toBe("Принтер");
  });

  it("says how many files and clean copies, with Ukrainian number agreement", () => {
    expect(translateText("3 files", "uk")).toBe("3 файли");
    expect(translateText("2 of 21 files need a look", "uk")).toBe("2 з 21 файла варто переглянути");
    expect(translateText("1 reveals something about you", "uk")).toBe("1 файл розкриває щось про вас");
    expect(translateText("5 reveal something about you", "uk")).toBe("5 файлів розкривають щось про вас");
    expect(translateText("Make 1 clean copy", "uk")).toBe("Зробити 1 чисту копію");
    expect(translateText("Make 3 clean copies", "uk")).toBe("Зробити 3 чисті копії");
    expect(translateText("Share 3 clean copies", "uk")).toBe("Поділитися 3 чистими копіями");
    expect(translateText("Made 5 clean copies.", "uk")).toBe("Готово: 5 чистих копій.");
    expect(translateText("Findings: 24 removed, 0 present, 3 unchecked.", "uk")).toBe("Знахідки: прибрано 24, залишилося 0, не перевірено 3.");
    expect(translateText("+4 more", "uk")).toBe("ще 4");
  });

  it("turns Ukrainian back into English, and leaves English as it is", () => {
    expect(untranslate("Віддячити розробнику")).toBe("Thank the developer");
    expect(untranslate(" Банка monobank ")).toBe(" Monobank jar ");
    // A full stop after a link is English already, whatever else "." translates.
    expect(untranslate(".")).toBeUndefined();
    expect(untranslate("Thank the developer")).toBeUndefined();
  });

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
      "Розкриває місце зйомки, фотоапарат і серійний номер фотоапарата.",
    );
    expect(translateText("Reveals where it was taken, the camera, the camera's serial number, the owner's name and 4 more things.", "uk")).toBe(
      "Розкриває місце зйомки, фотоапарат, серійний номер фотоапарата, ім’я власника і ще 4.",
    );
    expect(translateText("Reveals who wrote it and 1 more thing.", "uk")).toBe("Розкриває автора і ще 1.");
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

  it("uses scan-first Ukrainian names for the film-scan tool", () => {
    expect(translateText("Film scans", "uk")).toBe("Плівкові скани");
    expect(translateText("Process film scans · JPEG / TIFF", "uk")).toBe("Обробити плівкові скани · JPEG / TIFF");
    expect(translateText("Process film scans", "uk")).toBe("Обробити плівкові скани");
    expect(translateText("Published positive scans from NASA’s Apollo 11 archive.", "uk")).toBe(
      "Оцифровані позитивні кадри з архіву Apollo 11 NASA.",
    );
    expect(translateText("NASA Apollo 11 archive", "uk")).toBe("Архів NASA «Аполлон-11»");
    expect(translateText("NASA is not affiliated with hexscope.", "uk")).toBe("NASA не пов’язана з hexscope.");
    expect(translateText("Open frame AS11-40-5903 in hexscope →", "uk")).toBe("Відкрити кадр AS11-40-5903 у hexscope →");
    expect(translateText("See the synthetic frame-detection test", "uk")).toBe("Переглянути синтетичний тест пошуку кадру");
    expect(translateText("A simple illustration tests the detector; it is not a photograph.", "uk")).toBe(
      "Проста ілюстрація перевіряє детектор; це не фотографія.",
    );
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
    expect(translateText("hexscope cannot yet confirm whether this JPEG finding was removed.", "uk")).toBe(
      "hexscope поки не може підтвердити, чи видалено цей факт у форматі JPEG.",
    );
    expect(translateText("hexscope could not completely read the copy, so its findings were not checked.", "uk")).toBe(
      "hexscope не зміг повністю прочитати копію, тому факти в ній не перевірено.",
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
        "The selected area was redacted by the operation, but text inside its pixels was not checked because hexscope does not use OCR.",
        "uk",
      ),
    ).toBe("Вибрану ділянку зафарбовано, але текст у її пікселях не перевірено: hexscope не розпізнає текст на зображеннях.");
    expect(
      translateText(
        "The selected area was redacted by the operation, but text inside its pixels was not checked because hexscope does not use OCR. Check unavailable.",
        "uk",
      ),
    ).toBe(
      "Вибрану ділянку зафарбовано, але текст у її пікселях не перевірено: hexscope не розпізнає текст на зображеннях. Перевірка недоступна.",
    );
  });

  it("translates the optional clean-copy receipt action", () => {
    expect(translateText("Save verification receipt", "uk")).toBe("Зберегти квитанцію перевірки");
    expect(translateText("A receipt includes a stable SHA-256 fingerprint; it can be compared with a known copy.", "uk")).toBe(
      "Квитанція містить сталий відбиток SHA-256; його можна зіставити з відомою копією.",
    );
    expect(translateText("Copies over 10 MiB have no fingerprint.", "uk")).toContain("10 МіБ");
    expect(translateText("Try again", "uk")).toBe("Спробувати ще раз");
    expect(translateText("Preparing receipt…", "uk")).toBe("Готуємо квитанцію…");
    expect(translateText("Verification receipt saved", "uk")).toBe("Квитанцію перевірки збережено");
    expect(translateText("Verification receipt saved without a fingerprint", "uk")).toBe("Квитанцію перевірки збережено без відбитка");
    expect(translateText("Could not save verification receipt", "uk")).toBe("Не вдалося зберегти квитанцію перевірки");
  });

  it("explains the opt-in batch fingerprint behavior", () => {
    expect(translateText("Include stable SHA-256 copy fingerprints in ZIP receipt (up to 10 MiB per copy)", "uk")).toContain("SHA-256");
    expect(translateText("Direct phone sharing sends copies only; Save as a ZIP includes the receipt and fingerprints.", "uk")).toBe(
      "Пряме поширення з телефона надсилає лише копії; у ZIP будуть квитанція та відбитки.",
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

describe("every way a file is said to give itself away", () => {
  it("is said in Ukrainian, alone and in a list", async () => {
    const { REVEALS } = await import("./verdict");
    const { CATEGORIES } = await import("./share");
    const phrases = [...new Set([...Object.values(REVEALS), ...CATEGORIES.map(([, words]) => words)])];
    for (const phrase of phrases) {
      expect(translateText(`Reveals ${phrase}.`, "uk"), phrase).not.toContain(phrase);
    }
    expect(translateText("Reveals who wrote it, the company and where its files were downloaded from.", "uk")).toBe(
      "Розкриває автора, компанію і звідки завантажено файли.",
    );
  });
});

describe("what a clean copy leaves out", () => {
  it("is said in Ukrainian for every kind of file", async () => {
    const { CLEAN_NOTES } = await import("./cleaner");
    for (const [kind, note] of Object.entries(CLEAN_NOTES)) {
      expect(translateText(note, "uk"), kind).not.toBe(note);
    }
  });
});

describe("sizes", () => {
  it("say their bytes in Ukrainian too", () => {
    expect(translateText("105.2 KB · 107,746 bytes", "uk")).toBe("105,2 КБ · 107\u00a0746 байтів");
    expect(translateText("1.5 KB · 1,503 bytes", "uk")).toBe("1,5 КБ · 1\u00a0503 байти");
    expect(translateText("145 B", "uk")).toBe("145 Б");
    expect(translateText("Photo  ·  3.8 MB", "uk")).toBe("Фото · 3,8 МБ");
    expect(translateText("photo-clean.jpg · 101.3 KB", "uk")).toBe("photo-clean.jpg · 101,3 КБ");
  });

  it("say what a file is made of in Ukrainian", () => {
    expect(translateText("Of its 105.2 KB, 96% goes to the picture and 4% to information about it — a second, small copy of the picture among it.", "uk")).toBe(
      "Із 105,2 КБ: зображення — 96% і відомості про файл — 4%; серед відомостей — друга, маленька копія зображення.",
    );
    expect(translateText("Of its 2.0 KB, 20% goes to the pages, fonts, images, 5% to information about it, 58% to the structure that holds it together and 18% to bytes the format does not account for.", "uk")).toBe(
      "Із 2,0 КБ: сторінки, шрифти й зображення — 20%, відомості про файл — 5%, структура, що його скріплює, — 58% і байти, яких формат не пояснює, — 18%.",
    );
  });
});

describe("what a verdict lists", () => {
  it("is Ukrainian to its end, the rest counted", () => {
    expect(translateText("Possible signs of impersonation: replies go to another domain, and 4 more things.", "uk")).toBe(
      "Можливі ознаки підміни відправника: відповіді йдуть на інший домен і ще 4.",
    );
    expect(translateText("Hidden in it: a file carried inside the document, and 1 more thing.", "uk")).toBe("Приховано у файлі: файл, вкладений у документ і ще 1.");
    expect(translateText("Where replies really go", "uk")).toBe("Справжня адреса для відповідей");
    expect(translateText("broken.png is open. Damaged: 1 place could not be read properly. It may not open, or open only partly.", "uk")).toMatch(/^broken\.png відкрито\. Пошкоджень: 1\./);
  });
});

describe("a sensitivity label", () => {
  it("names its organisation in Ukrainian", () => {
    expect(
      translateText("“Confidential”, organisation 72f988bf-86f1-41af-91ab-2d7cd011db47, set by olena@acme.example on 2024-03-02 08:15 UTC", "uk"),
    ).toBe("«Confidential», організація 72f988bf-86f1-41af-91ab-2d7cd011db47, встановив olena@acme.example, 2024-03-02 08:15 UTC");
  });
});

describe("advice", () => {
  it("is given in Ukrainian for every finding", async () => {
    const { RULES } = await import("./advice");
    for (const rule of RULES) {
      expect(translateText(rule.text, "uk"), rule.text.slice(0, 40)).not.toBe(rule.text);
    }
  });
});

describe("a file hexscope does not read", () => {
  it("is named, and what hexscope reads is said, in Ukrainian", () => {
    for (const [, what, note] of OTHERS) {
      const english = `This looks like ${what}. hexscope does not read it: ${READS}.${note ? ` ${note}` : ""}`;
      expect(translateText(english, "uk"), what).toMatch(/^Схоже, це [а-яіїєґ]/);
    }
    expect(translateText(`hexscope does not know this kind of file: ${READS}.`, "uk")).toMatch(/^hexscope не знає/);
    expect(
      translateText("Plain text: not a format hexscope takes apart. Its words read in the column beside the bytes.", "uk"),
    ).toMatch(/^Звичайний текст/);
  });
});
