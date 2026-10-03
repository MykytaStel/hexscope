// The copies made of the file on screen: clean, blacked out, repaired. Each
// is made in the worker, then saved — except on a phone that can share
// files, where it waits for "Share" or "Save": a download there is a dialog
// in the way of sending it on.
import type { CleanResult, RepairResult } from "./cleancard";
import type { FileModel } from "./model";
import type { WorkerResponse } from "./worker";
import { call } from "./rpc";
import { cleanName, phoneCanShare, redactedName, repairedName, saveAs } from "./files";

const why = (r: WorkerResponse) => (r.type === "error" ? r.message : "unexpected reply");
const noCopy = { copy: new Blob([]), name: "", saved: false, removed: [], orientation: 0, verification: null };

/** Saves `copy` as `name` unless it waits to be shared; says which. */
function hand(name: string, copy: Blob, error: string): boolean {
  const saved = !error && !phoneCanShare(name);
  if (saved) saveAs(name, copy);
  return saved;
}

export async function cleanCopy(m: FileModel, notes = false): Promise<CleanResult> {
  const r = await call({ type: "clean", source: m.source ?? new Blob([m.bytes as BlobPart]), notes });
  if (r.type !== "cleaned") return { ...noCopy, error: why(r) };
  const name = cleanName(m);
  return { copy: r.copy, name, saved: hand(name, r.copy, r.error), removed: r.removed, orientation: r.orientation, error: r.error, verification: r.verification };
}

/** A PDF's clean copy with `areas` blacked out as well. */
export async function redactCopy(m: FileModel, areas: Float64Array): Promise<CleanResult> {
  const r = await call({ type: "redact", bytes: m.bytes.slice(), areas });
  if (r.type !== "cleaned") return { ...noCopy, error: why(r) };
  const name = redactedName(m);
  return { copy: r.copy, name, saved: hand(name, r.copy, r.error), removed: r.removed, orientation: 0, error: r.error, verification: r.verification };
}

/** A copy of a damaged file with what survived, always saved: a repair is not sent on. */
export async function repairCopy(m: FileModel): Promise<RepairResult> {
  const r = await call({ type: "repair", bytes: m.bytes.slice() });
  if (r.type !== "repaired") return { bytes: new Uint8Array(0), name: "", fixed: [], error: why(r) };
  const name = repairedName(m);
  if (!r.error) saveAs(name, r.bytes);
  return { bytes: r.bytes, name, fixed: r.fixed, error: r.error };
}
