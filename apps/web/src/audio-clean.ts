/** Whether the parsed audio file has a metadata block the core can remove. */
export function hasCleanableAudioTags(
  format: string,
  labels: readonly string[],
  metadataComplete = false,
  facts: readonly { kind: string }[] = [],
): boolean {
  if (format === "mp3") {
    return labels.includes("audio frames (not decoded)")
      && labels.some((label) =>
        label.startsWith("ID3v2.")
        || label === "ID3v1 tag"
        || label.startsWith("APE audio metadata"),
      );
  }
  if (format === "flac") {
    return metadataComplete
      && labels.includes("audio frames (not decoded)")
      && (labels.includes("FLAC Vorbis comments block") || labels.includes("FLAC picture block"));
  }
  const vorbisComments = labels.filter((label) => label === "Vorbis comment packet").length;
  const vorbisSetups = labels.filter((label) => label === "Vorbis setup packet (not decoded)").length;
  return format === "ogg"
    && metadataComplete
    && facts.length > 0
    && vorbisComments === vorbisSetups
    && (vorbisComments > 0 || labels.includes("Opus tags packet"));
}
