import { describe, expect, it } from "vitest";
import { familyOf } from "./family";

const bytes = (...parts: (string | number[])[]) =>
  Uint8Array.from(parts.flatMap((p) => (typeof p === "string" ? [...p].map((c) => c.charCodeAt(0)) : p)));

const oggIdentification = (packet: Uint8Array) =>
  bytes(
    "OggS",
    [0, 2],
    new Array(8).fill(0),
    [1, 0, 0, 0],
    [0, 0, 0, 0],
    new Array(4).fill(0),
    [1, packet.length],
    [...packet],
  );

describe("the build a file loads", () => {
  it.each([
    ["a JPEG", bytes([0xff, 0xd8, 0xff, 0xe0])],
    ["an MP4", bytes([0, 0, 0, 0x18], "ftypisom")],
    ["an SVG drawing", bytes('<?xml version="1.0"?>\n<svg xmlns="http://www.w3.org/2000/svg"/>')],
    ["plain text", bytes("shopping list\nmilk\n")],
    ["an unknown binary", bytes([0x7f, 0x45, 0x4c, 0x46, 2, 1, 1, 0])],
  ])("%s: the small build", (_, b) => {
    expect(familyOf(b)).toBe("media");
  });

  it.each([
    ["a Word document", bytes("PK", [3, 4], "word/document.xml")],
    ["an archive behind a program", bytes("MZ", new Array(100).fill(0), "PK", [5, 6], new Array(18).fill(0))],
    ["an Outlook message", bytes([0xd0, 0xcf, 0x11, 0xe0, 0xa1, 0xb1, 0x1a, 0xe1])],
    ["RTF", bytes("{\\rtf1\\ansi hello}")],
    ["a WebAssembly module", bytes([0, 0x61, 0x73, 0x6d, 1, 0, 0, 0])],
    ["an email", bytes("Received: from mail.example.com\r\nFrom: a@example.com\r\n\r\nhi")],
    ["a mailbox", bytes("From a@example.com Thu Oct 16 10:00:00 2026\nFrom: a@example.com\n")],
  ])("%s: the documents' build", (_, b) => {
    expect(familyOf(b)).toBe("docs");
  });

  it("a PDF: the PDF build", () => {
    expect(familyOf(bytes("%PDF-1.7\n"))).toBe("pdf");
  });

  it.each([
    ["ID3-tagged MP3", bytes("ID3", [4, 0, 0, 0, 0, 0, 0])],
    ["MPEG audio frame", bytes([0xff, 0xfb, 0x90, 0x64])],
    ["FLAC", bytes("fLaC")],
    ["Ogg Vorbis", oggIdentification(bytes([1], "vorbis"))],
    ["Ogg Opus", oggIdentification(bytes("OpusHead"))],
  ])("%s: the audio build", (_, b) => {
    expect(familyOf(b)).toBe("audio");
  });

  it.each([
    ["MPEG Layer I", bytes([0xff, 0xff, 0x90, 0x64])],
    ["MPEG Layer II", bytes([0xff, 0xfd, 0x90, 0x64])],
  ])("%s does not load the MP3-only build", (_, b) => {
    expect(familyOf(b)).toBe("media");
  });

  it("keeps an Ogg video out of the audio build", () => {
    expect(familyOf(oggIdentification(bytes([0x80], "theora")))).toBe("media");
    expect(familyOf(bytes("OggS"))).toBe("media");
  });
});
