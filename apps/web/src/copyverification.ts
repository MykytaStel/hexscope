// Loaded only after the user makes a copy: the comparison and its report do
// not add to the first bundle a person needs to open a file.
import type { FileModel } from "./model";
import { call } from "./rpc";
import { el } from "./dom";
import { FACT_LABELS, KEPT, KEPT_NOTE } from "./knowledge";
import {
  checkCopySafely,
  fileFindings,
  type VerificationReport,
  type VerificationSelection,
} from "./verification";

const VERIFY_TIMEOUT_MS = 10_000;
const FACT_LABELS_WITH_LOCATION = { ...FACT_LABELS, location: "Location" };

export function verifyCopyInWorker(
  model: FileModel,
  copy: Blob,
  selections: VerificationSelection[],
): Promise<VerificationReport> {
  const sourceFindings = fileFindings(model.file.format, model.file.facts, model.file.location, FACT_LABELS_WITH_LOCATION);
  const kept = KEPT[model.file.format] ?? [];
  const retainedReasons = Object.fromEntries(
    kept.map((kind) => [kind, KEPT_NOTE[model.file.format] ?? "This finding is part of the file's content and stays in the clean copy."]),
  );
  return checkCopySafely(copy.size, sourceFindings, selections, () =>
    call(
      {
        type: "verifyCopy",
        copy,
        sourceFormat: model.file.format,
        sourceFindings,
        retainedReasons,
        ...(selections.length ? { selections } : {}),
      },
      VERIFY_TIMEOUT_MS,
    ),
  );
}

export function renderCopyVerification(region: HTMLElement, verification: Promise<VerificationReport>): void {
  const render = (report: VerificationReport) => {
    if (report.present.length || report.unchecked.length) {
      region.closest(".cleaner")?.querySelector(".before-after .ba-side.is-clear")?.classList.remove("is-clear");
    }
    const groups: [string, VerificationReport["removed"]][] = [
      ["Removed", report.removed],
      ["Still present", report.present],
      ["Not checked", report.unchecked],
    ];
    region.replaceChildren(
      ...groups.map(([heading, items]) => {
        const group = el("section", "copy-verification-group");
        group.append(el("h3", undefined, heading));
        const list = el("ul", "copy-verification-list");
        for (const item of items) {
          const row = el("li", "copy-verification-item");
          row.append(el("span", "copy-verification-label", item.label));
          const reason = item.reason ?? (item.unexpected ? "This finding appeared in the copy but was not found in the source file." : undefined);
          if (reason) row.append(el("p", "verify-reason", reason));
          list.append(row);
        }
        group.append(list);
        return group;
      }),
    );
  };

  void verification.then(render, () =>
    render({
      removed: [],
      present: [],
      unchecked: [{ kind: "verification", label: "Copy verification", reason: "Check unavailable." }],
    }),
  );
}
