import React, { useEffect, useMemo, useRef, useState } from 'react';
import { MobileButton, MobileCard } from '@openbitfun/ui/mobile';
import { useI18n } from '../i18n';
import type { RemoteModelCatalog, RemoteModelConfig } from '../services/RemoteSessionManager';

const MOBILE_LAST_SELECTED_MODEL_ID_KEY = 'openbitfun.mobile.last_selected_model_id';

const SparklesIcon: React.FC<{ className?: string; size?: number }> = ({ className, size = 10 }) => (
  <svg
    className={className}
    width={size}
    height={size}
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    strokeWidth="2"
    strokeLinecap="round"
    strokeLinejoin="round"
    aria-hidden="true"
  >
    <path d="M9.937 15.5A2 2 0 0 0 8.5 14.063l-6.135-1.582a.5.5 0 0 1 0-.962L8.5 9.937A2 2 0 0 0 9.937 8.5l1.582-6.135a.5.5 0 0 1 .962 0L14.063 8.5A2 2 0 0 0 15.5 9.937l6.135 1.582a.5.5 0 0 1 0 .962L15.5 14.063a2 2 0 0 0-1.437 1.437l-1.582 6.135a.5.5 0 0 1-.962 0z" />
    <path d="M20 3v4" />
    <path d="M22 5h-4" />
    <path d="M4 17v2" />
    <path d="M5 18H3" />
  </svg>
);

type ModelSelectionValue = 'auto' | 'primary' | 'fast' | string;

function formatProviderName(provider: string): string {
  const normalized = provider.trim();
  if (!normalized) return 'Unknown';
  return normalized.charAt(0).toUpperCase() + normalized.slice(1);
}

function getModelProviderLabel(model: RemoteModelConfig): string {
  const configuredName = model.name?.trim();
  if (configuredName) return configuredName;
  return formatProviderName(model.provider);
}

function formatContextWindow(contextWindow?: number): string | null {
  if (!contextWindow) return null;
  return `${Math.round(contextWindow / 1000)}k`;
}

function isChatCapableModel(model: RemoteModelConfig): boolean {
  return model.enabled && Array.isArray(model.capabilities) && model.capabilities.includes('text_chat');
}

export function normalizeSelectedModelId(
  selectedModelId: string | null | undefined,
  catalog: RemoteModelCatalog | null,
): string {
  const value = selectedModelId?.trim();
  if (!value || value === 'auto' || value === 'default') return 'auto';
  if (value === 'primary' || value === 'fast') {
    const defaultId = value === 'primary'
      ? catalog?.default_models?.primary
      : catalog?.default_models?.fast;
    return defaultId && resolveModelSelection(defaultId, catalog) ? value : 'auto';
  }
  return resolveModelSelection(value, catalog) ? value : 'auto';
}

export function loadLastSelectedModelId(): string | null {
  if (typeof window === 'undefined') return null;
  const value = window.localStorage.getItem(MOBILE_LAST_SELECTED_MODEL_ID_KEY)?.trim();
  return value || null;
}

export function persistLastSelectedModelId(modelId: string): void {
  if (typeof window === 'undefined') return;
  const value = modelId.trim();
  if (!value) {
    window.localStorage.removeItem(MOBILE_LAST_SELECTED_MODEL_ID_KEY);
    return;
  }
  window.localStorage.setItem(MOBILE_LAST_SELECTED_MODEL_ID_KEY, value);
}

export function resolvePreferredModelSelection(
  preferredModelId: string | null,
  catalog: RemoteModelCatalog | null,
): { modelId: string | null; fellBackToAuto: boolean } {
  const value = preferredModelId?.trim();
  if (!value) {
    return { modelId: null, fellBackToAuto: false };
  }

  const normalizedModelId = normalizeSelectedModelId(value, catalog);
  const fellBackToAuto = normalizedModelId === 'auto' && value !== 'auto' && value !== 'default';
  return {
    modelId: normalizedModelId,
    fellBackToAuto,
  };
}

function resolveModelSelection(
  modelId: string,
  catalog: RemoteModelCatalog | null,
): RemoteModelConfig | null {
  if (!catalog) return null;
  return catalog.models.find(model => model.id === modelId) || null;
}

function resolveConcreteModelSelection(
  modelId: string,
  catalog: RemoteModelCatalog | null,
): RemoteModelConfig | null {
  const normalizedModelId = normalizeSelectedModelId(modelId, catalog);
  if (normalizedModelId === 'auto' || normalizedModelId === 'primary') {
    return resolveModelSelection(catalog?.default_models?.primary || '', catalog);
  }
  if (normalizedModelId === 'fast') {
    return resolveModelSelection(catalog?.default_models?.fast || '', catalog)
      || resolveModelSelection(catalog?.default_models?.primary || '', catalog);
  }
  return resolveModelSelection(normalizedModelId, catalog);
}

function buildModelProviderMeta(model: RemoteModelConfig | null): string | null {
  if (!model) return null;
  const parts = [getModelProviderLabel(model)];
  const context = formatContextWindow(model.context_window);
  if (context) parts.push(context);
  return parts.join(' · ');
}

function getModelDisplayName(model: RemoteModelConfig | null): string {
  if (!model) return '';
  return model.model_name || model.name || '';
}

function getSelectedModelInfo(
  selectedModelId: string,
  catalog: RemoteModelCatalog | null,
  t: (key: string, params?: Record<string, string | number>) => string,
): {
  label: string;
  meta: string | null;
  enableThinking: boolean;
} {
  if (selectedModelId === 'auto') {
    const resolved = resolveConcreteModelSelection(selectedModelId, catalog);
    return {
      label: t('chat.modelAuto'),
      meta: t('chat.modelAutoDesc'),
      enableThinking: resolved?.reasoning?.status === 'known',
    };
  }

  if (selectedModelId === 'primary' || selectedModelId === 'fast') {
    const resolved = resolveConcreteModelSelection(selectedModelId, catalog);
    return {
      label: resolved
        ? (selectedModelId === 'primary' ? t('chat.modelPrimary') : t('chat.modelFast'))
        : t('chat.modelAuto'),
      meta: buildModelProviderMeta(resolved) || t('chat.modelAutoDesc'),
      enableThinking: resolved?.reasoning?.status === 'known',
    };
  }

  const resolved = resolveModelSelection(selectedModelId, catalog);
  if (!resolved) {
    return {
      label: t('chat.modelAuto'),
      meta: t('chat.modelAutoDesc'),
      enableThinking: false,
    };
  }

  return {
    label: getModelDisplayName(resolved),
    meta: buildModelProviderMeta(resolved),
    enableThinking: resolved.reasoning?.status === 'known',
  };
}

export const ModelSelectorPill: React.FC<{
  catalog: RemoteModelCatalog | null;
  selectedModelId: string;
  disabled?: boolean;
  onSelect: (modelId: string) => void | Promise<void>;
}> = ({ catalog, selectedModelId, disabled, onSelect }) => {
  const { t } = useI18n();
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);
  const normalizedSelectedModelId = useMemo(
    () => normalizeSelectedModelId(selectedModelId, catalog),
    [catalog, selectedModelId],
  );

  const availableModels = useMemo(
    () => (catalog?.models || []).filter(isChatCapableModel),
    [catalog],
  );
  const resolvedPrimaryModel = useMemo(
    () => resolveModelSelection(catalog?.default_models?.primary || '', catalog),
    [catalog],
  );
  const resolvedFastModel = useMemo(
    () => resolveModelSelection(catalog?.default_models?.fast || '', catalog),
    [catalog],
  );
  const selectedInfo = useMemo(
    () => getSelectedModelInfo(normalizedSelectedModelId, catalog, t),
    [catalog, normalizedSelectedModelId, t],
  );

  useEffect(() => {
    if (!open) return;
    const handleOutside = (event: MouseEvent) => {
      if (rootRef.current && !rootRef.current.contains(event.target as Node)) {
        setOpen(false);
      }
    };
    document.addEventListener('mousedown', handleOutside);
    return () => document.removeEventListener('mousedown', handleOutside);
  }, [open]);

  if (!catalog) return null;

  const handleSelect = (modelId: string) => {
    setOpen(false);
    void onSelect(modelId);
  };

  return (
    <div className="chat-model-selector" ref={rootRef}>
      <MobileButton
        appearance="secondary"
        className={`chat-model-selector__trigger${open ? ' chat-model-selector__trigger--open' : ''}`}
        type="button"
        onClick={() => setOpen(prev => !prev)}
        disabled={disabled}
        aria-label={t('chat.modelSelection')}
      >
        <span className="chat-model-selector__name">
          <span className="chat-model-selector__name-text">{selectedInfo.label}</span>
          {selectedInfo.enableThinking && (
            <SparklesIcon className="chat-model-selector__thinking" size={9} />
          )}
        </span>
        <span className="chat-model-selector__chevron" aria-hidden="true">
          <svg width="10" height="10" viewBox="0 0 16 16" fill="none">
            <path d="M4 6L8 10L12 6" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" />
          </svg>
        </span>
      </MobileButton>

      {open && (
        <MobileCard appearance="elevated" padding="none" className="chat-model-selector__dropdown">
          <div className="chat-model-selector__header">{t('chat.modelSelection')}</div>
          <MobileButton
            appearance="plain"
            block
            className={`chat-model-selector__option${normalizedSelectedModelId === 'auto' ? ' is-selected' : ''}`}
            type="button"
            onClick={() => void handleSelect('auto')}
          >
            <span className="chat-model-selector__option-main">
              <span className="chat-model-selector__option-name">{t('chat.modelAuto')}</span>
              <span className="chat-model-selector__option-meta">{t('chat.modelAutoDesc')}</span>
            </span>
          </MobileButton>
          <MobileButton
            appearance="plain"
            block
            className={`chat-model-selector__option${normalizedSelectedModelId === 'primary' ? ' is-selected' : ''}`}
            type="button"
            onClick={() => void handleSelect('primary')}
          >
            <span className="chat-model-selector__option-main">
              <span className="chat-model-selector__option-name">{t('chat.modelPrimary')}</span>
              <span className="chat-model-selector__option-meta chat-model-selector__option-meta--stacked">
                <span className="chat-model-selector__option-meta-line">
                  {getModelDisplayName(resolvedPrimaryModel) || t('chat.modelAuto')}
                </span>
                <span className="chat-model-selector__option-meta-line">
                  {buildModelProviderMeta(resolvedPrimaryModel) || t('chat.modelAutoDesc')}
                </span>
              </span>
            </span>
          </MobileButton>
          <MobileButton
            appearance="plain"
            block
            className={`chat-model-selector__option${normalizedSelectedModelId === 'fast' ? ' is-selected' : ''}`}
            type="button"
            onClick={() => void handleSelect('fast')}
          >
            <span className="chat-model-selector__option-main">
              <span className="chat-model-selector__option-name">{t('chat.modelFast')}</span>
              <span className="chat-model-selector__option-meta chat-model-selector__option-meta--stacked">
                <span className="chat-model-selector__option-meta-line">
                  {getModelDisplayName(resolvedFastModel) || t('chat.modelAuto')}
                </span>
                <span className="chat-model-selector__option-meta-line">
                  {buildModelProviderMeta(resolvedFastModel) || t('chat.modelAutoDesc')}
                </span>
              </span>
            </span>
          </MobileButton>
          <div className="chat-model-selector__divider" />
          <div className="chat-model-selector__list">
            {availableModels.map(model => {
              const isSelected = normalizedSelectedModelId === model.id;
              return (
                <MobileButton
                  appearance="plain"
                  block
                  key={model.id}
                  className={`chat-model-selector__option${isSelected ? ' is-selected' : ''}`}
                  type="button"
                  onClick={() => void handleSelect(model.id)}
                >
                  <span className="chat-model-selector__option-main">
                    <span className="chat-model-selector__option-name">
                      <span className="chat-model-selector__option-name-text">
                        {getModelDisplayName(model)}
                      </span>
                      {model.reasoning?.status === 'known' && (
                        <SparklesIcon className="chat-model-selector__option-thinking" size={10} />
                      )}
                    </span>
                    <span className="chat-model-selector__option-meta">
                      {buildModelProviderMeta(model) || formatProviderName(model.provider)}
                    </span>
                  </span>
                </MobileButton>
              );
            })}
          </div>
        </MobileCard>
      )}
    </div>
  );
};
export const ReasoningPresetPill: React.FC<{
  catalog: RemoteModelCatalog | null;
  selectedModelId: string;
  disabled?: boolean;
  onSelect: (presetId: string | null) => void | Promise<void>;
}> = ({ catalog, selectedModelId, disabled, onSelect }) => {
  const { t } = useI18n();
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);
  const model = useMemo(
    () => resolveConcreteModelSelection(selectedModelId, catalog),
    [catalog, selectedModelId],
  );
  const presets = useMemo(
    () => [...(model?.reasoning?.presets || [])].sort((a, b) => a.order - b.order),
    [model],
  );
  const selectedPresetId = catalog?.session_reasoning_preset?.trim() || null;
  const selectedLabel = selectedPresetId
    ? presets.find(preset => preset.id === selectedPresetId)?.label || selectedPresetId
    : t('chat.reasoningAuto');
  const selectionSupported = catalog?.reasoning_preset_selection_supported === true;
  const isDisabled = disabled || !selectionSupported;

  useEffect(() => {
    if (!open) return;
    const handleOutside = (event: MouseEvent) => {
      if (rootRef.current && !rootRef.current.contains(event.target as Node)) {
        setOpen(false);
      }
    };
    document.addEventListener('mousedown', handleOutside);
    return () => document.removeEventListener('mousedown', handleOutside);
  }, [open]);

  if (model?.reasoning?.status !== 'known' || presets.length === 0) return null;

  const handleSelect = (presetId: string | null) => {
    setOpen(false);
    void onSelect(presetId);
  };

  return (
    <div className="chat-model-selector chat-reasoning-selector" ref={rootRef}>
      <MobileButton
        appearance="secondary"
        className={`chat-model-selector__trigger chat-reasoning-selector__trigger${open ? ' chat-model-selector__trigger--open' : ''}`}
        type="button"
        onClick={() => setOpen(prev => !prev)}
        disabled={isDisabled}
        aria-label={t('chat.reasoningSelection')}
      >
        <SparklesIcon className="chat-model-selector__thinking" size={10} />
        <span className="chat-model-selector__name chat-reasoning-selector__name">
          <span className="chat-model-selector__name-text">{selectedLabel}</span>
        </span>
        <span className="chat-model-selector__chevron" aria-hidden="true">
          <svg width="10" height="10" viewBox="0 0 16 16" fill="none">
            <path d="M4 6L8 10L12 6" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" />
          </svg>
        </span>
      </MobileButton>

      {open && selectionSupported && (
        <MobileCard appearance="elevated" padding="none" className="chat-model-selector__dropdown chat-reasoning-selector__dropdown">
          <div className="chat-model-selector__header">{t('chat.reasoningSelection')}</div>
          <div className="chat-model-selector__list">
            <MobileButton
              appearance="plain"
              block
              className={`chat-model-selector__option${selectedPresetId === null ? ' is-selected' : ''}`}
              type="button"
              onClick={() => void handleSelect(null)}
            >
              <span className="chat-model-selector__option-name">{t('chat.reasoningAuto')}</span>
            </MobileButton>
            {presets.map(preset => (
              <MobileButton
                appearance="plain"
                block
                key={preset.id}
                className={`chat-model-selector__option${selectedPresetId === preset.id ? ' is-selected' : ''}`}
                type="button"
                onClick={() => void handleSelect(preset.id)}
              >
                <span className="chat-model-selector__option-name">{preset.label}</span>
              </MobileButton>
            ))}
          </div>
        </MobileCard>
      )}
    </div>
  );
};
