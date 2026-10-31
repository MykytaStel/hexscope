import { describe, expect, it } from "vitest";
import { hasCleanableAudioTags } from "./audio-clean";

describe("hasCleanableAudioTags", () => {
  it("requires an MP3 edge tag and readable audio frame", () => {
    expect(hasCleanableAudioTags("mp3", ["ID3v2.4 metadata", "audio frames (not decoded)"])).toBe(true);
    expect(hasCleanableAudioTags("mp3", ["ID3v2.4 metadata"])).toBe(false);
  });

  it("offers complete FLAC files with comments or cover art", () => {
    expect(hasCleanableAudioTags("flac", ["FLAC Vorbis comments block", "audio frames (not decoded)"], true)).toBe(true);
    expect(hasCleanableAudioTags("flac", ["FLAC picture block", "audio frames (not decoded)"], true)).toBe(true);
    expect(hasCleanableAudioTags("flac", ["FLAC Vorbis comments block"], false)).toBe(false);
    expect(hasCleanableAudioTags("flac", ["FLAC Vorbis comments block"], true)).toBe(false);
    expect(hasCleanableAudioTags("flac", ["FLAC STREAMINFO block"], true)).toBe(false);
    expect(hasCleanableAudioTags("ogg", ["Vorbis comment packet"], true)).toBe(false);
  });
});
