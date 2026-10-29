### Fixed

- **The "What it's made of" bar shows the whole of a large file.** An archive with thousands of entries drew one piece per header and per entry, so the bar showed only the start of the file and the page held two elements per entry. Past 400 pieces, the bar is now drawn at a lower resolution: about 200 equal stretches, each with its colours in proportion, so every byte still counts. Opening an archive of 100,000 entries leaves about 200,000 fewer elements on the page.
