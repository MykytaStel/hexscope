import { AlignLeft, Archive, Binary, Ellipsis, Eye, FileText, GitCompare, ListTree, createElement } from "lucide";

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
