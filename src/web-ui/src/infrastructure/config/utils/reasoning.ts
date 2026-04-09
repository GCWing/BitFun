import type { AIModelConfig, ReasoningMode } from '../types';

export const DEFAULT_REASONING_MODE: ReasoningMode = 'default';

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
