// What a screen reader should hear when something happens that is not a
// page load: a file opened, a copy saved, places found. One polite region,
// read after whatever is being read now.

let region: HTMLElement | null = null;

export function announce(message: string): void {
  if (!region) {
    region = document.createElement("div");
    region.className = "visually-hidden";
    region.setAttribute("role", "status");
    region.setAttribute("aria-live", "polite");
    document.body.append(region);
  }
  const r = region;
  // Emptied first, so the same words said twice are heard twice.
  r.textContent = "";
  setTimeout(() => (r.textContent = message), 60);
}

/** A copy made: the line that says so on the page, said to a screen reader too. */
export function done(text: string): HTMLParagraphElement {
  announce(text.replace(/ (Removed|What was done):$/, ""));
  const p = document.createElement("p");
  p.className = "clean-done";
  p.textContent = text;
  return p;
}
