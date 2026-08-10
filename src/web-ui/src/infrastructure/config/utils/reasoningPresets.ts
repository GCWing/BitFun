import type {
  AIModelConfig,
  ReasoningConfig,
  ReasoningCatalogProjection,
  ReasoningPreset,
  ReasoningPresetAction,
} from '../types';

export const COMMON_REASONING_EFFORT_VALUES = [
  'none',
  'minimal',
  'low',
  'medium',
  'high',
  'xhigh',
  'max',
] as const;

export function resolveReasoningEffortValues(
  projection?: ReasoningCatalogProjection | null,
): string[] {
  const catalogValues = projection?.presets
    ?.filter(preset => preset.source !== 'model_config')
    .flatMap(preset => preset.actions ?? [])
    .filter((action): action is Extract<ReasoningPresetAction, { type: 'effort' }> => action.type === 'effort')
    .map(action => action.value.trim())
    .filter(Boolean) ?? [];
  const values = Array.from(new Set(catalogValues));
  return values.length > 0 ? values : [...COMMON_REASONING_EFFORT_VALUES];
}

export function resolveDefaultReasoningEffortValue(
  projection?: ReasoningCatalogProjection | null,
): string {
  const effortValues = resolveReasoningEffortValues(projection);
  const projectedDefault = projection?.presets
    ?.find(preset => (
      preset.source !== 'model_config'
      && preset.id === projection.default_preset
    ))
    ?.actions.find(
      (action): action is Extract<ReasoningPresetAction, { type: 'effort' }> => action.type === 'effort',
    )
    ?.value.trim();

  if (projectedDefault && effortValues.includes(projectedDefault)) return projectedDefault;
  return effortValues.includes('medium') ? 'medium' : effortValues[0];
}

function nonEmpty(value: unknown): value is string {
  return typeof value === 'string' && value.trim().length > 0;
}

export function canonicalReasoningConfig(config?: Partial<AIModelConfig> | null): ReasoningConfig {
  return normalizeReasoningConfig(config?.reasoning);
}

export function normalizeReasoningConfig(config?: ReasoningConfig | null): ReasoningConfig {
  return {
    catalog: config?.catalog ?? { source: 'auto' },
    default_preset: nonEmpty(config?.default_preset) ? config.default_preset.trim() : undefined,
    presets: Array.isArray(config?.presets) ? config.presets.map(clonePreset) : [],
  };
}

export function clonePreset(preset: ReasoningPreset): ReasoningPreset {
  return {
    ...preset,
    actions: Array.isArray(preset.actions)
      ? JSON.parse(JSON.stringify(preset.actions)) as ReasoningPresetAction[]
      : [],
  };
}

export function cloneReasoningConfig(config: ReasoningConfig): ReasoningConfig {
  return normalizeReasoningConfig(config);
}

function isJsonObject(value: unknown): value is Record<string, unknown> {
  return Boolean(value) && typeof value === 'object' && !Array.isArray(value);
}

function validAction(action: ReasoningPresetAction): boolean {
  switch (action.type) {
    case 'effort':
      return nonEmpty(action.value);
    case 'toggle':
      return typeof action.enabled === 'boolean';
    case 'budget_tokens':
      return Number.isSafeInteger(action.value) && action.value > 0;
    case 'request_patch':
      return isJsonObject(action.body);
  }
}

export const REASONING_ACTION_TYPES: ReasoningPresetAction['type'][] = [
  'effort',
  'toggle',
  'budget_tokens',
  'request_patch',
];

const SINGLETON_REASONING_ACTION_TYPES = new Set<ReasoningPresetAction['type']>([
  'effort',
  'toggle',
  'budget_tokens',
]);

function isSingletonReasoningActionType(
  type: ReasoningPresetAction['type'],
): boolean {
  return SINGLETON_REASONING_ACTION_TYPES.has(type);
}

export function availableReasoningActionTypes(
  actions: ReasoningPresetAction[],
  currentIndex?: number,
): ReasoningPresetAction['type'][] {
  const currentType = currentIndex === undefined ? undefined : actions[currentIndex]?.type;
  const usedByOtherActions = new Set(
    actions
      .filter((_, index) => index !== currentIndex)
      .map(action => action.type)
      .filter(isSingletonReasoningActionType),
  );

  return REASONING_ACTION_TYPES.filter(type => (
    !isSingletonReasoningActionType(type)
    || type === currentType
    || !usedByOtherActions.has(type)
  ));
}

export function nextReasoningActionType(
  actions: ReasoningPresetAction[],
): ReasoningPresetAction['type'] {
  return availableReasoningActionTypes(actions)[0] ?? 'request_patch';
}

function hasDuplicateSingletonActions(actions: ReasoningPresetAction[]): boolean {
  const seen = new Set<ReasoningPresetAction['type']>();
  for (const action of actions) {
    if (isSingletonReasoningActionType(action.type) && seen.has(action.type)) return true;
    seen.add(action.type);
  }
  return false;
}

export type ReasoningConfigValidationError =
  | 'catalog_binding'
  | 'preset_id'
  | 'duplicate_preset_id'
  | 'preset_setting'
  | 'default_preset';

export function validateReasoningConfig(
  config?: ReasoningConfig | null,
  additionalPresetIds: Iterable<string> = [],
): ReasoningConfigValidationError | null {
  const normalized = normalizeReasoningConfig(config);
  if (
    normalized.catalog?.source === 'models_dev'
    && (!nonEmpty(normalized.catalog.provider) || !nonEmpty(normalized.catalog.model))
  ) {
    return 'catalog_binding';
  }

  const ids = new Set<string>();
  const selectableIds = new Set(
    Array.from(additionalPresetIds, presetId => presetId.trim()).filter(Boolean),
  );
  for (const preset of normalized.presets ?? []) {
    const id = preset.id.trim();
    if (!id) return 'preset_id';
    if (ids.has(id)) return 'duplicate_preset_id';
    ids.add(id);
    if (
      !preset.disabled
      && (
        !preset.actions?.length
        || !preset.actions.every(validAction)
        || hasDuplicateSingletonActions(preset.actions)
      )
    ) {
      return 'preset_setting';
    }
    if (!preset.disabled && preset.actions?.length) selectableIds.add(id);
  }

  if (normalized.default_preset && !selectableIds.has(normalized.default_preset)) {
    return 'default_preset';
  }

  return null;
}
