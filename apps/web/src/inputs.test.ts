import { describe, expect, it, vi } from "vitest";
import { clipboardImages } from "./inputs";

describe("clipboard image intake", () => {
  it("collects image items with stable file names", async () => {
    const items = [
      { types: ["text/plain", "image/jpeg"], getType: vi.fn().mockResolvedValue(new Blob(["first"], { type: "image/jpeg" })) },
      { types: ["image/png"], getType: vi.fn().mockResolvedValue(new Blob(["second"], { type: "image/png" })) },
    ] as unknown as ClipboardItems;

    const files = await clipboardImages(items);

    expect(files.map(({ name, type }) => [name, type])).toEqual([
      ["pasted-picture.jpg", "image/jpeg"],
      ["pasted-picture.png", "image/png"],
    ]);
  });

  it("rejects the whole read when an image item cannot be retrieved", async () => {
    const items = [
      { types: ["image/png"], getType: vi.fn().mockResolvedValue(new Blob(["first"], { type: "image/png" })) },
      { types: ["image/jpeg"], getType: vi.fn().mockRejectedValue(new Error("permission expired")) },
    ] as unknown as ClipboardItems;

    await expect(clipboardImages(items)).rejects.toThrow("permission expired");
  });
});
