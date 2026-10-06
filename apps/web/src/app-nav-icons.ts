import { AlignLeft, Archive, Binary, Eye, FileText, GitCompare, ListTree, createElement } from "lucide";

const icons = {
  overview: Eye,
  metadata: FileText,
  content: AlignLeft,
  structure: ListTree,
  bytes: Binary,
  compression: Archive,
  compare: GitCompare,
};

export function installAppNavigationIcons(root: HTMLElement): void {
  for (const button of root.querySelectorAll<HTMLButtonElement>("button")) {
    const key = button.dataset.appView ?? button.dataset.appAction;
    const icon = key ? icons[key as keyof typeof icons] : undefined;
    if (icon) {
      button.prepend(createElement(icon, {
        "aria-hidden": "true",
        class: "app-nav-icon",
        focusable: "false",
        "stroke-width": 1.7,
      }));
    }
  }
}
