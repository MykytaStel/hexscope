import { el } from "./dom";
import { registerLazyTranslations } from "./i18n";
import type { FilmScanEdge, FilmScanReport } from "./filmscan";
import type { FileModel } from "./model";
import { dismissTour } from "./tour";

registerLazyTranslations({
  "Film-scan clues": "Ознаки сканування плівкового фото",
  "Prepare a film photo to share": "Підготувати плівкове фото для надсилання",
  "The film lab could not load. Close and reopen it to try again.": "Лабораторія плівкового фото не завантажилася. Закрийте й відкрийте її знову.",
  "Could not inspect the image pixels in this browser.": "Цей браузер не зміг прочитати пікселі зображення.",
  "No clear repeated edge pattern was found. Cropping or low contrast can hide these clues.": "Не знайдено чітких повторюваних ознак біля краю. Обрізання або низький контраст могли їх приховати.",
  "Signal agreement: limited": "Знайдено один тип візуальних ознак",
  "Signal agreement: corroborated": "Узгоджуються два різні типи візуальних ознак",
  "Repeated high-contrast openings": "Повторювані контрастні отвори",
  "Maximum repeated marks on one edge": "Найбільша кількість повторів на одному краї",
  "Frame-like boundary": "Межа, схожа на рамку кадру",
  "Top edge": "Верхній край",
  "Right edge": "Правий край",
  "Bottom edge": "Нижній край",
  "Left edge": "Лівий край",
  "These visual clues do not prove that the picture came from film. Digital borders can look similar; grain is not analyzed.": "Ці візуальні ознаки не доводять, що фото зроблено на плівку. Цифрові рамки можуть виглядати схоже; зернистість не аналізується.",
});

const EDGE_LABELS: Record<FilmScanEdge, string> = {
  top: "Top edge",
  right: "Right edge",
  bottom: "Bottom edge",
  left: "Left edge",
};

/** Render the evidence separately from metadata and avoid implying film provenance. */
export function filmScanCard(result: FilmScanReport, model: FileModel): HTMLElement {
  const card = el("section", "group film-scan-card");
  card.setAttribute("aria-label", "Film-scan clues");
  card.setAttribute("aria-live", "polite");
  card.dataset.state = result.availability === "inspected" ? "complete" : "unavailable";
  card.append(el("h2", undefined, "Film-scan clues"));

  if (result.availability === "unavailable") {
    card.append(el("p", "hint", "Could not inspect the image pixels in this browser."));
  } else if (result.evidenceStrength === "none") {
    card.append(el("p", "hint", "No clear repeated edge pattern was found. Cropping or low contrast can hide these clues."));
  } else {
    card.append(el("p", "film-scan-strength", result.evidenceStrength === "corroborated"
      ? "Signal agreement: corroborated"
      : "Signal agreement: limited"));
    const addEvidence = (label: string, edges: FilmScanEdge[], repeats?: number) => {
      const evidence = el("div", "film-scan-evidence");
      evidence.append(el("h3", undefined, label));
      if (repeats !== undefined) {
        const count = el("p", "film-scan-count");
        count.append(
          el("span", undefined, "Maximum repeated marks on one edge"),
          el("output", undefined, repeats.toLocaleString()),
        );
        evidence.append(count);
      }
      const edgeList = el("ul", "film-scan-edges");
      edges.forEach((edge) => edgeList.append(el("li", undefined, EDGE_LABELS[edge])));
      evidence.append(edgeList);
      card.append(evidence);
    };
    if (result.perforationEdges.length > 0) {
      addEvidence("Repeated high-contrast openings", result.perforationEdges, result.perforationRepeats);
    }
    if (result.frameEdges.length > 0) addEvidence("Frame-like boundary", result.frameEdges);
  }

  card.append(el("p", "hint film-scan-caveat", "These visual clues do not prove that the picture came from film. Digital borders can look similar; grain is not analyzed."));
  if (model.file.dimensions) {
    const entry = el("details", "film-lab-entry");
    const content = el("div");
    entry.append(el("summary", undefined, "Prepare a film photo to share"), content);
    let dispose: (() => void) | undefined;
    let generation = 0;
    entry.addEventListener("toggle", async () => {
      const version = ++generation;
      dispose?.(); dispose = undefined; content.replaceChildren();
      if (!entry.open) return;
      dismissTour();
      try {
        const { filmLab } = await import("./film-lab-ui");
        if (!entry.open || version !== generation || !entry.isConnected) return;
        const lab = filmLab(model, result); dispose = lab.dispose; content.append(lab.view);
      } catch { if (entry.open) content.append(el("p", "hint", "The film lab could not load. Close and reopen it to try again.")); }
    });
    card.append(entry);
  }
  return card;
}
