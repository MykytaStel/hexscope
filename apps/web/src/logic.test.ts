// What the page says about each sample, from the real parser: the headline,
// the verdict, the tips, what a share card counts, and an email's way.
// The WebAssembly build is loaded from disk, as the worker loads it; run
// `pnpm wasm` first.
import { readFileSync } from "node:fs";
import { beforeAll, describe as group, expect, it } from "vitest";
import { cleanCopy, initSync, parse } from "./wasm/hexscope_wasm.js";
import { describe } from "./describe";
import { FileModel } from "./model";
import { headline, listed } from "./headline";
import { verdict, misfit } from "./verdict";
import { advice } from "./advice";
import { categories, categoryCount } from "./share";
import { address, mailRoute, receivedBy } from "./emailpath";
import { cleanName, kindOf, redactedName, repairedName, tooLarge, typeOf } from "./files";
import { searchable } from "./redactor";
import type { PageGlyphs } from "./worker";
import { translateText } from "./i18n";
import { decodeVerification } from "./verification";

const here = (path: string) => new URL(path, import.meta.url);

beforeAll(() => {
  initSync({ module: readFileSync(here("./wasm/hexscope_wasm_bg.wasm")) });
});

function open(name: string, bytes?: Uint8Array): FileModel {
  const b = bytes ?? new Uint8Array(readFileSync(here(`../public/samples/${name}`)));
  return new FileModel(describe(parse(b)), b, name);
}

group("incomplete PDF form checks", () => {
  it("preserves incompleteness through search and describes why cleaning can fail", () => {
    const page: PageGlyphs = {
      media: [0, 0, 100, 100],
      areas: new Float64Array(0),
      texts: [],
      boxes: new Float64Array(0),
      complete: false,
    };
    expect(searchable([page])[0].complete).toBe(false);

    const bytes = new Uint8Array(readFileSync(here("../e2e/fixtures/incomplete-form.pdf")));
    const lines = verdict(open("incomplete-form.pdf", bytes));
    expect(lines).toContainEqual(expect.objectContaining({ kind: "warning" }));
    expect(translateText("Some PDF form content could not be fully checked. Search may miss text.", "uk")).toBe(
      "Не весь вміст PDF-форм вдалося перевірити. Пошук може пропустити текст.",
    );
    expect(
      translateText(
        "no copy was made because hexscope could not fully inspect or isolate form content on a PDF page",
        "uk",
      ),
    ).toBe("Копію не створено: hexscope не зміг повністю перевірити або відокремити вміст форм на сторінці PDF");
  });
});

group("clean-copy verification", () => {
  it("compares the real cleaner output with the source parse in WASM", () => {
    const bytes = new Uint8Array(readFileSync(here("../public/samples/photo.jpg")));
    const source = parse(bytes);
    const copy = cleanCopy(bytes, false);

    const report = decodeVerification(source.verifyCopy(copy));

    expect(report.removed.map((item) => item.kind)).toContain("location");
    expect(report.present).toEqual([]);
    expect(report.unchecked.some((item) => item.kind === "lens")).toBe(true);
    expect(JSON.stringify(report)).not.toContain("Olena");
    copy.free();
    source.free();
  });
});

group("an Outlook message and an old Word file", () => {
  it("reads a message saved from Outlook as the email it came as", () => {
    const m = open("phishing.msg");
    expect(m.file.format).toBe("msg");
    expect(headline(m)).toEqual({ tone: "danger", text: "This email may not be from who it says" });
    expect(m.file.attachments).toEqual(["invoice.pdf.html"]);
    expect(advice(m).some((a) => a.text.startsWith("A link that says one site"))).toBe(true);
  });

  it("names who wrote an old Word file, and cleans it", () => {
    const m = open("plan.doc");
    expect(m.file.format).toBe("office97");
    expect(headline(m).text).toBe("This document gives away 2 things");
    expect(kindOf(m)).toBe("Word 97–2003");
    expect(misfit(m)).toBeNull();
  });
});

group("the headline", () => {
  it("is said shorter in a list of files, where the row names the file", () => {
    expect(listed(headline(open("photo.jpg")))).toMatch(/^Gives away \d+ things$/);
    expect(listed(headline(open("phishing.eml")))).toBe("May not be from who it says");
    expect(listed(headline(open("broken.png")))).toBe("Damaged");
    expect(listed(headline(open("sample.png")))).toBe("Nothing personal found");
  });

  it("counts what a photo gives away, in red for a place", () => {
    const h = headline(open("photo.jpg"));
    expect(h.tone).toBe("danger");
    expect(h.text).toMatch(/^This photo gives away \d+ things$/);
  });

  it("says a forged email may not be from who it says", () => {
    expect(headline(open("phishing.eml"))).toEqual({ tone: "danger", text: "This email may not be from who it says" });
  });

  it("counts what an honest email gives away, without calling it forged", () => {
    const h = headline(open("message.eml"));
    expect(h.text).toMatch(/^This email gives away \d+ things$/);
  });

  it("names a document, not an archive, for Word and Excel", () => {
    expect(headline(open("report.docx")).text).toMatch(/^This document /);
    expect(headline(open("budget.xlsx")).text).toMatch(/^This document /);
    expect(headline(open("deflate-demo.zip")).text).toMatch(/archive/);
  });

  it("says a damaged file is damaged", () => {
    expect(headline(open("broken.png"))).toEqual({ tone: "danger", text: "This picture is damaged" });
  });

  it("does not claim a file it does not read says nothing", () => {
    const h = headline(open("junk.bin", new TextEncoder().encode("Rar!\x1a\x07\x00 not really")));
    expect(h.tone).toBe("neutral");
  });

  it("counts the same things a share card does", () => {
    for (const name of ["photo.jpg", "report.pdf", "report.docx", "message.eml", "video.mov"]) {
      const m = open(name);
      const n = categories(m).length;
      const h = headline(m);
      if (n > 0 && !h.text.includes("may not be")) expect(h.text, name).toContain(`${n} thing`);
    }
  });
});

group("the verdict", () => {
  it("opens a damaged file on its damage", () => {
    expect(verdict(open("broken.png"))[0].kind).toBe("damage");
  });

  it("puts what a photo reveals before its health", () => {
    const kinds = verdict(open("photo.jpg")).map((l) => l.kind);
    expect(kinds.indexOf("reveals")).toBeLessThan(kinds.indexOf("healthy"));
  });

  it("points every line it can at a part of the file", () => {
    for (const name of ["photo.jpg", "redacted.pdf", "phishing.eml", "budget.xlsx"]) {
      const m = open(name);
      for (const l of verdict(m)) if (l.kind !== "healthy") expect(l.node, `${name}: ${l.text}`).toBeGreaterThanOrEqual(0);
    }
  });

  it("tells a picture named for another format by its extension", () => {
    const bytes = new Uint8Array(readFileSync(here("../public/samples/sample.png")));
    expect(misfit(open("holiday.jpg", bytes))).toMatchObject({ ext: "jpg", fits: ".png" });
    expect(misfit(open("sample.png"))).toBeNull();
  });
});

group("the tips", () => {
  it("lead with the place for a photo, and link a guide", () => {
    const tips = advice(open("photo.jpg"));
    expect(tips[0].text).toMatch(/location/);
    expect(tips[0].guide?.[0]).toBe("remove-location-from-photo.html");
  });

  it("warn about a forged email's links and files first", () => {
    expect(advice(open("phishing.eml"))[0].text).toMatch(/Do not click or open them/);
  });

  it("are three at most", () => {
    for (const name of ["photo.jpg", "redacted.pdf", "phishing.eml", "report.docx"]) expect(advice(open(name)).length).toBeLessThanOrEqual(3);
  });

  it("name a screenshot by its file name alone", () => {
    const bytes = new Uint8Array(readFileSync(here("../public/samples/sample.png")));
    expect(advice(open("Screenshot 2026-09-28 at 10.00.00.png", bytes)).some((t) => /screenshot/i.test(t.text))).toBe(true);
  });
});

group("a share card", () => {
  // What a file does or how it is locked is not something it gives away.
  const NOT_TOLD = ["auth", "encryption", "opens", "launch", "scripts", "submits"];

  it("has a category for every kind of fact a sample gives", () => {
    const samples = ["photo.jpg", "photo.heic", "cropped.jpg", "progressive.jpg", "sample.png", "video.mov", "report.pdf", "redacted.pdf",
      "report.docx", "budget.xlsx", "message.eml", "phishing.eml", "hello.wasm"];
    for (const name of samples) {
      for (const f of open(name).file.facts) {
        if (!NOT_TOLD.includes(f.kind)) expect(categoryCount([f.kind]), `${name}: ${f.kind}`).toBe(1);
      }
    }
  });

  it("counts categories, never the same one twice", () => {
    expect(categoryCount(["location", "location", "serial"])).toBe(2);
    expect(categoryCount(["nothing-we-know"])).toBe(0);
  });
});

group("file names and kinds", () => {
  it("names a copy after the file, with the extension that fits its bytes", () => {
    expect(cleanName(open("photo.jpg"))).toBe("photo-clean.jpg");
    const png = new Uint8Array(readFileSync(here("../public/samples/sample.png")));
    expect(cleanName(open("holiday.jpg", png))).toBe("holiday-clean.png");
    expect(repairedName(open("broken.png"))).toBe("broken-repaired.png");
    expect(redactedName(open("redacted.pdf"))).toBe("redacted-redacted.pdf");
    expect(cleanName(open("folder/no-extension", png))).toBe("no-extension-clean");
  });

  it("says what kind of file it is in a word", () => {
    expect(kindOf(open("report.docx"))).toBe("Word document");
    expect(kindOf(open("budget.xlsx"))).toBe("Excel workbook");
    expect(kindOf(open("message.eml"))).toBe("Email");
    expect(kindOf(open("photo.jpg"))).toBe("JPEG");
    expect(kindOf(open("deflate-demo.zip"))).toBe("ZIP");
  });

  it("types a copy by its extension, for a share sheet", () => {
    expect(typeOf("a.JPG")).toBe("image/jpeg");
    expect(typeOf("a.docx")).toMatch(/wordprocessingml/);
    expect(typeOf("noext")).toBe("");
  });

  it("says what reads a file too large for a tab", () => {
    expect(tooLarge({ name: "movie.mov", size: 3 * 1024 ** 3 })).toBe(
      'movie.mov is 3.0 GB: hexscope reads files up to 2 GB in a browser tab. The command line tool reads any size: hexscope check "movie.mov".',
    );
  });
});

group("an email's way", () => {
  it("reads addresses and servers", () => {
    expect(address('"Example Bank" <security@example-bank.com>')).toEqual({ name: "Example Bank", addr: "security@example-bank.com" });
    expect(address("plain@example.com")).toEqual({ name: "", addr: "plain@example.com" });
    expect(receivedBy("from a (b [1.2.3.4]) by mx.example.net (Postfix) with ESMTP")).toBe("mx.example.net");
    expect(receivedBy("from nowhere")).toBe("a server");
  });

  it("marks a forged message's sender, reply address, link and attachment", () => {
    const r = mailRoute(open("phishing.eml"))!;
    expect(r.stops[0]).toMatchObject({ main: "security@example-bank.com", tone: "bad" });
    expect(r.stops.map((s) => s.label)).toEqual(["Says it is from", "Sent from", "Passed through", "To"]);
    expect(r.off.map((s) => s.main)).toEqual(["verify-account@example.info", "login.example.info", "invoice.pdf.html"]);
  });

  it("lists an honest message's servers in the order they passed it on", () => {
    const r = mailRoute(open("message.eml"))!;
    expect(r.stops[0].tone).toBe("good");
    expect(r.stops.filter((s) => s.label === "Passed through").map((s) => s.main)).toEqual(["smtp.example.org", "mx.example.net", "inbox.example.net"]);
    expect(r.stops[1].notes).toContain("MacBook-Pro-Olena.local");
    expect(r.off).toEqual([]);
  });
});
