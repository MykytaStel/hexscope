import type { Drawer } from "./drawer";
import type { FileModel } from "./model";
import { currentLocale, translateText } from "./i18n";

type OriginView = "overview" | "metadata" | "content" | "map";

export interface EvidenceReturnPosition {
  scrollTop: number;
  focusTarget: HTMLElement | null;
}

interface EvidenceReturnContext extends EvidenceReturnPosition {
  model: FileModel;
  view: OriginView;
}

export interface EvidenceNavigation {
  clear(): void;
  remember(model: FileModel): void;
  followSource(id: number, model: FileModel | null): void;
  takeReturn(model: FileModel | null, view: string): EvidenceReturnPosition | null;
}

/** Keeps a short-lived route from a selected byte back to the result it came from. */
export function createEvidenceNavigation(
  drawer: Drawer,
  host: HTMLElement,
  requests: HTMLElement,
  currentModel: () => FileModel | null,
  currentView: () => string,
  setView: (view: OriginView, scrollToSection?: boolean, recordHistory?: boolean) => void,
  toBytes: () => void,
  selectPart: (id: number) => void,
): EvidenceNavigation {
  let context: EvidenceReturnContext | null = null;

  const clear = () => {
    context = null;
    drawer.setEvidenceReturn(null);
    delete requests.dataset.inspectorRequested;
  };

  const updateLabel = () => {
    const saved = context;
    if (!saved || currentModel() !== saved.model) return;
    drawer.setEvidenceReturn(translateText("Back to previous place", currentLocale()), () => {
      if (context !== saved || currentModel() !== saved.model) return;
      setView(saved.view, false, true);
    });
  };

  const remember = (model: FileModel) => {
    const view = currentView();
    if (currentModel() !== model || !(view === "overview" || view === "metadata" || view === "content" || view === "map")) return;
    const file = host.querySelector<HTMLElement>(".drawer-file");
    if (!file) return;
    const active = document.activeElement;
    context = {
      model,
      view,
      scrollTop: file.scrollTop,
      focusTarget: active instanceof HTMLElement && file.contains(active) ? active : null,
    };
    updateLabel();
  };

  const takeReturn = (model: FileModel | null, view: string): EvidenceReturnPosition | null => {
    if (!context || context.model !== model || context.view !== view) return null;
    const position = { scrollTop: context.scrollTop, focusTarget: context.focusTarget };
    clear();
    return position;
  };

  window.addEventListener("hexscope:locale", updateLabel);

  return {
    clear,
    remember,
    takeReturn,
    followSource(id, model) {
      if (!model || currentModel() !== model) return;
      remember(model);
      toBytes();
      selectPart(id);
      if (context?.model === model) requestAnimationFrame(() => drawer.focusEvidenceReturn());
    },
  };
}
