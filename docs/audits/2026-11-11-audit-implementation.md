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

## Перевірка

- Rust workspace з усіма features — пройшов.
- Web unit/typecheck — 518 тестів пройшли до фінальної перевірки.
- Release performance — 6/6 large-file tests пройшли.
- Stronger clean-roundtrip fuzz smoke — 6741 запуск за 31 секунду, без падіння.
- ExifTool/Pillow independent gate — 2 fixtures пройшли.
- Dependency audit після OCR additions — known vulnerabilities не знайдено.
- Accessibility desktop/phone — 64 passed, 4 штатні skipped у першому проході.
- OCR browser example — реальний engine розпізнав synthetic email, ручне
  підтвердження/Undo/Redo пройшли, зовнішніх запитів не було.
- Перший cross-browser run: 16 passed + 2 retries через конфлікт локального
  Playwright output directory з паралельним OCR run (`ENOENT` trace files).
  Фінальні прогони використовують окремі output directories; це не прихована
  нестабільність продуктових assertions.

Фінальний статус CI/merge/cleanup записується після завершення цих дій.

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
