// "Save clean copies" for a list of files: a copy of each that gives
// something away, made one after another in the worker. On a computer
// they go into one ZIP, written as they are made; on a phone that shares
// files, they wait for "Share", one file each.
import { call } from "./rpc";
import { cleanable, type BatchItem, type BatchView } from "./batch";
import { StoredZip } from "./zipwrite";
import { phoneCanShare, saveAs } from "./files";

/** Clean copies shared one file each, at most: past this, or this many bytes, they go as a ZIP. */
const MAX_SHARED = 50;
const MAX_SHARED_BYTES = 400 * 1024 * 1024;

/**
 * Makes a clean copy of every file that gives something away. On a
 * computer they are saved as one ZIP; on a phone that can share files,
 * they wait for "Share", one file each, so they can go straight to a chat.
 */
export async function cleanCopies(items: BatchItem[], keepNames: boolean, view: BatchView, current: () => boolean): Promise<void> {
  view.result = "Making the clean copies…";
  const todo = items.filter((i) => cleanable(i) && !i.skip);
  // Kept names keep a folder's own folders, in the ZIP.
  const copyName = (i: BatchItem) => (keepNames ? i.file.webkitRelativePath || i.file.name : i.cleanName);
  const bytesTotal = todo.reduce((n, i) => n + i.file.size, 0);
  const share = phoneCanShare() && todo.length <= MAX_SHARED && bytesTotal <= MAX_SHARED_BYTES;
  // Each copy goes into the archive as soon as it is made, and out of memory.
  const zip = new StoredZip();
  const shared: File[] = [];
  const failed: string[] = [];
  let nothing = 0;
  let done = 0;
  for (const item of todo) {
    const r = await call({ type: "clean", bytes: new Uint8Array(await item.file.arrayBuffer()) });
    if (!current()) return;
    try {
      if (r.type === "cleaned" && !r.error) {
        if (share) shared.push(new File([r.bytes as BlobPart], copyName(item).split("/").pop() ?? item.cleanName, { type: item.file.type }));
        else zip.add(copyName(item), r.bytes);
      } else if (r.type === "cleaned" && r.error.startsWith("there is nothing")) nothing++;
      else failed.push(`${item.file.name}: ${r.type === "cleaned" ? r.error : "it could not be read"}`);
    } catch (e) {
      failed.push(`${item.file.name}: ${e instanceof Error ? e.message : String(e)}`);
    }
    done++;
    if (todo.length > 10) view.result = `Making the clean copies… ${done} of ${todo.length}`;
  }
  const said: string[] = [];
  const name = "hexscope-clean-copies.zip";
  const saveZip = (z: StoredZip) => saveAs(name, z.finish());
  if (zip.size > 0) {
    saveZip(zip);
    said.push(`Saved ${zip.size === 1 ? "1 clean copy" : `${zip.size} clean copies`} as ${name}.`);
  }
  if (nothing > 0) said.push(`${nothing === 1 ? "1 file had" : `${nothing} files had`} nothing hexscope can remove.`);
  if (failed.length > 0) said.push(`Not cleaned — ${failed.join("; ")}.`);
  if (shared.length > 0) {
    const n = shared.length === 1 ? "1 clean copy" : `${shared.length} clean copies`;
    const shareBtn = document.createElement("button");
    shareBtn.className = "btn btn-primary";
    shareBtn.textContent = `Share ${n}`;
    shareBtn.addEventListener("click", () => {
      navigator.share({ files: shared }).catch(() => {
        // Cancelled, or refused: the copies are still here to save.
      });
    });
    const saveBtn = document.createElement("button");
    saveBtn.className = "btn";
    saveBtn.textContent = "Save as a ZIP";
    saveBtn.addEventListener("click", async () => {
      const z = new StoredZip();
      for (const f of shared) z.add(f.name, new Uint8Array(await f.arrayBuffer()));
      saveZip(z);
    });
    view.offer(`Made ${n}. ${said.join(" ")}`.trim() + " ", shareBtn, saveBtn);
    return;
  }
  view.result = said.join(" ") || "Nothing to clean.";
}
