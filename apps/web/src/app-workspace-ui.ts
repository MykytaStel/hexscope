import { AlignLeft, Archive, Binary, Ellipsis, Eye, FileText, GitCompare, ListTree, createElement } from "lucide";
import { dismissable } from "./dialogs";
import { currentLocale, translateText } from "./i18n";

const icons = {
  overview: Eye,
  metadata: FileText,
  content: AlignLeft,
  structure: ListTree,
  bytes: Binary,
  compression: Archive,
  compare: GitCompare,
  more: Ellipsis,
};

export function installAppNavigationIcons(root: HTMLElement): void {
  for (const control of root.querySelectorAll<HTMLElement>("button, summary")) {
    const key = control.dataset.appView ?? control.dataset.appAction;
    const icon = key ? icons[key as keyof typeof icons] : undefined;
    if (icon) {
      control.prepend(createElement(icon, {
        "aria-hidden": "true",
        class: "app-nav-icon",
        focusable: "false",
        "stroke-width": 1.7,
      }));
    }
  }
}

function createInspectorSummary(): {
  element: HTMLDivElement;
  label: HTMLElement;
  range: HTMLSpanElement;
  details: HTMLButtonElement;
} {
  const element = document.createElement("div");
  element.className = "inspector-summary";
  const copy = document.createElement("div");
  copy.className = "inspector-summary-copy";
  const label = document.createElement("strong");
  label.className = "inspector-summary-label";
  const range = document.createElement("span");
  range.className = "inspector-summary-range";
  copy.append(label, range);

  const details = document.createElement("button");
  details.type = "button";
  details.className = "inspector-details-toggle";
  details.setAttribute("aria-expanded", "false");
  details.setAttribute("aria-controls", "inspector-details");
  details.textContent = "Details";
  element.append(copy, details);
  return { element, label, range, details };
}

function inspectorFact(node: HTMLElement, name: string): string {
  const labels = new Set([name, translateText(name, currentLocale())]);
  for (const term of node.querySelectorAll<HTMLElement>(".facts dt")) {
    if (labels.has(term.textContent?.trim() ?? "")) return term.nextElementSibling?.textContent?.trim() ?? "";
  }
  return "";
}

function updateInspectorSummary(node: HTMLElement, label: HTMLElement, range: HTMLElement): void {
  const selectedLabel = node.querySelector<HTMLElement>(".crumb.is-current")?.textContent?.trim();
  label.textContent = selectedLabel || node.querySelector<HTMLElement>(".node-label")?.textContent?.trim() || "Selected part";
  const offset = inspectorFact(node, "Offset");
  const length = inspectorFact(node, "Length");
  range.textContent = [
    offset && `${translateText("Offset", currentLocale())}: ${offset}`,
    length && `${translateText("Length", currentLocale())}: ${length}`,
  ]
    .filter(Boolean)
    .join(" · ");
  range.setAttribute("aria-label", range.textContent);
}

/** A selected part stays available beside the tree and bytes on phones. */
export function createMobileInspector(drawer: HTMLElement, requests: HTMLElement): void {
  const file = drawer.querySelector<HTMLElement>(".drawer-file");
  const dialog = document.createElement("dialog");
  dialog.className = "inspector-sheet";
  dialog.dataset.expanded = "false";

  const { element: summary, label, range, details } = createInspectorSummary();
  dialog.append(summary);

  const phone = matchMedia("(max-width: 900px)");
  let requested = requests.dataset.inspectorRequested === "true";
  const dismiss = () => {
    requested = false;
    delete requests.dataset.inspectorRequested;
  };
  dismissable(dialog, true, dismiss);
  document.body.append(dialog);

  const currentNode = () => drawer.querySelector<HTMLElement>(".drawer-node");
  const dockNode = () => dialog.querySelector<HTMLElement>(".drawer-node");
  const nodeForDetails = () => dockNode() ?? currentNode();
  const setExpanded = (expanded: boolean, node = nodeForDetails()) => {
    dialog.dataset.expanded = String(expanded);
    details.setAttribute("aria-expanded", String(expanded));
    details.textContent = expanded ? "Hide details" : "Details";
    if (node) node.hidden = !expanded;
  };
  details.addEventListener("click", () => setExpanded(dialog.dataset.expanded !== "true"));

  const restore = () => {
    const node = dockNode();
    if (dialog.open || !node || !file) return;
    node.hidden = false;
    drawer.insertBefore(node, file);
  };
  const close = () => {
    if (dialog.open) dialog.close();
    restore();
  };
  dialog.addEventListener("close", restore);
  document.addEventListener("keydown", (event) => {
    if (event.key !== "Escape" || !dialog.open || document.querySelector("dialog:modal")) return;
    event.preventDefault();
    event.stopImmediatePropagation();
    dismiss();
    dialog.close();
  }, true);
  const node = currentNode();
  if (node) {
    node.id = "inspector-details";
    new MutationObserver(() => {
      const selected = dockNode();
      if (!dialog.open || !selected) return;
      updateInspectorSummary(selected, label, range);
      setExpanded(false, selected);
    }).observe(node, { childList: true });
  }

  const open = () => {
    if (!phone.matches || dialog.open) return;
    restore();
    const selected = currentNode();
    if (!selected) return;
    dialog.setAttribute("aria-label", selected.querySelector<HTMLElement>(".node-label")?.textContent?.trim() || "Selected part");
    selected.id = "inspector-details";
    updateInspectorSummary(selected, label, range);
    dialog.append(selected);
    setExpanded(false, selected);
    dialog.show();
  };
  const update = () => {
    requested = requests.dataset.inspectorRequested === "true";
    if (requested) open();
    else close();
  };
  new MutationObserver(update).observe(requests, { attributes: true, attributeFilter: ["data-inspector-requested"] });
  phone.addEventListener("change", (event) => {
    if (event.matches && requested) open();
    else if (!event.matches) close();
  });
  if (requested) open();

}
