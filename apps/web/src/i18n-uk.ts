// The Ukrainian words of the site: a dictionary keyed by the English, the
// patterns for sentences with numbers and names in them, and the way back.
// Loaded only when a page reads in Ukrainian (i18n.ts loadUkrainian), or
// imported where a build or a test needs it; either way it installs itself.
import { lazyReverse, lazyTranslations, useDictionary, type Locale } from "./i18n";

// Keep the source copy as the key: the English UI remains the canonical copy,
// and untranslated technical terms stay readable until they have a reviewed
// Ukrainian equivalent.
const UK: Record<string, string> = {
  "Process film scans · JPEG / TIFF": "Обробити плівкові скани · JPEG / TIFF",
  "Process film scans": "Обробити плівкові скани",
  "Published positive scans from NASA’s Apollo 11 archive.": "Оцифровані позитивні кадри з архіву Apollo 11 NASA.",
  "NASA Apollo 11 archive": "Архів NASA «Аполлон-11»",
  "NASA is not affiliated with hexscope.": "NASA не пов’язана з hexscope.",
  "APOLLO 11 · ARCHIVE": "АПОЛЛО-11 · АРХІВ",
  "Open frame AS11-40-5903 in hexscope →": "Відкрити кадр AS11-40-5903 у hexscope →",
  "4 distinct frames · 70 mm": "4 різні кадри · 70 мм",
  "See the synthetic frame-detection test": "Переглянути синтетичний тест пошуку кадру",
  "A simple illustration tests the detector; it is not a photograph.": "Проста ілюстрація перевіряє детектор; це не фотографія.",
  "Try a synthetic film negative →": "Спробувати синтетичний плівковий негатив →",
  "Where this data lives": "Де зберігаються ці дані",
  "In the file": "У файлі",
  "Sample file · example metadata": "Зразок файла · демонстраційні метадані",
  "JPEG · EXIF · demonstration values": "JPEG · EXIF · демонстраційні значення",
  "Apollo 11 frame AS11-40-5903: astronaut standing on the lunar surface": "Кадр Apollo 11 AS11-40-5903: астронавт стоїть на поверхні Місяця",
  "Apollo 11 frame AS11-40-5902: astronaut beside the lunar module": "Кадр Apollo 11 AS11-40-5902: астронавт біля місячного модуля",
  "Apollo 11 frame AS11-40-5875: astronaut and flag on the lunar surface": "Кадр Apollo 11 AS11-40-5875: астронавт і прапор на поверхні Місяця",
  "Apollo 11 frame KSC-AS11-40-5869: astronaut at the lunar module ladder": "Кадр Apollo 11 KSC-AS11-40-5869: астронавт біля трапа місячного модуля",
  "Photo privacy mosaic": "Мозаїка приватності фото",
  "View": "Вигляд",
  "Before": "До очищення",
  "Clean copy": "Очищена копія",
  "thing it gave away": "факт було розкрито",
  "things it gave away": "фактів було розкрито",
  "Checking the copy…": "Перевіряємо копію…",
  "Copy not checked": "Копію не перевірено",
  "Confirmed removed": "прибрано й перевірено",
  "Findings remain in the copy": "У копії залишилися дані",
  "Copy not fully checked": "Копію перевірено не повністю",
  "Write to the developer": "Написати розробнику",
  "Support the project": "Підтримати проєкт",
  // What every article adds for itself (page.ts), on a /uk/ page too, where the articles' own copy is not loaded.
  "On this page": "На цій сторінці",
  // The ways to support, which the support page draws for itself (support.ts).
  "From anywhere: a card or PayPal, once or every month.": "Звідусіль: картка чи PayPal, разово або щомісяця.",
  "Monobank jar": "Банка monobank",
  "A card from any Ukrainian bank, Apple Pay or Google Pay.": "Картка будь-якого банку, Apple Pay чи Google Pay.",
  "Something is wrong": "Щось не так",
  "An idea": "Ідея",
  "Thanks": "Подяка",
  "What it is about": "Про що повідомлення",
  "Your message": "Ваше повідомлення",
  "How to answer you": "Як вам відповісти",
  "What happened, and what you expected. Which file it was, in words — the file itself is never sent.": "Що сталося і що ви очікували. Що це був за файл — словами: сам файл ніколи не надсилається.",
  "What would make hexscope more useful to you?": "Що зробило б hexscope кориснішим для вас?",
  "A word for the developer.": "Кілька слів розробнику.",
  "Email or Telegram, if you want an answer — optional": "Email або Telegram, якщо хочете відповідь, — не обов’язково",
  "Attach the open file's structure — its layout, none of its content": "Додати структуру відкритого файла — будову, без жодного вмісту",
  "See exactly what is attached": "Подивитися, що саме буде додано",
  "Also sent: the version of hexscope, your browser and system in two words, and this page's address. Never your file, never your IP address.": "Також надсилається: версія hexscope, ваш браузер і система двома словами та адреса цієї сторінки. Ніколи — ваш файл чи IP-адреса.",
  "Send": "Надіслати",
  "hexscope is made by one person. A problem you met, an idea, a word of thanks — it is read, and it makes the next version better.": "hexscope робить одна людина. Проблема, ідея чи подяка — усе прочитаю, і наступна версія стане кращою.",
  "Write a few words first.": "Спершу напишіть кілька слів.",
  "Sending…": "Надсилаємо…",
  "Sent. Thank you — it means a lot.": "Надіслано. Дякую — це дуже приємно.",
  "Sent. Thank you — it will be read.": "Надіслано. Дякую — обов’язково прочитаю.",
  "Not sent.": "Не надіслано.",
  "There is no connection.": "Немає з’єднання.",
  "Too many messages today. Try again tomorrow.": "Сьогодні надіслано забагато повідомлень. Спробуйте завтра.",
  "The site could not take it right now.": "Сайт зараз не зміг прийняти повідомлення.",
  "Write on GitHub instead ↗": "Написати на GitHub ↗",
  "Send to the developer": "Надіслати розробнику",
  "Report a problem with this file": "Повідомити про проблему з цим файлом",
  "Copy": "Копіювати",
  "Copied": "Скопійовано",
  "Open an issue on GitHub ↗": "Створити issue на GitHub ↗",
  "This is the file's layout — its parts, where they are and how big — and none of what it says. Read it before you share it: nothing is sent until you choose to send it.": "Це будова файла — його частини, де вони і якого розміру, — без жодного вмісту. Перегляньте її, перш ніж ділитися: нічого не надсилається, доки ви цього не захочете.",
  "Something wrong or missing? Write to the developer": "Щось не так або чогось бракує? Напишіть розробнику",
  "hexscope is free, with no ads and no tracking, and made by one person. If it helped,": "hexscope безкоштовний, без реклами й стеження, і його робить одна людина. Якщо він допоміг,",
  "you can thank them": "можна віддячити",
  "Do not show this again": "Більше не показувати",
  "No findings found in the copy": "У копії нічого не знайдено",
  "removed — nothing is left in the copy": "прибрано — у копії нічого не лишилося",
  "Compare with the original": "Порівняти з оригіналом",
  "File A": "Файл A",
  "File B": "Файл B",
  "Preview of file A": "Попередній перегляд файла A",
  "Preview of file B": "Попередній перегляд файла B",
  "Finding summary": "Зведення знахідок",
  "Only in A": "Лише у файлі A",
  "Only in B": "Лише у файлі B",
  "In both": "В обох файлах",
  "Neither file gives anything away.": "У жодному файлі не виявлено знахідок.",
  "File issues": "Проблеми у файлах",
  "Problems": "Проблеми",
  "Damage": "Пошкодження",
  "Same parts": "Однакові частини",
  "Changed parts": "Змінені частини",
  "Different": "Відмінні",
  "The bytes are the same.": "Байти збігаються.",
  "Inspect byte differences": "Переглянути відмінності байтів",
  "Close comparison": "Закрити порівняння",
  "The first difference is in": "Перша відмінність у",
  "Empty cells are past the end of a file.": "Порожні клітинки — за кінцем файла.",
  "and": "і",
  "more": "ще",
  "What the copy took out, part by part": "Що саме копія видалила",
  "What the clean copy took out": "Що очищена копія видалила",
  "Save receipt as JSON": "Зберегти звіт як JSON",
  "Downloads the batch receipt as a separate file.": "Завантажує звіт пакета як окремий файл.",
  "The ZIP could not include the receipt; save it separately.": "Звіт не вмістився у ZIP. Завантажте його окремо.",
  "Summary": "Зведення",
  "Overview": "Огляд",
  "Metadata": "Метадані",
  "Content": "Вміст",
  "Compression": "Стиснення",
  "File workspace": "Робоча область файла",
  "Bytes": "Байти",
  "Contains location": "Містить геолокацію",
  "This photo records where it was taken — click to see it": "У фото записано місце зйомки — натисніть, щоб побачити",
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
  "Hide details": "Згорнути деталі",
  "Files": "Файли",
  "Start": "Початок",
  "See what your file reveals about you": "Дізнайтеся, що файл розкриває про вас",
  "Check metadata, hidden content and file structure right in your browser.": "Перевірте метадані, прихований вміст і структуру файла прямо у браузері.",
  "See examples": "Переглянути приклади",
  "Different files, different findings": "Різні файли — різні знахідки",
  "Try a photo, document or email sample to see what can be hidden inside.": "Виберіть приклад фото, документа чи листа, щоб побачити, що може бути приховано всередині.",
  "Document": "Документ",
  "Film scans": "Плівкові скани",
  "A real file, in the real analyzer": "Справжній файл у справжньому аналізаторі",
  "Go deeper": "Зазирнути глибше",
  "Follow one sample from a plain-language finding to the file structure and its bytes.": "Простежте шлях від зрозумілого опису знахідки до структури файла та його байтів.",
  "Read the finding": "Прочитайте знахідку",
  "See what the file reveals, explained in plain language.": "Дізнайтеся простою мовою, що розкриває файл.",
  "Trace the structure": "Простежте структуру",
  "Switch views to see how the file format is built.": "Перемикайте вигляд, щоб побачити, як улаштований формат файла.",
  "Inspect the bytes": "Перегляньте байти",
  "Open the raw data behind the selected part.": "Перейдіть до вихідних даних вибраної частини.",
  "Try this sample in hexscope →": "Перевірити цей приклад у hexscope →",
  "hexscope workspace": "Робоча область hexscope",
  "photo.jpg · sample analysis": "photo.jpg · аналіз прикладу",
  "hexscope analyzing a sample photo": "hexscope аналізує тестове фото",
  "hexscope analyzing a sample photo across its finding, structure and bytes": "hexscope аналізує тестове фото: знахідка, структура й байти",
  "Apollo 11 film scan contact sheet": "Контактний аркуш скану плівки «Аполлона-11»",
  "The DEFLATE player": "Програвач DEFLATE",
  "The word 'byte' written once, then a back-reference pointing ten bytes back instead of writing it again.": "Слово «byte» записане один раз, а далі замість повтору — посилання на десять байтів назад.",
  "Examples": "Приклади",
  "Formats": "Формати",
  "Menu": "Меню",
  "Local processing.": "Обробка локально.",
  "Nothing is uploaded; your file stays in this tab.": "Нічого не передається: файл залишається в цій вкладці.",
  "Works offline once opened.": "Після першого відкриття працює й без мережі.",
  "The file is read by WebAssembly in your browser: the Network tab of its developer tools shows that no file is sent.": "Файл читає WebAssembly у вашому браузері: вкладка «Мережа» в інструментах розробника покаже, що файл нікуди не надсилається.",
  "No account.": "Без облікового запису.",
  "No tracking.": "Без відстеження.",
  "Open source.": "Відкритий код.",
  "that reads your file.": "який читає ваш файл.",
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
  "BEFORE YOU SEND": "ПЕРЕД НАДСИЛАННЯМ",
  "Before you send": "Перед надсиланням",
  "Specialist tools": "Спеціальні інструменти",
  "Inspect a set frame by frame, then process local JPEG or TIFF copies in your browser.": "Переглядайте кадри окремо й обробляйте локальні копії JPEG або TIFF у браузері.",
  "Choose multiple photos for a privacy mosaic →": "Вибрати кілька фото для мозаїки приватності →",
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
  "Metadata fields": "Поля метаданих",
  "Findings only in A": "Знахідки лише у файлі A",
  "Findings only in B": "Знахідки лише у файлі B",
  "Findings in both": "Знахідки в обох файлах",
  "Parts only in A": "Частини лише у файлі A",
  "Parts only in B": "Частини лише у файлі B",
  "What they give away": "Що розкривають файли",
  "Where it was taken": "Де зроблено знімок",
  "The camera's serial number": "Серійний номер камери",
  "The owner's name": "Ім’я власника",
  "The camera": "Камера",
  "The lens": "Об’єктив",
  "When it was taken": "Коли зроблено знімок",
  "The software used": "Програма обробки",
  "A hidden preview of the picture": "Прихований ескіз зображення",
  "Text that was deleted": "Видалений текст",
  "Where its photos were taken": "Місця зйомки у фотографіях",
  "Who wrote it": "Автор",
  "Who saved it last": "Хто востаннє зберіг",
  "The company": "Компанія",
  "Who commented": "Автор коментарів",
  "Who changed what": "Хто й що змінив",
  "When it was written": "Коли створено",
  "Its title": "Назва",
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
  "a photo with the wrong name": "фото з неправильною назвою",
  "a LibreOffice document": "документ LibreOffice",
  "an iPhone photo": "фото з iPhone",
  "a Wi-Fi QR code": "QR-код Wi-Fi",
  "an AI-made picture": "картинка, створена ШІ",
  "an iPhone video": "відео з iPhone",
  "a Word document": "документ Word",
  "a spreadsheet": "електронна таблиця",
  "an edited PDF": "відредагований PDF",
  "an email": "лист",
  "an Outlook message": "лист Outlook",
  "an old Word file": "старий файл Word",
  "a progressive JPEG": "прогресивний JPEG",
  "a WebAssembly module": "модуль WebAssembly",
  "Open source · no accounts · no tracking · no uploads": "Відкритий код · без облікових записів · без стеження · без передавання файлів",
  "How your files stay private": "Як ми захищаємо ваші файли",
  "Source on GitHub": "Код на GitHub",
  "Or press ? anywhere": "Або натисніть ? будь-де",
  "Keyboard shortcuts": "Сполучення клавіш",
  "Take the tour": "Показати огляд",
  "Drop to open": "Перетягніть, щоб відкрити",
  "Parsing…": "Читаємо файл…",
  "The short answer": "Короткий висновок",
  "THE SHORT ANSWER": "КОРОТКИЙ ВИСНОВОК",
  "Name mismatch": "Невідповідна назва",
  "Hidden content": "Прихований вміст",
  "Partial check": "Не все вдалося перевірити",
  "Personal information": "Особисті дані",
  "Format note": "Особливість формату",
  "File health": "Справність файла",
  "File type": "Тип файла",
  "Reveals where it was taken, the camera, the camera's serial number and the owner's name.": "Розкриває місце зйомки, фотоапарат, його серійний номер та ім’я власника.",
  "The file itself is fine: nothing in it is damaged.": "Файл справний: пошкоджень не знайдено.",
  "The file is fine: nothing in it is damaged or out of place.": "Файл справний: пошкоджень і невідповідностей не знайдено.",
  "Found by reading the file's structure. It is not a virus scan.": "Висновок зроблено за структурою файла. Це не антивірусна перевірка.",
  "Show me": "Показати",
  "Find in file": "Знайти у файлі",
  "Go to this finding in the file": "Перейти до цього факту у файлі",
  "This guide is currently available in English. The page controls are in Ukrainian.": "Цей посібник поки доступний лише англійською. Елементи керування сторінкою перекладені українською.",
  "Show where in the file this is": "Показати це місце у файлі",
  "Remove it — save a clean copy": "Зберегти копію без цих даних",
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
  "hexscope could not completely read the copy, so its findings were not checked.": "hexscope не зміг повністю прочитати копію, тому факти в ній не перевірено.",
  "No comparable findings were available to check.": "Не було фактів, які можна було порівняти.",
  "No searchable text was available for this selection.": "У вибраному місці не було доступного для пошуку тексту.",
  "The source page was incomplete, so this selection could not be checked.": "Вихідну сторінку прочитано не повністю, тому вибраний текст не перевірено.",
  "The corresponding output page could not be checked.": "Не вдалося перевірити відповідну сторінку копії.",
  "The output page was incomplete, so this selection could not be checked.": "Сторінку копії прочитано не повністю, тому вибраний текст не перевірено.",
  "The selected area was redacted by the operation, but text inside its pixels was not checked because hexscope does not use OCR.": "Вибрану ділянку зафарбовано, але текст у її пікселях не перевірено: hexscope не розпізнає текст на зображеннях.",
  "Kept: the QR code, which is part of the picture. Black it out with “Black out part of the picture” before sending.": "Залишено QR-код, бо він є частиною зображення. Перед надсиланням зафарбуйте його інструментом «Приховати частину зображення».",
  "Kept: form answers, links, and anything it does when opened, which are part of the document, and files an e-invoice or a PDF/A-3 file names as part of itself. Delete them in a PDF editor — or print to a new PDF, which keeps only the pages — if they should not travel with it.": "Залишено відповіді у формах, посилання та дії під час відкриття — це частини документа, а також файли, які e-інвойс чи файл PDF/A-3 називає своєю частиною. Якщо їх не слід надсилати, видаліть їх у редакторі PDF або надрукуйте новий PDF лише зі сторінками.",
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
  "See what hexscope can open. What it can inspect or change depends on the format.": "Дізнайтеся, які файли може відкрити hexscope. Можливості перевірки й очищення залежать від формату.",
  "Photos & images": "Фото й зображення",
  "Documents & Office": "Документи й файли Office",
  "Email, archives & technical files": "Електронна пошта, архіви й технічні файли",
  "TIFF scans work in Film Lab.": "Плівкові скани TIFF можна обробити у Film Lab.",
  "Includes MOV and M4V files.": "Зокрема файли MOV і M4V.",
  "Older Word, Excel and PowerPoint formats, and LibreOffice's, included.": "Також підтримуються старі формати Word, Excel і PowerPoint і документи LibreOffice.",
  "Common file types inside ZIP archives are supported too.": "Також підтримуються поширені типи файлів усередині ZIP-архівів.",
  "Image formats": "Формати зображень",
  "Video formats": "Відеоформати",
  "Document formats": "Формати документів",
  "Email, archive and technical formats": "Формати електронної пошти, архівів і технічних файлів",
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
  "General information": "Основна інформація",
  "Camera & device": "Камера й пристрій",
  "Dates": "Дати",
  "Author & software": "Автор і програма",
  "Additional fields": "Додаткові поля",
  "Dimensions": "Розміри",
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
  "The selection still contains searchable text.": "У виділенні досі є текст, який можна знайти пошуком.",
  "Review the file before sharing.": "Перевірте файл, перш ніж ділитися ним.",
  "Film Lab": "Плівкова лабораторія",
  "— the structure beside the hex, every part with its spec reference": "— структура поруч із hex, кожна частина з посиланням на специфікацію",
  "— text or hex, and the part of the file it falls in": "— текст або hex і частина файла, де він трапляється",
  "— two files, or a file and its clean copy": "— два файли або файл і його очищена копія",
  "— the whole structure, with its explanations": "— уся структура з поясненнями",
  "— the same checks in": "— ті самі перевірки в",
  "Remove the location from a photo": "Як прибрати геолокацію з фото",
  "Black out a PDF properly": "Як правильно замалювати PDF",
  "What an edited PDF still holds": "Що досі зберігає відредагований PDF",
  "Why a PNG won't open": "Чому PNG не відкривається",
  "How DEFLATE works, step by step": "Як працює DEFLATE, крок за кроком",
  "hexscope — see what your file reveals before you send it": "hexscope — дізнайтеся, що файл розкриває, перш ніж його надіслати",
  "View and remove metadata from photos and PDFs — hexscope": "Перегляд і видалення метаданих фото та PDF — hexscope",
  "Free EXIF and metadata viewer and remover. See a photo's location and camera, a PDF's author and hidden text, a Word file's comments — and save a clean copy. Nothing is uploaded.": "Безкоштовний перегляд і видалення EXIF та метаданих. Дізнайтеся, де зроблено фото й якою камерою, хто автор PDF і що в ньому приховано, чиї коментарі у файлі Word, — і збережіть очищену копію. Нічого не передається.",
  "Check what a photo or document gives away before you send it — its location, the owner's name, text under black boxes, comments — and save a clean copy. In your browser; nothing is uploaded.": "Перевірте, що фото чи документ розкриває, перш ніж його надіслати: геолокацію, ім’я власника, текст під чорними прямокутниками, коментарі — і збережіть очищену копію. Прямо в браузері, нічого не завантажується.",
  "Where your photo was taken, who wrote your document, why a file won't open — shown byte by byte, in your browser. Nothing is uploaded.": "Де зроблено фото, хто написав документ, чому файл не відкривається — байт за байтом, у вашому браузері. Нічого не завантажується.",
  "hexscope: a sample photo and what it reveals — where it was taken, the camera's serial number, the owner and the date.": "hexscope: зразок фото і що воно розкриває — місце зйомки, серійний номер фотоапарата, власника й дату.",
  "Subject": "Тема",
  "Description": "Опис",
  "Category": "Категорія",
  "Status": "Статус",
  "Manager": "Керівник",
  "Sensitivity label": "Мітка конфіденційності",
  "Custom properties": "Власні властивості",
  "Downloaded from": "Звідки завантажено",
  "Saved in": "Збережено в теці",
  "Finder tags": "Теги Finder",
  "Printer": "Принтер",
  "Printed": "Надруковано",
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
  "Save it": "Зберегти",
  // Several files at once.
  "Nothing leaves this tab.": "Ніщо не залишає цієї вкладки.",
  "No personal metadata found by the available checks. Visible content has not been cleared.": "Доступні перевірки не знайшли особистих метаданих. Видимий вміст не очищено.",
  "No personal metadata found by the available checks.": "Доступні перевірки не знайшли особистих метаданих.",
  "Make clean copies": "Зробити чисті копії",
  "Save clean copies (.zip)": "Зберегти чисті копії (.zip)",
  "Makes the copies in this tab, to share or save: nothing is uploaded": "Копії робляться в цій вкладці — щоб поділитися чи зберегти; нічого не завантажується",
  "Makes the copies in this tab and saves them as one ZIP: nothing is uploaded": "Копії робляться в цій вкладці й зберігаються одним ZIP; нічого не завантажується",
  "Keep the original names": "Зберегти початкові назви",
  "More tools": "Ще інструменти",
  "Show": "Показати",
  "Open this file": "Відкрити цей файл",
  "Still reading": "Ще читаю",
  "Making the clean copies…": "Роблю чисті копії…",
  "ZIP includes an index-only JSON receipt.": "У ZIP додано звіт JSON — лише номери файлів, без самих даних.",
  "Visible content is not cleared by these metadata checks.": "Те, що видно на фото чи сторінці, ці перевірки метаданих не очищають.",
  "Save as a ZIP": "Зберегти як ZIP",
  "Nothing to clean.": "Нічого чистити.",
  "Copy not created · unchecked": "Копію не створено · не перевірено",
  "Install the app": "Встановити застосунок",
  "Install hexscope on this iPhone": "Встановіть hexscope на цей iPhone",
  "Install hexscope on this iPad": "Встановіть hexscope на цей iPad",
  "It opens from the home screen like an app, without the browser's bars, and works offline once opened.": "Він відкриватиметься з екрана «Додому», як застосунок, без панелей браузера, і працюватиме без інтернету після першого відкриття.",
  "Tap Share — the square with an arrow. If it is not in view, tap ••• or ≡ beside the address first.": "Натисніть «Поділитися» — квадрат зі стрілкою. Якщо його не видно, спершу натисніть ••• або ≡ біля адреси.",
  "Choose Add to Home Screen. Scroll down the list if it is not in view.": "Виберіть «На екран „Додому“». Якщо пункту не видно, прогорніть список донизу.",
  "Tap Add.": "Натисніть «Додати».",
  "Got it": "Зрозуміло",
  // A file found inside another, as the list of them names it.
  "A 7-Zip archive": "Архів 7-Zip",
  "A GIF picture": "Зображення GIF",
  "A JPEG picture": "Зображення JPEG",
  "A PDF document": "Документ PDF",
  "A PNG picture": "Зображення PNG",
  "A RAR archive": "Архів RAR",
  "A WebAssembly module": "Модуль WebAssembly",
  "A WebP picture": "Зображення WebP",
  "A ZIP archive": "Архів ZIP",
  "A gzip archive": "Архів gzip",
  "An ELF program": "Програма ELF",
  "An MP3 file": "Файл MP3",
  "An MP4, HEIC or QuickTime file": "Файл MP4, HEIC або QuickTime",
  "An Ogg audio or video file": "Аудіо чи відео Ogg",
  "Kept only the orientation, so the picture stays the right way up.": "Залишено лише орієнтацію, щоб фото не перевернулося.",
  "Colours": "Кольори",
  "identifier": "ідентифікатор",
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
  "Some PDF page or form content could not be fully checked. Search may miss text.": "Не весь вміст сторінок або форм у PDF вдалося перевірити. Пошук може пропустити текст.",
  "PDF not fully checked": "PDF перевірено не повністю",
  "Some PDF page or form content may be missing.": "У PDF може бракувати частини вмісту сторінки або форми.",
  "no copy was made because PDF checks are incomplete": "Копію не створено: перевірку PDF не завершено",
  "no copy was made because PDF checks are incomplete.": "Копію не створено: перевірку PDF не завершено.",
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
  "The answer first, in plain words: damaged, hiding something, giving something away, or healthy. “Find in file” takes you to the fact in the summary, or to its bytes when there isn't one.": "Спершу коротка відповідь: файл пошкоджений, щось приховує, розкриває дані чи справний. «Знайти у файлі» веде до факту у зведенні, а якщо його там немає — до байтів.",
  "Places, names, serial numbers, deleted text. “Remove it — save a clean copy” makes a copy without them, here in the tab: nothing is uploaded.": "Місця, імена, серійні номери, видалений текст. «Зберегти копію без цих даних» створює копію тут, у вкладці. Нічого не передається.",
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
  "How to remove the location from a photo": "Як прибрати геолокацію з фото",
  "What a screenshot gives away": "Що розкриває знімок екрана",
  "How to black out a PDF properly": "Як правильно приховати дані в PDF",
  // The DEFLATE player, on its own page and in the analyzer.
  "Back to the start (Home)": "На початок (Home)",
  "Step back (←)": "Крок назад (←)",
  "Play or pause (Space)": "Відтворити чи призупинити (пробіл)",
  "Step forward (→)": "Крок уперед (→)",
  "Close (Esc)": "Закрити (Esc)",
  "Click to jump": "Клацніть, щоб перейти",
  "Loading the stream…": "Завантажуємо потік…",
  "Fetching steps…": "Отримуємо кроки…",
  "New block": "Новий блок",
  "— stored: the bytes that follow are copied in as they are.": "— без стиснення: наступні байти копіюються як є.",
  "— fixed Huffman: codes come from the table built into the format.": "— сталий Гаффман: коди беруться з таблиці, вбудованої у формат.",
  "Literal": "Літерал",
  "Back-reference": "Зворотне посилання",
  "The copy overlaps what it is writing, so it repeats a pattern — this is how long runs compress to a few bits.": "Копія накладається на те, що вона пише, тож повторює візерунок — так довгі повтори стискаються до кількох бітів.",
  "End of block": "Кінець блока",
  "— an end-of-block code closes it.": "— код кінця блока його закриває.",
  "Decoding stops here": "Тут декодування зупиняється",
  "— the compressed data is damaged. Everything before this point decoded; nothing after it can.": "— стиснені дані пошкоджено. Усе до цього місця декодовано; після нього — нічого.",
  "Stored block: no codes — its bytes are copied as they are.": "Блок без стиснення: кодів немає — його байти копіюються як є.",
  "Literal/length codes": "Коди літералів і довжин",
  "Distance codes": "Коди відстаней",
  "literals in hex · 256 ends the block · 257+ are lengths": "літерали в hex · 256 закінчує блок · 257+ — довжини",
  "symbols are distance codes": "символи — коди відстаней",
  "BFINAL · last block": "BFINAL · останній блок",
  "BFINAL · more follow": "BFINAL · далі ще блоки",
  "padding to a byte": "доповнення до байта",
  "NLEN · LEN inverted": "NLEN · інвертована LEN",
  "code-length code": "код довжин кодів",
  "end of block": "кінець блока",
  "fixed Huffman": "сталий Гаффман",
  "dynamic Huffman": "динамічний Гаффман",
  "the input ran out": "вхідні дані скінчилися",
  "no code in this block's table starts with these bits": "жоден код у таблиці цього блока не починається з цих бітів",
  "the code lengths do not form a valid code": "довжини кодів не утворюють правильного коду",
  "the distance reaches before the start of the output": "відстань сягає раніше за початок виведених даних",
  "the symbol is not defined in DEFLATE": "такого символу в DEFLATE немає",
  "the zlib header does not describe DEFLATE": "заголовок zlib не описує DEFLATE",
  "the checksum does not match": "контрольна сума не збігається",
  "the output would exceed the size limit": "виведені дані перевищили б обмеження розміру",
  "the decoder could not resume here": "декодер не зміг продовжити звідси",
  "decoding failed": "декодування не вдалося",
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
  "the company": "компанію",
  "when it was written": "дату створення",
  "how long it was worked on": "тривалість роботи над ним",
  "earlier versions of itself": "попередні версії",
  "what was filled into its form": "дані, внесені у форму",
  "files attached to it": "вкладені файли",
  "whole files kept inside it": "цілі файли всередині",
  "sheets hidden from view": "приховані аркуші",
  "hidden rows and columns": "приховані рядки й стовпці",
  "paths on the author's computer": "шляхи на комп’ютері автора",
  "the speaker's notes": "нотатки доповідача",
  "hidden slides": "приховані слайди",
  "text an edit took off the page": "текст, прибраний зі сторінки під час редагування",
  "the camera behind its photos": "фотоапарат, яким зроблено фото всередині",
  "who commented": "авторів коментарів",
  "who changed what": "хто що змінював",
  "how many photos the camera has taken": "кількість кадрів, знятих фотоапаратом",
  "IDs that link it to other shots": "ідентифікатори, що пов’язують його з іншими кадрами",
  "how long the phone had been on": "як довго телефон був увімкнений",
  "the user name of the computer that built it": "ім’я користувача комп’ютера, на якому його зібрано",
  "its functions' names": "назви його функцій",
  "the compiler that built it": "компілятор, яким його зібрано",
  "that it is a screenshot": "що це знімок екрана",
  "the prompt it was made from": "запит, за яким його створено",
  "that AI made it": "участь ШІ",
  "who or what made it": "хто або що його створило",
  "the mail app": "поштову програму",
  "the sender's time zone": "часовий пояс відправника",
  "where its source is": "де лежить його вихідний код",
  "debug info from its source": "налагоджувальні дані з вихідного коду",
  "a Wi-Fi password, in a QR code": "пароль Wi-Fi у QR-коді",
  "a two-factor secret, in a QR code": "секрет двофакторної автентифікації у QR-коді",
  "a place, in a QR code": "місце в QR-коді",
  "contact details, in a QR code": "контактні дані в QR-коді",
  "a QR code that hides where it goes": "QR-код, що приховує, куди веде",
  "where a QR code leads": "куди веде QR-код",
  "a place in a QR code": "місце в QR-коді",
  "where its links go": "куди ведуть його посилання",
  "contact details in a QR code": "контактні дані в QR-коді",
  "where a QR code in it leads": "куди веде QR-код у ньому",
  "what a QR code in it says": "що містить QR-код у ньому",
  "where bounces go": "куди повертаються недоставлені листи",
  "its caption": "підпис",
  "the lens": "об’єктив",
  "when it was last changed": "дату останньої зміни",
  "how many times it was saved": "скільки разів його зберігали",
  "its editing history": "історію редагування",
  "the software used": "використану програму",
  "the program it was written in": "програму, у якій його написано",
  "the program that made the PDF": "програму, яка створила PDF",
  "the template it came from": "шаблон, з якого його створено",
  "its title": "назву",
  "its subject": "тему",
  "its keywords": "ключові слова",
  "a hidden preview of the picture": "приховане прев’ю зображення",
  "the language it was written in": "мову, якою його написано",
  "what it asks its host to do": "що він просить зробити середовище",
  "where its source map is": "де лежить його карта коду",
  "where its files were downloaded from": "звідки завантажено файли",
  "the folder it was saved in": "теку, де його збережено",
  "its Finder tags": "теги Finder",
  "the organisation it belongs to": "організацію, якій він належить",
  "its manager's name": "ім’я керівника",
  "the printer's name": "назву принтера",
  "its custom properties": "власні властивості",
  "who printed it": "хто його друкував",
  "the printer it was printed on": "принтер, на якому його друкували",
  "who printed it and when": "хто й коли його друкував",
  "its description": "опис",
};

/** The same kinds of finding standing alone, as a list's heading names them: "Where replies really go". */
const HEADINGS: Record<string, string> = {
  "text that was blacked out but not removed": "текст, який зачорнили, але не прибрали",
  "text hidden from view": "прихований текст",
  "where it was taken": "місце зйомки",
  "a Wi-Fi password in a QR code": "пароль Wi-Fi у QR-коді",
  "a two-factor secret in a QR code": "секрет двофакторної автентифікації в QR-коді",
  "a QR code that hides where it goes": "QR-код, що приховує, куди веде",
  "a place in a QR code": "місце в QR-коді",
  "that its sender's domain did not vouch for it": "домен відправника не підтвердив лист",
  "where replies really go": "справжня адреса для відповідей",
  "a link that says one site and goes to another": "посилання, що веде не туди, куди обіцяє",
  "an address in lookalike letters": "адреса зі схожими літерами",
  "an attachment that runs or opens a site": "вкладення, що запускає програму або відкриває сайт",
  "the address it was sent from": "адреса, з якої надіслано лист",
  "the sender's computer's name": "назва комп’ютера відправника",
  "where its files were downloaded from": "звідки завантажено його файли",
  "the folder it was saved in": "тека, де його збережено",
  "earlier versions of itself": "його попередні версії",
  "text that was deleted": "видалений текст",
  "text an edit took off the page": "текст, прибраний зі сторінки під час редагування",
  "what was filled into its form": "що вписано в його форму",
  "files attached to it": "вкладені файли",
  "where its links go": "куди ведуть його посилання",
  "contact details in a QR code": "контакти в QR-коді",
  "where a QR code in it leads": "куди веде QR-код усередині",
  "what a QR code in it says": "що каже QR-код усередині",
  "where bounces go": "куди йдуть листи-відмови",
  "the mail app": "поштова програма",
  "the sender's time zone": "часовий пояс відправника",
  "whole files kept inside it": "цілі файли всередині",
  "sheets hidden from view": "приховані аркуші",
  "hidden rows and columns": "приховані рядки й стовпці",
  "the speaker's notes": "нотатки доповідача",
  "hidden slides": "приховані слайди",
  "paths on the author's computer": "шляхи на комп’ютері автора",
  "where its photos were taken": "де зроблено фото всередині",
  "the camera's serial number": "серійний номер фотоапарата",
  "the owner's name": "ім’я власника",
  "who wrote it": "хто його написав",
  "who saved it last": "хто зберіг його останнім",
  "its manager's name": "ім’я керівника",
  "the company": "компанія",
  "the organisation it belongs to": "організація, якій він належить",
  "who commented": "хто коментував",
  "who changed what": "хто що змінював",
  "the camera behind its photos": "фотоапарат, яким зроблено фото всередині",
  "the camera": "фотоапарат",
  "how many photos the camera has taken": "скільки знімків зробив фотоапарат",
  "IDs that link it to other shots": "ідентифікатори, що пов’язують його з іншими знімками",
  "how long the phone had been on": "скільки часу телефон був увімкнений",
  "the place it names": "згадане місце",
  "the file it was made from": "файл, з якого його зроблено",
  "its caption": "підпис",
  "the lens": "об’єктив",
  "when it was taken": "коли зроблено знімок",
  "when it was written": "коли його написано",
  "when it was last changed": "коли його востаннє змінювали",
  "how long it was worked on": "скільки часу над ним працювали",
  "how many times it was saved": "скільки разів його зберігали",
  "its editing history": "історія редагування",
  "the prompt it was made from": "запит, з якого його створено",
  "that AI made it": "що його створив ШІ",
  "who or what made it": "хто або що його створило",
  "the software used": "використана програма",
  "the program it was written in": "програма, у якій його написано",
  "the program that made the PDF": "програма, що створила PDF",
  "the template it came from": "шаблон, з якого його створено",
  "its custom properties": "власні властивості",
  "the printer it was printed on": "принтер, на якому його друкували",
  "who printed it and when": "хто й коли його друкував",
  "its title": "назва",
  "its subject": "тема",
  "its keywords": "ключові слова",
  "its description": "опис",
  "its Finder tags": "теги Finder",
  "a hidden preview of the picture": "прихований попередній перегляд зображення",
  "the user name of the computer that built it": "ім’я користувача комп’ютера, де його зібрано",
  "its functions' names": "назви його функцій",
  "the compiler that built it": "компілятор, яким його зібрано",
  "the language it was written in": "мова, якою його написано",
  "what it asks its host to do": "що він просить у середовища",
  "where its source map is": "де лежить карта коду",
  "debug info from its source": "налагоджувальні дані з вихідного коду",
  "may not be from who it says": "може бути не від того, ким назвався",
};

function translateRevealList(value: string): string {
  const vocabulary = Object.keys(REVEAL_WORDS).sort((a, b) => b.length - a.length);
  const parts: string[] = [];
  let rest = value;
  while (rest) {
    // "… and 4 more things": the count of the rest, last in the list.
    const more = /^(\d+) more things?$/.exec(rest);
    if (more) {
      parts.push(`ще ${more[1]}`);
      break;
    }
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
  // "а, б і в", as the English says "a, b and c".
  return parts.length < 2 ? parts.join("") : `${parts.slice(0, -1).join(", ")} і ${parts[parts.length - 1]}`;
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

/** What a misnamed file is, as the line about its name says it. */
const KIND_IN_WORDS: Record<string, string> = {
  "a JPEG": "JPEG",
  "a PNG": "PNG",
  "a HEIF image (HEIC or AVIF)": "зображення HEIF (HEIC або AVIF)",
  "a HEIC image": "зображення HEIC",
  "an AVIF image": "зображення AVIF",
  "a WebP image": "зображення WebP",
  "a GIF": "GIF",
  "a TIFF image": "зображення TIFF",
  "an MP4 or QuickTime movie": "відео MP4 або QuickTime",
  "a PDF": "PDF",
  "a ZIP archive": "архів ZIP",
  "a WebAssembly module": "модуль WebAssembly",
  "an email": "електронний лист",
  "an Outlook message": "повідомлення Outlook",
  "a Word, Excel or PowerPoint 97–2003 file": "файл Word, Excel чи PowerPoint 97–2003",
  "a Rich Text document": "документ RTF",
  "a Word document": "документ Word",
  "an Excel workbook": "книга Excel",
  "a PowerPoint presentation": "презентація PowerPoint",
  "a LibreOffice text document": "текстовий документ LibreOffice",
  "a LibreOffice spreadsheet": "таблиця LibreOffice",
  "a LibreOffice presentation": "презентація LibreOffice",
  "an EPUB book": "книга EPUB",
  "an Android app": "застосунок Android",
};

/** "з 3 файлів", "з 21 файла": the genitive after "з". */
function ofFiles(count: number): string {
  return count % 10 === 1 && count % 100 !== 11 ? "файла" : "файлів";
}

/** "1 чисту копію", "3 чисті копії", "5 чистих копій"; standing alone "1 чиста копія"; shared "3 чистими копіями". */
function cleanCopies(count: number, form: "nominative" | "accusative" | "instrumental"): string {
  if (form === "instrumental") return ukrainianCountWord(count, "чистою копією", "чистими копіями", "чистими копіями");
  return ukrainianCountWord(count, form === "nominative" ? "чиста копія" : "чисту копію", "чисті копії", "чистих копій");
}

function ukrainianCountWord(count: number, one: string, few: string, many: string): string {
  const lastTwo = count % 100;
  const last = count % 10;
  if (lastTwo >= 11 && lastTwo <= 14) return many;
  if (last === 1) return one;
  if (last >= 2 && last <= 4) return few;
  return many;
}


/** "105.2 KB" as a Ukrainian reader writes it: "105,2 КБ". */
function size(amount: string, unit: string): string {
  return `${amount.replace(".", ",")} ${{ B: "Б", KB: "КБ", MB: "МБ", GB: "ГБ" }[unit] ?? unit}`;
}

/** "3 bits", "1 byte", "12 blocks" in Ukrainian, wherever they stand in a sentence. */
function units(text: string): string {
  const forms: Record<string, [string, string, string]> = {
    bit: ["біт", "біти", "бітів"],
    byte: ["байт", "байти", "байтів"],
    block: ["блок", "блоки", "блоків"],
    row: ["рядок", "рядки", "рядків"],
    symbol: ["символ", "символи", "символів"],
  };
  return text.replace(/(\d[\d,]*) (bit|byte|block|row|symbol)s?\b/g, (_all, n: string, unit: string) => {
    const count = Number(n.replaceAll(",", ""));
    return `${count.toLocaleString("uk")} ${ukrainianCountWord(count, ...forms[unit])}`;
  });
}

/** What a share of the file goes to, in the sentence under "What it's made of". */
const MADE_OF: Record<string, string> = {
  "the picture": "зображення",
  "the files": "файли",
  "the pages, fonts, images": "сторінки, шрифти й зображення",
  "the message and attachments": "лист і вкладення",
  "the code and data": "код і дані",
  "the document": "документ",
  "information about it": "відомості про файл",
  "the structure that holds it together": "структура, що його скріплює,",
  "bytes the format does not account for": "байти, яких формат не пояснює,",
};

/** Which colours a JPEG scan carries, and how much of them. */
function scanWords(value: string): string {
  const of = (x: string) => (x === "every colour" ? "усіх кольорів" : x === "grey" ? "сірого" : x);
  let m = /^all of (.+)$/.exec(value);
  if (m) return `усе для ${of(m[1])}`;
  m = /^averages of (.+?)(, one more bit)?$/.exec(value);
  if (m) return `середні значення ${of(m[1])}${m[2] ? ", ще один біт" : ""}`;
  m = /^detail (\S+) of (.+?)(, one more bit)?$/.exec(value);
  if (m) return `деталі ${m[1]} ${of(m[2])}${m[3] ? ", ще один біт" : ""}`;
  return value;
}

function translateText(value: string, locale: Locale): string {
  if (locale === "en") return value;
  const leading = value.match(/^\s*/)?.[0] ?? "";
  const trailing = value.match(/\s*$/)?.[0] ?? "";
  const content = value.slice(leading.length, value.length - trailing.length);
  const normalized = content.replace(/\s+/g, " ");
  const exact = UK[normalized] ?? lazyTranslations[normalized];
  if (exact) {
    // The source's own spaces stand; one the translation adds is kept where the source had none.
    const said = leading ? exact.trimStart() : exact;
    return `${leading}${trailing ? said.trimEnd() : said}${trailing}`;
  }
  // A phrase that starts a line or a heading: "Where replies really go".
  const lower = normalized.charAt(0).toLowerCase() + normalized.slice(1);
  const asPhrase = lower !== normalized ? (HEADINGS[lower] ?? UK[lower] ?? lazyTranslations[lower] ?? REVEAL_WORDS[lower]) : undefined;
  if (asPhrase) return `${leading}${asPhrase.charAt(0).toUpperCase()}${asPhrase.slice(1)}${trailing}`;
  const pictureReason = /^The selected area was redacted by the operation, but text inside its pixels was not checked because hexscope does not use OCR\.(?: (.*))?$/.exec(normalized);
  if (pictureReason) {
    const reason = "Вибрану ділянку зафарбовано, але текст у її пікселях не перевірено: hexscope не розпізнає текст на зображеннях.";
    return `${leading}${reason}${pictureReason[1] ? ` ${translateText(pictureReason[1], locale)}` : ""}${trailing}`;
  }
  const coverage = /^hexscope cannot yet confirm whether this ([A-Z]+) finding was removed\.$/.exec(normalized);
  if (coverage) return `${leading}hexscope поки не може підтвердити, чи видалено цей факт у форматі ${coverage[1]}.${trailing}`;
  if (content && content !== value) return `${leading}${translateText(content, locale)}${trailing}`;
  // Spaced for the eye ("Photo  ·  105.2 KB") or by a line break: read as one line.
  value = normalized;
  const label = /^“(.+)”, organisation ([\w-]+)(?:, set by (\S+))?(?: on (.+))?$/.exec(value);
  if (label) {
    return `«${label[1]}», організація ${label[2]}${label[3] ? `, встановив ${label[3]}` : ""}${label[4] ? `, ${label[4]}` : ""}`;
  }
  const sized = /^(\d[\d.]* (?:B|KB|MB|GB)) · (\d[\d,]*) bytes?$/.exec(value);
  if (sized) return `${translateText(sized[1], locale)} · ${translateText(`${sized[2]} bytes`, locale)}`;
  const amount = /^(\d+(?:\.\d+)?) (B|KB|MB|GB)$/.exec(value);
  if (amount) return size(amount[1], amount[2]);
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
  if (match) return `${translateText(match[1], locale)} · ${translateText(match[2], locale)}`;
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
  // A list is one sentence: a line of several is said sentence by sentence below.
  match = /^Reveals ([^]+)\.$/.exec(value);
  if (match && !match[1].includes(". ")) return `Розкриває ${translateRevealList(match[1])}.`;
  match = /^Tells you about its sender: (.+)\.$/.exec(value);
  if (match && !match[1].includes(". ")) return `Що лист розповідає про відправника: ${translateRevealList(match[1])}.`;
  match = /^(Hidden in it|Possible signs of impersonation): (.+?)(?:, and (\d+) more things?)?\.$/.exec(value);
  if (match && !match[2].includes(". ")) {
    const head = match[1] === "Hidden in it" ? "Приховано у файлі" : "Можливі ознаки підміни відправника";
    return `${head}: ${translateText(match[2], locale)}${match[3] ? ` і ще ${match[3]}` : ""}.`;
  }
  match = /^Damaged: (\d+) (place|places) could not be read properly\. It may not open, or open only partly\.$/.exec(value);
  if (match) return `Пошкоджень: ${match[1]}. Частину даних не вдалося прочитати; файл може не відкритися або відкритися не повністю.`;
  match = /^Damaged: (\d+) places? could not be read properly\.$/.exec(value);
  if (match) return `Пошкоджень: ${match[1]}. Частину даних не вдалося прочитати.`;
  match = /^(Word|Excel|PowerPoint) 97–2003 (document|workbook|presentation)$/.exec(value);
  if (match) return `${{ document: "Документ", workbook: "Книга", presentation: "Презентація" }[match[2]]} ${match[1]} 97–2003`;
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
  match = /^(\d+) files?$/.exec(value);
  if (match) return `${match[1]} ${ukrainianCountWord(Number(match[1]), "файл", "файли", "файлів")}`;
  match = /^(\d+) of (\d+) files? needs? a look$/.exec(value);
  if (match) return `${match[1]} з ${match[2]} ${ofFiles(Number(match[2]))} варто переглянути`;
  match = /^Reading (\d+) of (\d+)…$/.exec(value);
  if (match) return `Читаю ${match[1]} з ${match[2]}…`;
  match = /^Nothing personal found in these (\d+) files$/.exec(value);
  if (match) return `У цих ${match[1]} файлах особистих даних не знайдено`;
  match = /^(\d+) reveals? something about you(\.?)$/.exec(value);
  if (match) return `${match[1]} ${ukrainianCountWord(Number(match[1]), "файл розкриває", "файли розкривають", "файлів розкривають")} щось про вас${match[2]}`;
  match = /^(\d+) hides? something$/.exec(value);
  if (match) return `${match[1]} ${ukrainianCountWord(Number(match[1]), "файл щось приховує", "файли щось приховують", "файлів щось приховують")}`;
  match = /^(\d+) damaged$/.exec(value);
  if (match) return `${match[1]} ${ukrainianCountWord(Number(match[1]), "пошкоджений", "пошкоджені", "пошкоджених")}`;
  match = /^(All|Give something away|Damaged or hiding something) · (\d+)$/.exec(value);
  if (match) return `${{ All: "Усі", "Give something away": "Розкривають дані", "Damaged or hiding something": "Пошкоджені або приховують" }[match[1]]} · ${match[2]}`;
  match = /^\+(\d+) more$/.exec(value);
  if (match) return `ще ${match[1]}`;
  match = /^Make (\d+) clean cop(?:y|ies)$/.exec(value);
  if (match) return `Зробити ${match[1]} ${cleanCopies(Number(match[1]), "accusative")}`;
  match = /^Share (\d+) clean cop(?:y|ies)$/.exec(value);
  if (match) return `Поділитися ${match[1]} ${cleanCopies(Number(match[1]), "instrumental")}`;
  match = /^(\d+) removed · (\d+) present · (\d+) unchecked$/.exec(value);
  if (match) return `прибрано ${match[1]} · залишилося ${match[2]} · не перевірено ${match[3]}`;
  match = /^Save (\d+) clean cop(?:y|ies) \(\.zip\)$/.exec(value);
  if (match) return `Зберегти ${match[1]} ${cleanCopies(Number(match[1]), "accusative")} (.zip)`;
  match = /^Made (\d+) clean cop(?:y|ies)\.$/.exec(value);
  if (match) return `Готово: ${match[1]} ${cleanCopies(Number(match[1]), "nominative")}.`;
  match = /^Saved (\d+) clean cop(?:y|ies) as (.+)\.$/.exec(value);
  if (match) return `Збережено ${match[1]} ${cleanCopies(Number(match[1]), "accusative")} як ${match[2]}.`;
  match = /^Making the clean copies… (\d+) of (\d+)$/.exec(value);
  if (match) return `Роблю чисті копії… ${match[1]} з ${match[2]}`;
  match = /^Findings: (\d+) removed, (\d+) present, (\d+) unchecked\.$/.exec(value);
  if (match) return `Знахідки: прибрано ${match[1]}, залишилося ${match[2]}, не перевірено ${match[3]}.`;
  match = /^(\d+) files? had nothing hexscope can remove\.$/.exec(value);
  if (match) return `${match[1]} ${ukrainianCountWord(Number(match[1]), "файл не має", "файли не мають", "файлів не мають")} нічого, що hexscope може прибрати.`;
  match = /^Not cleaned — (.+)\.$/.exec(value);
  if (match) return `Не очищено — ${match[1]}.`;
  match = /^(\d+) files? read\. (.+)$/.exec(value);
  if (match) return `Прочитано ${match[1]} ${ukrainianCountWord(Number(match[1]), "файл", "файли", "файлів")}. ${translateText(match[2], locale)}`;
  match = /^Named \.(\S+), but it is (.+)\. Apps that go by the name may refuse it or call it damaged; renaming it to (\S+) fixes that\.$/.exec(value);
  if (match) return `Названо .${match[1]}, але це ${KIND_IN_WORDS[match[2]] ?? match[2]}. Програми, що зважають на назву, можуть не відкрити його або назвати пошкодженим; перейменування на ${match[3]} це виправить.`;
  match = /^(Hidden in it|Files inside): (\d+) files?$/.exec(value);
  if (match) {
    const count = Number(match[2]);
    return `${match[1] === "Files inside" ? "Файли всередині" : "Приховано у файлі"}: ${count} ${ukrainianCountWord(count, "файл", "файли", "файлів")}`;
  }
  match = /^parsed in ([\d.]+) ms$/.exec(value);
  if (match) return `розібрано за ${match[1].replace(".", ",")} мс`;
  match = /^(\d+) tracked changes, accepted$/.exec(value);
  if (match) return `${match[1]} ${ukrainianCountWord(Number(match[1]), "виправлення", "виправлення", "виправлень")} прийнято`;
  match = /^(\d+) comments and who wrote them$/.exec(value);
  if (match) return `${match[1]} ${ukrainianCountWord(Number(match[1]), "коментар", "коментарі", "коментарів")} і хто їх написав`;
  match = /^(\d+) tracked changes and comment marks, accepted or removed$/.exec(value);
  if (match) return `${match[1]} виправлень і позначок коментарів прийнято або прибрано`;
  match = /^(\d+) pieces of text no one can see$/.exec(value);
  if (match) return `${match[1]} ${ukrainianCountWord(Number(match[1]), "фрагмент", "фрагменти", "фрагментів")} тексту, якого ніхто не бачить`;
  // What a clean copy took out of a PDF's pages, and a picture's text notes.
  match = /^Text under black boxes, marked for redaction, or hidden: (\d+) characters?$/.exec(value);
  if (match) return `Текст під чорними прямокутниками, позначений для приховування або невидимий: ${match[1]} ${ukrainianCountWord(Number(match[1]), "символ", "символи", "символів")}`;
  match = /^Text notes: (.+)$/.exec(value);
  if (match) return `Текстові нотатки: ${match[1]}`;
  // A pass that a server the message never came through wrote.
  match = /^says (.+) passed, but (.+), which says so, is not a server it came through$/.exec(value);
  if (match) return `каже, що ${match[1]} пройдено, але ${match[2]}, який це стверджує, не є сервером, через який пройшов лист`;
  // What a custom XML part of a document says about people: "SharePoint properties: who, who".
  match = /^(SharePoint properties|custom XML): (.+)$/.exec(value);
  if (match) return `${match[1] === "custom XML" ? "власний XML" : "властивості SharePoint"}: ${match[2]}`;
  match = /^What was removed · (.+)$/.exec(value);
  if (match) return `Що видалено · ${match[1]}`;
  match = /^Saved as “(.+)” — look for it in your downloads\.$/.exec(value);
  if (match) return `Збережено як «${match[1]}» — файл у теці «Завантаження».`;
  match = /^A map with a pin where it was taken: (.+)$/.exec(value);
  if (match) return `Карта з позначкою місця зйомки: ${match[1]}`;
  match = /^(.+?) is open\. (.+)$/.exec(value);
  if (match) return `${match[1]} відкрито. ${sentences(match[2], locale, true)}`;
  // The sentence to paste beside a shared card.
  match = /^My (photo|image|video|email|document) was giving away (.+)\. I checked it in my browser with hexscope, and nothing was uploaded: (\S+)$/.exec(value);
  if (match) {
    const mine = { photo: "Моє фото розкривало", image: "Моє зображення розкривало", video: "Моє відео розкривало", email: "Мій лист розкривав", document: "Мій документ розкривав" }[match[1]];
    return `${mine} ${translateRevealList(match[2])}. Перевірено в браузері за допомогою hexscope, нічого не завантажувалося: ${match[3]}`;
  }
  // Opening, the views' captions and the sentence under "What it's made of".
  match = /^Parsing (.+)…$/.exec(value);
  if (match) return `Читаємо ${match[1]}…`;
  match = /^(.+?) · (\d+(?:\.\d+)?) (B|KB|MB|GB)$/.exec(value);
  if (match) return `${translateText(match[1], locale)} · ${size(match[2], match[3])}`;
  match = /^Of its (\d+(?:\.\d+)?) (B|KB|MB|GB), (.+?)( — a second, small copy of the picture among it)?\.$/.exec(value);
  if (match) {
    const shares = [...match[3].matchAll(/(under 1%|\d+%) (?:goes )?to (?:the )?(the picture|the files|the pages, fonts, images|the message and attachments|the code and data|the document|information about it|the structure that holds it together|bytes the format does not account for)/g)];
    if (shares.length) {
      const said = shares.map(([, pct, what]) => `${MADE_OF[what]} — ${pct === "under 1%" ? "менше 1%" : pct}`);
      const list = said.length > 1 ? `${said.slice(0, -1).join(", ")} і ${said[said.length - 1]}` : said[0];
      return `Із ${size(match[1], match[2])}: ${list}${match[4] ? "; серед відомостей — друга, маленька копія зображення" : ""}.`;
    }
  }
  match = /^(\d+) attachments$/.exec(value);
  if (match) return `${match[1]} ${ukrainianCountWord(Number(match[1]), "вкладення", "вкладення", "вкладень")}`;
  match = /^Save a copy with (\d+) places blacked out$/.exec(value);
  if (match) return `Зберегти копію з ${match[1]} прихованими місцями`;
  match = /^Page (\d+): what anyone can still (read|see) under the boxes$/.exec(value);
  if (match) return `Сторінка ${match[1]}: що досі можна ${match[2] === "read" ? "прочитати" : "побачити"} під прямокутниками`;
  match = /^Page (\d+): as a viewer shows it$/.exec(value);
  if (match) return `Сторінка ${match[1]}: як її показує переглядач`;
  // The way an email came: what the receiving server found.
  match = /^✓ (.+) passed$/.exec(value);
  if (match) return `✓ ${match[1]}: пройдено`;
  match = /^✗ (.+) failed — not vouched for$/.exec(value);
  if (match) return `✗ ${match[1]}: не пройдено — домен лист не підтвердив`;
  // A picture built up scan by scan, or pass by pass.
  match = /^All (\d+) scans: the whole picture\.$/.exec(value);
  if (match) return `Усі скани (${match[1]}): усе зображення.`;
  match = /^After scan (\d+) of (\d+) \((.+)\): (\d+)% of the file\.$/.exec(value);
  if (match) return `Після скану ${match[1]} із ${match[2]} (${scanWords(match[3])}): ${match[4]}% файла.`;
  match = /^All (\d+) passes: the whole picture(?:, (.+) of the file)?\.$/.exec(value);
  if (match) return `Усі проходи (${match[1]}): усе зображення${match[2] ? `, ${match[2]} файла` : ""}.`;
  match = /^After pass (\d) of 7: (.+) of the pixels(?:, (.+) of the file)?\.$/.exec(value);
  if (match) return `Після проходу ${match[1]} із 7: ${match[2]} пікселів${match[3] ? `, ${match[3]} файла` : ""}.`;
  // What a byte or a pixel of a JPEG is.
  match = /^Byte (\S+) is part of block (\d+), (\d+)(?: in scan (\d+) of (\d+) \((.+)\))? — (\d+×\d+) pixels, written in (.+)\.$/.exec(value);
  if (match) {
    const scan = match[4] ? ` у скані ${match[4]} із ${match[5]} (${scanWords(match[6])})` : "";
    return `Байт ${match[1]} — частина блока ${match[2]}, ${match[3]}${scan}: ${match[7]} пікселів, записано ${units(match[8])}.`;
  }
  match = /^Pixel (\d+), (\d+): its block comes after where the scan stops\.$/.exec(value);
  if (match) return `Піксель ${match[1]}, ${match[2]}: його блок іде після місця, де скан обривається.`;
  match = /^Pixel (\d+), (\d+): not decoded\.$/.exec(value);
  if (match) return `Піксель ${match[1]}, ${match[2]}: не декодовано.`;
  match = /^Pixel (\d+), (\d+) is in block (\d+), (\d+): (\d+×\d+) pixels, (.+?) of 8×8 samples, written in (.+?) — (.+?) from (\S+)\.$/.exec(value);
  if (match) return `Піксель ${match[1]}, ${match[2]} — у блоці ${match[3]}, ${match[4]}: ${match[5]} пікселів, ${units(match[6])} відліків 8×8, записано ${units(match[7])} — ${units(match[8])} від ${match[9]}.`;
  match = /^An average block takes (.+?); this one has (.+)\.$/.exec(value);
  if (match) return `Середній блок займає ${units(match[1])}; у цього — ${translateText(match[2], locale)}.`;
  match = /^Pixel (\d+), (\d+) is written a little in each of (\d+) scans — (.+) in all\.$/.exec(value);
  if (match) return `Піксель ${match[1]}, ${match[2]} записано потроху в кожному з ${match[3]} сканів — разом ${units(match[4])}.`;
  match = /^Scan (\d+)'s part: block (\d+), (\d+), (\d+×\d+) pixels, (.+?) from (\S+)\.$/.exec(value);
  if (match) return `Частина скану ${match[1]}: блок ${match[2]}, ${match[3]}, ${match[4]} пікселів, ${units(match[5])} від ${match[6]}.`;
  match = /^(all of|averages of|detail \S+ of) .+$/.exec(value);
  if (match) return scanWords(value);
  match = /^\d[\d,]* (bits?|bytes?)$/.exec(value);
  if (match) return units(value);
  // What a byte or a pixel of a PNG is.
  match = /^Pixel (\d+), (\d+) · (?:pass (\d) of 7, its row (\d+)|row (\d+)) filtered with (.+)\.$/.exec(value);
  if (match) {
    const where = match[3] ? `прохід ${match[3]} із 7, його рядок ${match[4]}` : `рядок ${match[5]}`;
    return `Піксель ${match[1]}, ${match[2]} · ${where}, фільтр ${match[6].replace(/^type /, "тип ")}.`;
  }
  match = /^These bits wrote (.+?) of (?:pass (\d), row (\d+)|row (\d+))( and on)?\.$/.exec(value);
  if (match) return `Ці біти записали ${units(match[1])} ${match[2] ? `проходу ${match[2]}, рядка ${match[3]}` : `рядка ${match[4]}`}${match[5] ? " і далі" : ""}.`;
  match = /^Its byte, (0x[0-9A-F]+), is a literal: spelled out in the file's bits\.$/.exec(value);
  if (match) return `Його байт, ${match[1]}, — літерал: записаний у бітах файла як є.`;
  match = /^It was copied: (.+?) from ([\d,  ]+) back(?: — exactly (.+?) up( in its pass)?| on the same row( in its pass)?)?, shown in blue\.$/.exec(value);
  if (match) {
    const rows = match[3] ? ` — рівно на ${units(match[3])} вище${match[4] ? " у його проході" : ""}` : / on the same row/.test(value) ? " у тому самому рядку" : "";
    return `Скопійовано: ${units(match[1])}, що стоять на ${match[2]} назад${rows}; показано синім.`;
  }
  match = /^Step ([\d,  ]+) in the player$/.exec(value);
  if (match) return `Крок ${match[1]} у плеєрі`;
  // The DEFLATE player.
  match = /^Step ([\d,  ]+) of ([\d,  ]+)$/.exec(value);
  if (match) return `Крок ${match[1]} із ${match[2]}`;
  match = /^Read (.+?) \(bits ([\d,  ]+)–([\d,  ]+)\) · output byte ([\d,  ]+)$/.exec(value);
  if (match) return `Прочитано ${units(match[1])} (біти ${match[2]}–${match[3]}) · вихідний байт ${match[4]}`;
  match = /^Stopped at compressed bit ([\d,  ]+) · output so far ([\d,  ]+) bytes$/.exec(value);
  if (match) return `Зупинилися на стисненому біті ${match[1]} · виведено ${match[2]} байтів`;
  match = /^(stored|fixed Huffman|dynamic Huffman) block$/.exec(value);
  if (match) return `${{ stored: "Блок без стиснення", "fixed Huffman": "Блок зі сталим Гаффманом", "dynamic Huffman": "Блок із динамічним Гаффманом" }[match[1]]}`;
  match = /^— dynamic Huffman: this block ships its own code tables, ([\d,  ]+) bits of them\.$/.exec(value);
  if (match) return `— динамічний Гаффман: блок несе власні таблиці кодів, ${units(`${match[1]} bits`)}.`;
  match = /^— write byte (0x[0-9A-F]{2})( '.')? as it is\.$/.exec(value);
  if (match) return `— записати байт ${match[1]}${match[2] ?? ""} як є.`;
  match = /^— copy ([\d,  ]+) bytes from ([\d,  ]+) bytes back\.$/.exec(value);
  if (match) return `— скопіювати ${units(`${match[1]} bytes`)}, що стоять на ${units(`${match[2]} bytes`)} раніше.`;
  match = /^This header builds (\d+) literal\/length and (\d+) distance codes, using a (\d+)-symbol code for their lengths\.$/.exec(value);
  if (match) return `Цей заголовок будує ${match[1]} кодів літералів і довжин та ${match[2]} кодів відстаней, а їхні довжини записано кодом на ${match[3]} символи.`;
  match = /^code lengths · (\d+) symbols$/.exec(value);
  if (match) return `довжини кодів · ${units(`${match[1]} symbols`)}`;
  match = /^literal (0x[0-9A-F]{2})$/.exec(value);
  if (match) return `літерал ${match[1]}`;
  match = /^length code (\d+)$/.exec(value);
  if (match) return `код довжини ${match[1]}`;
  match = /^distance code (\d+)$/.exec(value);
  if (match) return `код відстані ${match[1]}`;
  match = /^(\S+) is past this range$/.exec(value);
  if (match) return `${match[1]} виходить за цей діапазон`;
  const fact = factSentence(value);
  if (fact) return fact;
  // Several sentences, none of them known as a whole: each on its own.
  const whole = sentences(value, locale);
  return whole;
}

/**
 * A line made of several sentences, none known as a whole: the longest run
 * of them that is known, then the rest — "Named .jpg, but it is a HEIC
 * image. Apps that go by the name…" is two sentences and one translation.
 */
function sentences(value: string, locale: Locale, whole = false): string {
  const parts = value.split(/(?<=\.) (?=[A-Z“])/);
  if (parts.length < 2) return whole ? translateText(value, locale) : value;
  const out: string[] = [];
  for (let i = 0; i < parts.length; ) {
    let j = parts.length;
    for (; j > i + 1; j--) {
      // The whole line was tried already, unless the caller asks for it.
      if (!whole && i === 0 && j === parts.length) continue;
      const run = parts.slice(i, j).join(" ");
      const said = translateText(run, locale);
      if (said !== run) {
        out.push(said);
        break;
      }
    }
    if (j === i + 1) out.push(translateText(parts[i], locale));
    i = j;
  }
  return out.join(" ");
}

/** "Olena and Petro", "A, B and 2 others", as the core joins names. */
const names = (list: string) =>
  list.replace(/ and (\d+) others?$/, (_, n: string) => ` та ще ${n}`).replace(/ and (?=[^,]*$)/, " і ");

/** A count of things in words: "2 hidden rows" → "2 приховані рядки". */
function counted(text: string): string {
  const words: [RegExp, [string, string, string]][] = [
    [/^insertions?$/, ["вставка", "вставки", "вставок"]],
    [/^deletions?$/, ["видалення", "видалення", "видалень"]],
    [/^comments?$/, ["коментар", "коментарі", "коментарів"]],
    [/^hidden rows?$/, ["прихований рядок", "приховані рядки", "прихованих рядків"]],
    [/^hidden columns?$/, ["прихований стовпець", "приховані стовпці", "прихованих стовпців"]],
    [/^files?$/, ["файл", "файли", "файлів"]],
    [/^links?$/, ["посилання", "посилання", "посилань"]],
    [/^steps?$/, ["крок", "кроки", "кроків"]],
    [/^functions?$/, ["функція", "функції", "функцій"]],
    [/^function names?$/, ["назва функції", "назви функцій", "назв функцій"]],
  ];
  return text.replace(/(\d+) ([a-z ]+?)(?=$| and |,)/g, (all, n: string, word: string) => {
    const forms = words.find(([re]) => re.test(word))?.[1];
    return forms ? `${n} ${ukrainianCountWord(Number(n), ...forms)}` : all;
  }).replace(/ and (?=\d)/g, " і ");
}

/** What a module asks its host to do, phrase by phrase. */
const CAN: Record<string, string> = {
  "reach the network": "виходити в мережу",
  "open files by name": "відкривати файли за назвою",
  "write to files or the console": "писати у файли чи консоль",
  "read files or input": "читати файли чи введення",
  "keep data in the browser": "зберігати дані в браузері",
  "read environment variables": "читати змінні середовища",
  "read its command line": "читати свій командний рядок",
  "read the clock": "читати годинник",
  "draw random numbers": "брати випадкові числа",
  "change the page it runs in": "змінювати сторінку, на якій працює",
  "use the clipboard": "користуватися буфером обміну",
  "ask where you are": "питати, де ви",
  "use the camera or microphone": "користуватися камерою чи мікрофоном",
};

/** How text on a PDF page is hidden. */
const HIDDEN: Record<string, string> = {
  "drawn invisibly": "намальований невидимим",
  "in white on white": "білим по білому",
  "off the page": "поза сторінкою",
  "too small to see": "задрібний, щоб побачити",
};

/** What an attachment is, in an email's warning. */
const RISKY: Record<string, string> = {
  "a web page: opened, it can show a sign-in page for any site, or send you to one": "вебсторінка: відкрита, вона може показати сторінку входу будь-якого сайту або перенаправити на неї",
  "a program: opened, it runs on your computer": "програма: відкрита, вона запускається на вашому комп’ютері",
  "a shortcut that opens a program or a site": "ярлик, що відкриває програму чи сайт",
  "a disk image, which can carry programs past your computer's warnings": "образ диска, який може пронести програми повз попередження комп’ютера",
  "an Office file with macros: programs that run when allowed": "файл Office з макросами: програмами, що запускаються, коли їм дозволити",
};

/**
 * The sentences the core writes as what a file gives away — "1 deletion by
 * Petro, not accepted yet" — with the names, addresses and quotes they hold
 * kept as they are.
 */
function factSentence(v: string): string | undefined {
  let m: RegExpExecArray | null;
  if ((m = /^((?:\d+ (?:insertions?|deletions?))(?: and \d+ (?:insertions?|deletions?))?)(?: by (.+))?, not accepted yet$/.exec(v)))
    return `${counted(m[1])}${m[2] ? ` від ${names(m[2])}` : ""}, ще не прийнято`;
  if ((m = /^(\d+ comments?) by (.+)$/.exec(v))) return `${counted(m[1])} від ${names(m[2])}`;
  if ((m = /^(\d+ files?): (.+)$/.exec(v))) return `${counted(m[1])}: ${m[2]}`;
  if ((m = /^(\d+ links?), to (.+)$/.exec(v))) return `${counted(m[1])}, на ${m[2].replace(/ and (\d+) more$/, " і ще $1")}`;
  if (v.startsWith("a link reads “"))
    return v.replace(/a link reads “(.+?)” and goes to ([^;]+)/g, "посилання з текстом «$1» веде на $2").replace(/; and (\d+) more$/, "; і ще $1");
  if ((m = /^(.+), shown as “(.+)”: letters of other alphabets that pass for Latin ones$/.exec(v)))
    return `${m[1]}, показано як «${m[2]}»: літери інших абеток, схожі на латинські`;
  if ((m = /^(.+) — not the sender's (.+)$/.exec(v))) return `${m[1]} — а не ${m[2]}, як у відправника`;
  if ((m = /^bounces go to (.+), not the sender's (.+)$/.exec(v))) return `листи-відмови йдуть на ${m[1]}, а не на ${m[2]} відправника`;
  if ((m = /^(.+) passed: the sender's domain vouched for it$/.exec(v))) return `${m[1]}: пройдено — домен відправника підтвердив лист`;
  if ((m = /^(.+) failed: the domain it claims to come from did not vouch for it$/.exec(v)))
    return `${m[1]}: не пройдено — домен, від імені якого надіслано лист, його не підтвердив`;
  if ((m = /^(UTC[^,]*), the sender's clock when it was sent$/.exec(v))) return `${m[1]}, годинник відправника на момент надсилання`;
  if ((m = /^(.+?), where the sender connected from(?:, and (.+?), the computer's address on its own network)?, in the first server's (.+) line$/.exec(v)))
    return `${m[1]} — звідки підключився відправник${m[2] ? `, і ${m[2]} — адреса комп’ютера в його власній мережі` : ""}, у рядку ${m[3]} першого сервера`;
  if ((m = /^“(.+)” is( named to look like a (\.\w+),)? (.+)$/.exec(v)) && RISKY[m[4]])
    return `«${m[1]}» —${m[3] ? ` названо так, ніби це ${m[3]},` : ""} ${RISKY[m[4]]}`;
  if ((m = /^(.+), but it opens without a password, so anyone can read it$/.exec(v)))
    return `${m[1]}, але відкривається без пароля, тож прочитати може будь-хто`;
  if ((m = /^embedded JPEG, (\d+) bytes$/.exec(v))) return `вбудований JPEG, ${m[1]} ${ukrainianCountWord(Number(m[1]), "байт", "байти", "байтів")}`;
  if ((m = /^a picture of the first page, (\d+×\d+)$/.exec(v))) return `зображення першої сторінки, ${m[1]}`;
  if ((m = /^on (\d+) slides?: (.+)$/.exec(v)))
    return `на ${m[1]} ${ukrainianCountWord(Number(m[1]), "слайді", "слайдах", "слайдах")}: ${m[2]}`;
  if ((m = /^slides? (\d[\d, ]*(?: and \d+ more)?)$/.exec(v))) return `${/,/.test(m[1]) ? "слайди" : "слайд"} ${m[1].replace(/ and (\d+) more$/, " і ще $1")}`;
  if ((m = /^page (\d+): an area marked for redaction was never applied: (.+)$/.exec(v)))
    return `сторінка ${m[1]}: ділянку, позначену для приховування, так і не приховано: ${m[2]}`;
  if ((m = /^page (\d+): (“.+)$/.exec(v))) return `сторінка ${m[1]}: ${m[2]}`;
  if ((m = /^page (\d+), (drawn invisibly|in white on white|off the page|too small to see): (.+)$/.exec(v)))
    return `сторінка ${m[1]}, ${HIDDEN[m[2]]}: ${m[3]}`;
  if (v === "once after it was first saved; the earlier version is still inside") return "раз після першого збереження; попередня версія досі всередині";
  if ((m = /^(\d+) times after it was first saved; the earlier versions are still inside$/.exec(v)))
    return `${m[1]} ${ukrainianCountWord(Number(m[1]), "раз", "рази", "разів")} після першого збереження; попередні версії досі всередині`;
  if (/^\d+ hidden (rows?|columns?)( and \d+ hidden columns?)?$/.test(v)) return counted(v);
  if ((m = /^(\d+ steps?) in (.+), last on (.+)$/.exec(v))) return `${counted(m[1])} у ${m[2]}, востаннє ${m[3]}`;
  if ((m = /^(.+), in paths like (.+)$/.exec(v))) return `${m[1]}, у шляхах на кшталт ${m[2]}`;
  if ((m = /^(?:module “(.+)”, )?(\d+ function names?)$/.exec(v))) return `${m[1] ? `модуль «${m[1]}», ` : ""}${counted(m[2])}`;
  if ((m = /^(?:(.+) \()?(\d+ functions?) from (.+?)\)?$/.exec(v)) && (!m[1] || m[1].split(/, | and /).every((c) => CAN[c]))) {
    const can = m[1] ? m[1].split(/, | and /).map((c) => CAN[c]) : [];
    const list = can.length > 1 ? `${can.slice(0, -1).join(", ")} і ${can[can.length - 1]}` : (can[0] ?? "");
    const from = `${counted(m[2])} з ${m[3].replace(/ and (\d+) more$/, " і ще $1")}`;
    return list ? `${list} (${from})` : from;
  }
  if ((m = /^made with (.+?)(?:, model (.+))?$/.exec(v))) return `створено в ${m[1]}${m[2] ? `, модель ${m[2]}` : ""}`;
  if ((m = /^(?:(\d+) h)?\s?(?:(\d+) min)?$/.exec(v)) && (m[1] || m[2]))
    return [m[1] && `${m[1]} год`, m[2] && `${m[2]} хв`].filter(Boolean).join(" ");
  if ((m = /^“(.+)”, from (.+)$/.exec(v))) return `«${m[1]}», з ${m[2]}`;
  if ((m = /^(\d+ steps?) recorded$/.exec(v))) return `записано ${counted(m[1])}`;
  if ((m = /^(.+), with a password to open it: hexscope cannot read its contents$/.exec(v)))
    return `${m[1]}, відкривається лише з паролем: hexscope не може прочитати вміст`;
  // A photo placed in a document: where, on what, whose.
  if (/: taken at |· serial |· owner /.test(v))
    return v.replace(": taken at ", ": знято в ").replace("· serial ", "· серійний номер ").replace("· owner ", "· власник ");
  return undefined;
}

const reverse = new Map(Object.entries(UK).map(([english, ukrainian]) => [ukrainian, english]));


/** The English a Ukrainian text was translated from, keeping the spaces around it. */
function untranslate(value: string): string | undefined {
  const content = value.trim();
  const normalized = content.replace(/\s+/g, " ");
  // Text that is already English stays as it is, though it may also be some
  // other text's Ukrainian: a lone "." after a link is just a full stop.
  if (Object.hasOwn(UK, normalized) || Object.hasOwn(lazyTranslations, normalized)) return undefined;
  const english = reverse.get(content) ?? lazyReverse.get(normalized);
  return english === undefined ? undefined : value.replace(content, english);
}

useDictionary({ translate: (value) => translateText(value, "uk"), untranslate });
