import { GeneralChatDraftController } from './GeneralChatDraftController';

export class GeneralChatDraftLifecycleController {
  private readonly draftController: GeneralChatDraftController;
  private readonly homeDraftId: string;
  private readonly visibleDraftId: () => string;

  constructor(
    draftController: GeneralChatDraftController,
    homeDraftId: string,
    visibleDraftId: () => string
  ) {
    this.draftController = draftController;
    this.homeDraftId = homeDraftId;
    this.visibleDraftId = visibleDraftId;
  }

  cancel(): void {
    this.draftController.cancel();
  }

  clear(draftId: string): void {
    this.draftController.clear(draftId);
  }

  clearHome(): void {
    this.clear(this.homeDraftId);
  }

  async clearHomeNow(): Promise<void> {
    await this.draftController.clearNow(this.homeDraftId);
  }

  async restore(draftId: string): Promise<string> {
    return this.draftController.restore(draftId);
  }

  async restoreHome(): Promise<string> {
    return this.restore(this.homeDraftId);
  }

  scheduleVisible(text: string): void {
    const draftId = this.visibleDraftId();
    if (draftId.length === 0) {
      return;
    }
    this.draftController.scheduleSave(draftId, text);
  }

  persistVisible(text: string): void {
    const draftId = this.visibleDraftId();
    if (draftId.length === 0) {
      return;
    }
    this.draftController.persistNow(draftId, text);
  }

  async saveNow(draftId: string, text: string): Promise<void> {
    await this.draftController.saveNow(draftId, text);
  }
}
