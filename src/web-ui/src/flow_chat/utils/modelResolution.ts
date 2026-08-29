/**
 * Model selector resolution shared by session creation and the composer.
 *
 * Leaf module: session drivers and flow-chat-manager modules both depend on
 * it, so it must not import either.
 */

import { createLogger } from '@/shared/utils/logger';
import type {
  AIModelConfig,
  AgentModelDefaultsConfig,
  DefaultModelsConfig,
} from '@/infrastructure/config/types';
import { aiApi } from '@/infrastructure/api/service-api/AIApi';
import { getRecentReasoningPreset } from './reasoningPresets';

const log = createLogger('ModelResolution');

function findEnabledModel(models: AIModelConfig[], modelRef: string | null | undefined): AIModelConfig | null {
  const value = modelRef?.trim();
  if (!value) return null;
  return models.find(model =>
    model.enabled !== false
    && (model.id === value || model.name === value || model.model_name === value)
  ) ?? null;
}

function resolveModelForContextWindow(
  modelRef: string | null | undefined,
  models: AIModelConfig[],
  defaultModels: DefaultModelsConfig,
): AIModelConfig | null {
  const value = modelRef?.trim();
  if (!value) return null;

  if (value === 'primary') {
    return findEnabledModel(models, defaultModels.primary);
  }

  if (value === 'fast') {
    return findEnabledModel(models, defaultModels.fast) ?? findEnabledModel(models, defaultModels.primary);
  }

  return findEnabledModel(models, value);
}

export async function getModelMaxTokens(modelName?: string, agentType?: string): Promise<number> {
  try {
    const configManager = await import('@/infrastructure/config/services/ConfigManager').then(m => m.configManager);
    const configData = await configManager.getConfigs([
      'ai.models',
      'ai.default_models',
      'ai.agent_model_defaults',
    ]);
    const models = (configData['ai.models'] as AIModelConfig[] | undefined) || [];
    const defaultModels = (configData['ai.default_models'] as DefaultModelsConfig | undefined) || {};
    const agentModelDefaults = configData['ai.agent_model_defaults'] as AgentModelDefaultsConfig | undefined;

    const normalizedModelName = modelName?.trim();
    const explicitModel = resolveModelForContextWindow(modelName, models, defaultModels);
    if (explicitModel?.context_window) {
      return explicitModel.context_window;
    }

    // Only legacy sessions without a model selector inherit the current mode
    // default. Session-owned selectors are resolved above.
    if (!normalizedModelName) {
      const modeModel = resolveModelForContextWindow(
        agentModelDefaults?.mode,
        models,
        defaultModels,
      );
      if (modeModel?.context_window) {
        return modeModel.context_window;
      }
    }

    const primaryModel = resolveModelForContextWindow('primary', models, defaultModels);
    if (primaryModel?.context_window) {
      return primaryModel.context_window;
    }

    log.debug('Model context_window config not found, using default', { modelName, agentType });
    return 128128;
  } catch (error) {
    log.warn('Failed to get model max tokens', { modelName, agentType, error });
    return 128128;
  }
}

export async function resolveReasoningPresetForSessionCreation(
  modelName: string,
): Promise<string | undefined> {
  try {
    const catalog = await aiApi.getModelCatalog();
    const selectedModelId = modelName === 'primary'
      ? catalog.default_models.primary ?? undefined
      : modelName === 'fast'
        ? catalog.default_models.fast ?? catalog.default_models.primary ?? undefined
        : modelName;
    const concreteModelId = selectedModelId
      && catalog.models.some(model => model.id === selectedModelId)
      ? selectedModelId
      : catalog.default_models.primary ?? undefined;
    if (!concreteModelId) return undefined;
    const projection = catalog.models.find(model => model.id === concreteModelId)?.reasoning;
    if (projection?.status !== 'known') return undefined;
    const preset = getRecentReasoningPreset(concreteModelId);
    return projection.presets?.some(item => item.id === preset) ? preset : undefined;
  } catch (error) {
    log.warn('Failed to resolve recent reasoning preset during session creation', { error });
    return undefined;
  }
}
