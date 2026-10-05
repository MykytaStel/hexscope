import type { BatchItem } from "./batch";
import type { CleanCopiesView } from "./batchclean";

export type BatchAnalysis = Pick<BatchItem, "kind" | "lines" | "headline" | "reveals" | "cleanName" | "photoSignal">;

/** The output that batch analysis and clean-copy work share with its view. */
export interface BatchViewPort extends CleanCopiesView {
  render(items: BatchItem[]): void;
}

export interface BatchControllerHooks {
  analyze(file: File): Promise<BatchAnalysis>;
  openFile(file: File): void;
  saveClean(items: BatchItem[], keepNames: boolean, output: BatchViewPort, isCurrent: () => boolean): Promise<void>;
  announce(message: string): void;
}

/** Owns the queue while BatchView only draws its current state. */
export class BatchController {
  private current: BatchItem[] | null = null;
  private paused = true;
  private running = false;
  private generation = 0;
  private announced = -1;

  constructor(
    private readonly view: BatchViewPort,
    private readonly hooks: BatchControllerHooks,
  ) {}

  get items(): BatchItem[] | null {
    return this.current;
  }

  /** Starts a new list. The composition root decides when its batch view is shown. */
  start(files: File[]): void {
    this.generation++;
    this.announced = -1;
    this.paused = true;
    this.current = files.map((file) => ({
      file,
      state: "waiting",
      kind: "",
      lines: [],
      headline: null,
      reveals: [],
      cleanName: file.name,
      note: "",
      skip: false,
    }));
    this.view.result = "";
  }

  /** Invalidates any analysis or clean-copy work still using the previous list. */
  clear(): void {
    this.generation++;
    this.current = null;
    this.paused = true;
    this.announced = -1;
  }

  pause(): void {
    this.paused = true;
  }

  resume(): void {
    const items = this.current;
    if (!items) return;
    this.paused = false;
    this.view.render(items);
    void this.run(items, this.generation);
  }

  open(index: number): void {
    const item = this.current?.[index];
    if (!item || item.state === "waiting" || item.state === "reading") return;
    this.pause();
    this.hooks.openFile(item.file);
  }

  async saveClean(keepNames: boolean): Promise<void> {
    const items = this.current;
    if (!items) return;
    const generation = this.generation;
    await this.hooks.saveClean(items, keepNames, this.view, () => this.isCurrent(items, generation));
  }

  private isCurrent(items: BatchItem[], generation: number): boolean {
    return this.current === items && this.generation === generation;
  }

  private async run(items: BatchItem[], generation: number): Promise<void> {
    if (this.running) return;
    this.running = true;
    try {
      for (const item of items) {
        if (!this.isCurrent(items, generation) || this.paused) break;
        if (item.state !== "waiting") continue;
        item.state = "reading";
        this.view.render(items);
        try {
          const analysis = await this.hooks.analyze(item.file);
          if (!this.isCurrent(items, generation)) return;
          Object.assign(item, analysis, { state: "done" as const });
        } catch (error) {
          if (!this.isCurrent(items, generation)) return;
          item.state = "failed";
          item.note = error instanceof Error ? error.message : String(error);
        }
        if (this.isCurrent(items, generation)) this.view.render(items);
      }
    } finally {
      this.running = false;
    }

    if (!this.isCurrent(items, generation)) {
      // A new list can be shown before an old worker request settles.
      const next = this.current;
      if (next && !this.paused) void this.run(next, this.generation);
      return;
    }
    if (!this.paused && items.some((item) => item.state === "waiting")) {
      void this.run(items, generation);
      return;
    }
    if (!this.paused && this.announced !== generation && items.every((item) => item.state === "done" || item.state === "failed")) {
      this.announced = generation;
      const revealing = items.filter((item) => item.reveals.length > 0).length;
      this.hooks.announce(
        `${items.length} files read. ${revealing === 0 ? "No personal metadata found by the available checks." : `${revealing} reveal something about you.`}`,
      );
    }
  }
}
