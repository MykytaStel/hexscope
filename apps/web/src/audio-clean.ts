/** Whether the parsed audio file has a metadata block the core can remove. */
export function hasCleanableAudioTags(
  format: string,
  labels: readonly string[],
  metadataComplete = false,
): boolean {
  if (format === "mp3") {
    return labels.includes("audio frames (not decoded)")
      && labels.some((label) =>
        label.startsWith("ID3v2.")
        || label === "ID3v1 tag"
        || label.startsWith("APE audio metadata"),
      );
  }
  return format === "flac"
    && metadataComplete
    && labels.includes("audio frames (not decoded)")
    && labels.includes("FLAC Vorbis comments block");
}
