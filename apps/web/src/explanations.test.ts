import { describe, expect, it } from "vitest";
import audio from "./explanations/explain-audio.json";
import { explain, explanation } from "./explanations";

describe("lazy audio explanations", () => {
  it("loads the audio catalogue for MP3 files", async () => {
    const entry = Object.entries(audio).find(([, value]) =>
      Array.isArray(value) && value[0].startsWith("An MP3 stream:"),
    );
    if (!entry || !Array.isArray(entry[1])) throw new Error("the audio catalogue has no MP3 explanation");

    await explain("mp3");

    expect(explanation(Number(entry[0]))?.text).toBe(entry[1][0]);
  });
});
