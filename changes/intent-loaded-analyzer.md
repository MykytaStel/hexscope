### Changed

- **The file analyzer loads when someone chooses a file.** Browsing the landing page no longer parses the analyzer on an idle timer. Opening a file picker starts loading it, while sample links and files shared to the app still open directly. The full offline cache remains unchanged; this reduces analyzer work before file intent, not total first-visit transfer.
