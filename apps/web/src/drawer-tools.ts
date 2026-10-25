import { announce } from "./announce";
import type { CleanActions } from "./cleancard";
import { el } from "./dom";
import type { FileModel } from "./model";
import type { Shown } from "./pdf-pictures";
import { currentLocale, translateText } from "./i18n";

/** The PDF editor UI is a separate chunk; non-PDF views do not import or parse it. */
export function pdfRedactorPlaceholder(
  m: FileModel,
  cleaning: CleanActions,
  picturesOf: (page: number) => Promise<Shown[]>,
): HTMLElement {
  const group = el("div", "group redactor");
  const status = el("p", "hint", translateText("Loading PDF tools…", currentLocale()));
  status.setAttribute("role", "status");
  group.setAttribute("aria-busy", "true");
  group.append(status);

  const load = () => {
    group.setAttribute("aria-busy", "true");
    status.textContent = translateText("Loading PDF tools…", currentLocale());
    group.replaceChildren(status);
    void import("./pdf-redactor-view")
      .then(({ pdfRedactorView }) => {
        if (group.isConnected) group.replaceWith(pdfRedactorView(m, cleaning, picturesOf));
      })
      .catch(() => {
        if (!group.isConnected) return;
        group.setAttribute("aria-busy", "false");
        status.textContent = translateText("PDF tools could not be loaded. Try again.", currentLocale());
        const retry = el("button", "link", translateText("Try again", currentLocale()));
        retry.addEventListener("click", load);
        group.append(retry);
      });
  };
  load();
  return group;
}

/** Load bug-report details only when the person asks to see or share them. */
export function openStructureReport(m: FileModel): void {
  void import("./report")
    .then(({ openReport }) => openReport(m))
    .catch(() => announce(translateText("Try again", currentLocale())));
}

/** Load the value-bearing JSON export only when the person asks to save it. */
export function saveStructureJson(m: FileModel): void {
  void import("./export")
    .then(({ saveStructure }) => saveStructure(m))
    .catch(() => announce(translateText("Try again", currentLocale())));
}
