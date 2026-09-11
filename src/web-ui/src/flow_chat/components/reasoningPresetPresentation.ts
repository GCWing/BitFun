import type { useTranslation } from 'react-i18next';
import type { ReasoningPresetDescriptor } from '@/infrastructure/config/types';

export function presetLabel(
  preset: ReasoningPresetDescriptor,
  t: ReturnType<typeof useTranslation>['t'],
): string {
  return t(`reasoningEffort.${preset.id}`, { defaultValue: preset.label || preset.id });
}

export function presetDisplayLabel(
  preset: ReasoningPresetDescriptor,
  t: ReturnType<typeof useTranslation>['t'],
): string {
  const fallback = presetLabel(preset, t);
  const semanticKey = presetSemanticKey(preset);
  return semanticKey
    ? t(`reasoningSelector.levels.${semanticKey}`, { defaultValue: fallback })
    : fallback;
}

export type ReasoningIntensityLevel = 0 | 1 | 2 | 3 | 4;

type ReasoningPresetSemanticKey =
  | 'off'
  | 'on'
  | 'minimal'
  | 'low'
  | 'medium'
  | 'high'
  | 'xhigh'
  | 'max';

const REASONING_PRESET_SEMANTIC_KEYS = new Set<ReasoningPresetSemanticKey>([
  'off',
  'on',
  'minimal',
  'low',
  'medium',
  'high',
  'xhigh',
  'max',
]);

function asReasoningPresetSemanticKey(value: string): ReasoningPresetSemanticKey | undefined {
  const normalized = value.trim().toLowerCase();
  if (normalized === 'none') return 'off';
  return REASONING_PRESET_SEMANTIC_KEYS.has(normalized as ReasoningPresetSemanticKey)
    ? normalized as ReasoningPresetSemanticKey
    : undefined;
}

function presetSemanticKey(
  preset: ReasoningPresetDescriptor,
): ReasoningPresetSemanticKey | undefined {
  const idKey = asReasoningPresetSemanticKey(preset.id);
  if (idKey) return idKey;

  // Generated presets can prefix or replace their semantic id (for example
  // effort-off and budget-max). Their action is the stable meaning to localize.
  // Custom presets keep their authored label instead of being renamed by an
  // implementation detail in their request actions.
  if (preset.source === 'model_config') return undefined;

  for (const action of preset.actions) {
    if (action.type === 'toggle') return action.enabled ? 'on' : 'off';
    if (action.type === 'effort') {
      const effortKey = asReasoningPresetSemanticKey(action.value);
      if (effortKey) return effortKey;
    }
    if (action.type === 'budget_tokens') {
      if (preset.id.toLowerCase().includes('max')) return 'max';
      if (preset.id.toLowerCase().includes('high')) return 'high';
    }
  }

  return undefined;
}

function presetDisablesReasoning(preset: ReasoningPresetDescriptor): boolean {
  if (presetSemanticKey(preset) === 'off') return true;
  return preset.actions.some(action => (
    action.type === 'toggle' && !action.enabled
  ));
}

export function reasoningIntensityLevel(
  preset: ReasoningPresetDescriptor | undefined,
  orderedPresets: ReasoningPresetDescriptor[],
): ReasoningIntensityLevel {
  if (!preset) return 0;
  if (presetDisablesReasoning(preset)) return 0;

  const activePresets = orderedPresets.filter(item => !presetDisablesReasoning(item));
  const activeIndex = activePresets.findIndex(item => item.id === preset.id);
  if (activeIndex < 0) return 0;
  if (activePresets.length === 1) return 1;

  // The catalog may merge toggle, effort and token-budget presets. Ranking by
  // the catalog order keeps the visual series monotonic even when ids come from
  // different action families (off, on, low, high, max).
  return Math.min(
    4,
    Math.max(1, Math.round((activeIndex / (activePresets.length - 1)) * 3) + 1),
  ) as 1 | 2 | 3 | 4;
}
