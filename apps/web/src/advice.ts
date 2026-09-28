// What to do about what a file reveals: a few plain sentences, chosen by
// what was found and in what kind of file, most pressing first. Knowing a
// photo holds a location is half of it; the other half is when that
// matters — posting on a social network usually drops it, sending the file
// itself does not — and how to stop the next file carrying it.
import type { FileModel } from "./model";

/** How phones and computers name a screenshot, in a few languages. */
export const SCREENSHOT_NAME = /screen ?shot|знімок екрана|снимок экрана|bildschirmfoto|capture d.écran|captura de pantalla|schermata|zrzut ekranu/i;

/** Tips shown at most, so the ones that matter are read. */
const MAX = 3;

interface Rule {
  /** The fact kinds any of which make this tip apply. */
  when: string[];
  /** File formats it is for; all when absent. */
  formats?: string[];
  text: string;
  /** A guide that says more: its page, and what it is called. */
  guide?: [string, string];
}

/** A tip, and the guide that says more about it. */
export interface Tip {
  text: string;
  guide?: [string, string];
}

const RULES: Rule[] = [
  {
    when: ["prompt", "ai", "credentials"],
    text: "A picture an image generator made carries the prompt it was given, the program and often the model — and Content Credentials name who or what made it and how it was edited. Whoever gets the file can read them. The clean copy leaves them out; to be open that AI made it, say so where you post it.",
  },
  {
    when: ["screenshot"],
    text: "A screenshot shows whatever was on the screen: notifications and who sent them, other tabs, your name or photo in a corner, the time and the network. Look along its edges before sending it, and black out what should not go with “Black out part of the picture”.",
    guide: ["what-a-screenshot-gives-away.html", "What a screenshot gives away"],
  },
  {
    when: ["linkmismatch", "linkidn", "riskyfile"],
    formats: ["eml"],
    text: "A link that says one site and goes to another, an address in lookalike letters, a file that is a program or a web page: each is how a fake message gets what it wants. Do not click or open them. To visit the site, type its address yourself.",
    guide: ["is-this-email-real.html", "Is this email real?"],
  },
  {
    when: ["replyto", "authfail"],
    formats: ["eml"],
    text: "If replies go to another domain, or the sender's domain did not vouch for the message, it may not be from who it says. Do not reply, open its files or follow its links; ask the sender another way — a number you already have.",
    guide: ["is-this-email-real.html", "Is this email real?"],
  },
  {
    when: ["sentfrom", "computer"],
    formats: ["eml"],
    text: "The first server wrote down where the message was sent from — often a home or office address, which says roughly where the sender was — and some mail apps put the computer's name in the message's ID. Webmail such as Gmail usually leaves the sender's address out; a desktop mail app sending through its provider may not.",
  },
  {
    when: ["location"],
    formats: ["jpeg", "heif", "png", "webp", "gif"],
    text: "Instagram, Facebook and X usually remove the location when you post. Sending the photo itself keeps it: by email, as a file in Telegram or WhatsApp, through a cloud link, on a forum or a marketplace. For those, save a clean copy. To keep new photos from recording it, turn off location for the camera app.",
    guide: ["remove-location-from-photo.html", "How to remove the location from a photo"],
  },
  {
    when: ["location"],
    formats: ["video"],
    text: "A video sent as a file — by email, in a messenger, through a cloud link — keeps where it was recorded. Save a clean copy before sending it; turn off location for the camera app to keep new ones from recording it.",
  },
  {
    when: ["covered"],
    formats: ["pdf"],
    text: "A black box drawn over text, or over part of a scanned page, hides it only on screen and on paper: anyone can still select, copy or search the text, or take the picture out whole. Do not send this file. The clean copy takes out what is under the boxes — the letters, and the picture's pixels there — and applies any marks for redaction, leaving the rest in place; open it here to check.",
    guide: ["black-out-a-pdf.html", "How to black out a PDF properly"],
  },
  {
    when: ["form", "attachments"],
    formats: ["pdf"],
    text: "A filled-in form keeps its answers as data, and attached files ride along inside: anyone who opens the file can read both, whatever the pages show. If only the pages should be sent, print the PDF to a new PDF, which flattens the form and leaves attachments behind.",
  },
  {
    when: ["opens", "launch", "linkmismatch", "scripts", "submits"],
    formats: ["pdf"],
    text: "This PDF does more than show pages: it runs a script, asks to open a program, sends a form's answers away, or has a link that says one site and goes to another. If you did not expect that, open it with scripts turned off (Acrobat: Preferences → JavaScript), never let it start a program, and type any address yourself.",
  },
  {
    when: ["hiddentext"],
    formats: ["pdf", "zip"],
    text: "Text no one can see on the page is still read by search, by screen readers and by programs that read the file — hiring systems and AI tools among them. If you did not put it there, ask who did. The clean copy removes it.",
  },
  {
    when: ["updates"],
    text: "What earlier edits deleted or replaced is still in the file, and any PDF reader can bring it back. Save a clean copy before sending it: it keeps only what the pages show now.",
  },
  {
    when: ["deleted", "tracked", "comments"],
    formats: ["zip"],
    text: "Tracked changes and comments travel with the file: whoever gets it can show them (Review → All Markup) and read what was deleted, and who wrote what. The clean copy accepts every change and deletes the comments, as Word's Accept All Changes and Delete All Comments would; to reject a change instead, do it in Word first.",
  },
  {
    when: ["hiddensheets", "hiddencells", "hiddenslides", "notes", "links"],
    formats: ["zip"],
    text: "Hiding a sheet, a row or a slide only folds it away: whoever opens the file can unhide it in a click, and speaker notes travel with every copy of a deck. Delete what should not be sent, rather than hiding it; Excel's links to other files name where they are on your computer — break them (Data → Edit Links) before sending.",
  },
  {
    when: ["embedded"],
    formats: ["zip"],
    text: "A chart pasted from Excel brings the whole workbook with it: every sheet and every number, not only the ones the chart shows, and who made it. Anyone can open it (right-click the chart → Edit Data). To send only the picture, paste the chart as a picture (Paste Special → Picture); otherwise delete what the workbook should not hold. The clean copy takes out who made the workbook, not its sheets — open it here to see them.",
  },
  {
    when: ["photoplace"],
    formats: ["zip", "pdf"],
    text: "A photo placed in a document keeps its own location and camera data. The clean copy removes it from every photo inside.",
  },
  {
    when: ["paths"],
    text: "The paths come from the computer that built it. Rebuild with them rewritten — --remap-path-prefix for Rust, -ffile-prefix-map for C and C++ — so the next build does not carry a user name.",
  },
  {
    when: ["serial", "shutter"],
    formats: ["jpeg", "heif", "png", "webp", "gif"],
    text: "A camera's serial number is in every photo it takes, so photos shared under different names can be traced to one camera. The clean copy removes it.",
  },
  {
    when: ["linked", "uptime"],
    text: "These IDs, and how long the phone had been on, tie this photo to others from the same phone. The clean copy removes them.",
  },
  {
    when: ["place", "history", "original"],
    formats: ["jpeg", "heif", "png", "webp", "gif"],
    text: "Editors such as Lightroom and Photoshop add their own record: the place typed in, the original file's name, each step of the edit. Turning off location on the phone does not touch it — export without metadata, or save a clean copy.",
  },
  {
    when: ["owner"],
    formats: ["jpeg", "heif", "png", "webp", "gif"],
    text: "The owner's name comes from the camera's own settings: change it there, and new photos will stop carrying it.",
  },
  {
    when: ["author", "editor", "company"],
    formats: ["zip"],
    text: "Word, Excel and PowerPoint write the name of the account that made and saved a file. The clean copy removes it; in Office, File → Info → Check for Issues → Inspect Document does too.",
  },
  {
    when: ["author", "application", "producer", "created", "modified"],
    formats: ["pdf"],
    text: "The document's properties say who made it, with what and when. The clean copy leaves them out; so does exporting again with document properties turned off.",
  },
  {
    when: ["names", "toolchain", "sourcemap", "debug", "debuginfo"],
    formats: ["wasm"],
    text: "Custom sections can be left out at build time: wasm-bindgen --remove-name-section --remove-producers-section, or wasm-opt --strip-debug --strip-producers. The clean copy does it for this file.",
  },
  {
    when: ["thumbnail"],
    text: "The thumbnail is a second, small copy of the picture. Editors do not always update it, so it can still show what was cropped out.",
  },
];

/** The tips for this file, most pressing first. */
export function advice(m: FileModel): Tip[] {
  const f = m.file;
  const kinds = new Set([...(f.location ? ["location"] : []), ...f.facts.map((x) => x.kind)]);
  // Phones and computers that do not mark a screenshot still name it as one.
  if (SCREENSHOT_NAME.test(m.name.split("/").pop() ?? "")) kinds.add("screenshot");
  return RULES.filter((r) => (!r.formats || r.formats.includes(f.format)) && r.when.some((k) => kinds.has(k)))
    .slice(0, MAX)
    .map(({ text, guide }) => ({ text, guide }));
}
