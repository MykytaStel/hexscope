import { beforeEach, describe, expect, it, vi } from "vitest";
import type { FileModel } from "./model";

const { call } = vi.hoisted(() => ({ call: vi.fn() }));
vi.mock("./rpc", () => ({ call }));

import { addCodeFacts } from "./qrfacts";

describe("QR scan requests", () => {
  beforeEach(() => call.mockReset().mockResolvedValue({ type: "codes", codes: [] }));

  it("sends known picture dimensions so large photos can be downsampled during decode", async () => {
    const model = {
      file: { format: "jpeg", dimensions: [4032, 3024], facts: [] },
      source: new Blob([new Uint8Array([1])], { type: "image/jpeg" }),
      bytes: new Uint8Array([1]),
      codeBoxes: [],
    } as unknown as FileModel;

    await addCodeFacts(model);

    expect(call).toHaveBeenCalledWith(expect.objectContaining({
      type: "codes",
      pictureDimensions: [4032, 3024],
    }));
  });
});
