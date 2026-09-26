// What to do about what a file reveals: a few plain sentences, chosen by
// what was found and in what kind of file, most pressing first. Knowing a
// photo holds a location is half of it; the other half is when that
// matters — posting on a social network usually drops it, sending the file
// itself does not — and how to stop the next file carrying it.
import type { FileModel } from "./model";

/** Tips shown at most, so the ones that matter are read. */
const MAX = 3;

interface Rule {
  /** The fact kinds any of which make this tip apply. */
  when: string[];
  /** File formats it is for; all when absent. */
  formats?: string[];
  text: string;
}

const RULES: Rule[] = [
  {
    when: ["location"],
    formats: ["jpeg", "heif", "png"],
    text: "Instagram, Facebook and X usually remove the location when you post. Sending the photo itself keeps it: by email, as a file in Telegram or WhatsApp, through a cloud link, on a forum or a marketplace. For those, save a clean copy. To keep new photos from recording it, turn off location for the camera app.",
  },
  {
    when: ["location"],
    formats: ["video"],
    text: "A video sent as a file — by email, in a messenger, through a cloud link — keeps where it was recorded. Save a clean copy before sending it; turn off location for the camera app to keep new ones from recording it.",
  },
  {
    when: ["covered"],
    formats: ["pdf"],
    text: "A black box drawn over text hides it only on screen and on paper: anyone can still select, copy or search it. Do not send this file. The clean copy takes the text out from under the boxes and applies any marks for redaction, leaving the rest of each line in place; open it here to check.",
  },
  {
    when: ["hiddentext"],
    formats: ["pdf"],
    text: "Text no one can see on the page is still read by search, by screen readers and by programs that read the file — hiring systems and AI tools among them. If you did not put it there, ask who did. The clean copy removes it.",
  },
  {
    when: ["updates"],
    text: "What earlier edits deleted or replaced is still in the file, and any PDF reader can bring it back. Save a clean copy before sending it: it keeps only what the pages show now.",
  },
  {
    when: ["deleted", "tracked", "comments"],
    formats: ["zip"],
    text: "Tracked changes and comments travel with the file: whoever gets it can show them (Review → All Markup) and read what was deleted, and who wrote what. Before sending it, accept or reject every change, delete the comments, and save.",
  },
  {
    when: ["hiddensheets", "hiddencells", "hiddenslides", "notes", "links"],
    formats: ["zip"],
    text: "Hiding a sheet, a row or a slide only folds it away: whoever opens the file can unhide it in a click, and speaker notes travel with every copy of a deck. Delete what should not be sent, rather than hiding it; Excel's links to other files name where they are on your computer — break them (Data → Edit Links) before sending.",
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
    formats: ["jpeg", "heif", "png"],
    text: "A camera's serial number is in every photo it takes, so photos shared under different names can be traced to one camera. The clean copy removes it.",
  },
  {
    when: ["linked", "uptime"],
    text: "These IDs, and how long the phone had been on, tie this photo to others from the same phone. The clean copy removes them.",
  },
  {
    when: ["place", "history", "original"],
    formats: ["jpeg", "heif", "png"],
    text: "Editors such as Lightroom and Photoshop add their own record: the place typed in, the original file's name, each step of the edit. Turning off location on the phone does not touch it — export without metadata, or save a clean copy.",
  },
  {
    when: ["owner"],
    formats: ["jpeg", "heif", "png"],
    text: "The owner's name comes from the camera's own settings: change it there, and new photos will stop carrying it.",
  },
  {
    when: ["author", "editor", "company"],
    formats: ["zip"],
    text: "Word, Excel and PowerPoint write the name of the account that made and saved a file. In Office, File → Info → Check for Issues → Inspect Document removes it; comments and tracked changes keep their authors either way.",
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
export function advice(m: FileModel): string[] {
  const f = m.file;
  const kinds = new Set([...(f.location ? ["location"] : []), ...f.facts.map((x) => x.kind)]);
  return RULES.filter((r) => (!r.formats || r.formats.includes(f.format)) && r.when.some((k) => kinds.has(k)))
    .slice(0, MAX)
    .map((r) => r.text);
}
