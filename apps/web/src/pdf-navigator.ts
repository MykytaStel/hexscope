// The PDF navigation layer ties parser-read page status and file-level
// findings to their source objects, then defers image extraction until asked.
import type { FileModel } from "./model";
import type { PageGlyphs } from "./worker";
import { el } from "./dom";
import { FACT_LABELS } from "./knowledge";
import { currentLocale, translateText } from "./i18n";
import type { Shown } from "./pdf-pictures";

const MAX_VISIBLE_PAGES = 100;
const tr = (text: string) => translateText(text, currentLocale());
const PAGE_FACT = /^page\s+(\d+)(?::|,)/i;

function pageObjects(m: FileModel, pages: PageGlyphs[]): Map<number, number> {
  const wanted = new Set(pages.map((page) => page.object).filter((object): object is number => !!object));
  const indexed = new Map<number, number>();
  if (wanted.size === 0) return indexed;
  for (let id = 0; id < m.count; id++) {
    const match = /^object (\d+)(?: \d+)?$/.exec(m.label(id));
    if (!match) continue;
    const object = Number(match[1]);
    if (wanted.has(object)) indexed.set(object, id);
  }
  return indexed;
}

function factGroup(
  title: string,
  facts: FileModel["file"]["facts"],
  select: (node: number) => void,
): HTMLElement | null {
  if (facts.length === 0) return null;
  const details = el("details", "pdf-nav-fact-group");
  details.append(el("summary", undefined, `${tr(title)} · ${facts.length.toLocaleString(currentLocale())}`));
  const list = el("ul", "pdf-nav-facts");
  for (const item of facts.slice(0, 100)) {
    const li = el("li");
    const button = el("button", "pdf-nav-fact", `${tr(FACT_LABELS[item.kind] ?? item.kind)}: ${item.text}`);
    button.type = "button";
    button.title = tr("Show this finding in the file structure");
    button.addEventListener("click", () => select(item.node));
    li.append(button);
    list.append(li);
  }
  if (facts.length > 100) list.append(el("li", "hint", tr("More findings are available in the evidence list.")));
  details.append(list);
  return details;
}

/** Page, attachment, form and action navigation for the existing PDF parser. */
export function pdfNavigator(
  m: FileModel,
  loadPages: () => Promise<PageGlyphs[]>,
  picturesOf: (page: number) => Promise<Shown[]>,
  pictureState: (page: number) => "loading" | "loaded" | "failed",
  select: (node: number) => void,
  openAttachment: (index: number) => void,
  focusPageTools: () => void,
): HTMLElement {
  const section = el("section", "pdf-navigator");
  section.append(
    el("h2", undefined, tr("PDF navigator")),
    el("p", "hint pdf-nav-intro", tr("Page text, supported pictures, parser findings and their source objects are kept together. Text inspection does not include OCR.")),
  );

  const encryption = m.file.facts.some((item) => item.kind === "encryption");
  if (encryption) section.append(el("p", "problem is-warning", tr("The PDF is encrypted; page content cannot be read without its password.")));

  const attachments = el("details", "pdf-nav-fact-group");
  attachments.append(el("summary", undefined, `${tr("Attachments")} · ${m.file.attachments.length.toLocaleString(currentLocale())}`));
  const attachmentList = el("ul", "pdf-nav-facts");
  if (m.file.attachments.length === 0) {
    attachmentList.append(el("li", "hint", tr("No attachments were listed by the parser.")));
  } else {
    m.file.attachments.slice(0, 100).forEach((name, index) => {
      const li = el("li");
      const open = el("button", "pdf-nav-fact", `${tr("Open attachment")}: ${name}`);
      open.type = "button";
      open.addEventListener("click", () => openAttachment(index));
      li.append(open);
      attachmentList.append(li);
    });
  }
  attachments.append(attachmentList);
  const forms = factGroup("Forms", m.file.facts.filter((item) => item.kind === "form"), select);
  const actions = factGroup("Document actions", m.file.facts.filter((item) => ["opens", "scripts", "launch", "submits", "weblinks"].includes(item.kind)), select);
  section.append(attachments);
  if (forms) section.append(forms);
  if (actions) section.append(actions);

  const load = el("button", "btn pdf-nav-load", tr("Read page status"));
  load.type = "button";
  const status = el("p", "hint pdf-nav-status");
  status.setAttribute("role", "status");
  status.setAttribute("aria-live", "polite");
  const pageSearch = el("input", "pdf-nav-search");
  pageSearch.type = "search";
  pageSearch.placeholder = tr("Filter by page number");
  pageSearch.setAttribute("aria-label", tr("Filter by page number"));
  pageSearch.hidden = true;
  const pages = el("ol", "pdf-nav-pages");
  pages.hidden = true;
  const more = el("p", "hint pdf-nav-more");
  more.hidden = true;
  const tools = el("button", "link pdf-nav-tools", tr("Open text search and page preview"));
  tools.type = "button";
  tools.hidden = true;
  tools.addEventListener("click", () => {
    focusPageTools();
  });
  if (encryption) {
    load.disabled = true;
    status.textContent = tr("Page status cannot be read while this PDF is encrypted.");
  }
  section.append(load, status, pageSearch, more, pages, tools);

  let pageData: PageGlyphs[] | null = null;
  let pageObjectNodes = new Map<number, number>();
  let pageFindings = new Map<number, FileModel["file"]["facts"]>();
  const renderPages = () => {
    if (!pageData) return;
    const query = pageSearch.value.trim();
    const found = pageData.filter((page, index) => {
      const number = page.page ?? index + 1;
      return !query || String(number).includes(query);
    });
    pages.replaceChildren();
    for (const page of found.slice(0, MAX_VISIBLE_PAGES)) {
      const number = page.page ?? pageData.indexOf(page) + 1;
      const li = el("li", "pdf-nav-page");
      const head = el("div", "pdf-nav-page-head");
      const source = page.object ? pageObjectNodes.get(page.object) ?? -1 : -1;
      if (source >= 0) {
        const object = el("button", "pdf-nav-object", `${tr("Page")} ${number} · ${tr("PDF object")} ${page.object}`);
        object.type = "button";
        object.addEventListener("click", () => select(source));
        head.append(object);
      } else {
        head.append(el("strong", undefined, `${tr("Page")} ${number}`));
      }
      const textStatus = page.complete ? "Text read" : "Text partly read";
      head.append(el("span", page.complete ? "pdf-nav-state" : "pdf-nav-state is-warning", tr(textStatus)));
      li.append(head);

      const hasText = page.texts.length > 0;
      const textDescription = hasText
        ? page.complete ? "Supported stored text was read. OCR is not used." : "Some stored text was read; other content may be missing. OCR is not used."
        : page.complete ? "No supported stored text was found. OCR is not used." : "No supported stored text was read completely. OCR is not used.";
      li.append(el("p", "hint pdf-nav-page-text", tr(textDescription)));

      const findings = pageFindings.get(number) ?? [];
      if (findings.length > 0) {
        const foundDetails = el("details", "pdf-nav-page-findings");
        foundDetails.append(el("summary", undefined, `${findings.length.toLocaleString(currentLocale())} ${tr("page findings")}`));
        const foundList = el("ul", "pdf-nav-facts");
        for (const item of findings.slice(0, 20)) {
          const itemRow = el("li");
          const finding = el("button", "pdf-nav-fact", `${tr(FACT_LABELS[item.kind] ?? item.kind)}: ${item.text}`);
          finding.type = "button";
          finding.addEventListener("click", () => select(item.node));
          itemRow.append(finding);
          foundList.append(itemRow);
        }
        foundDetails.append(foundList);
        li.append(foundDetails);
      }

      const imageStatus = el("p", "hint pdf-nav-image-status", tr("Picture status not checked."));
      const imageButton = el("button", "link pdf-nav-image-check", tr("Read supported picture status"));
      imageButton.type = "button";
      imageButton.addEventListener("click", async () => {
        imageButton.disabled = true;
        imageStatus.textContent = tr("Reading page images…");
        try {
          const pictures = await picturesOf(number);
          const readStatus = pictureState(number);
          imageStatus.textContent = readStatus === "failed"
            ? tr("Page images could not be read; the preview may omit content.")
            : pictures.length > 0
              ? `${pictures.length.toLocaleString(currentLocale())} ${tr("supported pictures extracted")}. ${tr("Other drawings may be omitted.")}`
              : tr("No supported extractable pictures were returned; vectors or unsupported images may be omitted.");
        } catch {
          imageStatus.textContent = tr("Page images could not be read; the preview may omit content.");
        } finally {
          imageButton.disabled = false;
        }
      });
      li.append(imageStatus, imageButton);
      pages.append(li);
    }
    const hiddenCount = found.length - MAX_VISIBLE_PAGES;
    more.hidden = hiddenCount <= 0;
    if (hiddenCount > 0) more.textContent = `${hiddenCount.toLocaleString(currentLocale())} ${tr("more pages match; filter by page number to reach them.")}`;
    if (found.length === 0) status.textContent = tr("No pages match this number.");
    else status.textContent = `${pageData.length.toLocaleString(currentLocale())} ${tr("pages read")} · ${pageData.filter((page) => page.complete).length.toLocaleString(currentLocale())} ${tr("text read")}`;
  };

  load.addEventListener("click", async () => {
    if (encryption) return;
    load.disabled = true;
    status.textContent = tr("Reading PDF pages…");
    try {
      pageData = await loadPages();
      pageObjectNodes = pageObjects(m, pageData);
      pageFindings = new Map();
      for (const item of m.file.facts) {
        const match = PAGE_FACT.exec(item.text);
        if (!match) continue;
        const number = Number(match[1]);
        const group = pageFindings.get(number) ?? [];
        group.push(item);
        pageFindings.set(number, group);
      }
      pageSearch.hidden = false;
      pages.hidden = false;
      tools.hidden = false;
      renderPages();
    } catch {
      status.textContent = tr("Could not read PDF page text.");
    } finally {
      load.disabled = false;
    }
  });
  pageSearch.addEventListener("input", renderPages);
  return section;
}
