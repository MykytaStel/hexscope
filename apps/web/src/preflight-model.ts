import type { ParsedFile } from "./model";

export interface PublicationProfile {
  schema: "hexscope.policy";
  version: 1;
  blocked_kinds: string[];
  require_complete: boolean;
}

export function readProfile(json: string): PublicationProfile {
  if (json.length > 65_536) throw new Error("Profile exceeds 64 KiB");
  const value = JSON.parse(json);
  if (value?.schema !== "hexscope.policy" || value.version !== 1 || typeof value.require_complete !== "boolean"
    || !Array.isArray(value.blocked_kinds) || value.blocked_kinds.length > 128
    || value.blocked_kinds.some((kind: unknown) => typeof kind !== "string" || !/^[a-z0-9_-]{1,64}$/.test(kind))) {
    throw new Error("Invalid publication profile");
  }
  return { schema: "hexscope.policy", version: 1, blocked_kinds: [...new Set<string>(value.blocked_kinds)], require_complete: value.require_complete };
}

/** Rules checked on a particular file, held only for that file's lifetime. */
export const checkedProfiles = new WeakMap<ParsedFile, PublicationProfile>();

/** Only actual parent edges; no inference about who made a file. */
export function evidencePath(file: Pick<ParsedFile, "parents" | "labels">, node: number): number[] {
  const path: number[] = [];
  const seen = new Set<number>();
  while (node >= 0 && node < file.parents.length && !seen.has(node) && path.length < 32) {
    seen.add(node);
    path.unshift(node);
    node = file.parents[node];
  }
  return path;
}

/** Duplicate labels with different stored values are clues, not tampering evidence. */
export function differingFields(file: Pick<ParsedFile, "labels" | "values" | "kinds">): { label: string; nodes: number[] }[] {
  const fields = new Map<string, number[]>();
  for (let node = 0; node < Math.min(file.labels.length, 100_000); node++) {
    if (file.kinds[node] !== 1 || !file.values[node] || file.values[node].length > 1024) continue;
    const label = file.labels[node];
    if (!/date|time|author|owner|latitude|longitude|serial|make|model/i.test(label)) continue;
    const nodes = fields.get(label) ?? [];
    if (nodes.length < 16) nodes.push(node);
    fields.set(label, nodes);
  }
  return [...fields].filter(([, nodes]) => new Set(nodes.map((node) => file.values[node])).size > 1)
    .slice(0, 20).map(([label, nodes]) => ({ label, nodes }));
}
