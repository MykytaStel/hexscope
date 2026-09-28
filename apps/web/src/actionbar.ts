// On a phone, the one thing to do about a file — save a clean copy, share
// it once made, save a repaired copy — kept at the bottom of the screen
// while its own button is scrolled out of sight. It presses that button;
// it does nothing of its own.

/** The buttons it stands in for, the first found winning: what to do next. */
const TARGETS: [string, string][] = [
  // Places chosen to black out: that copy is what is being made.
  [".redactor > .btn-primary:not([hidden])", "Save the blacked-out copy"],
  [".reveals .cleaner .btn-primary", "Share the clean copy"],
  [".reveals .btn-clean", "Save a clean copy"],
  [".btn-repair", "Save a repaired copy"],
];

export class ActionBar {
  private readonly bar = document.createElement("div");
  private readonly button = document.createElement("button");
  private target: HTMLElement | null = null;
  private seen = true;
  // Seen: the button itself, or the verdict's own button for the same thing.
  private readonly inView = new Set<Element>();
  private readonly visible = new IntersectionObserver((entries) => {
    for (const e of entries) {
      if (e.isIntersecting) this.inView.add(e.target);
      else this.inView.delete(e.target);
    }
    this.recount();
  });
  private readonly changes = new MutationObserver(() => this.pick());

  constructor() {
    this.bar.className = "actionbar";
    this.bar.hidden = true;
    this.button.type = "button";
    this.button.className = "btn btn-primary";
    this.button.addEventListener("click", () => {
      const t = this.target;
      if (!t) return;
      t.click();
      // Where the result will be said.
      t.closest(".cleaner, .repairer, .group")?.scrollIntoView({ behavior: "smooth", block: "start" });
    });
    this.bar.append(this.button);
    document.body.append(this.bar);
  }

  /** Follows the buttons in `root` as it is redrawn. */
  watch(root: HTMLElement): void {
    this.changes.disconnect();
    this.changes.observe(root, { childList: true, subtree: true });
    this.root = root;
    this.pick();
  }

  private root: HTMLElement | null = null;
  private cta: Element | null = null;

  private pick(): void {
    let found: HTMLElement | null = null;
    let label = "";
    for (const [sel, text] of TARGETS) {
      const b = this.root?.querySelector<HTMLButtonElement>(sel);
      if (b && !b.disabled) {
        found = b;
        label = text;
        break;
      }
    }
    if (found !== this.target) {
      if (this.target) this.forget(this.target);
      this.target = found;
      this.seen = true;
      if (found) this.visible.observe(found);
    }
    // A new file's verdict button replaces the last one's.
    const cta = this.root?.querySelector(".verdict-cta") ?? null;
    if (cta !== this.cta) {
      if (this.cta) this.forget(this.cta);
      this.cta = cta;
      if (cta) this.visible.observe(cta);
    }
    this.button.textContent = label;
    // Until the observer says where a new button is, it counts as seen: no flash of the bar.
    this.show();
  }

  /** Stops watching an element, and forgets it was in view: it may never say otherwise. */
  private forget(el: Element): void {
    this.visible.unobserve(el);
    this.inView.delete(el);
  }

  /** Seen: the button itself, or — for a clean copy — the verdict's own button for the same thing. */
  private recount(): void {
    const t = this.target;
    this.seen =
      !!t &&
      [...this.inView].some(
        (el) => el.isConnected && (el === t || (el === this.cta && !(el as HTMLElement).hidden && t.classList.contains("btn-clean"))),
      );
    this.show();
  }

  private show(): void {
    this.bar.hidden = !this.target || this.seen;
  }
}
