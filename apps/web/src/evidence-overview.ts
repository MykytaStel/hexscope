// The shared evidence panel for photos, documents and the other file families:
// it filters parser facts locally and keeps incomplete reads linked to their
// source node or byte range. It never assigns confidence beyond Core's status.
import { CATEGORIES } from "./finding-kinds";
import { Kind, type FileModel } from "./model";
import { el } from "./dom";
import { currentLocale, translateText } from "./i18n";

type EvidenceFilter = "all" | "privacy" | "metadata" | "attachments" | "unchecked";

interface UncheckedItem {
  label: string;
  detail: string;
  node?: number;
  range?: [number, number];
}

const PRIVACY_KINDS = new Set(CATEGORIES.map(([kind]) => kind));
const ATTACHMENT_KINDS = new Set(["attachments", "embedded"]);
const tr = (text: string) => translateText(text, currentLocale());

function uncheckedItems(m: FileModel): UncheckedItem[] {
  const items: UncheckedItem[] = [];
  const { coverageStatus, metadataComplete } = m.file;

  if (coverageStatus === "unsupported") {
    items.push({
      label: "Format not inspected",
      detail: "No format-aware parser read this file. Byte-level clues do not count as a structural inspection.",
      node: 0,
    });
  } else if (coverageStatus === "partial") {
    const errors = m.problems.filter((id) => m.kind(id) === Kind.Error);
    for (const node of errors.slice(0, 40)) {
      items.push({
        label: m.label(node),
        detail: m.doc(node)?.text || "This part could not be read as the format describes it.",
        node,
      });
    }
    if (errors.length === 0) {
      items.push({
        label: "Some supported parts were not fully read",
        detail: "The parser reported a partial read but did not attach a specific unreadable node.",
        node: 0,
      });
    }
    if (errors.length > 40) {
      items.push({
        label: "More unreadable parts",
        detail: "Additional parser errors are listed in the structure tree.",
        node: 0,
      });
    }
  }

  if (metadataComplete === false && !items.some((item) => item.label.toLowerCase().includes("metadata"))) {
    items.push({
      label: "Metadata read is incomplete",
      detail: "Some metadata fields may be missing from the findings list.",
      node: 0,
    });
  }

  if (m.missing && m.missing.total > 0) {
    const flat = m.file.missing;
    let shown = 0;
    for (let at = 0; at + 1 < flat.length && shown < 12; at += 2, shown++) {
      const start = flat[at];
      const len = flat[at + 1];
      items.push({
        label: "Deferred media bytes",
        detail: "This byte range was deferred during the initial inspection.",
        range: [start, len],
      });
    }
    if (flat.length / 2 > 12) {
      items.push({
        label: "More deferred media bytes",
        detail: "Additional deferred ranges are listed in the byte map.",
        range: [flat[24], flat[25]],
      });
    }
  }

  return items;
}

/** Adds one searchable, keyboard-operable evidence filter to existing fact rows. */
export function evidenceOverview(
  m: FileModel,
  fields: HTMLElement,
  select: (node: number) => void,
  selectRange: (start: number, len: number) => void,
): HTMLElement {
  const box = el("section", "evidence-overview");
  box.append(el("h3", "evidence-overview-title", "Findings and evidence"));

  const query = el("input", "evidence-search");
  query.type = "search";
  query.placeholder = tr("Search findings and evidence");
  query.setAttribute("aria-label", tr("Search findings and evidence"));
  box.append(query);

  const toolbar = el("div", "evidence-filters");
  toolbar.setAttribute("role", "group");
  toolbar.setAttribute("aria-label", tr("Filter findings"));
  const issueList = el("ul", "evidence-unchecked-list");
  const empty = el("p", "hint evidence-empty", "No matching findings.");
  const items = uncheckedItems(m);
  const issues = items.map((item) => {
    const li = el("li", "evidence-unchecked-item");
    const jump = el("button", "evidence-unchecked-link", item.label);
    jump.type = "button";
    jump.addEventListener("click", () => {
      if (item.range) selectRange(item.range[0], item.range[1]);
      else if (item.node !== undefined) select(item.node);
    });
    const detail = el("span", "evidence-unchecked-detail", item.detail);
    li.append(jump, detail);
    issueList.append(li);
    return { li };
  });
  const factRows = [...fields.querySelectorAll<HTMLElement>("dd[data-kind]")].map((dd) => {
    const kind = dd.dataset.kind ?? "";
    const dt = dd.previousElementSibling as HTMLElement | null;
    return {
      kind,
      dt,
      dd,
    };
  });

  let active: EvidenceFilter = factRows.length === 0 && issues.length > 0 ? "unchecked" : "all";
  const filters: { key: EvidenceFilter; label: string; count: number }[] = [
    { key: "all", label: "All findings", count: factRows.length },
    { key: "privacy", label: "Privacy", count: factRows.filter((row) => PRIVACY_KINDS.has(row.kind) && !ATTACHMENT_KINDS.has(row.kind)).length },
    { key: "metadata", label: "Metadata", count: factRows.filter((row) => !ATTACHMENT_KINDS.has(row.kind)).length },
    { key: "attachments", label: "Attachments", count: factRows.filter((row) => ATTACHMENT_KINDS.has(row.kind)).length },
    { key: "unchecked", label: "Not checked", count: issues.length },
  ];
  const buttons = new Map<EvidenceFilter, HTMLButtonElement>();
  for (const filter of filters) {
    const button = el("button", "evidence-filter");
    button.type = "button";
    button.append(document.createTextNode(tr(filter.label)), el("span", "evidence-filter-count", String(filter.count)));
    button.setAttribute("aria-pressed", filter.key === active ? "true" : "false");
    button.addEventListener("click", () => {
      active = filter.key;
      update();
    });
    buttons.set(filter.key, button);
    toolbar.append(button);
  }
  box.append(toolbar);

  const listRoot = el("div", "evidence-unchecked");
  listRoot.append(issueList);
  box.append(listRoot, empty);

  const update = () => {
    const needle = query.value.trim().toLocaleLowerCase(currentLocale());
    let visible = 0;
    const showFacts = active !== "unchecked";
    fields.hidden = !showFacts;
    listRoot.hidden = showFacts;
    const summary = fields.querySelector<HTMLElement>(".metadata-summary");
    if (summary) summary.hidden = !showFacts || active !== "all";

    for (const row of factRows) {
      const attachment = ATTACHMENT_KINDS.has(row.kind);
      const privacy = PRIVACY_KINDS.has(row.kind) && !attachment;
      const inFilter = active === "all" || (active === "privacy" && privacy) ||
        (active === "metadata" && !attachment) || (active === "attachments" && attachment);
      const rowText = `${row.dt?.textContent ?? ""} ${row.dd.textContent ?? ""}`.toLocaleLowerCase(currentLocale());
      const show = showFacts && inFilter && (!needle || rowText.includes(needle));
      if (row.dt) row.dt.hidden = !show;
      row.dd.hidden = !show;
      if (show) visible++;
    }
    const general = fields.querySelector<HTMLElement>('.metadata-group[data-metadata-group="general"]');
    let generalVisible = false;
    const generalList = general?.querySelector("dl");
    if (generalList) {
      const children = [...generalList.children] as HTMLElement[];
      for (let i = 0; i + 1 < children.length; i += 2) {
        const term = children[i];
        const value = children[i + 1];
        const text = `${term.textContent ?? ""} ${value.textContent ?? ""}`.toLocaleLowerCase(currentLocale());
        const show = active === "all" && (!needle || text.includes(needle));
        term.hidden = !show;
        value.hidden = !show;
        if (show) {
          generalVisible = true;
          visible++;
        }
      }
    }
    if (general) general.hidden = !showFacts || !generalVisible;
    for (const section of fields.querySelectorAll<HTMLElement>(".metadata-group")) {
      if (section === general) continue;
      const rows = [...section.querySelectorAll<HTMLElement>("dd[data-kind]")];
      section.hidden = showFacts && rows.length > 0 && !rows.some((row) => !row.hidden);
    }
    for (const { li } of issues) {
      const text = (li.textContent ?? "").toLocaleLowerCase(currentLocale());
      const show = active === "unchecked" && (!needle || text.includes(needle));
      li.hidden = !show;
      if (show) visible++;
    }
    for (const [key, button] of buttons) button.setAttribute("aria-pressed", key === active ? "true" : "false");
    empty.hidden = visible > 0;
    if (active === "unchecked" && issues.length === 0) empty.textContent = tr("The parser reported no unchecked parts.");
    else if (needle && visible === 0) empty.textContent = tr("No matching findings.");
    else empty.textContent = active === "unchecked" ? tr("The parser reported no unchecked parts.") : tr("No findings in this group.");
  };
  query.addEventListener("input", update);
  update();
  return box;
}
