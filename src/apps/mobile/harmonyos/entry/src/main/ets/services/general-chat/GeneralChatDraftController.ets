import { GeneralChatDraftStore } from './GeneralChatPort';

export interface GeneralChatDraftScheduler {
  setTimeout(callback: () => void, delayMs: number): number;
  clearTimeout(timerId: number): void;
}

class SystemGeneralChatDraftScheduler implements GeneralChatDraftScheduler {
  setTimeout(callback: () => void, delayMs: number): number {
    return setTimeout(callback, delayMs);
  }

  clearTimeout(timerId: number): void {
    clearTimeout(timerId);
  }
}

export class GeneralChatDraftController {
  private readonly store: GeneralChatDraftStore;
  private readonly delayMs: number;
  private readonly scheduler: GeneralChatDraftScheduler;
  private readonly onError: (err: Error) => void;
  private timerId: number = 0;

  constructor(
    store: GeneralChatDraftStore,
    delayMs: number = 250,
    onError: (err: Error) => void = (_err: Error) => {},
    scheduler: GeneralChatDraftScheduler = new SystemGeneralChatDraftScheduler()
  ) {
    this.store = store;
    this.delayMs = delayMs;
    this.scheduler = scheduler;
    this.onError = onError;
  }

  scheduleSave(draftId: string, text: string): void {
    if (draftId.length === 0) {
      return;
    }
    this.cancel();
    this.timerId = this.scheduler.setTimeout(() => {
      this.timerId = 0;
      this.save(draftId, text);
    }, this.delayMs);
  }

  persistNow(draftId: string, text: string): void {
    if (draftId.length === 0) {
      return;
    }
    this.cancel();
    this.saveNow(draftId, text).catch((_err: Error) => {});
  }

  async saveNow(draftId: string, text: string): Promise<void> {
    if (draftId.length === 0) {
      return;
    }
    this.cancel();
    try {
      await this.store.saveDraft(draftId, text);
    } catch (err) {
      this.onError(err as Error);
    }
  }

  async restore(draftId: string): Promise<string> {
    if (draftId.length === 0) {
      return '';
    }
    try {
      return await this.store.draft(draftId);
    } catch (err) {
      this.onError(err as Error);
      return '';
    }
  }

  clear(draftId: string): void {
    if (draftId.length === 0) {
      return;
    }
    this.cancel();
    this.clearNow(draftId).catch((_err: Error) => {});
  }

  async clearNow(draftId: string): Promise<void> {
    if (draftId.length === 0) {
      return;
    }
    this.cancel();
    try {
      await this.store.clearDraft(draftId);
    } catch (err) {
      this.onError(err as Error);
    }
  }

  cancel(): void {
    if (this.timerId !== 0) {
      this.scheduler.clearTimeout(this.timerId);
      this.timerId = 0;
    }
  }

  isPending(): boolean {
    return this.timerId !== 0;
  }

  private save(draftId: string, text: string): void {
    this.saveNow(draftId, text).catch((_err: Error) => {});
  }
}
