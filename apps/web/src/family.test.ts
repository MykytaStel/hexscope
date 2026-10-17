import { describe, expect, it } from "vitest";
import { familyOf } from "./family";

const bytes = (...parts: (string | number[])[]) =>
  Uint8Array.from(parts.flatMap((p) => (typeof p === "string" ? [...p].map((c) => c.charCodeAt(0)) : p)));

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
});
