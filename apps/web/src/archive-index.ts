// An archive's entry names as folders and files, and a filter over them.
// Names are kept exactly as the archive writes them: "..", a leading "/"
// and backslashes are part of what it says, not something to tidy away.

/** A folder (its path ends in "/"; "" is the top) or an entry, as one row of a list. */
export type ArchiveRow =
  | { kind: "folder"; path: string; name: string; entries: number }
  | { kind: "entry"; index: number; name: string; path: string; leaves: boolean };

export interface ArchiveIndex {
  /** Entries that are files; a folder's own entry is not one. */
  readonly entries: number;
  /** Folders, listed or only implied by a path. */
  readonly folders: number;
  /** A folder's rows: folders, then entries, each in name order. */
  list(folder: string): ArchiveRow[];
  /** Entries whose path holds `query`, ignoring case, in archive order. */
  filter(query: string): ArchiveRow[];
  /** The folder above, or null at the top. */
  parent(folder: string): string | null;
}

/** A path a careless unpacker would write outside its folder: a ".." segment, a leading "/" or "\", a drive letter, a backslash. */
export function leavesFolder(path: string): boolean {
  return path.startsWith("/") || path.includes("\\") || /^[A-Za-z]:|(^|\/)\.\.(\/|$)/.test(path);
}

interface Folder {
  folders: Set<string>;
  entries: number[];
  /** Entries in it and every folder under it. */
  total: number;
  rows?: ArchiveRow[];
}

const parentOf = (folder: string): string => folder.slice(0, folder.slice(0, -1).lastIndexOf("/") + 1);
const nameOf = (path: string): string => path.slice(path.lastIndexOf("/") + 1);
const byName = (a: ArchiveRow, b: ArchiveRow) =>
  a.name < b.name ? -1 : a.name > b.name ? 1 : (a.kind === "entry" ? a.index : 0) - (b.kind === "entry" ? b.index : 0);
/** Rows in name order; archivers mostly write them so, and checking costs less than sorting. */
const inOrder = (rows: ArchiveRow[]): ArchiveRow[] => (rows.every((r, i) => i === 0 || byName(rows[i - 1], r) <= 0) ? rows : rows.sort(byName));

export function archiveIndex(names: readonly string[]): ArchiveIndex {
  const folders = new Map<string, Folder>([["", { folders: new Set(), entries: [], total: 0 }]]);
  /** The folder at `path`, made with every folder above it. */
  const folderAt = (path: string): Folder => {
    let folder = folders.get(path);
    if (!folder) {
      folder = { folders: new Set(), entries: [], total: 0 };
      folders.set(path, folder);
      folderAt(parentOf(path)).folders.add(path);
    }
    return folder;
  };
  let entries = 0;
  names.forEach((name, i) => {
    if (name.endsWith("/")) {
      folderAt(name);
      return;
    }
    const at = name.slice(0, name.lastIndexOf("/") + 1);
    folderAt(at).entries.push(i);
    entries++;
  });
  // Each folder's count, deepest first: a folder holds its own entries and its folders' counts.
  for (const path of [...folders.keys()].sort((a, b) => b.length - a.length)) {
    const folder = folders.get(path)!;
    folder.total += folder.entries.length;
    if (path) folders.get(parentOf(path))!.total += folder.total;
  }

  const entryRow = (i: number): ArchiveRow => ({ kind: "entry", index: i, name: nameOf(names[i]), path: names[i], leaves: leavesFolder(names[i]) });
  let lower: string[] | null = null;
  return {
    entries,
    folders: folders.size - 1,
    list(path) {
      const folder = folders.get(path);
      if (!folder) return [];
      folder.rows ??= [
        ...inOrder([...folder.folders].map((p): ArchiveRow => ({ kind: "folder", path: p, name: nameOf(p.slice(0, -1)) || "/", entries: folders.get(p)!.total }))),
        ...inOrder(folder.entries.map(entryRow)),
      ];
      return folder.rows;
    },
    filter(query) {
      const q = query.toLowerCase();
      if (!q) return [];
      // A name already in lower case is kept, not copied: most are, and a copy of each costs megabytes.
      lower ??= names.map((n) => {
        const l = n.toLowerCase();
        return l === n ? n : l;
      });
      const out: ArchiveRow[] = [];
      lower.forEach((n, i) => {
        if (!names[i].endsWith("/") && n.includes(q)) out.push(entryRow(i));
      });
      return out;
    },
    parent: (path) => (path ? parentOf(path) : null),
  };
}
