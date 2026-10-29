// The Files view of a ZIP archive: its folders and files, a filter by name,
// and what each entry's own records say about it. Choosing an entry shows it
// in the details beside the list, where Archive Trace, Open and Watch it
// decompress already are. Nothing here reads an entry's bytes.
import { el, formatBytes } from "./dom";
import { registerLazyTranslations } from "./i18n";
import { openReason } from "./knowledge";
import type { FileModel } from "./model";
import { archiveIndex, type ArchiveIndex, type ArchiveRow } from "./archive-index";
import { ArchiveList } from "./archive-list";
import "./styles/archive-explorer.css";

/** Read past this, the core stops listing entries (`MAX_ENTRIES` in the core). */
export const TRUNCATED = "The archive lists more: hexscope read the first 100,000.";
const INCOMPLETE = "Its data in the file is not as long as the directory says.";
const OUTSIDE = "An unpacker that trusts the name would write this file outside the folder it unpacks into.";

registerLazyTranslations({
  "Files in the archive": "Файли в архіві",
  "Up": "Угору",
  "Up to the top level": "Угору до початку архіву",
  "All files": "Усі файли",
  "Filter by name": "Фільтр за назвою",
  "Nothing matches": "Нічого не знайдено",
  "Nothing in this folder": "У цій теці нічого немає",
  "Encrypted": "Зашифровано",
  "Cannot be unpacked here": "Тут не розпакувати",
  "Incomplete": "Неповний",
  "Unpacks outside its folder": "Розпакується поза теку",
  [TRUNCATED]: "Архів містить більше записів: hexscope прочитав перші 100 000.",
  [INCOMPLETE]: "Дані цього запису у файлі не такої довжини, як каже каталог.",
  [OUTSIDE]: "Розпакувальник, який довіряє назві, запише цей файл поза теку, у яку розпаковує.",
});

const count = (n: number) => n.toLocaleString("en");

/** "3 entries in 1 folder · 1.0 KB packed, 4.0 KB unpacked". */
export function summaryLine(entries: number, folders: number, packed: number, unpacked: number): string {
  const files = `${count(entries)} ${entries === 1 ? "entry" : "entries"}`;
  const where = folders ? ` in ${count(folders)} ${folders === 1 ? "folder" : "folders"}` : "";
  return `${files}${where} · ${formatBytes(packed)} packed, ${formatBytes(unpacked)} unpacked`;
}

/** What a filter found, said once it has looked. */
export function matchLine(found: number): string {
  if (found === 0) return "Nothing matches";
  return found === 1 ? "1 entry matches" : `${count(found)} entries match`;
}

export interface FilesHooks {
  /** Shows the entry's node in the details, without leaving the list. */
  choose(node: number): void;
}

export interface FilesView {
  readonly element: HTMLElement;
  /** The first call reads the names; later calls put the folder, filter, scroll and choice back. */
  show(): void;
  hide(): void;
  focusFilter(): void;
  dispose(): void;
}

const folderName = (path: string) => path.slice(0, -1).slice(path.slice(0, -1).lastIndexOf("/") + 1) || "/";

export function filesView(m: FileModel, hooks: FilesHooks): FilesView {
  const element = el("div", "archive-explorer");
  const summary = el("p", "archive-summary");
  const truncated = el("p", "hint archive-truncated", TRUNCATED);
  truncated.hidden = true;
  const bar = el("div", "archive-folderbar");
  const up = el("button", "btn archive-up", "Up");
  up.type = "button";
  const where = el("span", "archive-path");
  bar.append(up, where);
  const filter = el("input", "archive-filter");
  filter.type = "search";
  filter.placeholder = "Filter by name";
  filter.setAttribute("aria-label", "Filter by name");
  filter.spellcheck = false;
  filter.autocomplete = "off";
  const status = el("p", "archive-status");
  status.setAttribute("role", "status");
  status.setAttribute("aria-live", "polite");
  const host = el("div", "archive-list-host");
  element.append(summary, truncated, bar, filter, status, host);

  let index: ArchiveIndex | null = null;
  let list: ArchiveList | null = null;
  let folder = "";
  let query = "";
  let chosen: number | null = null;

  /** One row: its mark, its name as the archive writes it, its size or count, and its flags. */
  const draw = (row: ArchiveRow, into: HTMLElement) => {
    into.classList.add(row.kind === "folder" ? "is-folder" : "is-file");
    const mark = el("span", "archive-mark");
    mark.setAttribute("aria-hidden", "true");
    const name = el("span", "archive-name", (query && row.kind === "entry" ? row.path : row.name) || "—");
    name.translate = false;
    into.append(mark, name);
    if (row.kind === "folder") {
      into.append(el("span", "archive-meta", `${row.entries} ${row.entries === 1 ? "entry" : "entries"}`));
      return;
    }
    const e = m.entry(row.index);
    into.append(el("span", "archive-meta", formatBytes(e.uncompressed)));
    const flags: [string, string][] = [];
    if (e.flags & 1) flags.push(["Encrypted", openReason(e, 0) ?? ""]);
    else if (e.method !== 0 && e.method !== 8) flags.push(["Cannot be unpacked here", openReason(e, 0) ?? ""]);
    if (!m.archiveTrace(row.index).dataComplete) flags.push(["Incomplete", INCOMPLETE]);
    if (row.leaves) flags.push(["Unpacks outside its folder", OUTSIDE]);
    for (const [text, why] of flags) {
      const flag = el("span", "archive-flag", text);
      flag.title = why;
      into.append(flag);
    }
  };

  /** Lists the folder, or what the filter found; `keep` puts the scroll and active row back. */
  const render = (keep?: { scrollTop: number; active: number }) => {
    if (!index || !list) return;
    const rows = query ? index.filter(query) : index.list(folder);
    list.setRows(rows, keep);
    list.markChosen(chosen);
    status.textContent = query ? matchLine(rows.length) : rows.length ? "" : "Nothing in this folder";
    up.disabled = !!query || !folder;
    const parent = index.parent(folder);
    up.setAttribute("aria-label", parent ? `Up to “${folderName(parent)}”` : "Up to the top level");
    // A path is the archive's own words; "All files" is the page's.
    where.translate = !!query || !folder;
    where.classList.toggle("is-top", !query && !folder);
    where.textContent = query ? "" : folder || "All files";
  };

  const choose = (row: ArchiveRow) => {
    if (row.kind === "folder") {
      folder = row.path;
      render();
      return;
    }
    chosen = row.index;
    list?.markChosen(chosen);
    hooks.choose(m.entry(row.index).node);
  };

  const goUp = () => {
    if (!index || query || !folder) return;
    const from = folder;
    folder = index.parent(folder) ?? "";
    // Back in the folder above, on the folder just left.
    const active = Math.max(0, index.list(folder).findIndex((r) => r.kind === "folder" && r.path === from));
    render({ scrollTop: Math.max(0, active - 3) * (list?.rowHeight ?? 36), active });
  };

  /** Where the folder's list stood when a filter replaced it, to stand there again after. */
  let folderPlace: { scrollTop: number; active: number } | undefined;
  const setQuery = (next: string) => {
    if (next && !query && list) folderPlace = { scrollTop: list.scrollTop, active: list.active };
    const back = !next && query ? folderPlace : undefined;
    query = next;
    render(back);
  };

  up.addEventListener("click", goUp);
  filter.addEventListener("input", () => setQuery(filter.value.trim() ? filter.value : ""));
  filter.addEventListener("keydown", (e) => {
    if (e.key === "Escape" && filter.value) {
      e.preventDefault();
      e.stopPropagation();
      filter.value = "";
      setQuery("");
    } else if (e.key === "ArrowDown") {
      e.preventDefault();
      list?.focus();
    }
  });

  return {
    element,
    show() {
      if (!index) {
        const names: string[] = [];
        let packed = 0;
        let unpacked = 0;
        for (let i = 0; i < m.entryCount; i++) {
          const e = m.entry(i);
          names.push(m.label(e.node));
          packed += e.compressed;
          unpacked += e.uncompressed;
        }
        index = archiveIndex(names);
        summary.textContent = summaryLine(index.entries, index.folders, packed, unpacked);
        truncated.hidden = !m.problems.some((id) => m.label(id).startsWith("stopped after"));
        list = new ArchiveList(host, { label: "Files in the archive", draw, choose, up: goUp });
        render();
        return;
      }
      if (list) render({ scrollTop: list.scrollTop, active: list.active });
    },
    hide() {
      // The list keeps its own scroll and active row; nothing to put away.
    },
    focusFilter() {
      filter.focus();
      filter.select();
    },
    dispose() {
      list?.dispose();
      list = null;
      index = null;
      element.remove();
    },
  };
}
