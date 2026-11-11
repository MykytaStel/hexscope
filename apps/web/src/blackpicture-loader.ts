import type { FileModel } from "./model";
import type { PictureCopy } from "./blackpicture";

export function canBlackOut(m: FileModel): boolean {
  return ["jpeg", "png", "heif", "webp", "gif"].includes(m.file.format);
}

/** The editor and optional OCR stay off the initial inspection path. */
export async function openBlackPicture(m: FileModel, copy: PictureCopy): Promise<void> {
  const editor = await import("./blackpicture");
  await editor.openBlackPicture(m, copy);
}
