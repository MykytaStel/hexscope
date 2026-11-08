import { afterEach, expect, it, vi } from "vitest";
import { saveAs, shareFile, typeOf } from "./files";

afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

const photo = () => new File([new Uint8Array([0xff, 0xd8, 0xff, 0xd9])], "film-copy.jpg", { type: "image/jpeg" });

it("hands the one file to the share sheet and says it was shared", async () => {
  const share = vi.fn(async () => undefined);
  vi.stubGlobal("navigator", { share });
  const file = photo();
  await expect(shareFile(file)).resolves.toBe("shared");
  expect(share).toHaveBeenCalledWith({ files: [file] });
});

it("tells a dismissed share sheet from one that did not open", async () => {
  vi.stubGlobal("navigator", { share: async () => { throw new DOMException("Share cancelled", "AbortError"); } });
  await expect(shareFile(photo())).resolves.toBe("cancelled");
  vi.stubGlobal("navigator", { share: async () => { throw new DOMException("No file sharing", "NotAllowedError"); } });
  await expect(shareFile(photo())).resolves.toBe("failed");
  vi.stubGlobal("navigator", { share: async () => { throw new TypeError("Not a shareable file"); } });
  await expect(shareFile(photo())).resolves.toBe("failed");
});

it("gives a clean-copy download the media type its filename says", () => {
  vi.useFakeTimers();
  let saved: Blob | undefined;
  const createObjectURL = vi.spyOn(URL, "createObjectURL").mockImplementation((value) => {
    if (value instanceof Blob) saved = value;
    return "blob:clean-copy";
  });
  const click = vi.fn();
  vi.stubGlobal("document", {
    createElement: () => ({ set href(_value: string) {}, set download(_value: string) {}, click }),
  });

  saveAs("photo-clean.jpg", new Blob([new Uint8Array([0xff, 0xd8, 0xff, 0xd9])]));

  expect(createObjectURL).toHaveBeenCalledOnce();
  expect(saved?.type).toBe("image/jpeg");
});

it("gives a gzip-compressed SVG its registered SVG media type", () => {
  expect(typeOf("drawing.svgz")).toBe("image/svg+xml");
});

it("gives a batch CSV report its registered text media type", () => {
  expect(typeOf("hexscope-batch-coverage.csv")).toBe("text/csv");
});
