// What the page knows about each kind of fact and part: labels, what a
// clean copy keeps and cannot take out, role names, why an entry cannot be
// played or opened. Words and rules, no drawing.
import { Role, type ZipEntryInfo } from "./model";
import { el } from "./dom";

export const KIND_NAMES = ["Container", "Field", "Warning", "Error"];
export const COLOR_TYPES: Record<number, string> = {
  0: "Greyscale",
  2: "RGB",
  3: "Palette",
  4: "Greyscale + alpha",
  6: "RGBA",
};

/** Facts a clean copy keeps, because they are part of what the file does. */
/** What QR codes say: the picture's pixels, so a clean copy keeps them. */
const QR = ["qrwifi", "qrsecret", "qrtrick", "qrplace", "qrcontact", "qrlink", "qrtext"];

export const KEPT: Record<string, string[]> = {
  jpeg: QR,
  png: QR,
  heif: QR,
  webp: QR,
  gif: QR,
  wasm: ["paths", "imports"],
  zip: ["comments", "hiddensheets", "hiddencells", "links", "notes", "hiddenslides", "embedded", "weblinks", "linkmismatch", "linkidn", ...QR],
  // The copy keeps every object the document reaches, byte for byte: its
  // links, and what it does when opened, with them.
  pdf: ["comments", "form", "attachments", "weblinks", "linkmismatch", "opens", "scripts", "launch", "submits", ...QR],
};
const QR_KEPT = "Kept: the QR code, which is part of the picture. Black it out with “Black out part of the picture” before sending.";

export const KEPT_NOTE: Record<string, string> = {
  jpeg: QR_KEPT,
  png: QR_KEPT,
  heif: QR_KEPT,
  webp: QR_KEPT,
  gif: QR_KEPT,
  wasm: "Kept: what it imports, which is the program itself, and any paths in its data, which its error messages print. Only a new build, with the paths remapped, can take those out.",
  pdf: "Kept: comments, form answers, attached files, links, and anything it does when opened, which are part of the document. Delete them in a PDF editor — or print to a new PDF, which keeps only the pages — if they should not travel with it.",
  zip: "Kept: what is part of a workbook or a deck itself — its comments, hidden sheets, rows and slides, speaker notes, links to other files and to sites — and files kept inside it, such as the workbook behind a chart. Remove them in Word, Excel or PowerPoint, then save.",
};

/** Facts shown first in colour: what someone would least want to send. */
export const STRONG = ["covered", "hiddentext", "deleted", "earlier", "photoplace", "replyto", "authfail", "linkmismatch", "linkidn", "riskyfile", "opens", "launch", "qrwifi", "qrsecret", "qrtrick"];

/** A link that opens a place on OpenStreetMap, only when clicked. */
export function mapLink(latitude: number, longitude: number): HTMLAnchorElement {
  const map = el("a", "map-link", "Open in OpenStreetMap ↗");
  const [lat, lon] = [latitude.toFixed(6), longitude.toFixed(6)];
  map.href = `https://www.openstreetmap.org/?mlat=${lat}&mlon=${lon}#map=17/${lat}/${lon}`;
  map.target = "_blank";
  map.rel = "noopener noreferrer";
  map.title = "Opens openstreetmap.org in a new tab. The coordinates leave this page only if you click.";
  return map;
}

/** What a clean copy of each kind of file cannot take out, because it is what the file shows or says. */
export const LIMITS: Record<string, string[]> = {
  photo: [
    "What the picture shows: faces, street signs, house numbers, a screen or a letter in view, reflections. Black those out with “Black out part of the picture”.",
    "Marks some apps and AI tools hide in the pixels themselves.",
  ],
  video: ["What the video shows and what its sound says: voices, place names, anything in view."],
  audio: ["What the recording says: voices, names, places, anything heard in the background."],
  pdf: [
    "Anything still visible on the pages: a name or number you did not black out. “Black out text yourself” takes it out.",
    "Text that is part of a picture, such as a scanned page or a screenshot, under a black box: the copy cannot take letters out of a picture. Check those pages by eye, or black them out before scanning.",
    "Words and numbers in the text itself that say who wrote it or for whom.",
  ],
  zip: [
    "What the text says: names, addresses and details written in the document itself.",
    "What pasted pictures show: a screenshot can show a desktop, a name, an open tab.",
  ],
  office97: [
    "What the text says, and its comments and tracked changes: hexscope reads an old Office file's properties, not its text. Save it as .docx, .xlsx or .pptx and check that copy to see them.",
    "Text Word kept from before a fast save, inside the document itself.",
  ],
};

export const FACT_LABELS: Record<string, string> = {
  camera: "Camera",
  lens: "Lens",
  serial: "Serial number",
  owner: "Owner",
  taken: "Taken",
  software: "Software",
  thumbnail: "Thumbnail",
  place: "Place named",
  caption: "Caption",
  original: "Made from",
  title: "Title",
  author: "Author",
  editor: "Last saved by",
  created: "Created",
  modified: "Modified",
  revisions: "Revisions",
  editing: "Editing time",
  company: "Company",
  application: "Application",
  template: "Template",
  subject: "Subject",
  keywords: "Keywords",
  producer: "PDF made by",
  encryption: "Encryption",
  updates: "Edited",
  history: "Editing history",
  shutter: "Shutter count",
  uptime: "Phone on for",
  linked: "Linked shots",
  names: "Function names",
  screenshot: "Screenshot",
  ai: "Made with AI",
  credentials: "Content Credentials",
  prompt: "Prompt",
  sentfrom: "Sent from",
  computer: "Computer's name",
  mailer: "Mail app",
  timezone: "Time zone",
  replyto: "Replies go to",
  weblinks: "Links",
  opens: "When opened",
  scripts: "Scripts",
  launch: "Opens a program",
  submits: "Form goes to",
  linkmismatch: "Link goes elsewhere",
  linkidn: "Lookalike address",
  riskyfile: "Risky attachment",
  returnpath: "Bounces",
  auth: "Sender checks",
  authfail: "Sender checks",
  language: "Language",
  toolchain: "Built with",
  sdk: "SDK",
  sourcemap: "Source map",
  debuginfo: "Debug info at",
  debug: "Debug info",
  paths: "Built by user",
  imports: "Asks its host to",
  covered: "Hidden, not removed",
  hiddentext: "Hidden text",
  earlier: "Taken off the page",
  comments: "Comments",
  form: "Form answers",
  attachments: "Attached files",
  hiddensheets: "Hidden sheets",
  embedded: "File inside",
  hiddencells: "Hidden rows and columns",
  links: "Links to files",
  notes: "Speaker notes",
  hiddenslides: "Hidden slides",
  tracked: "Tracked changes",
  deleted: "Deleted text",
  photoplace: "Photo inside",
  photo: "Photo inside",
  qrwifi: "Wi-Fi in a QR code",
  qrsecret: "2FA secret in a QR code",
  qrtrick: "QR code",
  qrplace: "Place in a QR code",
  qrcontact: "Contact in a QR code",
  qrlink: "QR code",
  qrtext: "QR code",
};

/** Why an entry cannot be played, or null when it can. */
export function playReason(e: ZipEntryInfo): string | null {
  if (e.playable) return null;
  if (e.flags & 1) return "Encrypted: its bytes cannot be decompressed without the password.";
  if (e.method === 0) return "Stored: its bytes are the file itself, with nothing to decompress.";
  if (e.method !== 8) return "Compressed with a method this tool does not decompress.";
  if (e.compressed === 0) return "Empty: there is nothing to decompress.";
  return "Its data runs past the end of the file.";
}

/** Role names in plain words; "content" depends on what the file holds. */
export function roleName(role: number, format: string): string {
  switch (role) {
    case Role.Content:
      return format === "zip"
        ? "Files"
        : format === "pdf"
          ? "Pages, fonts, images"
          : format === "eml" || format === "msg"
            ? "Message and attachments"
          : format === "office97"
            ? "The document"
          : format === "wasm"
            ? "Code and data"
            : "Picture";
    case Role.Metadata:
      return "Metadata";
    case Role.Thumbnail:
      return "Thumbnail";
    case Role.Structure:
      return "Structure";
    case Role.Hidden:
      return "Hidden or unaccounted";
    default:
      return "Damaged";
  }
}

/** How deep archives may nest on screen, matching the worker's limit. */
export const MAX_NESTING = 4;

/** Why an entry cannot be opened as a file of its own, or null when it can. */
export function openReason(e: ZipEntryInfo, nested: number): string | null {
  if (nested >= MAX_NESTING) return `Files nest at most ${MAX_NESTING} deep here.`;
  if (e.openable) return null;
  if (e.flags & 1) return "Encrypted: its bytes cannot be read without the password.";
  if (e.method !== 0 && e.method !== 8) return "Compressed with a method this tool does not decompress.";
  if (e.uncompressed === 0) return "Empty: there is nothing to open.";
  return "Its data runs past the end of the file.";
}

/** Formats a damaged file of which hexscope can save what survived. */
export const REPAIRABLE = ["png", "jpeg", "zip"];

export const degrees = (v: number, pos: string, neg: string) =>
  `${Math.abs(v).toFixed(5)}° ${v >= 0 ? pos : neg}`;
