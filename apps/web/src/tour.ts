// The first time a file is open, four short notes point at what to read
// first: the verdict, what the file gives away and how to remove it, the
// bytes behind every finding, and the keys. Once seen — or skipped — they
// do not come back; "Show the tour" on the landing page brings them back.

const SEEN = "hexscope.tour";

interface Step {
  /** Where the note points; the step is left out when it is not on screen. */
  target: string;
  title: string;
  text: string;
}

const STEPS: Step[] = [
  {
    target: ".group.verdict",
    title: "What hexscope found",
    text: "The answer first, in plain words: damaged, hiding something, giving something away, or healthy. “Show me” goes to the bytes.",
  },
  {
    target: ".group.reveals",
    title: "What the file gives away",
    text: "Places, names, serial numbers, deleted text. “Remove it — save a clean copy” makes a copy without them, here in the tab: nothing is uploaded.",
  },
  {
    target: "#tree, .viewswitch",
    title: "Every byte, explained",
    text: "The structure, part by part: hover or tap anything and it says what it is and where the format defines it.",
  },
  {
    target: ".brand",
    title: "Keys",
    text: "Press ? for the keyboard: O opens a file, N goes to the next problem. Drop several files at once to check them all.",
  },
];

function seen(): boolean {
  try {
    return localStorage.getItem(SEEN) === "done";
  } catch {
    // Storage off: show the tour, and not again this visit.
    return sessionSeen;
  }
}

let sessionSeen = false;

function markSeen(): void {
  sessionSeen = true;
  try {
    localStorage.setItem(SEEN, "done");
  } catch {
    // Nothing to keep it in.
  }
}

/** Makes the tour show again on the next file. */
export function resetTour(): void {
  sessionSeen = false;
  try {
    localStorage.removeItem(SEEN);
  } catch {
    // Nothing to clear.
  }
}

function visible(el: Element | null): el is HTMLElement {
  if (!(el instanceof HTMLElement)) return false;
  const r = el.getBoundingClientRect();
  return r.width > 0 && r.height > 0;
}

/** Starts the tour if it has not been seen; call once a file is on screen. */
export function maybeTour(): void {
  if (seen() || document.querySelector(".tour")) return;
  // Each target is a list tried in order: the first that is on screen.
  const find = (target: string) =>
    target
      .split(",")
      .map((t) => document.querySelector(t.trim()))
      .find(visible) ?? null;
  const steps = STEPS.map((s) => ({ ...s, el: find(s.target) })).filter(
    (s): s is Step & { el: HTMLElement } => s.el !== null,
  );
  if (steps.length === 0) return;
  markSeen();

  const bubble = document.createElement("div");
  bubble.className = "tour";
  bubble.setAttribute("role", "dialog");
  bubble.setAttribute("aria-live", "polite");
  const title = document.createElement("strong");
  const text = document.createElement("p");
  const count = document.createElement("span");
  count.className = "tour-count";
  const skip = document.createElement("button");
  skip.className = "link";
  skip.textContent = "Skip";
  const next = document.createElement("button");
  next.className = "btn btn-primary";
  const bar = document.createElement("div");
  bar.className = "tour-bar";
  bar.append(count, skip, next);
  bubble.append(title, text, bar);
  document.body.append(bubble);

  let i = -1;
  let lit: HTMLElement | null = null;
  const end = () => {
    lit?.classList.remove("tour-lit");
    bubble.remove();
    window.removeEventListener("resize", place);
    window.removeEventListener("keydown", onKey, true);
  };
  function place(): void {
    if (!lit) return;
    const r = lit.getBoundingClientRect();
    const b = bubble.getBoundingClientRect();
    const margin = 12;
    // Below the part when there is room, above it when not, on screen
    // always; a part taller than the screen gets the note at the bottom.
    let top = r.bottom + margin;
    if (top + b.height > innerHeight - margin) top = r.top - b.height - margin;
    if (top < margin) top = innerHeight - b.height - margin;
    const left = Math.min(Math.max(margin, r.left), innerWidth - b.width - margin);
    bubble.style.top = `${Math.round(top)}px`;
    bubble.style.left = `${Math.round(left)}px`;
  }
  const show = (k: number) => {
    lit?.classList.remove("tour-lit");
    i = k;
    const s = steps[i];
    lit = s.el;
    // A tall part is read from its heading.
    lit.scrollIntoView({ block: lit.getBoundingClientRect().height > innerHeight * 0.6 ? "start" : "nearest" });
    lit.classList.add("tour-lit");
    title.textContent = s.title;
    text.textContent = s.text;
    count.textContent = `${i + 1} of ${steps.length}`;
    next.textContent = i === steps.length - 1 ? "Done" : "Next";
    requestAnimationFrame(place);
    next.focus();
  };
  function onKey(e: KeyboardEvent): void {
    if (e.key === "Escape") {
      e.stopPropagation();
      end();
    }
  }
  next.addEventListener("click", () => (i + 1 < steps.length ? show(i + 1) : end()));
  skip.addEventListener("click", end);
  window.addEventListener("resize", place);
  window.addEventListener("keydown", onKey, true);
  show(0);
}
