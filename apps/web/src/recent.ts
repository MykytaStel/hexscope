// The files opened in this tab, to go back to one without choosing it
// again. Only in memory: a File is a handle to what is on the disk (or, for
// a pasted picture, the picture itself), and nothing is written anywhere —
// closing the tab forgets the list.

/** Files remembered at most. */
const MAX = 8;

const files: File[] = [];

function same(a: File, b: File): boolean {
  return a.name === b.name && a.size === b.size && a.lastModified === b.lastModified;
}

/** Puts a file that was just opened first. */
export function remember(file: File): void {
  const at = files.findIndex((f) => same(f, file));
  if (at >= 0) files.splice(at, 1);
  files.unshift(file);
  files.length = Math.min(files.length, MAX);
}

/** The files opened in this tab, latest first, except the one on screen. */
export function recentFiles(): File[] {
  return files.slice(1);
}
