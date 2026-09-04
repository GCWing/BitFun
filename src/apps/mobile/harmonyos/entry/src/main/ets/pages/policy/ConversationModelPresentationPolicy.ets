import {
  ConversationUiModel,
  ConversationUiModelCatalog
} from '../state/ConversationUiModels';

export class ConversationModelPresentationPolicy {
  static enabledModels(catalog: ConversationUiModelCatalog): ConversationUiModel[] {
    return catalog.models.filter((model: ConversationUiModel) => model.enabled);
  }

  static selectedModel(
    catalog: ConversationUiModelCatalog,
    selectedModelId: string
  ): ConversationUiModel | undefined {
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
      const model = catalog.models.find((item: ConversationUiModel) => item.id === modelId && item.enabled);
      if (model) {
        return model;
      }
    }
    return undefined;
  }

  static primaryLabel(model: ConversationUiModel, fallback: string): string {
    const modelName = ConversationModelPresentationPolicy.cleanLabel(model.model_name || '');
    if (ConversationModelPresentationPolicy.isSpecificLabel(modelName)) {
      return modelName;
    }
    const name = ConversationModelPresentationPolicy.cleanLabel(model.name || '');
    if (ConversationModelPresentationPolicy.isSpecificLabel(name)) {
      return name;
    }
    const id = ConversationModelPresentationPolicy.cleanLabel(model.id || '');
    return id.length > 0 ? id : fallback;
  }

  static secondaryLabel(model: ConversationUiModel, fallback: string): string {
    const provider = ConversationModelPresentationPolicy.cleanLabel(model.provider || '');
    const name = ConversationModelPresentationPolicy.cleanLabel(model.name || '');
    const primary = ConversationModelPresentationPolicy.primaryLabel(model, fallback);
    if (provider.length > 0 && name.length > 0 && name !== primary && name !== provider) {
      return `${provider} · ${name}`;
    }
    if (provider.length > 0 && provider !== primary) {
      return provider;
    }
    if (name.length > 0 && name !== primary) {
      return name;
    }
    return model.id || primary;
  }

  private static cleanLabel(value: string): string {
    const trimmed = (value || '').trim();
    if (trimmed.length === 0) {
      return '';
    }
    const withoutScheme = trimmed.replace(/^openbitfun[:/_-]+/i, '').replace(/^anthropic[:/_-]+/i, '');
    const parts = withoutScheme.split(/[/:]/).filter((part: string) => part.length > 0);
    return parts.length > 0 ? parts[parts.length - 1] : withoutScheme;
  }

  private static isSpecificLabel(label: string): boolean {
    const normalized = label.toLowerCase();
    return label.length > 0 &&
      normalized !== 'openbitfun' &&
      normalized !== 'anthropic' &&
      normalized !== 'openai' &&
      normalized !== 'google' &&
      normalized !== 'azure' &&
      normalized !== 'openbitfun';
  }
}
