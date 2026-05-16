import type { AIModelConfig, ReasoningMode } from '../types';
import { getProviderTemplateId } from '../services/modelConfigs';

export const DEFAULT_REASONING_MODE: ReasoningMode = 'default';

const DEEPSEEK_REASONING_EFFORT_MODELS = new Set([
  'deepseek-v4-flash',
  'deepseek-v4-pro',
]);

export function getEffectiveReasoningMode(
  config?: Pick<AIModelConfig, 'reasoning_mode'> | null
): ReasoningMode {
  return config?.reasoning_mode ?? DEFAULT_REASONING_MODE;
}

export function isReasoningVisiblyEnabled(mode: ReasoningMode): boolean {
  return mode === 'enabled' || mode === 'adaptive';
}

export function supportsResponsesReasoning(provider?: string): boolean {
  return provider === 'response' || provider === 'responses';
}

export function supportsAnthropicReasoning(provider?: string): boolean {
  return provider === 'anthropic';
}

export function supportsAnthropicAdaptive(modelName?: string): boolean {
  const normalized = (modelName || '').trim().toLowerCase();
  return normalized.startsWith('claude-opus-4-6')
    || normalized.startsWith('claude-sonnet-4-6')
    || normalized.startsWith('claude-mythos');
}

export function supportsAnthropicThinkingBudget(modelName?: string): boolean {
  return (modelName || '').trim().toLowerCase().startsWith('claude');
}

export function supportsDeepSeekReasoningEffort(
  config?: Partial<Pick<AIModelConfig, 'name' | 'base_url' | 'model_name'>> | null
): boolean {
  const normalizedModelName = (config?.model_name || '').trim().toLowerCase();
  if (DEEPSEEK_REASONING_EFFORT_MODELS.has(normalizedModelName)) {
    return true;
  }

  return getProviderTemplateId({
    name: config?.name,
    base_url: config?.base_url,
    model_name: config?.model_name,
  }) === 'deepseek';
}
