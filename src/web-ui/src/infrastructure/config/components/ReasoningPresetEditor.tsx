import { Button, Combobox, Icon, IconButton, Input, Listbox, ListboxEmpty, ListboxOption, NumberInput, Select, Switch, Textarea, Tooltip, type ComboboxOption, type SelectOption } from '@bitfun/ui';
import React, { useMemo, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { AlertTriangle } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import type {
  ReasoningCatalogProjection,
  ReasoningConfig,
  ReasoningPreset,
  ReasoningPresetAction,
} from '../types';
import type { ModelsDevReasoningCatalog } from '@/infrastructure/api/service-api/AIApi';
import { getAppearanceOverlayHost } from '@/infrastructure/appearance/runtime/AppearanceOverlayHost';
import { useAnchoredPopoverPosition } from '@/shared/utils/useAnchoredPopoverPosition';
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
  requestFormatLabel?: string;
  disabled?: boolean;
  onValidationChange?: (invalid: boolean) => void;
}

type ModelsDevReasoningProvider = ModelsDevReasoningCatalog['providers'][number];
type ModelsDevReasoningModel = ModelsDevReasoningProvider['models'][number];

interface ModelsDevSearchResult {
  provider: ModelsDevReasoningProvider;
  model: ModelsDevReasoningModel;
  rank: number;
}

const MODELS_DEV_SEARCH_SEPARATOR = /[\s/_.-]+/u;

function normalizeModelsDevSearchText(value: string): string {
  return value
    .trim()
    .toLowerCase()
    .split(MODELS_DEV_SEARCH_SEPARATOR)
    .filter(Boolean)
    .join(' ');
}

function tokenizeModelsDevSearch(value: string): string[] {
  const normalized = normalizeModelsDevSearchText(value);
  return normalized ? normalized.split(' ') : [];
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
  requestFormatLabel,
  disabled = false,
  onValidationChange,
}) => {
  const { t } = useTranslation('settings/models');
  const modelsDevSearchListboxId = React.useId();
  const [jsonDrafts, setJsonDrafts] = useState<Record<string, string>>({});
  const [expandedPresetIndex, setExpandedPresetIndex] = useState<number | null>(null);
  const [modelsDevSearch, setModelsDevSearch] = useState('');
  const [modelsDevSearchOpen, setModelsDevSearchOpen] = useState(false);
  const [modelsDevSearchHighlight, setModelsDevSearchHighlight] = useState(0);
  const modelsDevSearchAnchorRef = useRef<HTMLDivElement>(null);
  const modelsDevSearchPopoverRef = useRef<HTMLDivElement>(null);
  const invalidJsonKeysRef = useRef<Set<string>>(new Set());
  const presets = useMemo(() => value.presets ?? [], [value.presets]);
  const catalog = value.catalog ?? { source: 'auto' as const };
  const actionOptions = useMemo<SelectOption[]>(() => [
    { label: t('reasoningPresets.settingEffort'), value: 'effort' },
    { label: t('reasoningPresets.settingToggle'), value: 'toggle' },
    { label: t('reasoningPresets.settingBudget'), value: 'budget_tokens' },
    { label: t('reasoningPresets.settingPatch'), value: 'request_patch' },
  ], [t]);
  const catalogOptions = useMemo<ComboboxOption[]>(() => [
    { description: t('reasoningPresets.catalogAutoTooltip'), label: t('reasoningPresets.catalogAuto'), value: 'auto' },
    { description: t('reasoningPresets.catalogModelsDevTooltip'), label: t('reasoningPresets.catalogModelsDev'), value: 'models_dev' },
    { description: t('reasoningPresets.catalogDisabledTooltip'), label: t('reasoningPresets.catalogDisabled'), value: 'disabled' },
  ], [t]);

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
  const unavailablePresetLabels = useMemo(() => (
    generatedProjection?.unavailable_presets
      ?.map(preset => preset.label || preset.id)
      .filter(Boolean) ?? []
  ), [generatedProjection?.unavailable_presets]);

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

  const modelsDevSearchResults = useMemo(() => {
    const query = normalizeModelsDevSearchText(modelsDevSearch);
    const queryTokens = tokenizeModelsDevSearch(modelsDevSearch);
    if (!query) return { items: [] as ModelsDevSearchResult[], total: 0 };
    const results: ModelsDevSearchResult[] = (modelsDevReasoningCatalog?.providers ?? []).flatMap(provider => (
      provider.models.map(model => {
        const providerId = provider.id.toLowerCase();
        const providerName = provider.name.toLowerCase();
        const modelId = model.id.toLowerCase();
        const modelName = (model.display_name ?? '').toLowerCase();
        const providerFields = [providerId, providerName];
        const modelFields = [modelId, modelName];
        const searchableFields = [...providerFields, ...modelFields];
        if (!queryTokens.every(token => searchableFields.some(field => field.includes(token)))) {
          return null;
        }

        const normalizedModelId = normalizeModelsDevSearchText(modelId);
        const normalizedModelName = normalizeModelsDevSearchText(modelName);
        const modelExact = normalizedModelId === query || normalizedModelName === query;
        const modelPrefix = normalizedModelId.startsWith(query) || normalizedModelName.startsWith(query);
        const providerExactToken = queryTokens.some(token => (
          token === providerId || token === normalizeModelsDevSearchText(providerName)
        ));
        const allTokensMatchModel = queryTokens.every(token => (
          modelFields.some(field => field.includes(token))
        ));
        const rank = modelExact
          ? 0
          : modelPrefix
            ? 1
            : providerExactToken
              ? 2
              : allTokensMatchModel
                ? 3
                : 4;
        return { provider, model, rank };
      })
    )).filter((result): result is ModelsDevSearchResult => result !== null);
    results.sort((left, right) => left.rank - right.rank
        || left.model.id.localeCompare(right.model.id)
        || left.provider.id.localeCompare(right.provider.id));
    return { items: results.slice(0, 50), total: results.length };
  }, [modelsDevReasoningCatalog, modelsDevSearch]);
  const showModelsDevSearchResults = modelsDevSearchOpen && Boolean(modelsDevSearch.trim());
  const modelsDevSearchLayout = useAnchoredPopoverPosition({
    open: showModelsDevSearchResults,
    anchorRef: modelsDevSearchAnchorRef,
    popoverRef: modelsDevSearchPopoverRef,
    preferredPlacement: 'bottom',
    gap: 4,
    matchAnchorWidth: true,
    layoutRevision: `${modelsDevSearchResults.items.length}:${modelsDevSearchResults.total}`,
  });

  const update = (next: ReasoningConfig) => onChange(cloneReasoningConfig(next));
  const defaultIsCustom = presets.some(preset => (
    preset.id === value.default_preset
    && !preset.disabled
    && Boolean(preset.actions?.length)
  ));
  const rebindCatalog = (nextCatalog: ReasoningConfig['catalog']) => {
    if (JSON.stringify(nextCatalog) === JSON.stringify(catalog)) return;
    update({
      ...value,
      default_preset: defaultIsCustom ? value.default_preset : undefined,
      catalog: nextCatalog,
    });
  };
  const selectModelsDevSearchResult = (result: ModelsDevSearchResult) => {
    rebindCatalog({
      source: 'models_dev',
      provider: result.provider.id,
      model: result.model.id,
    });
    setModelsDevSearch('');
    setModelsDevSearchOpen(false);
    setModelsDevSearchHighlight(0);
  };
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
            <Combobox
              className="bitfun-reasoning-preset-editor__primary-control"
              value={catalog.source}
              disabled={disabled}
              size="sm"
              aria-label={t('reasoningPresets.catalogSource')}
              options={catalogOptions}
              onValueChange={(next) => {
                const source = next as 'auto' | 'models_dev' | 'disabled';
                const nextCatalog = source === 'models_dev'
                  ? {
                      source,
                      provider: catalog.source === 'models_dev' ? catalog.provider : '',
                      model: catalog.source === 'models_dev' ? catalog.model : '',
                    }
                  : { source };
                rebindCatalog(nextCatalog);
              }}
            />
          </div>
          <div className="bitfun-reasoning-preset-editor__primary-setting">
            <span className="bitfun-reasoning-preset-editor__primary-setting-label">
              {t('reasoningPresets.defaultBehavior')}
            </span>
            <Select
              className="bitfun-reasoning-preset-editor__primary-control"
              value={value.default_preset ?? ''}
              disabled={disabled}
              size="sm"
              aria-label={t('reasoningPresets.defaultPreset')}
              options={defaultOptions}
              onValueChange={(next) => update({ ...value, default_preset: String(next) || undefined })}
            />
          </div>
        </div>

        {catalog.source === 'models_dev' && (
          <div
            className="bitfun-reasoning-preset-editor__models-dev-binding"
            data-bf-component="reasoning-preset-editor"
            data-bf-part="binding"
          >
            <div className="bitfun-reasoning-preset-editor__models-dev-search">
              <div className="bitfun-reasoning-preset-editor__models-dev-search-field">
                <div className="bitfun-reasoning-preset-editor__models-dev-search-input">
                  <span>{t('reasoningPresets.catalogSearch')}</span>
                  <div
                    ref={modelsDevSearchAnchorRef}
                    className="bitfun-reasoning-preset-editor__models-dev-search-control"
                  >
                    <Input
                      className="bitfun-reasoning-preset-editor__models-dev-search-field-control"
                      value={modelsDevSearch}
                      disabled={disabled}
                      placeholder={t('reasoningPresets.catalogSearchPlaceholder')}
                      aria-label={t('reasoningPresets.catalogSearch')}
                      role="combobox"
                      aria-autocomplete="list"
                      aria-controls={modelsDevSearchListboxId}
                      aria-expanded={modelsDevSearchOpen && Boolean(modelsDevSearch.trim())}
                      aria-activedescendant={modelsDevSearchOpen && modelsDevSearchResults.items.length > 0
                        ? `${modelsDevSearchListboxId}-${modelsDevSearchHighlight}`
                        : undefined}
                      autoComplete="off"
                      onFocus={() => setModelsDevSearchOpen(true)}
                      onChange={(event) => {
                        setModelsDevSearch(event.target.value);
                        setModelsDevSearchOpen(true);
                        setModelsDevSearchHighlight(0);
                      }}
                      onBlur={() => setModelsDevSearchOpen(false)}
                      onKeyDown={(event) => {
                        if (event.key === 'Escape') {
                          setModelsDevSearchOpen(false);
                          return;
                        }
                        if (!modelsDevSearchOpen || modelsDevSearchResults.items.length === 0) return;
                        if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
                          event.preventDefault();
                          const direction = event.key === 'ArrowDown' ? 1 : -1;
                          setModelsDevSearchHighlight(previous => (
                            (previous + direction + modelsDevSearchResults.items.length)
                            % modelsDevSearchResults.items.length
                          ));
                        } else if (event.key === 'Enter') {
                          event.preventDefault();
                          const result = modelsDevSearchResults.items[modelsDevSearchHighlight];
                          if (result) selectModelsDevSearchResult(result);
                        }
                      }}
                      size="sm"
                    />
                    {showModelsDevSearchResults && createPortal(
                      <div
                        ref={modelsDevSearchPopoverRef}
                        className="bitfun-reasoning-preset-editor__models-dev-search-results"
                        data-bf-placement={modelsDevSearchLayout?.placement ?? 'bottom'}
                        style={{
                          top: `${modelsDevSearchLayout?.top ?? 0}px`,
                          left: `${modelsDevSearchLayout?.left ?? 0}px`,
                          width: modelsDevSearchLayout?.width === undefined
                            ? undefined
                            : `${modelsDevSearchLayout.width}px`,
                          visibility: modelsDevSearchLayout ? 'visible' : 'hidden',
                        }}
                      >
                        <Listbox
                          aria-label={t('reasoningPresets.catalogSearchResults')}
                          className="bitfun-reasoning-preset-editor__models-dev-search-list"
                          focusMode="virtual"
                          id={modelsDevSearchListboxId}
                        >
                          {modelsDevSearchResults.items.length > 0
                            ? modelsDevSearchResults.items.map((result, index) => (
                            <ListboxOption
                              active={index === modelsDevSearchHighlight}
                              description={`${result.provider.id} / ${result.model.id}`}
                              id={`${modelsDevSearchListboxId}-${index}`}
                              key={`${result.provider.id}/${result.model.id}`}
                              onMouseDown={(event) => event.preventDefault()}
                              onClick={() => selectModelsDevSearchResult(result)}
                              value={`${result.provider.id}/${result.model.id}`}
                            >
                              {result.model.display_name || result.model.id}
                            </ListboxOption>
                          )) : (
                            <ListboxEmpty>
                              {t('reasoningPresets.catalogSearchEmpty')}
                            </ListboxEmpty>
                          )}
                        </Listbox>
                        {modelsDevSearchResults.total > modelsDevSearchResults.items.length && (
                          <div className="bitfun-reasoning-preset-editor__models-dev-search-limit">
                            {t('reasoningPresets.catalogSearchLimit')}
                          </div>
                        )}
                      </div>,
                      getAppearanceOverlayHost(),
                    )}
                  </div>
                </div>
                <span className="bitfun-reasoning-preset-editor__models-dev-search-hint">
                  {t('reasoningPresets.catalogSearchHint')}
                </span>
              </div>
            </div>
            <div className="bitfun-reasoning-preset-editor__binding-field">
              <span>{t('reasoningPresets.catalogProvider')}</span>
              <Combobox
                className="bitfun-reasoning-preset-editor__binding-control"
                size="sm"
                aria-label={t('reasoningPresets.catalogProvider')}
                value={catalog.provider}
                options={modelsDevProviderOptions}
                disabled={disabled}
                clearable
                onCreateValue={value => value}
                onValueChange={(next) => {
                  const provider = String(next || '');
                  rebindCatalog(provider
                    ? { ...catalog, provider, model: provider === catalog.provider ? catalog.model : '' }
                    : { source: 'auto' });
                }}
              />
            </div>
            <div className="bitfun-reasoning-preset-editor__binding-field">
              <span>{t('reasoningPresets.catalogModel')}</span>
              <Combobox
                className="bitfun-reasoning-preset-editor__binding-control"
                size="sm"
                aria-label={t('reasoningPresets.catalogModel')}
                value={catalog.model}
                options={modelsDevModelOptions}
                disabled={disabled}
                clearable
                onCreateValue={value => value}
                onValueChange={(next) => {
                  const model = String(next || '');
                  rebindCatalog(model
                    ? { ...catalog, model }
                    : { source: 'auto' });
                }}
              />
            </div>
          </div>
        )}

        {catalog.source === 'models_dev' && unavailablePresetLabels.length > 0 && (
          <div
            className="bitfun-reasoning-preset-editor__unavailable-warning"
            data-bf-component="reasoning-preset-editor"
            data-bf-part="unavailableWarning"
            role="status"
          >
            <AlertTriangle size={16} aria-hidden="true" />
            <div>
              <strong>{t('reasoningPresets.unavailableTitle')}</strong>
              <span>
                {t('reasoningPresets.unavailableDescription', {
                  format: requestFormatLabel || t('reasoningPresets.unknownRequestFormat'),
                  presets: unavailablePresetLabels.join(', '),
                })}
              </span>
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
                <Icon name="info" size="sm" aria-hidden="true" />
              </span>
            </Tooltip>
          </div>
          <Button variant="outline" size="sm" disabled={disabled} onClick={addPreset} leadingIcon={<Icon name="plus" size="sm" aria-hidden="true" />}>

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
                      aria-label={preset.label?.trim() || preset.id}
                    >
                      {expanded ? <Icon name="chevron-down" size="sm" /> : <Icon name="chevron-right" size="sm" />}
                    </button>
                    <div className="bitfun-reasoning-preset-editor__row-content">
                      {expanded ? (
                        <div className="bitfun-reasoning-preset-editor__row-name-editor">
                          <Input
                            className="bitfun-reasoning-preset-editor__row-name-input"
                            aria-label={t('reasoningPresets.label')}
                            value={preset.label ?? ''}
                            disabled={disabled}
                            placeholder={t('reasoningPresets.labelPlaceholder')}
                            onChange={(event) => updatePreset(presetIndex, {
                              label: event.target.value || undefined,
                            })}
                            size="sm"
                          />
                        </div>
                      ) : (
                        <button
                          type="button"
                          className="bitfun-reasoning-preset-editor__row-name"
                          onClick={() => setExpandedPresetIndex(presetIndex)}
                        >
                          {preset.label?.trim() || preset.id}
                        </button>
                      )}
                      <span className="bitfun-reasoning-preset-editor__row-preview">
                        {formatPresetSummary(preset)}
                      </span>
                    </div>
                    <div className="bitfun-reasoning-preset-editor__row-badges">
                      {value.default_preset === preset.id && (
                        <span className="bitfun-reasoning-preset-editor__badge">
                          {t('reasoningPresets.default')}
                        </span>
                      )}
                      <Switch
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
                      <Tooltip content={t('reasoningPresets.moveUp')}>
                        <IconButton
                          aria-label={t('reasoningPresets.moveUp')}
                          size="sm"
                          disabled={disabled || presetIndex === 0}
                          onClick={() => movePreset(presetIndex, -1)}
                          icon={<Icon name="arrow-up" size="sm" />}
                        />
                      </Tooltip>
                      <Tooltip content={t('reasoningPresets.moveDown')}>
                        <IconButton
                          aria-label={t('reasoningPresets.moveDown')}
                          size="sm"
                          disabled={disabled || presetIndex === presets.length - 1}
                          onClick={() => movePreset(presetIndex, 1)}
                          icon={<Icon name="arrow-down" size="sm" />}
                        />
                      </Tooltip>
                      <Tooltip content={t('reasoningPresets.remove')}>
                        <IconButton
                          aria-label={t('reasoningPresets.remove')}
                          size="sm"
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
                          icon={<Icon name="delete" size="sm" />}
                        />
                      </Tooltip>
                    </div>
                  </div>

                  {expanded && (
                    <div
                      className="bitfun-reasoning-preset-editor__row-editor"
                      data-bf-component="reasoning-preset-editor"
                      data-bf-part="presetEditor"
                    >
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
                          className="bitfun-reasoning-preset-editor__action-type"
                          size="sm"
                          value={action.type}
                          disabled={disabled}
                          options={actionOptions.filter(option => (
                            availableReasoningActionTypes(preset.actions ?? [], actionIndex)
                              .includes(option.value as ReasoningPresetAction['type'])
                          ))}
                          onValueChange={(next) => {
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
                            <Combobox
                              className="bitfun-reasoning-preset-editor__effort-select"
                              size="sm"
                              value={action.value}
                              disabled={disabled}
                              options={effortOptions}
                              onCreateValue={value => value}
                              aria-label={t('reasoningPresets.settingEffort')}
                              onValueChange={(next) => updateAction(presetIndex, actionIndex, {
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
                          <Switch checked={action.enabled} disabled={disabled} onChange={(event) => updateAction(presetIndex, actionIndex, { type: 'toggle', enabled: event.target.checked })} />
                        )}
                        {action.type === 'budget_tokens' && (
                          <NumberInput className="bitfun-reasoning-preset-editor__action-value" size="sm" value={action.value} min={1} max={2_000_000_000} step={1024} disabled={disabled} disableWheel onValueChange={(next) => updateAction(presetIndex, actionIndex, { type: 'budget_tokens', value: next })} />
                        )}
                        {action.type === 'request_patch' && (
                          <div className="bitfun-reasoning-preset-editor__json">
                            <Textarea
                              value={jsonValue}
                              disabled={disabled}
                              rows={4}
                              invalid={!jsonIsValid}
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
                          <Tooltip content={t('reasoningPresets.moveUp')}>
                            <IconButton
                              aria-label={t('reasoningPresets.moveUp')}
                              size="sm"
                              disabled={disabled || actionIndex === 0}
                              onClick={() => moveAction(presetIndex, actionIndex, -1)}
                              icon={<Icon name="arrow-up" size="sm" />}
                            />
                          </Tooltip>
                          <Tooltip content={t('reasoningPresets.moveDown')}>
                            <IconButton
                              aria-label={t('reasoningPresets.moveDown')}
                              size="sm"
                              disabled={disabled || actionIndex === (preset.actions?.length ?? 0) - 1}
                              onClick={() => moveAction(presetIndex, actionIndex, 1)}
                              icon={<Icon name="arrow-down" size="sm" />}
                            />
                          </Tooltip>
                          <Tooltip content={t('reasoningPresets.remove')}>
                            <IconButton
                              aria-label={t('reasoningPresets.remove')}
                              size="sm"
                              disabled={disabled || (preset.actions?.length ?? 0) <= 1}
                              onClick={() => {
                                resetJsonDraftState();
                                updatePreset(presetIndex, { actions: preset.actions?.filter((_, index) => index !== actionIndex) });
                              }}
                              icon={<Icon name="delete" size="sm" />}
                            />
                          </Tooltip>
                        </div>
                      </div>
                    );
                  })}
                  <Button
                    variant="outline"
                    size="sm"
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
                    leadingIcon={<Icon name="plus" size="sm" aria-hidden="true" />}
                  >

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
