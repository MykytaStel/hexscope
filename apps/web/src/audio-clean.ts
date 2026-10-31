/** Whether a parsed MP3 has an edge tag and a readable MPEG frame to preserve. */
export function hasCleanableMp3Tags(format: string, labels: readonly string[]): boolean {
  return format === "mp3"
    && labels.includes("audio frames (not decoded)")
    && labels.some((label) =>
      label.startsWith("ID3v2.")
      || label === "ID3v1 tag"
      || label.startsWith("APE audio metadata"),
    );
}
