import { invoke } from '@tauri-apps/api/core';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  createModelConfigFromTemplate,
  getOrderedProviders,
  PROVIDER_TEMPLATES,
  resolveProviderFormat,
  type ApiFormat,
  type ProviderTemplate,
} from '../data/modelProviders';
import type { RequestFormatValue } from '../data/modelRequestFormats';
import type { ConnectionTestResult, InstallOptions, ModelConfig, RemoteModelInfo } from '../types/installer';
import { previewRequestUrl, resolveRequestUrl } from '../utils/modelRequestUrl';

type TestStatus = 'idle' | 'testing' | 'success' | 'error';
const CUSTOM_MODEL_OPTION = '__custom_model__';

interface SelectOption {
  value: string;
  label: string;
  description?: string;
}

interface ModelSetupProps {
  options: InstallOptions;
  setOptions: React.Dispatch<React.SetStateAction<InstallOptions>>;
  onSkip: () => void;
  onNext: () => Promise<void>;
  onTestConnection: (modelConfig: ModelConfig) => Promise<ConnectionTestResult>;
}

interface SimpleSelectProps {
  value: string;
  options: SelectOption[];
  placeholder: string;
  onChange: (value: string) => void;
  onOpenChange?: (open: boolean) => void;
  disabled?: boolean;
}

function SimpleSelect({
  value,
  options,
  placeholder,
  onChange,
  onOpenChange,
  disabled = false,
}: SimpleSelectProps) {
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement | null>(null);
  const selected = useMemo(() => options.find((item) => item.value === value) || null, [options, value]);

  useEffect(() => {
    if (!open) return;
    const onPointerDown = (event: PointerEvent) => {
      if (!rootRef.current) return;
      if (!rootRef.current.contains(event.target as Node)) {
        setOpen(false);
        onOpenChange?.(false);
      }
    };
    document.addEventListener('pointerdown', onPointerDown);
    return () => document.removeEventListener('pointerdown', onPointerDown);
  }, [open, onOpenChange]);

  return (
    <div className="bf-select" ref={rootRef}>
      <button
        type="button"
        disabled={disabled}
        className={`bf-select-trigger ${open ? 'bf-select-trigger--open' : ''}`}
        onClick={() => {
          if (disabled) return;
          setOpen((prev) => {
            const next = !prev;
            if (next) onOpenChange?.(true);
            else onOpenChange?.(false);
            return next;
          });
        }}
      >
        <span className={`bf-select-value ${selected ? '' : 'bf-select-value--placeholder'}`}>
          {selected?.label || placeholder}
        </span>
        <span className={`bf-select-caret ${open ? 'bf-select-caret--open' : ''}`} aria-hidden="true">
          v
        </span>
      </button>

      {open && (
        <div className="bf-select-menu" role="listbox">
          {options.length > 0 ? (
            options.map((option) => (
              <button
                key={option.value}
                type="button"
                className={`bf-select-option ${option.value === value ? 'bf-select-option--active' : ''}`}
                onClick={() => {
                  onChange(option.value);
                  setOpen(false);
                  onOpenChange?.(false);
                }}
              >
                <span className="bf-select-option-label">{option.label}</span>
                {option.description && <span className="bf-select-option-desc">{option.description}</span>}
              </button>
            ))
          ) : (
            <div className="bf-select-empty">—</div>
          )}
        </div>
      )}
    </div>
  );
}

function FieldRow({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="model-setup-row">
      <div className="model-setup-row__label">{label}</div>
      <div className="model-setup-row__control">{children}</div>
    </div>
  );
}

export function ModelSetup({ options, setOptions, onSkip, onNext, onTestConnection }: ModelSetupProps) {
  const { t } = useTranslation();
  const providers = useMemo(() => getOrderedProviders(), []);
  const current = options.modelConfig;

  const [selectedProviderId, setSelectedProviderId] = useState(current?.provider || '');
  const [apiKey, setApiKey] = useState(current?.apiKey || '');
  const [showApiKey, setShowApiKey] = useState(false);
  const [baseUrl, setBaseUrl] = useState(current?.baseUrl || '');
  const [modelName, setModelName] = useState(current?.modelName || '');
  const [apiFormat, setApiFormat] = useState<ApiFormat>((current?.format as ApiFormat) || 'openai');
  const [customFormat, setCustomFormat] = useState<ApiFormat>((current?.format as ApiFormat) || 'openai');
  const [forceCustomModelInput, setForceCustomModelInput] = useState(false);

  const [remoteModels, setRemoteModels] = useState<RemoteModelInfo[]>([]);
  const [isFetchingRemoteModels, setIsFetchingRemoteModels] = useState(false);
  const [remoteModelsError, setRemoteModelsError] = useState<string | null>(null);
  const [testStatus, setTestStatus] = useState<TestStatus>('idle');
  const [testMessage, setTestMessage] = useState('');
  const [isSubmitting, setIsSubmitting] = useState(false);

  const isCustomProvider = selectedProviderId === 'custom';
  const template = useMemo<ProviderTemplate | null>(() => {
    if (!selectedProviderId || selectedProviderId === 'custom') return null;
    return PROVIDER_TEMPLATES[selectedProviderId] || null;
  }, [selectedProviderId]);

  const defaultProviderLabel = useMemo(() => {
    if (!template) return t('model.customProvider');
    return t(template.nameKey, { defaultValue: template.id });
  }, [template, t]);

  const effectiveBaseUrl = useMemo(() => {
    if (isCustomProvider) return baseUrl.trim();
    if (baseUrl.trim()) return baseUrl.trim();
    return template?.baseUrl || '';
  }, [isCustomProvider, baseUrl, template]);

  const effectiveModelName = useMemo(() => {
    if (modelName.trim()) return modelName.trim();
    return template?.models[0] || '';
  }, [modelName, template]);

  const resolvedApiFormat = useMemo<ApiFormat>(() => {
    if (isCustomProvider || !template) return customFormat;
    return apiFormat;
  }, [isCustomProvider, template, customFormat, apiFormat]);

  const previewResolvedUrl = useMemo(
    () => previewRequestUrl(effectiveBaseUrl, resolvedApiFormat),
    [effectiveBaseUrl, resolvedApiFormat],
  );

  const draftModelConfig = useMemo<ModelConfig | null>(() => {
    if (!selectedProviderId) return null;
    return {
      provider: selectedProviderId,
      apiKey,
      baseUrl: effectiveBaseUrl,
      modelName: effectiveModelName,
      format: resolvedApiFormat,
      configName: defaultProviderLabel,
    };
  }, [selectedProviderId, apiKey, effectiveBaseUrl, effectiveModelName, resolvedApiFormat, defaultProviderLabel]);

  const canContinue = Boolean(
    selectedProviderId && apiKey.trim() && effectiveBaseUrl && effectiveModelName && draftModelConfig,
  );

  const canTestConnection = canContinue && testStatus !== 'testing';

  useEffect(() => {
    setOptions((prev) => ({
      ...prev,
      modelConfig: draftModelConfig,
    }));
  }, [draftModelConfig, setOptions]);

  const resetTestState = useCallback(() => {
    setTestStatus('idle');
    setTestMessage('');
  }, []);

  const resetRemoteDiscovery = useCallback(() => {
    setRemoteModels([]);
    setRemoteModelsError(null);
  }, []);

  const fetchRemoteModels = useCallback(async () => {
    if (!draftModelConfig || !apiKey.trim()) {
      setRemoteModelsError(t('model.fillApiKeyBeforeFetch'));
      return;
    }
    setIsFetchingRemoteModels(true);
    setRemoteModelsError(null);
    try {
      const list = await invoke<RemoteModelInfo[]>('list_model_config_models', {
        modelConfig: draftModelConfig,
      });
      setRemoteModels(list);
      if (list.length === 0) {
        setRemoteModelsError(t('model.fetchEmptyFallback'));
      }
    } catch {
      setRemoteModels([]);
      setRemoteModelsError(t('model.fetchFailedFallback'));
    } finally {
      setIsFetchingRemoteModels(false);
    }
  }, [draftModelConfig, apiKey, t]);

  const handleProviderSelect = useCallback(
    (providerId: string) => {
      resetTestState();
      resetRemoteDiscovery();
      setSelectedProviderId(providerId);
      setForceCustomModelInput(false);
      if (providerId === 'custom') {
        setBaseUrl('');
        setModelName('');
        setCustomFormat('openai');
        setApiFormat('openai');
        return;
      }
      const nextTemplate = PROVIDER_TEMPLATES[providerId];
      if (!nextTemplate) return;
      const next = createModelConfigFromTemplate(nextTemplate, null);
      setBaseUrl(next.baseUrl);
      setModelName(next.modelName);
      setApiFormat(resolveProviderFormat(nextTemplate, next.baseUrl));
      setCustomFormat(next.format);
    },
    [resetTestState, resetRemoteDiscovery],
  );

  const handleBaseUrlOptionSelect = useCallback(
    (url: string) => {
      setBaseUrl(url);
      resetTestState();
      resetRemoteDiscovery();
      if (template?.baseUrlOptions) {
        const opt = template.baseUrlOptions.find((o) => o.url === url.trim());
        if (opt) setApiFormat(opt.format);
      }
    },
    [template, resetTestState, resetRemoteDiscovery],
  );

  const handleTestConnection = useCallback(async () => {
    if (!draftModelConfig || !canTestConnection) return;
    setTestStatus('testing');
    setTestMessage(t('model.testing'));
    try {
      const result = await onTestConnection(draftModelConfig);
      if (result.success) {
        setTestStatus('success');
        setTestMessage(t('model.testSuccess'));
      } else {
        setTestStatus('error');
        setTestMessage(result.errorDetails || t('model.testFailed'));
      }
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setTestStatus('error');
      setTestMessage(message || t('model.testFailed'));
    }
  }, [draftModelConfig, canTestConnection, onTestConnection, t]);

  const handleContinue = useCallback(async () => {
    if (!canContinue) return;
    setIsSubmitting(true);
    try {
      await onNext();
    } catch (error) {
      setTestStatus('error');
      setTestMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setIsSubmitting(false);
    }
  }, [canContinue, onNext]);

  const providerOptions = useMemo<SelectOption[]>(() => {
    return [
      { value: 'custom', label: t('model.customProvider') },
      ...providers.map((provider) => ({
        value: provider.id,
        label: t(provider.nameKey, { defaultValue: provider.id }),
      })),
    ];
  }, [providers, t]);

  const baseUrlOptions = useMemo<SelectOption[]>(() => {
    if (!template?.baseUrlOptions?.length) return [];
    return template.baseUrlOptions.map((opt) => ({
      value: opt.url,
      label: opt.url,
      description: `${opt.format.toUpperCase()} · ${opt.noteKey ? t(opt.noteKey) : ''}`,
    }));
  }, [template, t]);

  const formatSelectOptions = useMemo<SelectOption[]>(
    () => [
      { value: 'openai', label: t('model.formats.openaiCompatible') },
      { value: 'responses', label: t('model.formats.responsesApi') },
      { value: 'anthropic', label: t('model.formats.claudeApi') },
      { value: 'gemini', label: t('model.formats.geminiApi') },
    ],
    [t],
  );

  const mergedModelIds = useMemo(() => {
    const preset = template?.models ?? [];
    const remoteIds = remoteModels.map((m) => m.id);
    return [...new Set([...preset, ...remoteIds])];
  }, [template, remoteModels]);

  const modelOptions = useMemo<SelectOption[]>(() => {
    if (!template && !isCustomProvider) return [];
    if (isCustomProvider) {
      return [];
    }
    return [
      ...mergedModelIds.map((id) => {
        const dn = remoteModels.find((m) => m.id === id)?.displayName;
        return {
          value: id,
          label: dn ? `${id} (${dn})` : id,
        };
      }),
      {
        value: CUSTOM_MODEL_OPTION,
        label: t('model.addCustomModel'),
      },
    ];
  }, [template, isCustomProvider, mergedModelIds, remoteModels, t]);

  const modelSelectionValue = useMemo(() => {
    if (!template) return '';
    if (forceCustomModelInput) return CUSTOM_MODEL_OPTION;
    const trimmed = modelName.trim();
    if (!trimmed) return mergedModelIds[0] || CUSTOM_MODEL_OPTION;
    if (mergedModelIds.includes(trimmed)) return trimmed;
    return CUSTOM_MODEL_OPTION;
  }, [template, modelName, forceCustomModelInput, mergedModelIds]);

  const modelFetchHint = useMemo(() => {
    if (isFetchingRemoteModels) return t('model.fetchingModels');
    if (remoteModelsError) return remoteModelsError;
    if (remoteModels.length > 0) return null;
    if (template?.models?.length) return t('model.usingPresetModels');
    return null;
  }, [isFetchingRemoteModels, remoteModelsError, remoteModels.length, template, t]);

  const storedRequestUrlReadonly = useMemo(
    () => resolveRequestUrl(effectiveBaseUrl, resolvedApiFormat, effectiveModelName),
    [effectiveBaseUrl, resolvedApiFormat, effectiveModelName],
  );

  return (
    <div className="model-setup-page">
      <div className="model-setup-scroll">
        <div className="model-setup-container" style={{ animation: 'fadeIn 0.4s ease-out' }}>
          <div className="model-setup-intro">{t('model.subtitle')}</div>
          <div className="model-setup-desc">{t('model.description')}</div>

          <FieldRow label={t('model.providerLabel')}>
            <SimpleSelect
              value={selectedProviderId}
              options={providerOptions}
              placeholder={t('model.selectProvider')}
              onChange={handleProviderSelect}
            />
          </FieldRow>

          {template && <div className="model-setup-provider-desc">{t(template.descriptionKey)}</div>}

          {!!selectedProviderId && (
            <div className="model-setup-fields">
              <FieldRow label={t('model.form.apiKey')}>
                <div className="model-setup-inline">
                  <input
                    className="input"
                    type={showApiKey ? 'text' : 'password'}
                    placeholder={t('model.form.apiKeyPlaceholder')}
                    value={apiKey}
                    onChange={(e) => {
                      setApiKey(e.target.value);
                      resetTestState();
                      resetRemoteDiscovery();
                    }}
                  />
                  <button type="button" className="btn btn-ghost model-setup-secret-btn" onClick={() => setShowApiKey((s) => !s)}>
                    {showApiKey ? t('model.hideSecret') : t('model.showSecret')}
                  </button>
                </div>
              </FieldRow>

              <FieldRow label={t('model.form.baseUrl')}>
                <div className="model-setup-stack">
                  {baseUrlOptions.length > 0 ? (
                    <SimpleSelect
                      value={template?.baseUrlOptions?.some((o) => o.url === effectiveBaseUrl) ? effectiveBaseUrl : ''}
                      options={baseUrlOptions}
                      placeholder={t('model.baseUrlPlaceholder')}
                      onChange={(next) => handleBaseUrlOptionSelect(next)}
                    />
                  ) : null}
                  <input
                    className="input"
                    type="url"
                    placeholder={template?.baseUrl || t('model.baseUrlPlaceholder')}
                    value={baseUrl}
                    onChange={(e) => {
                      setBaseUrl(e.target.value);
                      resetTestState();
                      resetRemoteDiscovery();
                      if (template && !isCustomProvider) {
                        setApiFormat(resolveProviderFormat(template, e.target.value));
                      }
                    }}
                  />
                </div>
              </FieldRow>

              {!!effectiveBaseUrl && (
                <FieldRow label={t('model.form.resolvedUrlLabel').replace(/[:：]\s*$/, '').trim()}>
                  <input className="input input--readonly" readOnly value={previewResolvedUrl} title={storedRequestUrlReadonly} />
                </FieldRow>
              )}

              <FieldRow label={t('model.form.provider')}>
                <SimpleSelect
                  value={isCustomProvider ? customFormat : apiFormat}
                  options={formatSelectOptions}
                  placeholder={t('model.form.providerPlaceholder')}
                  onChange={(next) => {
                    const v = next as RequestFormatValue;
                    if (isCustomProvider) setCustomFormat(v);
                    else setApiFormat(v);
                    resetTestState();
                    resetRemoteDiscovery();
                  }}
                />
              </FieldRow>

              <FieldRow label={t('model.form.modelSelection')}>
                {template ? (
                  <div className="model-setup-stack">
                    <SimpleSelect
                      value={modelSelectionValue}
                      options={modelOptions}
                      placeholder={t('model.modelNameSelectPlaceholder')}
                      disabled={isFetchingRemoteModels}
                      onOpenChange={(open) => {
                        if (open) void fetchRemoteModels();
                      }}
                      onChange={(next) => {
                        if (next === CUSTOM_MODEL_OPTION) {
                          setForceCustomModelInput(true);
                          if (mergedModelIds.includes(modelName.trim())) {
                            setModelName('');
                          }
                          resetTestState();
                          return;
                        }
                        setForceCustomModelInput(false);
                        setModelName(next);
                        resetTestState();
                      }}
                    />
                    {(forceCustomModelInput || (modelName.trim() && !mergedModelIds.includes(modelName.trim()))) && (
                      <input
                        className="input"
                        placeholder={t('model.modelNamePlaceholder')}
                        value={modelName}
                        onChange={(e) => {
                          setModelName(e.target.value);
                          resetTestState();
                        }}
                      />
                    )}
                  </div>
                ) : (
                  <input
                    className="input"
                    placeholder={t('model.modelNamePlaceholder')}
                    value={modelName}
                    onChange={(e) => {
                      setModelName(e.target.value);
                      resetTestState();
                    }}
                  />
                )}
              </FieldRow>

              {modelFetchHint && <div className="model-setup-fetch-hint">{modelFetchHint}</div>}
            </div>
          )}

          {!!selectedProviderId && (
            <div className="model-setup-test-row">
              <button className="btn" disabled={!canTestConnection} onClick={handleTestConnection}>
                {testStatus === 'testing' ? t('model.testing') : t('model.testConnection')}
              </button>
              {testStatus === 'success' && <span className="model-setup-test-msg model-setup-test-msg--ok">{testMessage}</span>}
              {testStatus === 'error' && <span className="model-setup-test-msg model-setup-test-msg--err">{testMessage}</span>}
            </div>
          )}
        </div>
      </div>

      <div className="model-setup-footer">
        <button className="btn btn-ghost" onClick={onSkip}>
          {t('model.skip')}
        </button>
        <button className="btn btn-primary" onClick={handleContinue} disabled={!canContinue || isSubmitting}>
          {t('model.nextTheme')}
        </button>
      </div>
    </div>
  );
}
