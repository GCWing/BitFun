/**
 * Settings sheet navigation and editor drafts live here so a fold-driven
 * rebuild does not send the user back to the home list or wipe in-progress
 * local-model fields.
 */
@ObservedV2
export class SettingsSheetState {
  @Trace page: string = 'main';
  @Trace savedGeneralChatApiUrl: string = '';
  @Trace savedGeneralChatModelName: string = '';
  @Trace savedGeneralChatHasApiKey: boolean = false;
  @Trace draftApiUrl: string = '';
  @Trace draftApiKey: string = '';
  @Trace draftModelName: string = '';
  @Trace draftClearApiKey: boolean = false;
  @Trace editorDraftHydrated: boolean = false;

  setPage(page: string): void {
    this.page = page;
  }

  syncSaved(apiUrl: string, modelName: string, hasApiKey: boolean): void {
    this.savedGeneralChatApiUrl = apiUrl;
    this.savedGeneralChatModelName = modelName;
    this.savedGeneralChatHasApiKey = hasApiKey;
  }

  hydrateEditorDraft(apiUrl: string, modelName: string): void {
    if (this.editorDraftHydrated) {
      return;
    }
    this.draftApiUrl = apiUrl;
    this.draftModelName = modelName;
    this.draftApiKey = '';
    this.draftClearApiKey = false;
    this.editorDraftHydrated = true;
  }

  resetOnClose(): void {
    this.page = 'main';
    this.draftApiUrl = '';
    this.draftApiKey = '';
    this.draftModelName = '';
    this.draftClearApiKey = false;
    this.editorDraftHydrated = false;
  }
}
