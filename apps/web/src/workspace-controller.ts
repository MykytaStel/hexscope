import { tooLarge } from "./files";
import type { MovieProgress } from "./movie";
import type { FileModel } from "./model";

export type { MovieProgress } from "./movie";

export interface ParseOptions {
  signal: AbortSignal;
  onProgress(progress: MovieProgress): void;
}

export interface WorkspaceLevel {
  model: FileModel;
  selected: number;
}

export interface WorkspaceServices {
  parse(file: File, options: ParseOptions): Promise<FileModel>;
  openEntry(index: number, name: string): Promise<FileModel>;
  openBytes(bytes: Uint8Array, name: string): Promise<FileModel>;
  back(depth: number): Promise<void>;
}

export interface WorkspaceView {
  loading(name: string): void;
  progress(progress: MovieProgress): void;
  show(model: FileModel, levels: readonly WorkspaceLevel[]): void;
  select(id: number): void;
  failed(message: string): void;
  note(message: string): void;
  remember(file: File): void;
  afterOpen(model: FileModel): void;
  clearPlayer(): void;
}

export interface WorkspaceControllerOptions {
  services: WorkspaceServices;
  view: WorkspaceView;
  maxFileBytes: number;
  getSelected(): number;
}

interface PendingFileOpen {
  request: number;
  file: File;
  controller: AbortController;
  workerReset: boolean;
  retries: number;
}

type ParseResult = { ok: true; model: FileModel } | { ok: false; error: unknown };

/** Current file and nested-file history, independent of the DOM composition. */
export class WorkspaceController {
  private current: FileModel | null = null;
  private history: WorkspaceLevel[] = [];
  private request = 0;
  private pendingOpen: PendingFileOpen | null = null;

  constructor(private readonly options: WorkspaceControllerOptions) {}

  get model(): FileModel | null {
    return this.current;
  }

  get levels(): readonly WorkspaceLevel[] {
    return this.history;
  }

  get isOpeningFile(): boolean {
    return this.pendingOpen?.request === this.request;
  }

  /** Rebuilds the current parser state after its worker has been terminated. */
  async reopenCurrent(): Promise<void> {
    const current = this.current;
    if (!current) return;
    const file = this.fileFor(current);
    await this.openFile(file);
  }

  /** Retries an in-flight file open, or rebuilds the current parser state. */
  async recoverAfterWorkerReset(): Promise<void> {
    const pending = this.pendingOpen;
    if (pending && pending.request === this.request) {
      pending.workerReset = true;
      return;
    }
    await this.reopenCurrent();
  }

  /** Clears the open workspace when the app changes to a different flow. */
  clear(): void {
    this.request++;
    this.stopPendingOpen();
    this.current = null;
    this.history = [];
    this.options.view.clearPlayer();
  }

  async openFile(file: File): Promise<void> {
    this.stopPendingOpen();
    const request = ++this.request;
    const pending = { request, file, controller: new AbortController(), workerReset: false, retries: 0 };
    this.pendingOpen = pending;
    await this.openFileAttempt(pending);
  }

  /** Stops the active file read and returns to the previously open workspace. */
  cancelOpen(): void {
    if (!this.pendingOpen || this.pendingOpen.request !== this.request) return;
    const pending = this.pendingOpen;
    const previous = pending.workerReset || pending.retries > 0 ? this.current : null;
    this.request++;
    this.stopPendingOpen();
    if (previous) {
      const request = this.request;
      const file = this.fileFor(previous);
      const recovery = { request, file, controller: new AbortController(), workerReset: false, retries: 0 };
      this.pendingOpen = recovery;
      void this.openFileAttempt(recovery);
      return;
    }
    this.options.view.failed("Reading was cancelled.");
  }

  private stopPendingOpen(): void {
    this.pendingOpen?.controller.abort();
    this.pendingOpen = null;
  }

  private async openFileAttempt(pending: PendingFileOpen): Promise<void> {
    const { request, file } = pending;
    this.options.view.clearPlayer();
    this.options.view.loading(file.name);
    if (file.size > this.options.maxFileBytes) {
      if (request === this.request) this.options.view.failed(tooLarge(file));
      if (this.pendingOpen === pending) this.pendingOpen = null;
      return;
    }

    let parsed: FileModel;
    try {
      parsed = await this.parseWithRecovery(pending);
    } catch (error) {
      await this.failOpenAfterReset(pending, error);
      return;
    }

    if (request !== this.request) return;
    if (this.pendingOpen === pending) this.pendingOpen = null;
    this.current = parsed;
    this.history = [];
    this.options.view.remember(file);
    this.options.view.show(parsed, this.history);
    this.options.view.afterOpen(parsed);
  }

  private async failOpenAfterReset(pending: PendingFileOpen, error: unknown): Promise<void> {
    if (pending.request !== this.request) return;
    const message = error instanceof Error ? error.message : String(error);
    if (pending.retries > 0 && this.current) {
      const restoreError = await this.restorePreviousAfterFailedOpen(pending);
      if (restoreError) {
        this.finishOpenFailure(pending, `Could not read ${pending.file.name}: ${message}. The previous file could not be restored: ${restoreError}`);
        return;
      }
    }
    this.finishOpenFailure(pending, `Could not read ${pending.file.name}: ${message}`);
  }

  private finishOpenFailure(pending: PendingFileOpen, message: string): void {
    if (pending.request !== this.request) return;
    if (this.pendingOpen === pending) this.pendingOpen = null;
    this.options.view.failed(message);
  }

  private async parseWithRecovery(pending: PendingFileOpen): Promise<FileModel> {
    const first = await this.tryParse(pending.file, pending);
    if (first.ok) return first.model;
    if (!pending.workerReset || pending.retries > 0) throw first.error;
    pending.retries++;
    pending.workerReset = false;
    const retry = await this.tryParse(pending.file, pending);
    if (retry.ok) return retry.model;
    throw retry.error;
  }

  private async restorePreviousAfterFailedOpen(pending: PendingFileOpen): Promise<string | null> {
    const previous = this.current;
    if (!previous) return null;
    const file = this.fileFor(previous);
    let result = await this.tryParse(file, pending);
    if (!result.ok && pending.workerReset) {
      pending.workerReset = false;
      result = await this.tryParse(file, pending);
    }
    if (!result.ok) return result.error instanceof Error ? result.error.message : String(result.error);
    if (pending.request !== this.request) return null;
    this.current = result.model;
    this.history = [];
    this.options.view.remember(file);
    this.options.view.show(result.model, this.history);
    this.options.view.afterOpen(result.model);
    return null;
  }

  private async tryParse(file: File, pending: PendingFileOpen): Promise<ParseResult> {
    try {
      return {
        ok: true,
        model: await this.options.services.parse(file, {
          signal: pending.controller.signal,
          onProgress: (progress) => {
            if (pending.request === this.request && this.pendingOpen === pending) this.options.view.progress(progress);
          },
        }),
      };
    } catch (error) {
      return { ok: false, error };
    }
  }

  private fileFor(model: FileModel): File {
    return new File([model.source ?? (model.bytes as BlobPart)], model.name);
  }

  async openEntry(index: number, name: string): Promise<void> {
    const parent = this.current;
    if (!parent) return;
    const request = ++this.request;
    this.stopPendingOpen();
    const selected = this.options.getSelected();
    this.options.view.clearPlayer();
    try {
      const opened = await this.options.services.openEntry(index, name);
      if (request !== this.request || this.current !== parent) return;
      this.history = [...this.history, { model: parent, selected }];
      this.current = opened;
      this.options.view.show(this.current, this.history);
      this.options.view.afterOpen(this.current);
    } catch (error) {
      if (request !== this.request || this.current !== parent) return;
      this.options.view.note(error instanceof Error ? error.message : String(error));
    }
  }

  async openInside(bytes: Uint8Array, name: string): Promise<void> {
    const parent = this.current;
    if (!parent) return;
    const request = ++this.request;
    this.stopPendingOpen();
    const selected = this.options.getSelected();
    this.options.view.clearPlayer();
    try {
      const opened = await this.options.services.openBytes(bytes.slice(), name);
      if (request !== this.request || this.current !== parent) return;
      this.history = [...this.history, { model: parent, selected }];
      this.current = opened;
      this.options.view.show(this.current, this.history);
      this.options.view.afterOpen(this.current);
    } catch (error) {
      if (request !== this.request || this.current !== parent) return;
      this.options.view.note(error instanceof Error ? error.message : String(error));
    }
  }

  async back(depth: number): Promise<void> {
    if (depth >= this.history.length) return;
    const request = ++this.request;
    this.stopPendingOpen();
    const target = this.history[depth];
    this.options.view.clearPlayer();
    try {
      await this.options.services.back(depth);
    } catch (error) {
      if (request === this.request) {
        this.options.view.note(error instanceof Error ? error.message : String(error));
      }
      return;
    }
    if (request !== this.request) return;
    this.current = target.model;
    this.history = this.history.slice(0, depth);
    this.options.view.show(this.current, this.history);
    this.options.view.select(target.selected);
  }
}
