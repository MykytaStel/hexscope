export type Locale = "en" | "uk";

const LANGUAGE_KEY = "hexscope.language";
const lazyTranslations: Record<string, string> = {};

/** Adds copy for an optional, dynamically loaded feature without bloating the first page bundle. */
export function registerLazyTranslations(translations: Readonly<Record<string, string>>): void {
  Object.assign(lazyTranslations, translations);
}

// Keep the source copy as the key: the English UI remains the canonical copy,
// and untranslated technical terms stay readable until they have a reviewed
// Ukrainian equivalent.
const UK: Record<string, string> = {
  "View": "Вигляд",
  "Summary": "Зведення",
  "Bytes": "Байти",
  "Contains location": "Містить геолокацію",
  "This photo records where it was taken — click to see the bytes": "У фото записано місце зйомки — натисніть, щоб побачити дані",
  "What was found": "Що знайдено",
  "The picture": "Зображення",
  "The picture, with its 8×8 blocks": "Зображення з блоками 8×8",
  "Draw boxes over what the picture itself shows, and save a new picture with them in it": "Обведіть чорним те, що видно на зображенні, щоб зберегти його копію з прихованими ділянками",
  "Step through the DEFLATE decompression (P)": "Переглядати розпакування DEFLATE (P)",
  "Watch it decompress": "Як це розпаковується",
  "Open": "Відкрити",
  "Open file": "Відкрити файл",
  "Open a file": "Відкрити файл",
  "file": "файл",
  "that reads it.": "— той самий, що читає файл.",
  "Structure": "Структура",
  "Details": "Деталі",
  "Files": "Файли",
  "Start": "Початок",
  "See what a file gives away before you send it": "Дізнайтеся, що файл розкриває про вас, перш ніж надіслати його",
  "Where a photo was taken, whose camera took it, the text under a PDF's black boxes, the comments left in a Word file. hexscope finds them right here, in your browser, and saves a clean copy without them.": "Де зроблено фото, яким фотоапаратом, що приховано під чорними прямокутниками в PDF і які коментарі лишилися у Word. hexscope знаходить це прямо у браузері й зберігає очищену копію.",
  "Choose a file": "Вибрати файл",
  "Try a sample": "Спробувати приклад",
  "Other ways to open a file": "Інші способи відкрити файл",
  "Choose a whole folder": "Вибрати цілу теку",
  "Paste a copied picture": "Вставити скопійоване зображення",
  "You can pick several files at once.": "Можна вибрати кілька файлів одразу.",
  "You can also drop files anywhere on this page.": "Файли також можна перетягнути на сторінку.",
  "or a whole folder": "або цілу теку",
  "or paste a copied picture": "або вставити скопійоване зображення",
  "— pick several at once, if you like": "— за бажанням можна вибрати кілька файлів",
  "— or drop them anywhere on this page": "— або перетягнути їх на сторінку",
  "Nothing is uploaded.": "Файли нікуди не передаються.",
  "The file never leaves this tab: no account, no tracking.": "Файл не залишає цю вкладку: без облікового запису й стеження.",
  "Read the code": "Переглянути код",
  " that reads it.": " — той самий, що читає файл.",
  "Reads photos and pictures (JPEG, HEIC, PNG, AVIF, WebP, GIF), videos, PDFs, Word, Excel and PowerPoint — old .doc, .xls and .ppt too — ZIP, emails, Outlook messages and WebAssembly.": "Читає фото (JPEG, HEIC, PNG, AVIF, WebP, GIF), відео, PDF, Word, Excel, PowerPoint, старі .doc/.xls/.ppt, ZIP, листи Outlook та WebAssembly.",
  "A demonstration on a sample image": "Приклад на тестовому зображенні",
  "Where it was taken — drawn here, without asking any map site.": "Місце зйомки показано тут — без запиту до картографічного сервісу.",
  "Open in OpenStreetMap ↗": "Відкрити в OpenStreetMap ↗",
  "Opens openstreetmap.org in a new tab. The coordinates leave this page only if you click.": "Відкриває openstreetmap.org у новій вкладці. Координати покинуть цю сторінку, лише якщо ви натиснете посилання.",
  "How to remove the location from a photo →": "Як прибрати геолокацію з фото →",
  "Before you send a photo": "Перед надсиланням фото",
  "For the curious: point at a pixel, see the bytes that made it": "Для допитливих: наведіть на піксель і побачте байти, з яких його створено",
  "For the curious: a progressive JPEG, scan by scan": "Для допитливих: прогресивний JPEG, скан за сканом",
  "A photo, as a camera or phone saves it. Inside it:": "Фото таким, як його зберігає камера чи телефон. Усередині:",
  "Anyone you send it to can read all of that.": "Усе це зможе прочитати кожен, кому ви надішлете фото.",
  "The clean copy: the same picture, with its camera data removed, in your browser.": "Очищена копія: те саме зображення без даних фотоапарата, створена у вашому браузері.",
  "Same photo. Camera details removed in your browser.": "Те саме фото. Дані камери видалено у вашому браузері.",
  "WebAssembly module": "Модуль WebAssembly",
  "Word document": "Документ Word",
  "Excel workbook": "Таблиця Excel",
  "PowerPoint deck": "Презентація PowerPoint",
  "OpenDocument": "Документ OpenDocument",
  "EPUB book": "Книга EPUB",
  "Android app": "Застосунок Android",
  "Outlook message": "Лист Outlook",
  "Where": "Місце",
  "Camera serial": "Серійний номер фотоапарата",
  "the whole file": "увесь файл",
  "BEFORE YOU SEND": "ПЕРЕД НАДСИЛАННЯМ",
  "Before you send": "Перед надсиланням",
  "Check a photo": "Перевірити фото",
  "Where it was taken, the camera's serial number, the owner's name — and how to remove them.": "Де його зроблено, серійний номер фотоапарата, ім’я власника — і як це прибрати.",
  "Try a sample photo →": "Перевірити тестове фото →",
  "Check a document before you send it": "Перевірити документ перед надсиланням",
  "Black boxes that hide nothing, deleted text still inside, comments and who wrote them — in PDFs and Word files.": "Текст під чорними прямокутниками, видалений текст, коментарі та їхні автори — у PDF і Word.",
  "Try a blacked-out PDF →": "Перевірити PDF із прихованим текстом →",
  "Before you trust an email": "Перш ніж довіряти листу",
  "Check an email before you trust it": "Перевірити лист, перш ніж йому довіряти",
  "See where replies go, whether sender checks passed, and which server sent it.": "Перевірте, куди підуть відповіді, чи пройшов відправник перевірки та з якого сервера надійшов лист.",
  "Try a suspicious email →": "Перевірити підозрілий лист →",
  "BEFORE YOU TRUST AN EMAIL": "ПЕРШ НІЖ ДОВІРЯТИ ЛИСТУ",
  "Is this email real?": "Цей лист справжній?",
  "Where replies really go, whether the sender's domain vouched for it, and the server it came from.": "Куди насправді підуть відповіді, чи підтвердив домен відправника лист і з якого сервера його надіслано.",
  "Try a fake bank email →": "Перевірити тестовий лист від банку →",
  "What an email says about its sender": "Що лист розповідає про відправника",
  "The address it was sent from, the computer's name, the time zone — in the headers of every message, yours too.": "Адреса відправлення, назва комп’ютера й часовий пояс — у заголовках кожного листа, зокрема вашого.",
  "Try a sample email →": "Перевірити тестовий лист →",
  "Look inside": "Зазирнути всередину",
  "LOOK INSIDE": "ЗАЗИРНУТИ ВСЕРЕДИНУ",
  "Why a file won't open, and how it is built: every byte explained, with where its format defines it.": "Чому файл не відкривається і як він улаштований: пояснення кожного байта з посиланням на специфікацію.",
  "Why won't it open?": "Чому файл не відкривається?",
  "The exact bytes that break a file, and what they should have been.": "Байти, які пошкодили файл, і значення, яке мало бути.",
  "Try a broken image →": "Перевірити пошкоджене зображення →",
  "Watch compression work": "Подивитися, як працює стиснення",
  "Step through DEFLATE bit by bit: every Huffman code, every copy from earlier bytes.": "Розбір DEFLATE побітово: кожен код Гаффмана й кожне копіювання попередніх байтів.",
  "Play it →": "Запустити →",
  "Bytes view": "Перегляд байтів",
  " — the structure beside the hex, every part with its spec reference": " — структура поруч із hex-даними та посиланням на специфікацію",
  "Find": "Пошук",
  " — text or hex, and the part of the file it falls in": " — тексту чи hex-значень у файлі",
  "Compare": "Порівняння",
  " — two files, or a file and its clean copy": " — двох файлів або файла з очищеною копією",
  " — the whole structure, with its explanations": " — повна структура з поясненнями",
  "Command line and CI": "Командний рядок і CI",
  " — the same checks in ": " — ті самі перевірки в ",
  "a CLI and a GitHub Action": "CLI та GitHub Action",
  "More": "Інше",
  "Guides": "Посібники",
  "GUIDES": "ПОСІБНИКИ",
  "Before you send or trust a file": "Перед надсиланням файла або довірою до нього",
  "Remove the location from a photo →": "Прибрати геолокацію з фото →",
  "What a screenshot gives away →": "Що розкриває знімок екрана →",
  "Black out a PDF properly →": "Як правильно приховати дані в PDF →",
  "Check a document before you send it →": "Перевірити документ перед надсиланням →",
  "Text no one can see in a PDF →": "Невидимий текст у PDF →",
  "What an edited PDF still holds →": "Що лишається у відредагованому PDF →",
  "Is this email real? →": "Цей лист справжній? →",
  "How files work": "Як улаштовані файли",
  "Why a PNG won't open →": "Чому PNG не відкривається →",
  "How DEFLATE works, step by step →": "Як працює DEFLATE: покроково →",
  "More samples to try": "Інші приклади",
  "MORE SAMPLES TO TRY": "ІНШІ ПРИКЛАДИ",
  "a cropped photo": "обрізане фото",
  "an iPhone photo": "фото з iPhone",
  "a Wi-Fi QR code": "QR-код Wi-Fi",
  "an iPhone video": "відео з iPhone",
  "a Word document": "документ Word",
  "a spreadsheet": "електронна таблиця",
  "an edited PDF": "відредагований PDF",
  "an email": "лист",
  "an Outlook message": "лист Outlook",
  "an old Word file": "старий файл Word",
  "a progressive JPEG": "прогресивний JPEG",
  "a WebAssembly module": "модуль WebAssembly",
  "Open source · no accounts · no analytics · no uploads": "Відкритий код · без облікових записів · без аналітики · без передавання файлів",
  "How your files stay private": "Як ми захищаємо ваші файли",
  "Source on GitHub": "Код на GitHub",
  "Or press ? anywhere": "Або натисніть ? будь-де",
  "Keyboard shortcuts": "Сполучення клавіш",
  "Take the tour": "Показати огляд",
  "Drop to open": "Перетягніть, щоб відкрити",
  "Parsing…": "Читаємо файл…",
  "What hexscope found": "Що знайшов hexscope",
  "WHAT HEXSCOPE FOUND": "ЩО ЗНАЙШОВ HEXSCOPE",
  "Reveals where it was taken, the camera, the camera's serial number and the owner's name.": "Розкриває місце зйомки, фотоапарат, його серійний номер та ім’я власника.",
  "The file itself is fine: nothing in it is damaged.": "Файл справний: пошкоджень не знайдено.",
  "The file is fine: nothing in it is damaged or out of place.": "Файл справний: пошкоджень і невідповідностей не знайдено.",
  "Found by reading the file's structure. It is not a virus scan.": "Висновок зроблено за структурою файла. Це не антивірусна перевірка.",
  "Show me": "Показати",
  "This guide is currently available in English. The page controls are in Ukrainian.": "Цей посібник поки доступний лише англійською. Елементи керування сторінкою перекладені українською.",
  "Show where in the file this is": "Показати це місце у файлі",
  "Remove it — save a clean copy": "Прибрати дані й зберегти чисту копію",
  "Checking the copy in this tab…": "Перевіряємо копію в цій вкладці…",
  "Removed": "Видалено",
  "Still present": "Залишилося в копії",
  "This finding is still present in the copy.": "Цей факт залишився в копії.",
  "Not checked": "Не перевірено",
  "Selected PDF text": "Вибраний текст PDF",
  "Text in the selected picture area": "Текст у вибраній ділянці зображення",
  "Copy verification": "Перевірка копії",
  "This finding appeared in the copy but was not found in the source file.": "Цього факту не було у вихідному файлі, але він з’явився в копії.",
  "This finding has no stable value to compare.": "Значення цього факту не вдалося надійно порівняти.",
  "This copy is larger than the 10 MiB verification limit.": "Копія перевищує ліміт перевірки 10 МіБ.",
  "Check unavailable.": "Перевірка недоступна.",
  "Hexscope could not completely read the copy, so its findings were not checked.": "Hexscope не зміг повністю прочитати копію, тому факти в ній не перевірено.",
  "No comparable findings were available to check.": "Не було фактів, які можна було порівняти.",
  "No searchable text was available for this selection.": "У вибраному місці не було доступного для пошуку тексту.",
  "The source page was incomplete, so this selection could not be checked.": "Вихідну сторінку прочитано не повністю, тому вибраний текст не перевірено.",
  "The corresponding output page could not be checked.": "Не вдалося перевірити відповідну сторінку копії.",
  "The output page was incomplete, so this selection could not be checked.": "Сторінку копії прочитано не повністю, тому вибраний текст не перевірено.",
  "The selected area was redacted by the operation, but text inside its pixels was not checked because Hexscope does not use OCR.": "Вибрану ділянку зафарбовано, але текст у її пікселях не перевірено: Hexscope не розпізнає текст на зображеннях.",
  "Kept: the QR code, which is part of the picture. Black it out with “Black out part of the picture” before sending.": "Залишено QR-код, бо він є частиною зображення. Перед надсиланням зафарбуйте його інструментом «Приховати частину зображення».",
  "Kept: comments, form answers, attached files, links, and anything it does when opened, which are part of the document. Delete them in a PDF editor — or print to a new PDF, which keeps only the pages — if they should not travel with it.": "Залишено коментарі, відповіді у формах, вкладені файли, посилання та дії під час відкриття — це частини документа. Якщо їх не слід надсилати, видаліть їх у редакторі PDF або надрукуйте новий PDF лише зі сторінками.",
  "Kept: what is part of a workbook or a deck itself — its comments, hidden sheets, rows and slides, speaker notes, links to other files and to sites — and files kept inside it, such as the workbook behind a chart. Remove them in Word, Excel or PowerPoint, then save.": "Залишено вміст самої таблиці чи презентації: коментарі, приховані аркуші, рядки й слайди, нотатки доповідача, посилання та вкладені файли. Видаліть їх у Word, Excel або PowerPoint і збережіть файл.",
  "Copy a clean picture": "Копіювати очищене зображення",
  "Puts the picture on the clipboard without anything else, to paste into a chat": "Копіює саме зображення без зайвих даних, щоб вставити його в чат",
  "Other ways": "Інші способи",
  "For the curious": "Для допитливих",
  "For the curious: look inside a file": "Для допитливих: зазирнути всередину файла",
  "Selected part": "Вибрана частина",
  "The part selected": "Вибрана частина",
  "The file's bytes, in hex and as text": "Байти файла у шістнадцятковому та текстовому вигляді",
  "Container": "Контейнер",
  "Field": "Поле",
  "Warning": "Попередження",
  "Error": "Помилка",
  "pinned": "закріплено",
  "What the file gives away": "Що розкриває файл",
  "Keys": "Клавіші",
  "Tap a byte": "Торкніться байта",
  "Hover a byte or row; click to pin": "Наведіть на байт чи рядок; натисніть, щоб закріпити",
  "Offset": "Зсув",
  "Length": "Довжина",
  "Kind": "Тип",
  "Value": "Значення",
  "Spec": "Специфікація",
  "Copy as": "Копіювати як",
  "a C array": "масив C",
  "copied": "скопійовано",
  "not allowed here": "Браузер заборонив копіювання",
  "SOI": "Початок зображення (SOI)",
  "marker": "маркер",
  "length": "довжина",
  "TIFF header": "Заголовок TIFF",
  "IFD0": "IFD0 · основний каталог",
  "Exif IFD": "Exif IFD · дані камери",
  "GPS IFD": "GPS IFD · геодані",
  "GPSLatitude": "GPSLatitude · широта",
  "GPSLongitude": "GPSLongitude · довгота",
  "A JPEG image: a run of segments, each opened by a two-byte marker, around the compressed picture.": "Зображення JPEG: послідовність сегментів із двобайтовими маркерами навколо стисненого зображення.",
  "Start of image: the two bytes every JPEG begins with.": "Початок зображення: перші два байти кожного JPEG.",
  "EXIF metadata: the camera, its settings, the time, and often where the picture was taken.": "Метадані EXIF: фотоапарат, його налаштування, час і часто місце зйомки.",
  "The two bytes, FF and a code, that say what this segment is.": "Два байти — FF і код — вказують тип цього сегмента.",
  "How long the segment is, counting these two bytes but not the marker.": "Довжина сегмента разом із цими двома байтами, але без маркера.",
  "The start of the EXIF data, laid out like a TIFF file: byte order, a check number, and where the first directory is.": "Початок даних EXIF у форматі TIFF: порядок байтів, контрольне число та розташування першого каталогу.",
  "The main image's directory: a list of tags, each a number, a type and a value or where to find it.": "Каталог основного зображення: перелік тегів із номером, типом і значенням або його розташуванням.",
  "The camera-settings directory: exposure, lens, dates.": "Каталог налаштувань камери: витримка, об’єктив, дати.",
  "The location directory: where the picture was taken.": "Каталог геоданих: де зроблено фото.",
  "“Bytes” shows the structure, part by part: hover or tap anything and it says what it is and where the format defines it.": "«Байти» показують структуру файла: торкніться байта або рядка, щоб побачити його призначення й посилання на формат.",
  "Where the location directory, the GPS IFD, starts: this picture records a place.": "Початок каталогу геоданих GPS IFD: це фото містить місце зйомки.",
  "In the bytes, once they have focus (Tab to them)": "У байтах, після переходу до них клавішею Tab",
  "Move one byte": "Перейти на один байт",
  "Move one row": "Перейти на один рядок",
  "The first or last byte in this row": "На перший або останній байт цього рядка",
  "Select this part": "Вибрати цю частину",
  "Supported formats": "Підтримувані формати",
  "Guides and more sample files": "Посібники та інші приклади файлів",
  "More about the file: what it is made of, its picture, its structure": "Детальніше про файл: склад, зображення та структура",
  "What this photo reveals": "Що розкриває це фото",
  "What this picture reveals": "Що розкриває це зображення",
  "What this document reveals": "Що розкриває цей документ",
  "What this email reveals": "Що розкриває цей лист",
  "WHAT THIS PHOTO REVEALS": "ЩО РОЗКРИВАЄ ЦЕ ФОТО",
  "What it's made of": "З чого складається файл",
  "WHAT IT'S MADE OF": "З ЧОГО СКЛАДАЄТЬСЯ ФАЙЛ",
  "Picture": "Зображення",
  "Photo": "Фото",
  "Video": "Відео",
  "Recording": "Аудіозапис",
  "Email": "Лист",
  "File": "Файл",
  "Size": "Розмір",
  "Location": "Місце зйомки",
  "Camera": "Фотоапарат",
  "Lens": "Об’єктив",
  "Serial number": "Серійний номер",
  "Owner": "Власник",
  "Taken": "Дата зйомки",
  "Software": "Програма",
  "Thumbnail": "Мініатюра",
  "Place named": "Згадане місце",
  "Caption": "Підпис",
  "Title": "Назва",
  "Author": "Автор",
  "Last saved by": "Останній редактор",
  "Created": "Створено",
  "Modified": "Змінено",
  "Comments": "Коментарі",
  "Tracked changes": "Виправлення",
  "Deleted text": "Видалений текст",
  "Hidden text": "Прихований текст",
  "Hidden, not removed": "Приховано, але не видалено",
  "Photo inside": "Фото всередині",
  "Made from": "Створено з",
  "Revisions": "Кількість змін",
  "Editing time": "Час редагування",
  "Company": "Компанія",
  "Application": "Застосунок",
  "Template": "Шаблон",
  "Subject": "Тема листа",
  "Keywords": "Ключові слова",
  "PDF made by": "Створено програмою",
  "Encryption": "Шифрування",
  "Edited": "Відредаговано",
  "Editing history": "Історія редагування",
  "Shutter count": "Лічильник затвора",
  "Phone on for": "Час роботи телефона",
  "Linked shots": "Пов’язані знімки",
  "Function names": "Назви функцій",
  "Made with AI": "Створено за допомогою ШІ",
  "Content Credentials": "Відомості про походження вмісту",
  "Prompt": "Запит для генератора",
  "Sent from": "Надіслано з",
  "Computer's name": "Назва комп’ютера",
  "Mail app": "Поштова програма",
  "Time zone": "Часовий пояс",
  "Replies go to": "Адреса для відповіді",
  "When opened": "Під час відкриття",
  "Scripts": "Сценарії",
  "Opens a program": "Запускає програму",
  "Form goes to": "Куди надсилається форма",
  "Link goes elsewhere": "Посилання веде в інше місце",
  "Lookalike address": "Схожа адреса",
  "Risky attachment": "Небезпечне вкладення",
  "Bounces": "Адреса повернення",
  "Sender checks": "Перевірки відправника",
  "Link": "Посилання",
  "Links": "Посилання",
  "Wi-Fi in a QR code": "Дані Wi-Fi у QR-коді",
  "Contact in a QR code": "Контакт у QR-коді",
  "What you can do": "Що можна зробити",
  "WHAT YOU CAN DO": "ЩО МОЖНА ЗРОБИТИ",
  "Share what it revealed": "Поділитися результатом",
  "Only the kinds of thing, never what they are.": "Лише типи знайдених даних — без самих значень.",
  "What was removed": "Що видалено",
  "Made a clean copy.": "Очищену копію створено.",
  "Open the clean copy": "Відкрити очищену копію",
  "Check it yourself: the card should now be empty": "Перевірте самі: тепер у картці не має бути цих даних",
  "What no clean copy can remove": "Що неможливо прибрати очищенням",
  "Black out part of the picture…": "Приховати частину зображення…",
  "Check another file": "Перевірити інший файл",
  "This email may not be from who it says": "Можливо, лист надіслано не тим, за кого себе видає відправник",
  "Do not reply, open its files or follow its links. If it matters, ask the sender another way — a number or address you already have.": "Не відповідайте, не відкривайте вкладення й не переходьте за посиланнями. Якщо лист важливий, зв’яжіться з відправником іншим способом — за відомим вам номером чи адресою.",
  "Possible signs of impersonation": "Можливі ознаки підміни відправника",
  "Tells you about its sender: ": "Що лист розповідає про відправника: ",
  "Nothing personal found in this file": "Особистих даних у файлі не знайдено",
  "Not a kind of file hexscope reads": "hexscope не читає цей тип файла",
  "This file does more than show pages": "Файл може виконувати дії, а не лише показувати сторінки",
  "No file is open": "Файл не відкрито",
  "No copy was made": "Копію не створено",
  "Some PDF form content could not be fully checked. Search may miss text.": "Не весь вміст PDF-форм вдалося перевірити. Пошук може пропустити текст.",
  "no copy was made because hexscope could not fully inspect or isolate form content on a PDF page": "Копію не створено: hexscope не зміг повністю перевірити або відокремити вміст форм на сторінці PDF",
  "no copy was made because hexscope could not fully inspect or isolate form content on a PDF page.": "Копію не створено: hexscope не зміг повністю перевірити або відокремити вміст форм на сторінці PDF.",
  "Making the copy…": "Створюємо копію…",
  "Choose": "Вибрати",
  "Save": "Зберегти",
  "Cancel": "Скасувати",
  "Close": "Закрити",
  "Done": "Готово",
  "Search": "Пошук",
  "Search the file": "Шукати у файлі",
  "Previous": "Назад",
  "Next": "Далі",
  "Skip": "Пропустити",
  "Theme: as your system (click for light)": "Тема: як у системі (натисніть для світлої)",
  "Theme: light (click for dark)": "Тема: світла (натисніть для темної)",
  "Theme: dark (click to follow your system)": "Тема: темна (натисніть для теми системи)",
  "The answer first, in plain words: damaged, hiding something, giving something away, or healthy. “Show me” goes to the bytes.": "Спершу коротка відповідь: файл пошкоджений, щось приховує, розкриває дані чи справний. «Показати» прокручує до знайденого факту.",
  "The answer first, in plain words: damaged, hiding something, giving something away, or healthy. “Show me” takes you to the fact in the summary, or to its bytes when there isn't one.": "Спершу коротка відповідь: файл пошкоджений, щось приховує, розкриває дані чи справний. «Показати» веде до факту у зведенні, а якщо його там немає — до байтів.",
  "Places, names, serial numbers, deleted text. “Remove it — save a clean copy” makes a copy without them, here in the tab: nothing is uploaded.": "Місця, імена, серійні номери, видалений текст. «Прибрати дані й зберегти чисту копію» створює копію тут, у вкладці. Нічого не передається.",
  "Every byte, explained": "Кожен байт із поясненням",
  "Press ? for the keyboard: O opens a file, N goes to the next problem. Drop several files at once to check them all.": "Натисніть ?, щоб побачити клавіші: O відкриває файл, N переходить до наступної проблеми. Перетягніть кілька файлів, щоб перевірити їх разом.",
  "Would you like to share what this file reveals?": "Поділитися тим, що розкриває цей файл?",
  "Nothing leaves this browser": "Жодні дані не залишають браузер",
  "This browser did not let the page read what you copied. Press": "Браузер не дозволив прочитати скопійоване. Натисніть",
  "There is no picture among what you copied: copy a screenshot or a photo first, then paste it here.": "Серед скопійованого немає зображення. Спершу скопіюйте знімок екрана або фото й вставте сюди.",
  "The file never leaves this tab": "Файл не залишає цю вкладку",
  "Privacy": "Приватність",
  "More about the file": "Докладніше про файл",
  "A file not read has no part to show.": "Якщо файл не вдалося прочитати, його частини не показуються.",
  "Nothing personal found": "Особистих даних не знайдено",
  "Save a clean copy": "Зберегти очищену копію",
  "Makes the copy in this tab: nothing is uploaded": "Створює копію в цій вкладці: файл нікуди не передається",
  "Share the clean copy": "Поділитися очищеною копією",
  "Save a repaired copy": "Зберегти відновлену копію",
  "Save the blacked-out copy": "Зберегти копію з прихованими даними",
  "The file is empty: there is not a single byte in it. A download or a copy may have stopped before it began.": "Файл порожній: у ньому немає жодного байта. Можливо, завантаження або копіювання перервалося, не розпочавшись.",
  "A camera's serial number is in every photo it takes, so photos shared under different names can be traced to one camera. The clean copy removes it.": "Серійний номер є в кожному фото з цього фотоапарата. За ним можна пов’язати знімки, навіть якщо їх надсилали під різними іменами. Очищена копія видаляє цей номер.",
  "The owner's name comes from the camera's own settings: change it there, and new photos will stop carrying it.": "Ім’я власника береться з налаштувань фотоапарата. Змініть його там, щоб нові фото більше не містили це ім’я.",
  "Removes the camera data, location, serial numbers, the maker's notes, thumbnail, comments and Content Credentials. The picture itself is copied unchanged.": "Очищена копія видаляє дані фотоапарата, геолокацію, серійні номери, примітки автора, мініатюру, коментарі та відомості про походження вмісту. Саме зображення копіюється без змін.",
  "Instagram, Facebook and X usually remove the location when you post. Sending the photo itself keeps it: by email, as a file in Telegram or WhatsApp, through a cloud link, on a forum or a marketplace. For those, save a clean copy. To keep new photos from recording it, turn off location for the camera app.": "Instagram, Facebook та X зазвичай видаляють геолокацію під час публікації. Якщо надіслати сам файл фото — електронною поштою, у Telegram чи WhatsApp, через хмарне посилання, на форумі або маркетплейсі — геолокація залишиться. У таких випадках збережіть очищену копію. Щоб нові фото не записували місце зйомки, вимкніть геолокацію для застосунку камери.",
  "A link that says one site and goes to another, an address in lookalike letters, a file that is a program or a web page: each is how a fake message gets what it wants. Do not click or open them. To visit the site, type its address yourself.": "Посилання може показувати одну адресу, а вести на іншу; адреса — складатися зі схожих літер; вкладення — бути програмою чи вебсторінкою. Так шахрайські листи намагаються виманити дані. Не натискайте посилання й не відкривайте вкладення. Щоб зайти на сайт, введіть його адресу самостійно.",
  "If replies go to another domain, or the sender's domain did not vouch for the message, it may not be from who it says. Do not reply, open its files or follow its links; ask the sender another way — a number you already have.": "Якщо відповіді надсилаються на інший домен або домен відправника не підтвердив лист, можливо, його надіслав не той, за кого себе видає відправник. Не відповідайте, не відкривайте вкладення й не переходьте за посиланнями. Зв’яжіться з відправником іншим способом — наприклад, за номером, який уже маєте.",
  "A black box drawn over text, or over part of a scanned page, hides it only on screen and on paper: anyone can still select, copy or search the text, or take the picture out whole. Do not send this file. The clean copy takes out what is under the boxes — the letters, and the picture's pixels there — and applies any marks for redaction, leaving the rest in place; open it here to check.": "Чорний прямокутник поверх тексту чи частини сканованої сторінки приховує їх лише візуально: текст можна виділити, скопіювати або знайти, а зображення — витягнути повністю. Не надсилайте цей файл. Очищена копія видаляє дані під прямокутниками — текст і відповідні пікселі зображення — та застосовує позначки приховування. Відкрийте її тут і перевірте результат.",
  "Text no one can see on the page is still read by search, by screen readers and by programs that read the file — hiring systems and AI tools among them. If you did not put it there, ask who did. The clean copy removes it.": "Тексту, якого не видно на сторінці, усе одно можуть прочитати пошук, програми екранного доступу та інші інструменти, зокрема системи найму й ШІ. Якщо ви не додавали його, з’ясуйте, звідки він узявся. Очищена копія його видалить.",
  "How to remove the location from a photo": "Як прибрати геолокацію з фото",
  "What a screenshot gives away": "Що розкриває знімок екрана",
  "How to black out a PDF properly": "Як правильно приховати дані в PDF",
};

const NOUNS: Record<string, string> = {
  photo: "Фото",
  picture: "Зображення",
  video: "Відео",
  recording: "Аудіозапис",
  PDF: "PDF",
  email: "Лист",
  document: "Документ",
  archive: "Архів",
  file: "Файл",
  module: "Модуль",
};

const REVEAL_WORDS: Record<string, string> = {
  "where it was taken": "місце зйомки",
  "the camera's serial number": "серійний номер фотоапарата",
  "the owner's name": "ім’я власника",
  "the camera": "фотоапарат",
  "when it was taken": "дату зйомки",
  "the place it names": "згадане місце",
  "the file it was made from": "початковий файл",
  "how it was edited": "спосіб редагування",
  "who wrote it": "автора",
  "who saved it last": "останнього редактора",
  "text hidden from view": "прихований текст",
  "text that was deleted": "видалений текст",
  "where its photos were taken": "місце зйомки фото всередині",
  "the address it was sent from": "адресу відправлення",
  "the sender's computer's name": "назву комп’ютера відправника",
  "that its sender's domain did not vouch for it": "непідтвердження листа доменом відправника",
  "where replies really go": "справжню адресу для відповідей",
  "a link that says one site and goes to another": "посилання, що веде не туди, куди обіцяє",
  "an address in lookalike letters": "адресу зі схожими літерами",
  "an attachment that runs or opens a site": "вкладення, що запускає програму або відкриває сайт",
  "a Wi-Fi password in a QR code": "пароль Wi-Fi у QR-коді",
  "a two-factor secret in a QR code": "секрет двофакторної автентифікації у QR-коді",
  "text that was blacked out but not removed": "текст, який лише приховали, але не видалили",
};

function translateRevealList(value: string): string {
  const vocabulary = Object.keys(REVEAL_WORDS).sort((a, b) => b.length - a.length);
  const parts: string[] = [];
  let rest = value;
  while (rest) {
    const item = vocabulary.find((phrase) => {
      if (!rest.startsWith(phrase)) return false;
      const tail = rest.slice(phrase.length);
      return tail === "" || tail.startsWith(", ") || tail.startsWith(" and ");
    });
    if (!item) return value;
    parts.push(REVEAL_WORDS[item]);
    rest = rest.slice(item.length);
    if (rest.startsWith(", ")) rest = rest.slice(2);
    else if (rest.startsWith(" and ")) rest = rest.slice(5);
    else if (rest) return value;
  }
  return parts.join(", ");
}

export function resolveLocale(saved: string | null, browserLanguages: readonly string[]): Locale {
  if (saved === "en" || saved === "uk") return saved;
  return browserLanguages.some((language) => language.toLowerCase().startsWith("uk")) ? "uk" : "en";
}

function factWord(count: number): string {
  const lastTwo = count % 100;
  const last = count % 10;
  if (last === 1 && lastTwo !== 11) return "факт";
  if (last >= 2 && last <= 4 && (lastTwo < 12 || lastTwo > 14)) return "факти";
  return "фактів";
}

function ruleWord(count: number): string {
  const lastTwo = count % 100;
  const last = count % 10;
  if (last === 1 && lastTwo !== 11) return "правило";
  if (last >= 2 && last <= 4 && (lastTwo < 12 || lastTwo > 14)) return "правила";
  return "правил";
}

function ukrainianCountWord(count: number, one: string, few: string, many: string): string {
  const lastTwo = count % 100;
  const last = count % 10;
  if (lastTwo >= 11 && lastTwo <= 14) return many;
  if (last === 1) return one;
  if (last >= 2 && last <= 4) return few;
  return many;
}

export function translateText(value: string, locale: Locale): string {
  if (locale === "en") return value;
  const leading = value.match(/^\s*/)?.[0] ?? "";
  const trailing = value.match(/\s*$/)?.[0] ?? "";
  const content = value.slice(leading.length, value.length - trailing.length);
  const normalized = content.replace(/\s+/g, " ");
  const exact = UK[normalized] ?? lazyTranslations[normalized];
  if (exact) return `${leading}${exact}${trailing}`;
  const pictureReason = /^The selected area was redacted by the operation, but text inside its pixels was not checked because Hexscope does not use OCR\.(?: (.*))?$/.exec(normalized);
  if (pictureReason) {
    const reason = "Вибрану ділянку зафарбовано, але текст у її пікселях не перевірено: Hexscope не розпізнає текст на зображеннях.";
    return `${leading}${reason}${pictureReason[1] ? ` ${translateText(pictureReason[1], locale)}` : ""}${trailing}`;
  }
  const coverage = /^Hexscope cannot yet confirm whether this ([A-Z]+) finding was removed\.$/.exec(normalized);
  if (coverage) return `${leading}Hexscope поки не може підтвердити, чи видалено цей факт у форматі ${coverage[1]}.${trailing}`;
  if (content && content !== value) return `${leading}${translateText(content, locale)}${trailing}`;
  let byteCount = /^(\d[\d,]*) bytes?$/.exec(value);
  if (byteCount) {
    const count = Number(byteCount[1].replaceAll(",", ""));
    return `${count.toLocaleString("uk")} ${ukrainianCountWord(count, "байт", "байти", "байтів")}`;
  }
  byteCount = /^Copied (\d[\d,]*) bytes? as (.+)\.$/.exec(value);
  if (byteCount) {
    const count = Number(byteCount[1].replaceAll(",", ""));
    const amount = `${count.toLocaleString("uk")} ${ukrainianCountWord(count, "байт", "байти", "байтів")}`;
    const format = byteCount[2] === "a C array" ? "масиву C" : translateText(byteCount[2], locale);
    return `Скопійовано ${amount} у форматі ${format}.`;
  }
  const entryCount = /^(\d+) entries?$/.exec(value);
  if (entryCount) {
    const count = Number(entryCount[1]);
    return `${count} ${ukrainianCountWord(count, "запис", "записи", "записів")}`;
  }
  const tourCount = /^(\d+) of (\d+)$/.exec(value);
  if (tourCount) return `Крок ${tourCount[1]} із ${tourCount[2]}`;
  let match = /^(Photo|Picture|Video|Recording|PDF|Email|File|Archive|Word document|WebAssembly module) · (.+)$/.exec(value);
  if (match) return `${translateText(match[1], locale)} · ${match[2]}`;
  match = /^This (photo|picture|video|recording|PDF|email|document|archive|file|module) gives away (\d+) things?$/.exec(value);
  if (match) {
    const count = Number(match[2]);
    return `${NOUNS[match[1]]} розкриває ${count} ${factWord(count)}`;
  }
  match = /^This (photo|picture|video|recording|PDF|email|document|archive|file|module) is damaged$/.exec(value);
  if (match) return `${NOUNS[match[1]]} пошкоджен${["photo", "picture", "video"].includes(match[1]) ? "е" : "ий"}`;
  match = /^This (photo|picture|video|recording|PDF|email|document|archive|file|module) does more than show pages$/.exec(value);
  if (match) return `${NOUNS[match[1]]} може виконувати дії, а не лише показувати сторінки`;
  match = /^Something is hidden in this (photo|picture|video|recording|PDF|email|document|archive|file|module)$/.exec(value);
  if (match) return `У файлі щось приховано`;
  match = /^Nothing personal found in this (photo|picture|video|recording|PDF|email|document|archive|file|module)$/.exec(value);
  if (match) return `Особистих даних у файлі не знайдено`;
  match = /^Reveals (.+)\.$/.exec(value);
  if (match) return `Розкриває ${translateRevealList(match[1])}.`;
  match = /^Tells you about its sender: (.+)\.$/.exec(value);
  if (match) return `Що лист розповідає про відправника: ${translateRevealList(match[1])}.`;
  match = /^Hidden in it: (.+)\.$/.exec(value);
  if (match) return `Приховано у файлі: ${match[1]}.`;
  match = /^Possible signs of impersonation: (.+)\.$/.exec(value);
  if (match) return `Можливі ознаки підміни відправника: ${match[1]}.`;
  match = /^Damaged: (\d+) (place|places) could not be read properly\. It may not open, or open only partly\.$/.exec(value);
  if (match) return `Пошкоджень: ${match[1]}. Частину даних не вдалося прочитати; файл може не відкритися або відкритися не повністю.`;
  match = /^Breaks (\d+) (rule|rules) of the format, usually harmlessly\.$/.exec(value);
  if (match) return `Порушує ${match[1]} ${ruleWord(Number(match[1]))} формату — зазвичай без наслідків.`;
  match = /^This (photo|picture|video|recording|PDF|email|document|archive|file|module) has the wrong name$/.exec(value);
  if (match) return `${NOUNS[match[1]]}: назва не відповідає вмісту`;
  match = /^Gives away (\d+) things?$/.exec(value);
  if (match) {
    const count = Number(match[1]);
    return `Розкриває ${count} ${factWord(count)}`;
  }
  match = /^(\d+)% of the file$/.exec(value);
  if (match) return `${match[1]}% файла`;
  match = /^([\d,]+) bytes of text, stored in ([\d,]+)$/.exec(value);
  if (match) return `${match[1]} байтів тексту, стиснених до ${match[2]} байтів`;
  match = /^Put together, these two ideas are all there is\. The ([\d,]+) bytes of text in the player take ([\d,]+): ([\d,]+) characters written out, and ([\d,]+) copies from earlier standing in for the other ([\d,]+) bytes\.$/.exec(value);
  if (match) return `Разом ці два принципи й утворюють весь алгоритм. Із ${match[1]} байтів тексту в плеєрі ${match[2]}: ${match[3]} символів записано без змін, а ще ${match[4]} копій замінюють решту ${match[5]} байтів.`;
  match = /^The demo could not load: (.+)$/.exec(value);
  if (match) return `Не вдалося завантажити демонстрацію: ${match[1]}`;
  match = /^The clean copy: the same picture, and none of that — (.+) bytes of it removed, in your browser\.$/.exec(value);
  if (match) return `Очищена копія: те саме зображення без цих даних — видалено ${match[1]} байтів у вашому браузері.`;
  match = /^Same photo\. ([\d,]+) bytes removed in your browser\.$/.exec(value);
  if (match) {
    const count = Number(match[1].replaceAll(",", ""));
    return `Те саме фото. У вашому браузері видалено ${count.toLocaleString("uk")} ${ukrainianCountWord(count, "байт", "байти", "байтів")}.`;
  }
  match = /^Scan 1 of (\d+): (\d+)% of the file, and the whole photo is there already — blurry, each block just its average\.$/.exec(value);
  if (match) return `Скан 1 із ${match[1]}: ${match[2]}% файла. Фото вже ціле, але розмите: кожен блок показує свій середній колір.`;
  match = /^All (\d+) scans: every detail\. hexscope shows which bytes draw which block, in each scan\.$/.exec(value);
  if (match) return `Усі скани (${match[1]}): усі деталі. hexscope показує, які байти формують кожен блок у кожному скані.`;
  match = /^Scan (\d+) of (\d+): (\d+)% of the file\. Each scan adds finer detail\.$/.exec(value);
  if (match) return `Скан ${match[1]} із ${match[2]}: ${match[3]}% файла. Кожен наступний скан додає деталі.`;
  match = /^Pixel (\d+), (\d+): a literal byte, 0x([\dA-F]+), spelled out in (\d+) bits\.$/.exec(value);
  if (match) return `Піксель ${match[1]}, ${match[2]}: байт 0x${match[3]} записано безпосередньо, у ${match[4]} бітах.`;
  match = /^Pixel (\d+), (\d+): part of a copy of (\d+) bytes from (.+) — (\d+) bits say so\.$/.exec(value);
  if (match) {
    const distance = match[4]
      .replace(/^exactly one row up$/, "рівно на один рядок вище")
      .replace(/^exactly (\d+) rows up$/, (_phrase, count: string) =>
        `рівно на ${count} ${ukrainianCountWord(Number(count), "рядок", "рядки", "рядків")} вище`,
      )
      .replace(/^(\d+) bytes back$/, (_phrase, count: string) =>
        `на ${count} ${ukrainianCountWord(Number(count), "байт", "байти", "байтів")} раніше`,
      );
    return `Піксель ${match[1]}, ${match[2]}: частина копії з ${match[3]} байтів, узятої ${distance}; це закодовано у ${match[5]} бітах.`;
  }
  match = /^What was removed · (.+)$/.exec(value);
  if (match) return `Що видалено · ${match[1]}`;
  match = /^A map with a pin where it was taken: (.+)$/.exec(value);
  if (match) return `Карта з позначкою місця зйомки: ${match[1]}`;
  match = /^(.+?) is open\. (.+)$/.exec(value);
  if (match) return `${match[1]} відкрито. ${match[2].split(/(?<=\.) /).map((line) => translateText(line, locale)).join(" ")}`;
  return value;
}

let activeLocale: Locale | undefined;
let installed = false;
const sourceText = new WeakMap<Text, { source: string; rendered: string }>();
const sourceAttributes = new WeakMap<Element, Map<string, { source: string; rendered: string }>>();
const reverse = new Map(Object.entries(UK).map(([english, ukrainian]) => [ukrainian, english]));

function storageValue(): string | null {
  try {
    return localStorage.getItem(LANGUAGE_KEY);
  } catch {
    return null;
  }
}

export function currentLocale(): Locale {
  if (activeLocale) return activeLocale;
  return resolveLocale(storageValue(), typeof navigator === "undefined" ? [] : [...navigator.languages, navigator.language]);
}

function localizeText(node: Text): void {
  const parent = node.parentElement;
  if (!parent || parent.closest("script, style, textarea, [data-language-control]")) return;
  let state = sourceText.get(node);
  if (!state || node.data !== state.rendered) {
    const source = reverse.get(node.data) ?? node.data;
    state = { source, rendered: node.data };
  }
  const translated = translateText(state.source, currentLocale());
  state.rendered = translated;
  sourceText.set(node, state);
  if (translated !== node.data) node.data = translated;
}

function localizeAttribute(element: Element, name: string): void {
  if (element.closest("[data-language-control]")) return;
  let values = sourceAttributes.get(element);
  if (!values) sourceAttributes.set(element, (values = new Map()));
  let state = values.get(name);
  const current = element.getAttribute(name) ?? "";
  if (!state || current !== state.rendered) {
    const source = reverse.get(current) ?? current;
    state = { source, rendered: current };
  }
  const translated = translateText(state.source, currentLocale());
  state.rendered = translated;
  values.set(name, state);
  if (translated !== element.getAttribute(name)) element.setAttribute(name, translated);
}

function localizeNode(root: Node): void {
  if (root.nodeType === Node.TEXT_NODE) {
    localizeText(root as Text);
    return;
  }
  if (!(root instanceof Element) || root.matches("script, style, textarea")) return;
  for (const element of [root, ...root.querySelectorAll("*")]) {
    if (element.matches("script, style, textarea")) continue;
    for (const name of ["aria-label", "title", "alt", "placeholder", "content"]) {
      if (element.hasAttribute(name)) localizeAttribute(element, name);
    }
  }
  const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
  let text: Text | null;
  while ((text = walker.nextNode() as Text | null)) localizeText(text);
}

function localizeDocument(): void {
  document.documentElement.lang = currentLocale();
  localizeNode(document.body);
  const title = document.querySelector("head title");
  if (title) localizeNode(title);
  for (const meta of document.querySelectorAll<HTMLMetaElement>('meta[name="description"], meta[property^="og:"]')) {
    localizeAttribute(meta, "content");
  }
}

export function setLocale(locale: Locale): void {
  activeLocale = locale;
  try {
    localStorage.setItem(LANGUAGE_KEY, locale);
  } catch {
    // The choice still applies for this page even when storage is disabled.
  }
  localizeDocument();
  window.dispatchEvent(new Event("hexscope:locale"));
}

/** Installs the saved/browser language and translates both static and later UI text. */
export function installLocale(): void {
  if (installed || typeof document === "undefined") return;
  installed = true;
  activeLocale = resolveLocale(storageValue(), typeof navigator === "undefined" ? [] : [...navigator.languages, navigator.language]);
  localizeDocument();
  new MutationObserver((records) => {
    for (const record of records) {
      if (record.type === "characterData") localizeText(record.target as Text);
      if (record.type === "attributes" && record.attributeName) localizeAttribute(record.target as Element, record.attributeName);
      if (record.type === "childList") for (const node of record.addedNodes) localizeNode(node);
    }
  }).observe(document.documentElement, {
    subtree: true,
    childList: true,
    characterData: true,
    attributes: true,
    attributeFilter: ["aria-label", "title", "alt", "placeholder"],
  });
}

export function languageButton(): HTMLButtonElement {
  const button = document.createElement("button");
  button.type = "button";
  button.className = "btn language-button";
  button.dataset.languageControl = "true";
  const refresh = () => {
    const target = currentLocale() === "en" ? "uk" : "en";
    button.textContent = target === "uk" ? "UA" : "EN";
    button.setAttribute("aria-label", target === "uk" ? "Перемкнути мову на українську" : "Switch language to English");
    button.title = target === "uk" ? "Перемкнути мову на українську" : "Switch language to English";
  };
  refresh();
  button.addEventListener("click", () => setLocale(currentLocale() === "en" ? "uk" : "en"));
  window.addEventListener("hexscope:locale", refresh);
  return button;
}
