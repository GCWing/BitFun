import { RemoteModelCatalog } from '../model/RemoteModels';
import { RemoteI18n } from '../i18n/RemoteI18n';
import { ConnectionErrorPolicy } from './ConnectionErrorPolicy';
import { RemoteUiState } from './RemoteUiState';

export interface RemoteModelClient {
  getModelCatalog(sessionId?: string): Promise<RemoteModelCatalog>;
  setSessionModel(sessionId: string, modelId: string): Promise<string>;
}

export interface RemoteModelPreferenceStore {
  saveLastModelId(modelId: string): Promise<void>;
}

export class RemoteModelController {
  private readonly client: RemoteModelClient;
  private readonly preferenceStore: RemoteModelPreferenceStore;
  private readonly onCatalogReset: (
    modelCatalog: RemoteModelCatalog,
    selectedModelId: string,
    knownModelCatalogVersion: number
  ) => void;
  private readonly onCatalogApplied: (
    modelCatalog: RemoteModelCatalog,
    selectedModelId: string,
    knownModelCatalogVersion: number
  ) => void;
  private readonly onStatusText: (statusText: string) => void;
  private readonly onBusy: (isBusy: boolean) => void;
  private modelCatalog: RemoteModelCatalog = RemoteUiState.emptyModelCatalog();
  private selectedModelId: string = '';

  constructor(
    client: RemoteModelClient,
    preferenceStore: RemoteModelPreferenceStore,
    onCatalogReset: (
      modelCatalog: RemoteModelCatalog,
      selectedModelId: string,
      knownModelCatalogVersion: number
    ) => void,
    onCatalogApplied: (
      modelCatalog: RemoteModelCatalog,
      selectedModelId: string,
      knownModelCatalogVersion: number
    ) => void,
    onStatusText: (statusText: string) => void,
    onBusy: (isBusy: boolean) => void
  ) {
    this.client = client;
    this.preferenceStore = preferenceStore;
    this.onCatalogReset = onCatalogReset;
    this.onCatalogApplied = onCatalogApplied;
    this.onStatusText = onStatusText;
    this.onBusy = onBusy;
  }

  setPreferredModelId(modelId: string): void {
    this.selectedModelId = modelId.trim();
  }

  clearCatalog(): void {
    this.modelCatalog = RemoteUiState.emptyModelCatalog();
    this.onCatalogReset(this.modelCatalog, '', 0);
  }

  async loadCatalog(
    sessionId: string,
    remoteAvailable: boolean,
    canApply: (sessionId: string) => boolean
  ): Promise<void> {
    if (sessionId.length === 0 || !remoteAvailable) {
      return;
    }
    this.clearCatalog();
    try {
      const catalog = await this.client.getModelCatalog(sessionId);
      if (!canApply(sessionId)) {
        return;
      }
      this.applyCatalog(catalog);
    } catch (err) {
      this.onStatusText(ConnectionErrorPolicy.errorText(err));
    }
  }

  async selectModel(
    modelId: string,
    sessionId: string,
    isBusy: boolean,
    remoteAvailable: boolean
  ): Promise<void> {
    if (sessionId.length === 0 || modelId.length === 0 || modelId === this.selectedModelId || isBusy || !remoteAvailable) {
      return;
    }
    try {
      this.onBusy(true);
      this.onStatusText(RemoteI18n.t('status.switchingModel'));
      const selected = await this.client.setSessionModel(sessionId, modelId);
      this.selectedModelId = selected;
      await this.preferenceStore.saveLastModelId(selected);
      this.modelCatalog = {
        version: this.modelCatalog.version,
        models: this.modelCatalog.models,
        default_models: this.modelCatalog.default_models,
        session_model_id: selected
      };
      this.onCatalogApplied(this.modelCatalog, selected, this.modelCatalog.version || 0);
      this.onStatusText(RemoteI18n.t('status.modelSwitched'));
    } catch (err) {
      this.onStatusText(ConnectionErrorPolicy.errorText(err));
    } finally {
      this.onBusy(false);
    }
  }

  applyCatalog(catalog: RemoteModelCatalog): void {
    this.modelCatalog = catalog;
    this.selectedModelId = RemoteUiState.selectedModelIdForCatalog(catalog, this.selectedModelId);
    this.onCatalogApplied(catalog, this.selectedModelId, catalog.version || 0);
  }
}
