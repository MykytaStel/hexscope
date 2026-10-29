import { afterEach, expect, it, vi } from "vitest";
import { shareFile } from "./files";

afterEach(() => vi.unstubAllGlobals());

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
