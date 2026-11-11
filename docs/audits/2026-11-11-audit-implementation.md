# Реалізація аудиту 2026-11-11

База: актуальний `origin/main` на `0418f80` (0.18.0). Старий checkout 0.14
із локальними змінами збережено комітом `54e3642`; інтеграція `b9fb2ba`
залишила новіші реалізації main та додала історичний звіт. Не виконувалось
повернення старих inspector/translation/styles поверх новіших.

## Що реалізовано

| Напрям аудиту | Результат |
|---|---|
| Пам'ять | Web admission 128 MiB, CLI bounded reads 512 MiB, shared tree budget 500k + limit error; existing sliced movies та per-format decoding limits збережені |
| Worker | 60s deadline важких операцій, ігнорування відповідей старого worker, grace period скасування parse, відновлення поточного файла з локальних байтів |
| Compare reader | Progress не завершує promise; terminal response/error/deadline звільняє worker |
| Core API | `Summary::inspection_status()` явно відрізняє unsupported від complete fallback byte tree |
| Clean-copy proof | Fuzz oracle перевіряє повторну очищуваність повної копії, capability/completeness і межі кількості claimed removals |
| Незалежна перевірка | ExifTool + Pillow gate для двох fixtures: identifying fields відсутні, декодовані пікселі збігаються |
| Модулі | Audio writers виділено з `clean.rs`; copy-plan wording — із cleaner; photo editor став lazy-loaded; preflight та OCR окремі модулі |
| UX/UI | Компактні попередження про неповноту/readback cap; optional plan/tools; collapsed metadata актуального main збережені |
| План змін | Спостережені finding kinds із core capabilities; окремо від фактичного підтвердження removal |
| Профілі | Web import/export JSON, `hexscope policy`, спільний Core evaluator; unknown/QR rules лишаються unchecked |
| Походження | Реальні parent paths та bounded differing-field view, із переходами до evidence; без висновків про підробку |
| Порівняння | Зміни значень одного finding kind; відсутність у partial read не називається видаленням; окремий byte/content статус |
| Пакет до публікації | Копія + value-free JSON receipt + HTML summary; batch ZIP також отримує summary |
| OCR | Opt-in Tesseract.js 7 із локальними EN/UK-моделями, image та first extractable PDF picture; тільки ручне додавання областей |
| Лабораторія | CRC-mutation лише synthetic sample у disposable comparison workers |
| Редагування | Photo Undo/Redo та bounded history; наявне keyboard area editing збережене |
| Receipts/i18n | Audio format parity між Core/Web; виправлено «один раз» у PDF revision |
| Browser gates | Critical PDF/ZIP/plan/profile/package сценарії у Firefox, desktop і mobile WebKit; окремий реальний OCR-тест |

Інструменти описані в [publication-tools.md](../publication-tools.md), CLI
контракти — у [cli.md](../cli.md). README, CONTRIBUTING, known issues та roadmap
оновлено. Жодного нового version tag або package release цей набір не створює.

## CI follow-up для 0.19.0

Post-merge CI виявив DNS timeouts саме на Ubuntu apt mirrors під час встановлення
ExifTool/Pillow, браузерних залежностей Playwright і `musl-tools`. Прибрано ці
apt-залежності з відповідних gates: незалежна перевірка бере ExifTool із
зафіксованого pnpm package, Pillow встановлюється як pinned wheel в окремий
venv, Playwright jobs використовують version-matched browser image, а Linux
musl release builds — зафіксований `cross`. Перший hosted запуск виявив, що
`cross` 0.2.5 на ARM runner намагається встановити несумісний x86 host
toolchain; matrix для `aarch64` перенесено на x86_64 runner. Цей build лишається
обов'язковим hosted PR gate. Cross-browser smoke використовує `HOME=/root`, як
вимагає Firefox у контейнері при запуску від root; ARM package smoke запускає
перевірку ELF machine id замість виконання foreign-architecture бінарника на
x86 host. x86 musl package як і раніше перевіряється запуском `--version`.
Також E2E відтворив вихід мобільного меню за лівий safe margin; його ширину
обмежено з урахуванням обох країв екрана.

Локально пройшли `actionlint` для змінених workflow (без застарілого переліку
runner labels), `git diff --check` та ExifTool/Pillow gate на двох fixtures.
У цьому середовищі Docker daemon недоступний, тож musl builds і самі hosted
browser jobs залишаються обов'язковими PR CI gates. Release tag і packages не
створювались.

## Перевірка

- Rust workspace з усіма features — пройшов.
- Web unit/typecheck — 520 тестів пройшли після regression-виправлення.
- Release performance — 6/6 large-file tests пройшли.
- Stronger clean-roundtrip fuzz smoke — 6741 запуск за 31 секунду, без падіння.
- ExifTool/Pillow independent gate — 2 fixtures пройшли.
- Dependency audit після OCR additions — known vulnerabilities не знайдено.
- Accessibility desktop/phone — 64 passed, 4 штатні skipped у першому проході.
- OCR browser tests — 3/3: реальний engine під production CSP розпізнав
  synthetic email, ручне підтвердження/Undo/Redo пройшли, зовнішніх запитів
  не було; скасування під час завантаження моделі звільнило worker.
  Український синтетичний PDF-скан підтвердив локальне розпізнавання,
  попередній перегляд рамки та її ручне додавання під незмінним CSP.
- Перший cross-browser run: 16 passed + 2 retries через конфлікт локального
  Playwright output directory з паралельним OCR run (`ENOENT` trace files).
  Фінальні прогони використовують окремі output directories; це не прихована
  нестабільність продуктових assertions.

Повний Chromium-прогін дав 476 passed / 67 skipped та виявив три падіння:
два стосувалися одного нового light-theme contrast порушення; третє — race
між таймером скасованого parse і batch resume. Обидві причини виправлені,
для другої додано unit regression. Повторний прогін зачеплених suites:
83 passed / 7 skipped. Фінальний critical Firefox/WebKit/mobile WebKit з
відкритою preflight-панеллю, axe та overflow check: 18/18 без retries.
Hosted advisory job також виявив незалежний workflow дефект: post-job cache
зберігав неіснуючий pnpm store у job без install. Непотрібний cache прибрано;
сканування advisories залишено.

Hosted Chromium E2E знайшов ще один крайовий випадок мобільної навігації:
панель вирівнюється по правому краю кнопки меню, тому ліміт `100vw - 32px`
не враховував відступ самої кнопки й давав лівий край 7 px замість мінімуму
8 px. Ліміт зменшено до `100vw - 40px`, щоб зберегти щонайменше 8 px
відступу від обох країв без зміни прив'язки панелі до кнопки.

Hosted checks і результат інтеграції: [PR #188](https://github.com/MykytaStel/hexscope/pull/188). Сім чистих застарілих worktree видалено після перевірки merged refs. Перед очищенням локально збережено Git bundle і 882 fuzz-файли з перевіркою SHA-256.

## Чесні межі

Це реалізовані обмежені інструменти й перевірки, а не доказ універсальної
безпеки або повноти форматів. Tree budget не обмежує всі сторонні allocations.
Наявні format decoding limits лишаються потрібними; global streaming decoder
для всіх форматів не з'явився. OCR має synthetic integration evidence, але
немає виміряної recall/false-positive оцінки на реальному корпусі.

Фізичні iPhone/Android, HDR screen, VoiceOver/TalkBack та користувацьке
дослідження розуміння unchecked-станів потребують доступу до пристроїв/людей.
Вони залишені явними acceptance gates, а не позначені завершеними за emulation.
Повноцінний PDF renderer і автоматичне OCR-redaction не додавались: PDF OCR
читає перше extractable picture і потребує ручного підтвердження області.
