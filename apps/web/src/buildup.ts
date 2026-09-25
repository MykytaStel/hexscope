// A picture that arrives in steps — a progressive JPEG's scans, an
// interlaced PNG's passes — shown after each one: a slider, and a button
// that plays them in turn.

/** Between steps when it plays. */
const PLAY_MS = 700;

export interface BuildUp {
  /** What the steps are, in a sentence, ending in a colon. */
  intro: string;
  count: number;
  /** Shows the picture after step `k`, 1 to `count`. */
  show(k: number): void;
  /** What the picture after step `k` is. */
  says(k: number): string | Promise<string>;
}

function el<K extends keyof HTMLElementTagNameMap>(tag: K, cls?: string, text?: string): HTMLElementTagNameMap[K] {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (text !== undefined) e.textContent = text;
  return e;
}

/** Fills `host` with the slider, set to the last step, and shows it. */
export function buildUp(host: HTMLElement, b: BuildUp): void {
  const label = el("label", "buildup-label");
  const range = el("input");
  range.type = "range";
  range.min = "1";
  range.max = String(b.count);
  range.value = String(b.count);
  range.setAttribute("aria-label", "Steps shown");
  const says = el("span", "buildup-says");
  const play = el("button", "btn btn-small", "Play");
  let timer = 0;
  let asked = 0;
  const set = (k: number) => {
    range.value = String(k);
    b.show(k);
    const n = ++asked;
    void Promise.resolve(b.says(k)).then((text) => {
      if (n === asked) says.textContent = text;
    });
  };
  const stop = () => {
    clearTimeout(timer);
    timer = 0;
    play.textContent = "Play";
  };
  range.addEventListener("input", () => {
    stop();
    set(Number(range.value));
  });
  play.addEventListener("click", () => {
    if (timer) {
      stop();
      return;
    }
    play.textContent = "Stop";
    let k = 1;
    const step = () => {
      // Another file, or another panel, took its place.
      if (!host.isConnected) return stop();
      set(k);
      if (k === b.count) return stop();
      k++;
      timer = window.setTimeout(step, PLAY_MS);
    };
    step();
  });
  label.append(range);
  host.replaceChildren(el("p", "hint", b.intro), label, play, says);
  set(b.count);
  host.hidden = false;
}
