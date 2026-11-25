// An archive's entry names as folders and files, and a filter over them.
// Names are kept exactly as the archive writes them: "..", a leading "/"
// and backslashes are part of what it says, not something to tidy away.

/** A folder (its path ends in "/"; "" is the top) or an entry, as one row of a list. */
export type ArchiveRow =
  | { kind: "folder"; path: string; name: string; entries: number }
  | { kind: "entry"; index: number; name: string; path: string; leaves: boolean };

/** A row view that creates objects only when the virtual list asks for them. */
export interface ArchiveRows extends Iterable<ArchiveRow> {
  readonly length: number;
  at(index: number): ArchiveRow | undefined;
  findEntry(index: number): number;
  findFolder(path: string): number;
}

export interface ArchiveIndex {
  /** Entries that are files; a folder's own entry is not one. */
  readonly entries: number;
  /** Folders, listed or only implied by a path. */
  readonly folders: number;
  /** A folder's rows: folders, then entries, each in name order. */
  list(folder: string): ArchiveRows;
  /** Entries whose path holds `query`, ignoring case, in archive order. */
  filter(query: string): ArchiveRows;
  /** The folder above, or null at the top. */
  parent(folder: string): string | null;
}

/** A path a careless unpacker would write outside its folder: a ".." segment, a leading "/" or "\\", a drive letter, a backslash. */
export function leavesFolder(path: string): boolean {
  return path.startsWith("/") || path.includes("\\") || /^[A-Za-z]:|(^|\/)\.\.(\/|$)/.test(path);
}

interface Folder {
  segment: string;
  parent: Folder | null;
  folders: Map<string, Folder>;
  entries: number[];
  /** Entries in it and every folder under it. */
  total: number;
  rows?: ArchiveRows;
}

/** Full paths are constructed only for rows the caller actually requests. */
function folderPath(folder: Folder): string {
  const parts: string[] = [];
  for (let at: Folder | null = folder; at?.parent; at = at.parent) parts.push(at.segment);
  return parts.reverse().join("/") + (parts.length ? "/" : "");
}

const parentOf = (folder: string): string => folder.slice(0, folder.slice(0, -1).lastIndexOf("/") + 1);
const nameOf = (path: string): string => path.slice(path.lastIndexOf("/") + 1);

class RowView implements ArchiveRows {
  constructor(
    private readonly folderPaths: readonly Folder[],
    private readonly entryIndexes: ArrayLike<number>,
    private readonly folderRow: (folder: Folder) => ArchiveRow,
    private readonly entryRow: (index: number) => ArchiveRow,
  ) {}

  get length(): number {
    return this.folderPaths.length + this.entryIndexes.length;
  }

  at(index: number): ArchiveRow | undefined {
    if (!Number.isInteger(index) || index < 0 || index >= this.length) return undefined;
    return index < this.folderPaths.length
      ? this.folderRow(this.folderPaths[index])
      : this.entryRow(this.entryIndexes[index - this.folderPaths.length]);
  }

  findEntry(entry: number): number {
    for (let i = 0; i < this.entryIndexes.length; i++) {
      if (this.entryIndexes[i] === entry) return this.folderPaths.length + i;
    }
    return -1;
  }

  findFolder(path: string): number {
    const segment = nameOf(path.slice(0, -1));
    return this.folderPaths.findIndex((folder) => folder.segment === segment && folderPath(folder) === path);
  }

  *[Symbol.iterator](): Iterator<ArchiveRow> {
    for (let i = 0; i < this.length; i++) yield this.at(i)!;
  }
}

const sortNumbersIfNeeded = (values: number[], compare: (a: number, b: number) => number): void => {
  for (let i = 1; i < values.length; i++) {
    if (compare(values[i - 1], values[i]) > 0) {
      values.sort(compare);
      return;
    }
  }
};

export function archiveIndex(names: readonly string[]): ArchiveIndex {
  const root: Folder = { segment: "", parent: null, folders: new Map(), entries: [], total: 0 };
  const folders = [root];
  /** Walk each slash-delimited segment once, retaining no full-path prefixes. */
  const folderAt = (path: string, create: boolean): Folder | undefined => {
    let at = root;
    let start = 0;
    while (start < path.length) {
      const slash = path.indexOf("/", start);
      if (slash < 0) return undefined;
      const segment = path.slice(start, slash);
      let child = at.folders.get(segment);
      if (!child) {
        if (!create) return undefined;
        child = { segment, parent: at, folders: new Map(), entries: [], total: 0 };
        at.folders.set(segment, child);
        folders.push(child);
      }
      at = child;
      start = slash + 1;
    }
    return at;
  };
  let entries = 0;
  names.forEach((name, i) => {
    if (name.endsWith("/")) {
      folderAt(name, true);
      return;
    }
    const at = name.slice(0, name.lastIndexOf("/") + 1);
    folderAt(at, true)!.entries.push(i);
    entries++;
  });
  // Each folder's count, deepest first: a folder holds its own entries and its folders' counts.
  // Creation order always puts parents before children.
  for (let i = folders.length - 1; i >= 0; i--) {
    const folder = folders[i];
    folder.total += folder.entries.length;
    if (folder.parent) folder.parent.total += folder.total;
  }

  const entryRow = (i: number): ArchiveRow => ({ kind: "entry", index: i, name: nameOf(names[i]), path: names[i], leaves: leavesFolder(names[i]) });
  const folderRow = (folder: Folder): ArchiveRow => ({
    kind: "folder",
    path: folderPath(folder),
    name: folder.segment || "/",
    entries: folder.total,
  });
  const folderPaths = (folder: Folder): Folder[] => [...folder.folders.values()].sort((a, b) => {
    const left = a.segment || "/";
    const right = b.segment || "/";
    return left < right ? -1 : left > right ? 1 : 0;
  });
  const entryNameOrder = (a: number, b: number): number => {
    const left = nameOf(names[a]);
    const right = nameOf(names[b]);
    return left < right ? -1 : left > right ? 1 : a - b;
  };
  let lower: string[] | null = null;
  return {
    entries,
    folders: folders.length - 1,
    list(path) {
      const folder = folderAt(path, false);
      if (!folder) return new RowView([], [], folderRow, entryRow);
      if (!folder.rows) {
        sortNumbersIfNeeded(folder.entries, entryNameOrder);
        folder.rows = new RowView(folderPaths(folder), folder.entries, folderRow, entryRow);
      }
      return folder.rows;
    },
    filter(query) {
      const q = query.toLowerCase();
      if (!q) return new RowView([], [], folderRow, entryRow);
      // A name already in lower case is kept, not copied: most are, and a copy of each costs megabytes.
      lower ??= names.map((n) => {
        const l = n.toLowerCase();
        return l === n ? n : l;
      });
      const matches: number[] = [];
      lower.forEach((n, i) => {
        if (!names[i].endsWith("/") && n.includes(q)) matches.push(i);
      });
      return new RowView([], matches, folderRow, entryRow);
    },
    parent: (path) => (path ? parentOf(path) : null),
  };
}
