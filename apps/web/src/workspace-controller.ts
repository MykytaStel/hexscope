import { tooLarge } from "./files";
import { FileModel } from "./model";

export interface WorkspaceLevel {
  model: FileModel;
  selected: number;
}

export interface WorkspaceServices {
  parse(file: File): Promise<FileModel>;
  openEntry(index: number): Promise<FileModel>;
  openBytes(bytes: Uint8Array): Promise<FileModel>;
  back(depth: number): Promise<void>;
}

export interface WorkspaceView {
  loading(name: string): void;
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

/** Current file and nested-file history, independent of the DOM composition. */
export class WorkspaceController {
  private current: FileModel | null = null;
  private history: WorkspaceLevel[] = [];
  private request = 0;

  constructor(private readonly options: WorkspaceControllerOptions) {}

  get model(): FileModel | null {
    return this.current;
  }

  get levels(): readonly WorkspaceLevel[] {
    return this.history;
  }

  /** Clears the open workspace when the app changes to a different flow. */
  clear(): void {
    this.request++;
    this.current = null;
    this.history = [];
    this.options.view.clearPlayer();
  }

  async openFile(file: File): Promise<void> {
    const request = ++this.request;
    this.options.view.clearPlayer();
    this.options.view.loading(file.name);
    if (file.size > this.options.maxFileBytes) {
      if (request === this.request) this.options.view.failed(tooLarge(file));
      return;
    }

    try {
      const parsed = await this.options.services.parse(file);
      if (request !== this.request) return;
      this.current = parsed;
      this.history = [];
      this.options.view.remember(file);
      this.options.view.show(parsed, this.history);
      this.options.view.afterOpen(parsed);
    } catch (error) {
      if (request !== this.request) return;
      const message = error instanceof Error ? error.message : String(error);
      this.options.view.failed(`Could not read ${file.name}: ${message}`);
    }
  }

  async openEntry(index: number, name: string): Promise<void> {
    const parent = this.current;
    if (!parent) return;
    const request = ++this.request;
    const selected = this.options.getSelected();
    this.options.view.clearPlayer();
    try {
      const opened = await this.options.services.openEntry(index);
      if (request !== this.request || this.current !== parent) return;
      this.history = [...this.history, { model: parent, selected }];
      this.current = this.withName(opened, name);
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
    const selected = this.options.getSelected();
    this.options.view.clearPlayer();
    try {
      const opened = await this.options.services.openBytes(bytes.slice());
      if (request !== this.request || this.current !== parent) return;
      this.history = [...this.history, { model: parent, selected }];
      this.current = this.withName(opened, name);
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

  /** The parser model is immutable; an entry only needs its user-facing name. */
  private withName(model: FileModel, name: string): FileModel {
    if (model.name === name) return model;
    return new FileModel(model.file, model.bytes, name, model.source);
  }
}
