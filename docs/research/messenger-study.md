# What messengers take out of a file: how to measure it

hexscope's guides say that messengers and social networks "usually" drop a
photo's location when it is sent as a photo, and keep everything when it is
sent as a file. That is what their makers and others report; it has not been
measured here. This is how to measure it, so that the guides can say what
each one does, on which date, with which version.

## The kit

Six files from `apps/web/public/samples`, each carrying a location, a
device, dates and names that ExifTool and hexscope both read:

| File | What it carries |
|---|---|
| `photo.jpg` | location, camera, serial number, owner, time, thumbnail |
| `photo.heic` | an iPhone-style HEIC with location and camera |
| `ai-picture.png` | an image generator's prompt and settings |
| `video.mov` | where and on what a video was recorded |
| `report.pdf` | author, programs, dates, an earlier version |
| `report.docx` | author, comments, tracked changes, a photo with a location |

## Sending

Use two accounts or two devices you own. For each channel, send each file in
each way the app offers, one file per message, and save what arrives at the
other end without opening it in an editor:

- **As a photo or video** — the app's gallery button: Telegram, WhatsApp,
  Viber, Signal, Facebook Messenger, Instagram direct messages, iMessage.
- **As a file or document** — Telegram's "File", WhatsApp's "Document",
  Viber's "File", Signal's file attachment.
- **Other ways** — Gmail and another mail service as an attachment, a Google
  Drive or iCloud link downloaded on the other side, AirDrop with "All
  Photos Data" on and off.

Name each saved file `CHANNEL__MODE__ORIGINAL`, for example
`telegram__photo__photo.jpg`, `whatsapp__document__report.pdf`,
`airdrop__alldata-off__photo.heic`. Keep the extension the channel gave it
after the original's name if it changed (`viber__photo__photo.heic.jpg`).
Note the app's version and the date in `received/README.txt`.

## Reading

```sh
cargo build --release -p hexscope-cli
python3 -I scripts/messenger-study.py received/
```

It reads each copy with ExifTool and hexscope, sets it beside the kit's
original, and writes `docs/research/messenger-study.md` (a table: what each
channel kept, what it removed, whether the picture was recompressed, the
size before and after) and `messenger-study.json`.

## What it cannot say

- One run is one version of each app on one phone; apps change what they do.
- A channel may treat a real camera photo differently from the kit's
  synthetic one; add a photo of your own to the kit to check.
- What a server keeps before it strips a file is not visible in what arrives.
