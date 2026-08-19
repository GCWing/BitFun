import { RemoteModelCatalog, RemoteModelConfig } from '../../model/RemoteModels';
import { GENERAL_CHAT_LOCAL_MODEL_ID } from '../../services/general-chat/GeneralChatConfigStore';

export type ModelServicePage = 'overview' | 'account' | 'local';

/**
 * Presentation rules for the general-chat model settings sheets.
 *
 * The settings row hosts system bindSheets (same contract as ComposerBar):
 * adaptive height, no hand-rolled scrim, and a checkmark that only means
 * "this is the selected model".
 */
export class ModelServiceSettingsPolicy {
  static readonly HEADER_HEIGHT: number = 56;
  static readonly CURRENT_ROW_HEIGHT: number = 64;
  static readonly SOURCE_ROW_HEIGHT: number = 62;
  static readonly ACCOUNT_MODEL_ROW_HEIGHT: number = 56;
  static readonly ACCOUNT_MODEL_ROW_GAP: number = 6;
  static readonly LANGUAGE_ROW_HEIGHT: number = 64;
  static readonly SECTION_LABEL_HEIGHT: number = 20;
  static readonly OVERVIEW_SECTION_GAP: number = 20;
  static readonly OVERVIEW_TOP_PADDING: number = 16;
  static readonly OVERVIEW_BOTTOM_PADDING: number = 24;
  static readonly SOURCE_DIVIDER_HEIGHT: number = 1;
  static readonly LIST_TOP_PADDING: number = 10;
  static readonly LIST_BOTTOM_PADDING: number = 16;
  static readonly EMPTY_ACCOUNT_HEIGHT: number = 80;
  static readonly LOCAL_EDITOR_HEIGHT: number = 560;
  static readonly MAX_ACCOUNT_VISIBLE_ROWS: number = 7;
  static readonly MAX_SHEET_HEIGHT: number = 480;
  static readonly LANGUAGE_LIST_TOP_PADDING: number = 8;
  static readonly LANGUAGE_LIST_BOTTOM_PADDING: number = 28;

  static accountModels(catalog: RemoteModelCatalog): RemoteModelConfig[] {
    return catalog.models.filter((model: RemoteModelConfig): boolean => {
      return ModelServiceSettingsPolicy.isAccountModel(model) && model.enabled;
    });
  }

  static currentModel(catalog: RemoteModelCatalog, selectedModelId: string): RemoteModelConfig | undefined {
    const candidates = [
      selectedModelId,
      catalog.session_model_id || '',
      catalog.default_models.primary || ''
    ];
    for (let index = 0; index < candidates.length; index += 1) {
      const modelId = candidates[index];
      if (modelId.length === 0) {
        continue;
      }
      const model = catalog.models.find((item: RemoteModelConfig): boolean => {
        return item.id === modelId && item.enabled;
      });
      if (model) {
        return model;
      }
    }
    return undefined;
  }

  static modelLabel(model: RemoteModelConfig | undefined, fallback: string): string {
    if (!model) {
      return fallback;
    }
    return model.model_name || model.name || model.id || fallback;
  }

  static isAccountModel(model: RemoteModelConfig): boolean {
    return model.id.startsWith('cloud:');
  }

  static isLocalModel(model: RemoteModelConfig): boolean {
    return model.id === GENERAL_CHAT_LOCAL_MODEL_ID;
  }

  static isAccountModelSelected(catalog: RemoteModelCatalog, selectedModelId: string): boolean {
    const current = ModelServiceSettingsPolicy.currentModel(catalog, selectedModelId);
    return current !== undefined && ModelServiceSettingsPolicy.isAccountModel(current);
  }

  static isLocalModelSelected(catalog: RemoteModelCatalog, selectedModelId: string): boolean {
    const current = ModelServiceSettingsPolicy.currentModel(catalog, selectedModelId);
    return current !== undefined && ModelServiceSettingsPolicy.isLocalModel(current);
  }

  static hasCompleteLocalModel(apiUrl: string, modelName: string, hasApiKey: boolean): boolean {
    return apiUrl.trim().length > 0 && modelName.trim().length > 0 && hasApiKey;
  }

  /**
   * A configured local row selects that model on the body tap.
   * The trailing chevron always opens the editor. An incomplete row
   * opens the editor from either target.
   */
  static localRowSelectsOnBodyTap(apiUrl: string, modelName: string, hasApiKey: boolean): boolean {
    return ModelServiceSettingsPolicy.hasCompleteLocalModel(apiUrl, modelName, hasApiKey);
  }

  static sourceRowShowsSelectionCheck(): boolean {
    return false;
  }

  static overviewSheetHeight(): number {
    return ModelServiceSettingsPolicy.HEADER_HEIGHT +
      ModelServiceSettingsPolicy.OVERVIEW_TOP_PADDING +
      ModelServiceSettingsPolicy.SECTION_LABEL_HEIGHT +
      ModelServiceSettingsPolicy.CURRENT_ROW_HEIGHT +
      ModelServiceSettingsPolicy.OVERVIEW_SECTION_GAP +
      ModelServiceSettingsPolicy.SECTION_LABEL_HEIGHT +
      ModelServiceSettingsPolicy.sourceGroupHeight() +
      ModelServiceSettingsPolicy.OVERVIEW_BOTTOM_PADDING;
  }

  static sourceGroupHeight(): number {
    return ModelServiceSettingsPolicy.SOURCE_ROW_HEIGHT +
      ModelServiceSettingsPolicy.SOURCE_DIVIDER_HEIGHT +
      ModelServiceSettingsPolicy.SOURCE_ROW_HEIGHT;
  }

  static accountListSheetHeight(modelCount: number): number {
    const contentHeight = ModelServiceSettingsPolicy.accountListContentHeight(modelCount);
    return Math.min(
      ModelServiceSettingsPolicy.MAX_SHEET_HEIGHT,
      ModelServiceSettingsPolicy.HEADER_HEIGHT +
        ModelServiceSettingsPolicy.LIST_TOP_PADDING +
        contentHeight +
        ModelServiceSettingsPolicy.LIST_BOTTOM_PADDING
    );
  }

  static accountListContentHeight(modelCount: number): number {
    if (modelCount <= 0) {
      return ModelServiceSettingsPolicy.EMPTY_ACCOUNT_HEIGHT;
    }
    const visibleRows = Math.min(modelCount, ModelServiceSettingsPolicy.MAX_ACCOUNT_VISIBLE_ROWS);
    return visibleRows * ModelServiceSettingsPolicy.ACCOUNT_MODEL_ROW_HEIGHT +
      Math.max(0, visibleRows - 1) * ModelServiceSettingsPolicy.ACCOUNT_MODEL_ROW_GAP;
  }

  static localEditorSheetHeight(): number {
    return ModelServiceSettingsPolicy.LOCAL_EDITOR_HEIGHT;
  }

  static languageSheetHeight(languageCount: number): number {
    const rows = Math.max(0, languageCount);
    return ModelServiceSettingsPolicy.HEADER_HEIGHT +
      ModelServiceSettingsPolicy.LANGUAGE_LIST_TOP_PADDING +
      rows * ModelServiceSettingsPolicy.LANGUAGE_ROW_HEIGHT +
      ModelServiceSettingsPolicy.LANGUAGE_LIST_BOTTOM_PADDING;
  }

  static sheetPage(page: string): ModelServicePage {
    if (page === 'account' || page === 'local') {
      return page;
    }
    return 'overview';
  }

  /**
   * Settings is already a bindSheet. Nested bindSheets on that content do not
   * present, so child pages replace the settings list in place.
   */
  static parentSettingsPage(page: string): string {
    if (page === 'account' || page === 'local') {
      return 'overview';
    }
    return 'main';
  }
}
