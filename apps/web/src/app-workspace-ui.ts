import { AlignLeft, Archive, Binary, Ellipsis, Eye, FileText, GitCompare, ListTree, createElement } from "lucide";
import { dismissable } from "./dialogs";

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

/** A selected part moves into a native modal sheet on phones, then returns to the details pane. */
export function createMobileInspector(drawer: HTMLElement, requests: HTMLElement): void {
  const file = drawer.querySelector<HTMLElement>(".drawer-file");
  const dialog = document.createElement("dialog");
  dialog.className = "inspector-sheet";
  dialog.setAttribute("aria-modal", "true");
  dismissable(dialog);
  document.body.append(dialog);
  const phone = matchMedia("(max-width: 900px)");
  let requested = requests.dataset.inspectorRequested === "true";
  let responsiveClosePending = false;

  const currentNode = () => drawer.querySelector<HTMLElement>(".drawer-node");
  const restore = () => {
    const node = dialog.querySelector<HTMLElement>(".drawer-node");
    if (dialog.open || !node || !file) return;
    drawer.insertBefore(node, file);
  };
  const close = (keepRequest = false) => {
    if (dialog.open) {
      responsiveClosePending = keepRequest;
      dialog.close();
    }
    restore();
  };
  dialog.addEventListener("close", () => {
    if (dialog.open) {
      responsiveClosePending = false;
      return;
    }
    restore();
    if (responsiveClosePending) {
      responsiveClosePending = false;
      return;
    }
    requested = false;
    delete requests.dataset.inspectorRequested;
  });
  const open = () => {
    const node = currentNode();
    if (!phone.matches || dialog.open || !node) return;
    const title = node.querySelector<HTMLElement>(".node-label");
    if (!title) return;
    dialog.setAttribute("aria-label", title.textContent?.trim() || "Selected part");
    dialog.append(node);
    dialog.showModal();
  };
  const update = () => {
    requested = requests.dataset.inspectorRequested === "true";
    if (requested) open();
    else close();
  };
  new MutationObserver(update).observe(requests, { attributes: true, attributeFilter: ["data-inspector-requested"] });
  phone.addEventListener("change", (event) => {
    if (event.matches && requested) open();
    else if (!event.matches) close(true);
  });
  if (requested) open();

}
