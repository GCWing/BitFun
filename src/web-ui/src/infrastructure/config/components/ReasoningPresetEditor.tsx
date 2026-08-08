import React, { useMemo, useRef, useState } from 'react';
import {
  AlertTriangle,
  ArrowDown,
  ArrowUp,
  ChevronDown,
  ChevronRight,
  Info,
  Plus,
  Trash2,
} from 'lucide-react';
import { useTranslation } from 'react-i18next';
import {
  Button,
  IconButton,
  Input,
  NumberInput,
  Select,
  Switch,
  Textarea,
  Tooltip,
  type SelectOption,
} from '@/component-library';
import type {
  ReasoningCatalogProjection,
  ReasoningConfig,
  ReasoningPreset,
  ReasoningPresetAction,
} from '../types';
import type { ModelsDevReasoningCatalog } from '@/infrastructure/api/service-api/AIApi';
import {
  availableReasoningActionTypes,
  cloneReasoningConfig,
  nextReasoningActionType,
  resolveDefaultReasoningEffortValue,
  resolveReasoningEffortValues,
} from '../utils/reasoningPresets';
import './ReasoningPresetEditor.scss';

interface ReasoningPresetEditorProps {
  value: ReasoningConfig;
  onChange: (value: ReasoningConfig) => void;
  generatedProjection?: ReasoningCatalogProjection | null;
  modelsDevReasoningCatalog?: ModelsDevReasoningCatalog | null;
  disabled?: boolean;
  onValidationChange?: (invalid: boolean) => void;
}

function defaultAction(
  type: ReasoningPresetAction['type'],
  defaultEffortValue: string,
): ReasoningPresetAction {
  switch (type) {
    case 'toggle': return { type, enabled: true };
    case 'budget_tokens': return { type, value: 8192 };
    case 'request_patch': return { type, body: {} };
    case 'effort': return { type, value: defaultEffortValue };
  }
}

function parseJsonObject(value: string): Record<string, unknown> | null {
  try {
    const parsed: unknown = JSON.parse(value);
    return parsed && typeof parsed === 'object' && !Array.isArray(parsed)
      ? parsed as Record<string, unknown>
      : null;
  } catch {
    return null;
  }
}

export const ReasoningPresetEditor: React.FC<ReasoningPresetEditorProps> = ({
  value,
  onChange,
  generatedProjection,
  modelsDevReasoningCatalog,
  disabled = false,
  onValidationChange,
}) => {
  const { t } = useTranslation('settings/ai-model');
  const [jsonDrafts, setJsonDrafts] = useState<Record<string, string>>({});
  const [expandedPresetIndex, setExpandedPresetIndex] = useState<number | null>(null);
  const invalidJsonKeysRef = useRef<Set<string>>(new Set());
  const presets = useMemo(() => value.presets ?? [], [value.presets]);
  const catalog = value.catalog ?? { source: 'auto' as const };
  const actionOptions = useMemo<SelectOption[]>(() => [
    { label: t('reasoningPresets.settingEffort'), value: 'effort' },
    { label: t('reasoningPresets.settingToggle'), value: 'toggle' },
    { label: t('reasoningPresets.settingBudget'), value: 'budget_tokens' },
    { label: t('reasoningPresets.settingPatch'), value: 'request_patch' },
  ], [t]);
  const catalogOptions = useMemo<SelectOption[]>(() => [
    { label: t('reasoningPresets.catalogAuto'), value: 'auto' },
    { label: t('reasoningPresets.catalogModelsDev'), value: 'models_dev' },
    { label: t('reasoningPresets.catalogDisabled'), value: 'disabled' },
  ], [t]);
  const catalogOptionTooltips = useMemo<Record<string, string>>(() => ({
    auto: t('reasoningPresets.catalogAutoTooltip'),
    models_dev: t('reasoningPresets.catalogModelsDevTooltip'),
    disabled: t('reasoningPresets.catalogDisabledTooltip'),
  }), [t]);

  const defaultOptions = useMemo<SelectOption[]>(() => [
    { label: t('reasoningPresets.auto'), value: '' },
    ...Array.from(new Map([
      ...(generatedProjection?.presets ?? [])
        .filter(preset => preset.source !== 'model_config')
        .map(preset => [preset.id, {
          label: `${preset.label || preset.id} (${preset.id})`,
          value: preset.id,
        }] as const),
      ...presets
        .filter(preset => !preset.disabled && Boolean(preset.actions?.length))
        .map(preset => [preset.id, {
          label: `${preset.label?.trim() || preset.id} (${preset.id})`,
          value: preset.id,
        }] as const),
    ]).values()),
  ], [generatedProjection?.presets, presets, t]);
  const effortValues = useMemo(
    () => resolveReasoningEffortValues(generatedProjection),
    [generatedProjection],
  );
  const effortOptions = useMemo<SelectOption[]>(() => effortValues.map(effort => ({
    label: effort,
    value: effort,
  })), [effortValues]);
  const defaultEffortValue = useMemo(
    () => resolveDefaultReasoningEffortValue(generatedProjection),
    [generatedProjection],
  );

  const modelsDevProviderOptions = useMemo<SelectOption[]>(() => {
    const options = (modelsDevReasoningCatalog?.providers ?? []).map(provider => ({
      label: provider.name || provider.id,
      value: provider.id,
    }));
    return [
      { label: t('reasoningPresets.catalogUnbound'), value: '' },
      ...options,
    ];
  }, [modelsDevReasoningCatalog, t]);

  const modelsDevProviderId = catalog.source === 'models_dev' ? catalog.provider : '';

  const modelsDevModelOptions = useMemo<SelectOption[]>(() => {
    if (!modelsDevProviderId) return [];
    const provider = modelsDevReasoningCatalog?.providers
      .find(candidate => candidate.id === modelsDevProviderId);
    return (provider?.models ?? []).map(model => ({
      label: model.display_name || model.id,
      value: model.id,
    }));
  }, [modelsDevReasoningCatalog, modelsDevProviderId]);

  const update = (next: ReasoningConfig) => onChange(cloneReasoningConfig(next));
  const updatePreset = (index: number, changes: Partial<ReasoningPreset>) => {
    const next = [...presets];
    next[index] = { ...next[index], ...changes };
    update({ ...value, presets: next });
  };
  const updateAction = (presetIndex: number, actionIndex: number, action: ReasoningPresetAction) => {
    const actions = [...(presets[presetIndex]?.actions ?? [])];
    actions[actionIndex] = action;
    updatePreset(presetIndex, { actions });
  };
  const setJsonValidation = (key: string, invalid: boolean) => {
    if (invalid) invalidJsonKeysRef.current.add(key);
    else invalidJsonKeysRef.current.delete(key);
    onValidationChange?.(invalidJsonKeysRef.current.size > 0);
  };
  const resetJsonDraftState = () => {
    setJsonDrafts({});
    invalidJsonKeysRef.current.clear();
    onValidationChange?.(false);
  };

  const formatPresetSummary = (preset: ReasoningPreset) => {
    const actions = preset.actions ?? [];
    const patchCount = actions.filter(action => action.type === 'request_patch').length;
    const summaries = actions.flatMap(action => {
      switch (action.type) {
        case 'effort':
          return [t('reasoningPresets.actionSummaryEffort', { value: action.value })];
        case 'toggle':
          return [t(action.enabled
            ? 'reasoningPresets.actionSummaryEnabled'
            : 'reasoningPresets.actionSummaryDisabled')];
        case 'budget_tokens':
          return [t('reasoningPresets.actionSummaryBudget', { value: action.value })];
        case 'request_patch':
          return [];
      }
    });
    if (patchCount > 0) {
      summaries.push(t('reasoningPresets.actionSummaryPatches', { count: patchCount }));
    }
    return summaries.length > 0 ? summaries.join(' · ') : t('reasoningPresets.noActions');
  };

  const addPreset = () => {
    const id = `custom-${Date.now().toString(36)}`;
    updatePreset(presets.length, {
      id,
      label: id,
      order: presets.length * 10,
      actions: [defaultAction('effort', defaultEffortValue)],
    });
    setExpandedPresetIndex(presets.length);
  };

  const movePreset = (index: number, direction: -1 | 1) => {
    const target = index + direction;
    if (target < 0 || target >= presets.length) return;
    const next = [...presets];
    [next[index], next[target]] = [next[target], next[index]];
    resetJsonDraftState();
    setExpandedPresetIndex(previous => {
      if (previous === index) return target;
      if (previous === target) return index;
      return previous;
    });
    update({ ...value, presets: next.map((preset, order) => ({ ...preset, order: order * 10 })) });
  };

  const moveAction = (presetIndex: number, actionIndex: number, direction: -1 | 1) => {
    const actions = [...(presets[presetIndex]?.actions ?? [])];
    const target = actionIndex + direction;
    if (target < 0 || target >= actions.length) return;
    [actions[actionIndex], actions[target]] = [actions[target], actions[actionIndex]];
    resetJsonDraftState();
    updatePreset(presetIndex, { actions });
  };

  return (
    <div
      className="bitfun-reasoning-preset-editor"
      data-bf-component="reasoning-preset-editor"
      data-bf-part="root"
      data-testid="settings-reasoning-preset-editor"
    >
      <section
        className="bitfun-reasoning-preset-editor__section"
        data-bf-component="reasoning-preset-editor"
        data-bf-part="section"
      >
        <div
          className="bitfun-reasoning-preset-editor__primary-settings"
          data-bf-component="reasoning-preset-editor"
          data-bf-part="primarySettings"
        >
          <div className="bitfun-reasoning-preset-editor__primary-setting">
            <span className="bitfun-reasoning-preset-editor__primary-setting-label">
              {t('reasoningPresets.catalogSource')}
            </span>
            <Select
              value={catalog.source}
              disabled={disabled}
              size="small"
              triggerAriaLabel={t('reasoningPresets.catalogSource')}
              options={catalogOptions}
              renderOption={(option) => (
                <Tooltip content={catalogOptionTooltips[String(option.value)]} placement="right">
                  <div className="bitfun-reasoning-preset-editor__catalog-option">
                    <span>{option.label}</span>
                    <Info
                      className="bitfun-reasoning-preset-editor__catalog-option-info"
                      size={13}
                      aria-hidden="true"
                    />
                  </div>
                </Tooltip>
              )}
              onChange={(next) => {
                const source = next as 'auto' | 'models_dev' | 'disabled';
                const defaultIsCustom = presets.some(preset => (
                  preset.id === value.default_preset
                  && !preset.disabled
                  && Boolean(preset.actions?.length)
                ));
                update({
                  ...value,
                  default_preset: defaultIsCustom ? value.default_preset : undefined,
                  catalog: source === 'models_dev'
                    ? {
                        source,
                        provider: catalog.source === 'models_dev' ? catalog.provider : '',
                        model: catalog.source === 'models_dev' ? catalog.model : '',
                      }
                    : { source },
                });
              }}
            />
          </div>
          <div className="bitfun-reasoning-preset-editor__primary-setting">
            <span className="bitfun-reasoning-preset-editor__primary-setting-label">
              {t('reasoningPresets.defaultBehavior')}
            </span>
            <Select
              value={value.default_preset ?? ''}
              disabled={disabled}
              size="small"
              triggerAriaLabel={t('reasoningPresets.defaultPreset')}
              options={defaultOptions}
              onChange={(next) => update({ ...value, default_preset: String(next) || undefined })}
            />
          </div>
        </div>

        {catalog.source === 'models_dev' && (
          <div
            className="bitfun-reasoning-preset-editor__models-dev-binding"
            data-bf-component="reasoning-preset-editor"
            data-bf-part="binding"
          >
            <div className="bitfun-reasoning-preset-editor__binding-field">
              <span>{t('reasoningPresets.catalogProvider')}</span>
              <Select
                size="small"
                aria-label={t('reasoningPresets.catalogProvider')}
                value={catalog.provider}
                options={modelsDevProviderOptions}
                disabled={disabled}
                searchable
                clearable
                allowCustomValue
                customValueHint={t('reasoningPresets.catalogProviderCustomValueHint')}
                searchPlaceholder={t('reasoningPresets.catalogProvider')}
                onChange={(next) => update({
                  ...value,
                  catalog: {
                    ...catalog,
                    provider: String(next || ''),
                    model: '',
                  },
                })}
              />
            </div>
            <div className="bitfun-reasoning-preset-editor__binding-field">
              <span>{t('reasoningPresets.catalogModel')}</span>
              <Select
                size="small"
                aria-label={t('reasoningPresets.catalogModel')}
                value={catalog.model}
                options={modelsDevModelOptions}
                disabled={disabled}
                searchable
                clearable
                allowCustomValue
                customValueHint={t('reasoningPresets.catalogModelCustomValueHint')}
                searchPlaceholder={t('reasoningPresets.catalogModel')}
                onChange={(next) => update({
                  ...value,
                  catalog: { ...catalog, model: String(next || '') },
                })}
              />
            </div>
          </div>
        )}

        {generatedProjection?.status === 'known'
          && (generatedProjection.presets?.some(preset => preset.source !== 'model_config') ?? false)
          && (
          <div
            className="bitfun-reasoning-preset-editor__generated"
            data-bf-component="reasoning-preset-editor"
            data-bf-part="generated"
          >
            <div className="bitfun-reasoning-preset-editor__generated-title">
              {t('reasoningPresets.generatedTitle')}
            </div>
            <div className="bitfun-reasoning-preset-editor__generated-list">
              {generatedProjection.presets?.filter(preset => preset.source !== 'model_config').map(preset => (
                <span key={preset.id} className="bitfun-reasoning-preset-editor__generated-item">
                  {preset.label || preset.id}
                </span>
              ))}
            </div>
          </div>
          )}
      </section>

      <section
        className="bitfun-reasoning-preset-editor__section"
        data-bf-component="reasoning-preset-editor"
        data-bf-part="section"
      >
        <div
          className="bitfun-reasoning-preset-editor__header"
          data-bf-component="reasoning-preset-editor"
          data-bf-part="header"
        >
          <div className="bitfun-reasoning-preset-editor__section-title-group">
            <div className="bitfun-reasoning-preset-editor__section-title">
              {t('reasoningPresets.customTitle')}
            </div>
            <Tooltip content={t('reasoningPresets.customTooltip')} placement="top">
              <span
                className="bitfun-reasoning-preset-editor__section-title-info"
                role="button"
                tabIndex={0}
                aria-label={t('reasoningPresets.customTooltip')}
              >
                <Info size={14} aria-hidden="true" />
              </span>
            </Tooltip>
          </div>
          <Button variant="secondary" size="small" disabled={disabled} onClick={addPreset}>
            <Plus size={14} aria-hidden="true" />
            {t('reasoningPresets.add')}
          </Button>
        </div>

        {presets.length === 0 ? (
          <div
            className="bitfun-reasoning-preset-editor__empty"
            data-bf-component="reasoning-preset-editor"
            data-bf-part="empty"
          >
            {t('reasoningPresets.empty')}
          </div>
        ) : (
          <div
            className="bitfun-reasoning-preset-editor__list"
            data-bf-component="reasoning-preset-editor"
            data-bf-part="list"
          >
            {presets.map((preset, presetIndex) => {
              const expanded = expandedPresetIndex === presetIndex;
              return (
                <div
                  key={`${preset.id}-${presetIndex}`}
                  className="bitfun-reasoning-preset-editor__row"
                  data-bf-component="reasoning-preset-editor"
                  data-bf-part="preset"
                  data-bf-state={expanded ? 'expanded' : undefined}
                  data-expanded={expanded ? 'true' : 'false'}
                >
                  <div
                    className="bitfun-reasoning-preset-editor__row-summary"
                    data-bf-component="reasoning-preset-editor"
                    data-bf-part="presetSummary"
                  >
                    <button
                      type="button"
                      className="bitfun-reasoning-preset-editor__row-toggle"
                      onClick={() => setExpandedPresetIndex(expanded ? null : presetIndex)}
                      aria-expanded={expanded}
                    >
                      {expanded ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
                      <span className="bitfun-reasoning-preset-editor__row-summary-text">
                        <strong>{preset.label?.trim() || preset.id}</strong>
                        <span>{formatPresetSummary(preset)}</span>
                      </span>
                    </button>
                    <div className="bitfun-reasoning-preset-editor__row-badges">
                      {value.default_preset === preset.id && (
                        <span className="bitfun-reasoning-preset-editor__badge">
                          {t('reasoningPresets.default')}
                        </span>
                      )}
                      <Switch
                        size="small"
                        checked={!preset.disabled}
                        disabled={disabled}
                        aria-label={t('reasoningPresets.enabled')}
                        onChange={(event) => updatePreset(presetIndex, {
                          disabled: !event.target.checked,
                          actions: event.target.checked && !preset.actions?.length
                            ? [defaultAction('effort', defaultEffortValue)]
                            : preset.actions,
                        })}
                      />
                      <IconButton size="small" variant="ghost" tooltip={t('reasoningPresets.moveUp')} disabled={disabled || presetIndex === 0} onClick={() => movePreset(presetIndex, -1)}><ArrowUp size={14} /></IconButton>
                      <IconButton size="small" variant="ghost" tooltip={t('reasoningPresets.moveDown')} disabled={disabled || presetIndex === presets.length - 1} onClick={() => movePreset(presetIndex, 1)}><ArrowDown size={14} /></IconButton>
                      <IconButton
                        size="small"
                        variant="ghost"
                        tooltip={t('reasoningPresets.remove')}
                        disabled={disabled}
                        onClick={() => {
                          resetJsonDraftState();
                          setExpandedPresetIndex(previous => {
                            if (previous === null) return null;
                            if (previous === presetIndex) return null;
                            return previous > presetIndex ? previous - 1 : previous;
                          });
                          update({
                            ...value,
                            presets: presets.filter((_, index) => index !== presetIndex),
                            default_preset: value.default_preset === preset.id ? undefined : value.default_preset,
                          });
                        }}
                      >
                        <Trash2 size={14} />
                      </IconButton>
                    </div>
                  </div>

                  {expanded && (
                    <div
                      className="bitfun-reasoning-preset-editor__row-editor"
                      data-bf-component="reasoning-preset-editor"
                      data-bf-part="presetEditor"
                    >
                      <div className="bitfun-reasoning-preset-editor__row-head">
                        <div className="bitfun-reasoning-preset-editor__row-head-field">
                          <span>{t('reasoningPresets.id')}</span>
                          <Input
                            size="small"
                            aria-label={t('reasoningPresets.id')}
                            value={preset.id}
                            disabled={disabled}
                            onChange={(event) => {
                              const nextId = event.target.value;
                              updatePreset(presetIndex, { id: nextId });
                              if (value.default_preset === preset.id) {
                                update({
                                  ...value,
                                  default_preset: nextId,
                                  presets: presets.map((item, index) => index === presetIndex ? { ...item, id: nextId } : item),
                                });
                              }
                            }}
                          />
                        </div>
                        <div className="bitfun-reasoning-preset-editor__row-head-field">
                          <span>{t('reasoningPresets.label')}</span>
                          <Input
                            size="small"
                            aria-label={t('reasoningPresets.label')}
                            value={preset.label ?? ''}
                            disabled={disabled}
                            placeholder={t('reasoningPresets.labelPlaceholder')}
                            onChange={(event) => updatePreset(presetIndex, { label: event.target.value || undefined })}
                          />
                        </div>
                      </div>

                      <div
                        className="bitfun-reasoning-preset-editor__actions"
                        data-bf-component="reasoning-preset-editor"
                        data-bf-part="actions"
                      >
                  {(preset.actions ?? []).map((action, actionIndex) => {
                    const jsonKey = `${presetIndex}:${actionIndex}`;
                    const jsonValue = jsonDrafts[jsonKey]
                      ?? (action.type === 'request_patch' ? JSON.stringify(action.body, null, 2) : '{}');
                    const jsonIsValid = action.type !== 'request_patch' || parseJsonObject(jsonValue) !== null;
                    return (
                      <div
                        key={jsonKey}
                        className="bitfun-reasoning-preset-editor__action"
                        data-bf-component="reasoning-preset-editor"
                        data-bf-part="action"
                      >
                        <Select
                          size="small"
                          value={action.type}
                          disabled={disabled}
                          options={actionOptions.filter(option => (
                            availableReasoningActionTypes(preset.actions ?? [], actionIndex)
                              .includes(option.value as ReasoningPresetAction['type'])
                          ))}
                          onChange={(next) => {
                            setJsonValidation(jsonKey, false);
                            setJsonDrafts(previous => {
                              const nextDrafts = { ...previous };
                              delete nextDrafts[jsonKey];
                              return nextDrafts;
                            });
                            updateAction(
                              presetIndex,
                              actionIndex,
                              defaultAction(next as ReasoningPresetAction['type'], defaultEffortValue),
                            );
                          }}
                        />
                        {action.type === 'effort' && (
                          <div className="bitfun-reasoning-preset-editor__effort-control">
                            <Select
                              size="small"
                              value={action.value}
                              disabled={disabled}
                              options={effortOptions}
                              searchable
                              allowCustomValue
                              customValueHint={t('reasoningPresets.effortCustomValueHint')}
                              searchPlaceholder={t('reasoningPresets.effortSearchPlaceholder')}
                              triggerAriaLabel={t('reasoningPresets.settingEffort')}
                              onChange={(next) => updateAction(presetIndex, actionIndex, {
                                type: 'effort',
                                value: String(next),
                              })}
                            />
                            {!effortValues.includes(action.value.trim()) && (
                              <Tooltip content={t('reasoningPresets.effortCustomWarning')} placement="top">
                                <span
                                  className="bitfun-reasoning-preset-editor__effort-warning"
                                  role="button"
                                  tabIndex={0}
                                  aria-label={t('reasoningPresets.effortCustomWarning')}
                                >
                                  <AlertTriangle size={13} aria-hidden="true" />
                                </span>
                              </Tooltip>
                            )}
                          </div>
                        )}
                        {action.type === 'toggle' && (
                          <Switch size="small" checked={action.enabled} disabled={disabled} onChange={(event) => updateAction(presetIndex, actionIndex, { type: 'toggle', enabled: event.target.checked })} />
                        )}
                        {action.type === 'budget_tokens' && (
                          <NumberInput size="small" value={action.value} min={1} max={2_000_000_000} step={1024} disabled={disabled} disableWheel onChange={(next) => updateAction(presetIndex, actionIndex, { type: 'budget_tokens', value: next })} />
                        )}
                        {action.type === 'request_patch' && (
                          <div className="bitfun-reasoning-preset-editor__json">
                            <Textarea
                              value={jsonValue}
                              disabled={disabled}
                              rows={4}
                              error={!jsonIsValid}
                              errorMessage={!jsonIsValid ? t('reasoningPresets.invalidJson') : undefined}
                              onChange={(event) => {
                                const nextText = event.target.value;
                                setJsonDrafts(previous => ({ ...previous, [jsonKey]: nextText }));
                                const body = parseJsonObject(nextText);
                                setJsonValidation(jsonKey, !body);
                                if (body) updateAction(presetIndex, actionIndex, { type: 'request_patch', body });
                              }}
                            />
                          </div>
                        )}
                        <div
                          className="bitfun-reasoning-preset-editor__action-controls"
                          data-bf-component="reasoning-preset-editor"
                          data-bf-part="actionControls"
                        >
                          <IconButton size="small" variant="ghost" tooltip={t('reasoningPresets.moveUp')} disabled={disabled || actionIndex === 0} onClick={() => moveAction(presetIndex, actionIndex, -1)}><ArrowUp size={14} /></IconButton>
                          <IconButton size="small" variant="ghost" tooltip={t('reasoningPresets.moveDown')} disabled={disabled || actionIndex === (preset.actions?.length ?? 0) - 1} onClick={() => moveAction(presetIndex, actionIndex, 1)}><ArrowDown size={14} /></IconButton>
                          <IconButton
                            size="small"
                            variant="ghost"
                            tooltip={t('reasoningPresets.remove')}
                            disabled={disabled || (preset.actions?.length ?? 0) <= 1}
                            onClick={() => {
                              resetJsonDraftState();
                              updatePreset(presetIndex, { actions: preset.actions?.filter((_, index) => index !== actionIndex) });
                            }}
                          >
                            <Trash2 size={14} />
                          </IconButton>
                        </div>
                      </div>
                    );
                  })}
                  <Button
                    variant="secondary"
                    size="small"
                    disabled={disabled}
                    onClick={() => updatePreset(presetIndex, {
                      actions: [
                        ...(preset.actions ?? []),
                        defaultAction(
                          nextReasoningActionType(preset.actions ?? []),
                          defaultEffortValue,
                        ),
                      ],
                    })}
                  >
                    <Plus size={14} aria-hidden="true" />
                    {t('reasoningPresets.addAction')}
                  </Button>
                      </div>
                    </div>
                  )}
                </div>
              );
            })}
          </div>
        )}
      </section>
    </div>
  );
};

export default ReasoningPresetEditor;
