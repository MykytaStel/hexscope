import type { ParsedFile } from "./model";

/** What each kind of clean copy leaves out, said before it is made. */
export const CLEAN_NOTES: Record<string, string> = {
  zip: "Removes the document's properties, the camera data and location of every photo in it, and the properties of a workbook or deck kept inside it. In a Word document, tracked changes are accepted — what was deleted goes, with its text — and comments are deleted, with their authors.",
  archive:
    "Leaves out what the Mac noted about each file in __MACOSX — where it was downloaded from, with which app, its tags. The files inside are copied byte for byte.",
  webp: "Leaves out the EXIF, XMP and Content Credentials chunks: camera, place, dates, editing history, who or what made it. The picture is copied byte for byte.",
  gif: "Leaves out the comments and the XMP. Every frame is copied byte for byte, with its timing.",
  heif: "Blanks the camera data, location, serial numbers and XMP where they lie, so the file keeps its size. The picture and its thumbnail are copied unchanged.",
  tiff: "Empties the camera, place, time and serial number tags where they lie, and the EXIF and GPS tables, so the file keeps its size. The picture is copied byte for byte.",
  png: "Removes the text notes — an image generator's prompt among them — EXIF, XMP, Content Credentials and the time it was last changed. The pixels are copied byte for byte.",
  video:
    "Blanks the location, the camera, the software and the dates where they lie, so the file keeps its size. The picture and sound are copied byte for byte.",
  audio:
    "Blanks the location, the device, the software and the dates where they lie, so the file keeps its size. The sound is copied byte for byte.",
  mp3: "Removes the ID3v2, APE, ID3v1 Plus and ID3v1 tag blocks from the start or end of the file. The compressed audio frames are copied byte for byte; they are not decoded or re-encoded.",
  flac: "Removes FLAC Vorbis comments and embedded pictures such as cover art. Keeps other supported metadata blocks and compressed audio frames byte for byte; the audio is not decoded or re-encoded. If a metadata block cannot be inspected, Hexscope does not make a copy.",
  wav: "Removes complete WAVE INFO and Broadcast Wave metadata chunks. This also removes Broadcast Wave time references, loudness values and coding history. WAVE format and audio data chunks are copied byte for byte; audio is not decoded or re-encoded. A copy is offered only after a complete structural read.",
  ogg: "Clears the Vorbis comments or Opus tags in every fully checked stream. The compressed audio packets are copied byte for byte; they are not decoded or re-encoded. Incomplete, interleaved or unsupported Ogg streams, and Vorbis streams without a setup header, stay read-only.",
  wasm: "Leaves out the custom sections that say who built it and how: function names, tools, source map and debug info links, DWARF. The code and data are copied byte for byte; paths inside the data are part of the program, and stay.",
  svg: "Leaves out the metadata, Inkscape's names, paths and settings, a comment naming the program and Illustrator's own copy of the drawing; a linked picture keeps only its file name, and a photo inside is cleaned as a photo of its own. The shapes, text and scripts are copied as they are.",
  rtf: "Leaves out the document's information — title, author, who saved it last, company, dates, editing time — the program and template that made it, its custom properties and the numbers that tie its editing sessions together. Tracked changes are accepted and comments deleted, with their authors; the rest of the text is copied as it is.",
  office97:
    "Blanks the document's properties where they lie — title, author, last editor, company, template, dates, editing time — and the sectors no part uses, where deleted text can remain. Nothing else in the file moves.",
  pdf: "Writes the document anew with only what its pages use: no author, programs or dates, no XMP, no earlier versions. Text under black boxes or hidden from view is taken out, and marks for redaction applied, with every other letter left where it was; a scanned page loses its pixels under a box; photos lose their camera data. Notes go, every other mark loses who made it, and attached files go — what was typed or signed on the page stays.",
  picture:
    "Removes the camera data, location, serial numbers, the maker's notes, thumbnail, comments and Content Credentials. The picture itself is copied unchanged.",
  jpegHdr:
    "Removes personal metadata from the main picture and its HDR gain map. Keeps both pictures and the technical HDR settings needed to display them; the photo may look less bright on HDR screens if the gain map is missing.",
  jpegMpfUnsupported:
    "This JPEG has an additional indexed picture that Hexscope cannot safely clean yet, so it will not make a clean copy.",
};

export function cleanCopyNote(file: Pick<ParsedFile, "format" | "labels" | "values">): string {
  if (file.format === "flac") return CLEAN_NOTES.flac;
  if (file.format === "jpeg") {
    const indexedPictures = file.labels
      .map((label, index) => (label === "picture listed in the MPF index" ? file.values[index] : undefined))
      .filter((value): value is string => value !== undefined);
    if (indexedPictures.length === 1 && indexedPictures[0] === "HDR gain map") return CLEAN_NOTES.jpegHdr;
    if (indexedPictures.length > 0) return CLEAN_NOTES.jpegMpfUnsupported;
  }
  return CLEAN_NOTES[file.format] ?? CLEAN_NOTES.picture;
}

