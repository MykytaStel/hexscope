// The guides, once: every page's list of the others is made from this one at
// build (vite.config.ts), so a new guide is linked from all of them at once.

type Topic = "photos" | "documents" | "email" | "files";

/** Each guide's page, its title in the lists, and what it is about. */
export const GUIDES: readonly { file: string; title: string; topic: Topic }[] = [
  { file: "exif-viewer.html", title: "EXIF viewer: what a photo's metadata says", topic: "photos" },
  { file: "remove-metadata-from-photo.html", title: "How to remove EXIF and metadata from a photo", topic: "photos" },
  { file: "remove-location-from-photo.html", title: "How to remove the location from a photo", topic: "photos" },
  { file: "remove-metadata-from-video.html", title: "How to remove metadata from a video", topic: "photos" },
  { file: "ai-image-metadata.html", title: "Was this picture made by AI?", topic: "photos" },
  { file: "photos-from-sensitive-places.html", title: "A photo from a sensitive place: what gives it away", topic: "photos" },
  { file: "what-a-screenshot-gives-away.html", title: "What a screenshot gives away", topic: "photos" },
  { file: "remove-metadata-from-pdf.html", title: "How to remove metadata from a PDF", topic: "documents" },
  { file: "remove-metadata-from-word.html", title: "How to remove metadata from a Word document", topic: "documents" },
  { file: "remove-metadata-from-libreoffice.html", title: "How to remove metadata from LibreOffice and RTF files", topic: "documents" },
  { file: "check-document-before-sending.html", title: "Check a document before you send it", topic: "documents" },
  { file: "black-out-a-pdf.html", title: "How to black out a PDF properly", topic: "documents" },
  { file: "hidden-text-in-pdf.html", title: "Text no one can see in a PDF", topic: "documents" },
  { file: "pdf-hidden-versions.html", title: "Deleted from a PDF, but still inside it", topic: "documents" },
  { file: "is-this-email-real.html", title: "Is this email real?", topic: "email" },
  { file: "metadata-removers-compared.html", title: "ExifTool, MAT2 or hexscope: what each leaves in a file", topic: "files" },
  { file: "what-file-type-is-this.html", title: "What type of file is this?", topic: "files" },
  { file: "png-wont-open.html", title: "Why won't my PNG open?", topic: "files" },
  { file: "your-files-stay-private.html", title: "Your files stay on your device", topic: "files" },
];

/**
 * The other guides as list items, for the page `file`: those on its own
 * topic first, then the rest, each in the order above.
 */
export function guideList(file: string): string {
  const own = GUIDES.find((g) => g.file === file)?.topic;
  const others = GUIDES.filter((g) => g.file !== file);
  return [...others.filter((g) => g.topic === own), ...others.filter((g) => g.topic !== own)]
    .map((g) => `<li><a href="${g.file}">${g.title.replace(/&/g, "&amp;")}</a></li>`)
    .join("\n            ");
}
