<!-- Written by scripts/compare-tools.py; do not edit by hand. -->

Tools: hexscope 0.5.0, ExifTool 13.55, mat2 0.15.0. 45 files.

| | hexscope | ExifTool | MAT2 |
|---|---|---|---|
| Files with something to remove | 37 | 37 | 37 |
| …of which a copy was made | 33 | 21 | 22 |
| Copies where ExifTool still reads identifying tags | 0 | 4 | 0 |
| Copies where hexscope still finds something | 3 | 8 | 10 |
| Pictures whose pixels changed | 0 | 0 | 5 |

| File | Found by ExifTool / hexscope | hexscope's copy | ExifTool's copy | MAT2's copy |
|---|---|---|---|---|
| `fixtures/ai-picture.png` | 1 / 2 | clean | clean | clean |
| `fixtures/android.mp4` | 7 / 2 | clean | ExifTool: CreateDate, MediaCreateDate, MediaModifyDate, ModifyDate, TrackCreateDate, TrackModifyDate; hexscope: taken | clean |
| `fixtures/budget.ods` | 6 / 17 | clean | no copy | hexscope: comments |
| `samples/budget.xlsx` | 4 / 6 | hexscope: hiddencells, hiddensheets | no copy | hexscope: hiddencells, hiddensheets |
| `fixtures/chrome-highlight.pdf` | 5 / 7 | clean | hexscope: application, covered, created, hiddentext, modified, producer, title, updates | hexscope: updates |
| `fixtures/compact.pdf` | 11 / 6 | clean | hexscope: application, author, created, history, producer, title, updates | hexscope: updates |
| `samples/cropped.jpg` | 24 / 11 | clean | clean | pixels differ by 0.457 |
| `fixtures/cwebp-photo.webp` | 16 / 8 | clean | clean | no copy |
| `fixtures/ffmpeg.gif` | 0 / 0 | no copy | clean | clean |
| `samples/film-negative.png` | 1 / 1 | clean | clean | clean |
| `fixtures/gif2webp-anim.webp` | 0 / 0 | no copy | clean | no copy |
| `samples/hello.wasm` | 0 / 5 | hexscope: imports, paths | no copy | no copy |
| `fixtures/icc.jpg` | 0 / 0 | no copy | clean | clean |
| `fixtures/icc.png` | 0 / 0 | no copy | clean | clean |
| `fixtures/icc.webp` | 0 / 0 | no copy | clean | no copy |
| `fixtures/iphone-hdr.jpg` | 16 / 8 | clean | clean | pixels differ by 2.951 |
| `fixtures/iphone.mov` | 11 / 4 | clean | ExifTool: CreateDate, MediaCreateDate, MediaModifyDate, ModifyDate, TrackCreateDate, TrackModifyDate; hexscope: taken | no copy |
| `fixtures/keynote.pptx` | 0 / 2 | hexscope: hiddenslides, notes | no copy | no copy |
| `fixtures/lease.odt` | 6 / 20 | clean | no copy | hexscope: comments |
| `fixtures/lease.rtf` | 9 / 15 | clean | no copy | no copy |
| `fixtures/loci.mp4` | 7 / 2 | clean | ExifTool: CreateDate, MediaCreateDate, MediaModifyDate, ModifyDate, TrackCreateDate, TrackModifyDate; hexscope: taken | clean |
| `fixtures/mac-report.zip` | 0 / 2 | clean | no copy | hexscope: downloaded, tags |
| `fixtures/message.eml` | 0 / 6 | no copy | no copy | no copy |
| `fixtures/notes.png` | 3 / 2 | clean | clean | clean |
| `fixtures/phishing-rtf.msg` | 0 / 9 | no copy | no copy | no copy |
| `fixtures/phishing.eml` | 0 / 10 | no copy | no copy | no copy |
| `fixtures/phishing.msg` | 0 / 10 | no copy | no copy | no copy |
| `fixtures/photo-grid.heic` | 18 / 7 | clean | clean | no copy |
| `fixtures/photo.avif` | 18 / 7 | clean | clean | clean |
| `fixtures/photo.heic` | 18 / 7 | clean | clean | no copy |
| `fixtures/photo.jpg` | 16 / 8 | clean | clean | pixels differ by 0.481 |
| `samples/photo.jpg` | 16 / 8 | clean | clean | pixels differ by 2.951 |
| `fixtures/photo.png` | 30 / 7 | clean | clean | clean |
| `fixtures/photo.tif` | 15 / 7 | clean | ExifTool: Make, Model, ModifyDate, Software; hexscope: camera, software, taken | clean |
| `fixtures/plan.doc` | 5 / 4 | clean | no copy | no copy |
| `fixtures/progressive.jpg` | 0 / 0 | no copy | clean | pixels differ by 2.608 |
| `fixtures/redacted.pdf` | 2 / 6 | clean | hexscope: attachments, comments, covered, hiddentext, producer, title, updates | hexscope: updates |
| `fixtures/report.docx` | 5 / 5 | clean | no copy | no copy |
| `fixtures/report.pdf` | 13 / 9 | clean | hexscope: application, author, created, earlier, history, modified, producer, title, updates | hexscope: updates |
| `samples/sample.png` | 0 / 0 | no copy | clean | clean |
| `fixtures/tracked.docx` | 5 / 9 | clean | no copy | no copy |
| `fixtures/voice.m4a` | 2 / 2 | clean | clean | no copy |
| `samples/wifi.png` | 0 / 0 | clean | clean | clean |
| `fixtures/windows.docx` | 3 / 7 | clean | no copy | hexscope: thumbnail |
| `fixtures/windows.xlsx` | 3 / 5 | clean | no copy | hexscope: folder |
