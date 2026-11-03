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
  folders: Set<string>;
  entries: number[];
  /** Entries in it and every folder under it. */
  total: number;
  /** Sorted paths and row view are compact; they do not retain an object per entry. */
  orderedFolders?: string[];
  rows?: ArchiveRows;
}

const parentOf = (folder: string): string => folder.slice(0, folder.slice(0, -1).lastIndexOf("/") + 1);
const nameOf = (path: string): string => path.slice(path.lastIndexOf("/") + 1);

class RowView implements ArchiveRows {
  constructor(
    private readonly folderPaths: readonly string[],
    private readonly entryIndexes: ArrayLike<number>,
    private readonly folderRow: (path: string) => ArchiveRow,
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
    const index = this.folderPaths.indexOf(path);
    return index;
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
  const folders = new Map<string, Folder>([["", { folders: new Set(), entries: [], total: 0 }]]);
  /** The folder at `path`, made with every folder above it. */
  const folderAt = (path: string): Folder => {
    const missing: string[] = [];
    let at = path;
    while (!folders.has(at)) {
      missing.push(at);
      at = parentOf(at);
    }
    for (let i = missing.length - 1; i >= 0; i--) {
      const child = missing[i];
      folders.set(child, { folders: new Set(), entries: [], total: 0 });
      folders.get(parentOf(child))!.folders.add(child);
    }
    return folders.get(path)!;
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
  const folderRow = (path: string): ArchiveRow => ({
    kind: "folder",
    path,
    name: nameOf(path.slice(0, -1)) || "/",
    entries: folders.get(path)!.total,
  });
  const folderPaths = (folder: Folder): string[] => {
    folder.orderedFolders ??= [...folder.folders].sort((a, b) => {
      const left = nameOf(a.slice(0, -1)) || "/";
      const right = nameOf(b.slice(0, -1)) || "/";
      return left < right ? -1 : left > right ? 1 : 0;
    });
    return folder.orderedFolders;
  };
  const entryNameOrder = (a: number, b: number): number => {
    const left = nameOf(names[a]);
    const right = nameOf(names[b]);
    return left < right ? -1 : left > right ? 1 : a - b;
  };
  let lower: string[] | null = null;
  return {
    entries,
    folders: folders.size - 1,
    list(path) {
      const folder = folders.get(path);
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
