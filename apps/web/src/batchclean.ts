// "Save clean copies" for a list of files: a copy of each that gives
// something away, made one after another in the worker. On a computer
// they go into one ZIP, written as they are made; on a phone that shares
// files, they wait for "Share", one file each.
import { call } from "./rpc";
import { batchReceipt, receiptRow, type ReceiptRow } from "./batch-receipt";
import { cleanable, type BatchItem } from "./batch";
import { StoredZip } from "./zipwrite";
import { phoneCanShare, saveAs } from "./files";
import { currentLocale, translateText } from "./i18n";

/** Clean copies shared one file each, at most: past this, or this many bytes, they go as a ZIP. */
const MAX_SHARED = 50;
const MAX_SHARED_BYTES = 400 * 1024 * 1024;

/** The batch output needed while clean copies are made. */
export interface CleanCopiesView {
  result: string;
  offer(text: string, ...actions: HTMLButtonElement[]): void;
  render?(items: BatchItem[]): void;
}

/**
 * Makes a clean copy of every file that gives something away. On a
 * computer they are saved as one ZIP; on a phone that can share files,
 * they wait for "Share", one file each, so they can go straight to a chat.
 */
export async function cleanCopies(items: BatchItem[], keepNames: boolean, view: CleanCopiesView, current: () => boolean): Promise<void> {
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
  const rows: ReceiptRow[] = [];
  const names = new Set<string>(["hexscope-report.json"]);
  let made = 0;
  let nothing = 0;
  let done = 0;
  for (const item of todo) {
    try {
      const r = await call({ type: "clean", source: item.file, verify: true });
      if (!current()) return;
      if (r.type === "cleaned" && !r.error) {
        const row = receiptRow(items.indexOf(item) + 1, "written", r.verification);
        let name = copyName(item).replace(/\\/g, "/").split("/").filter((part) => part && part !== "." && part !== "..").join("/") || item.cleanName;
        if (share) name = name.split("/").pop()!;
        if (names.has(name)) name = `${items.indexOf(item) + 1}-${name}`;
        while (names.has(name)) name = `copy-${name}`;
        names.add(name);
        if (share) shared.push(new File([r.copy], name, { type: item.file.type }));
        else await zip.add(name, r.copy);
        if (!current()) return;
        rows.push(row);
        item.copyStatus = `${row.verification.removed.length} removed · ${row.verification.present.length} present · ${row.verification.unchecked.length} unchecked`;
        made++;
      } else if (r.type === "cleaned" && r.error.startsWith("there is nothing")) nothing++;
      else failed.push(`${item.file.name}: ${r.type === "cleaned" ? r.error : "it could not be read"}`);
      if (r.type !== "cleaned" || r.error) { rows.push(receiptRow(items.indexOf(item) + 1, "not_created")); item.copyStatus = "Copy not created · unchecked"; }
    } catch (e) {
      if (!current()) return;
      failed.push(`${item.file.name}: ${e instanceof Error ? e.message : String(e)}`);
      rows.push(receiptRow(items.indexOf(item) + 1, "not_created")); item.copyStatus = "Copy not created · unchecked";
    }
    done++;
    if (todo.length > 10) view.result = `Making the clean copies… ${done} of ${todo.length}`;
  }
  if (!current()) return;
  view.render?.(items);
  const receipt = batchReceipt(rows);
  const reportBlob = new Blob([JSON.stringify(receipt, null, 2)], { type: "application/json" });
  const said: string[] = [`Findings: ${receipt.removed} removed, ${receipt.present} present, ${receipt.unchecked} unchecked. ZIP includes an index-only JSON receipt. Visible content is not cleared by these metadata checks.`];
  const name = "hexscope-clean-copies.zip";
  const saveZip = (z: StoredZip) => saveAs(name, z.finish());
  if (made > 0 && !share) {
    try {
      await zip.add("hexscope-report.json", reportBlob);
    } catch {
      if (!current()) return;
      saveZip(zip);
      const saveReceipt = document.createElement("button");
      saveReceipt.className = "btn";
      saveReceipt.textContent = translateText("Save receipt as JSON", currentLocale());
      saveReceipt.title = translateText("Downloads the batch receipt as a separate file.", currentLocale());
      saveReceipt.addEventListener("click", () => saveAs("hexscope-report.json", reportBlob));
      const fallbackNotice = translateText("The ZIP could not include the receipt; save it separately.", currentLocale());
      view.offer(`${said.join(" ")} ${fallbackNotice}`, saveReceipt);
      return;
    }
    if (!current()) return;
    saveZip(zip);
    said.push(`Saved ${made === 1 ? "1 clean copy" : `${made} clean copies`} as ${name}.`);
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
      for (const f of shared) await z.add(f.name, f);
      await z.add("hexscope-report.json", reportBlob);
      saveZip(z);
    });
    view.offer(`Made ${n}. ${said.join(" ")}`.trim() + " ", shareBtn, saveBtn);
    return;
  }
  view.result = said.join(" ") || "Nothing to clean.";
}
