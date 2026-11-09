// A bounded purpose map for Office, OpenDocument and EPUB ZIP packages. The
// outer archive remains the evidence source; this view does not claim that
// an entry was parsed until it is opened as a file of its own.
import { el, formatBytes } from "./dom";
import { MAX_NESTING, isOpenDocument } from "./knowledge";
import { type FileModel } from "./model";
import { currentLocale, translateText } from "./i18n";

type Purpose = "content" | "metadata" | "comments" | "media" | "navigation" | "other";

interface PackageEntry {
  index: number;
  name: string;
  purpose: Purpose;
}

const LIMIT_PER_GROUP = 80;
const tr = (text: string) => translateText(text, currentLocale());
const PURPOSES: { key: Purpose; label: string }[] = [
  { key: "content", label: "Document content" },
  { key: "metadata", label: "Properties and metadata" },
  { key: "comments", label: "Comments and revisions" },
  { key: "media", label: "Pictures and attachments" },
  { key: "navigation", label: "Package navigation" },
  { key: "other", label: "Other parts" },
];

function purposeOf(name: string): Purpose {
  const path = name.toLowerCase();
  if (/(?:comments?|people|persons|notes|tracked|revision|changes|deleted)/.test(path)) return "comments";
  if (/(?:docprops|core\.xml|app\.xml|custom\.xml|meta\.xml|metadata|settings\.xml)/.test(path)) return "metadata";
  if (/(?:embeddings?|attachments?|\/media\/|\/pictures\/|thumbnail|cover\.(?:jpe?g|png|webp))/.test(path)) return "media";
  if (/(?:^\[content_types\]\.xml$|^_rels\/|\.rels$|^meta-inf\/container\.xml$|^mimetype$|^meta-inf\/)/.test(path)) return "navigation";
  if (/(?:word\/document\.xml|xl\/worksheets\/|ppt\/slides\/|content\.xml|\.xhtml?$|\.opf$|\.ncx$)/.test(path)) return "content";
  return "other";
}

function packageKind(m: FileModel): "office" | "opendocument" | "epub" | null {
  if (m.file.format !== "zip") return null;
  const labels = m.file.labels;
  if (labels.includes("word/document.xml") || labels.includes("xl/workbook.xml") || labels.includes("ppt/presentation.xml")) return "office";
  if (isOpenDocument(labels)) return "opendocument";
  if (labels.includes("META-INF/container.xml")) return "epub";
  return null;
}

function state(m: FileModel, index: number, nested: number): string {
  const entry = m.entry(index);
  if (entry.flags & 1) return "Encrypted · not checked";
  if (entry.method !== 0 && entry.method !== 8) return "Unsupported compression · not checked";
  if (!m.archiveTrace(index).dataComplete) return "Entry bytes are incomplete · not checked";
  if (nested >= MAX_NESTING) return "The nesting limit prevents opening this entry";
  return entry.openable ? "Can be opened locally; contents not yet parsed" : "Not opened as a file";
}

function methodName(method: number): string {
  if (method === 0) return "Stored";
  if (method === 8) return "Deflate";
  return "Compression method";
}

/** Package parts by purpose, with local filtering and links to their ZIP records. */
export function documentMap(
  m: FileModel,
  select: (node: number) => void,
  open: (index: number) => void,
  nested: number,
): HTMLElement | null {
  const kind = packageKind(m);
  if (!kind && !["office97", "rtf"].includes(m.file.format)) return null;

  const section = el("section", "document-map");
  section.append(el("h2", undefined, tr("Document map")));
  if (!kind) {
    const text = m.file.format === "office97"
      ? "This older Office file is inspected for supported properties; its document text and revisions are not fully checked. Save it as a modern Office file to inspect its content."
      : "RTF text and supported properties are read from the document structure. The parser coverage status below still applies.";
    section.append(el("p", "hint", tr(text)));
    return section;
  }

  const kindLabel = kind === "office" ? "Office package" : kind === "opendocument" ? "OpenDocument package" : "EPUB package";
  const intro = el("p", "hint document-map-intro");
  intro.append(el("strong", undefined, tr(kindLabel)), ` · ${tr("Package entries are grouped by purpose; opening an entry runs its own format-aware inspection.")}`);
  section.append(intro);
  const search = el("input", "document-map-search");
  search.type = "search";
  search.placeholder = tr("Filter package parts");
  search.setAttribute("aria-label", tr("Filter package parts"));
  section.append(search);
  const statusLine = el("p", "hint document-map-status");
  statusLine.setAttribute("role", "status");
  section.append(statusLine);

  const entries = m.entryNames().map((name, index): PackageEntry => ({ index, name, purpose: purposeOf(name) }));
  const groups = new Map<Purpose, { details: HTMLDetailsElement; list: HTMLElement; count: HTMLElement }>();
  for (const purpose of PURPOSES) {
    const details = el("details", "document-map-group");
    const summary = el("summary");
    const label = el("span", undefined, tr(purpose.label));
    const count = el("span", "document-map-count", "0");
    summary.append(label, count);
    const list = el("ul", "document-map-list");
    details.append(summary, list);
    section.append(details);
    groups.set(purpose.key, { details, list, count });
  }

  section.append(el("p", "hint document-map-copy-note", tr("Clean-copy findings appear in the copy's Core read-back report; this map does not certify individual package entries as removed.")));

  const render = () => {
    const query = search.value.trim().toLocaleLowerCase(currentLocale());
    let matches = 0;
    for (const purpose of PURPOSES) {
      const group = groups.get(purpose.key)!;
      const found = entries.filter((entry) => entry.purpose === purpose.key && (!query || entry.name.toLocaleLowerCase(currentLocale()).includes(query)));
      matches += found.length;
      group.count.textContent = found.length.toLocaleString(currentLocale());
      group.details.hidden = found.length === 0;
      group.list.replaceChildren();
      for (const item of found.slice(0, LIMIT_PER_GROUP)) {
        const info = m.entry(item.index);
        const li = el("li", "document-map-entry");
        const row = el("div", "document-map-entry-head");
        const name = el("button", "document-map-entry-name", item.name);
        name.type = "button";
        name.translate = false;
        name.title = tr("Show this package entry in the file structure");
        name.addEventListener("click", () => select(info.node));
        row.append(name);
        if (info.openable && nested < MAX_NESTING) {
          const openButton = el("button", "link document-map-open", "Open entry");
          openButton.type = "button";
          openButton.title = tr("Open this package part as a file of its own");
          openButton.addEventListener("click", () => open(item.index));
          row.append(openButton);
        }
        const badges = el("p", "document-map-entry-meta");
        badges.append(
          el("span", undefined, `${formatBytes(info.compressed)} → ${formatBytes(info.uncompressed)}`),
          el("span", undefined, `${tr(methodName(info.method))}${methodName(info.method) === "Compression method" ? ` ${info.method}` : ""}`),
          el("span", "document-map-entry-state", tr(state(m, item.index, nested))),
        );
        li.append(row, badges);
        group.list.append(li);
      }
      const hiddenCount = found.length - LIMIT_PER_GROUP;
      if (hiddenCount > 0) group.list.append(el("li", "hint", tr("More parts are available in the Files view.")));
      if (query && found.length > 0) group.details.open = true;
    }
    statusLine.textContent = query ? `${matches.toLocaleString(currentLocale())} ${tr("matching package parts")}` : "";
  };
  search.addEventListener("input", render);
  render();
  return section;
}
