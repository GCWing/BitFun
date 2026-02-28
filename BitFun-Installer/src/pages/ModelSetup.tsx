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
import type { ConnectionTestResult, InstallOptions, ModelConfig } from '../types/installer';

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
  searchable?: boolean;
  searchPlaceholder?: string;
  emptyText?: string;
  alwaysVisibleValues?: string[];
}

function SimpleSelect({
  value,
  options,
  placeholder,
  onChange,
  searchable = false,
  searchPlaceholder,
  emptyText,
  alwaysVisibleValues = [],
}: SimpleSelectProps) {
  const [open, setOpen] = useState(false);
  const [search, setSearch] = useState('');
  const rootRef = useRef<HTMLDivElement | null>(null);
  const selected = useMemo(() => options.find((item) => item.value === value) || null, [options, value]);
  const filteredOptions = useMemo(() => {
    if (!searchable || !search.trim()) return options;
    const keyword = search.trim().toLowerCase();
    return options.filter((item) => {
      if (alwaysVisibleValues.includes(item.value)) return true;
      const label = item.label.toLowerCase();
      const desc = item.description?.toLowerCase() || '';
      return label.includes(keyword) || desc.includes(keyword);
    });
  }, [options, search, searchable, alwaysVisibleValues]);

  useEffect(() => {
    if (!open) return;
    const onPointerDown = (event: PointerEvent) => {
      if (!rootRef.current) return;
      if (!rootRef.current.contains(event.target as Node)) {
        setOpen(false);
      }
    };
    document.addEventListener('pointerdown', onPointerDown);
    return () => document.removeEventListener('pointerdown', onPointerDown);
  }, [open]);

  return (
    <div className="bf-select" ref={rootRef}>
      <button
        type="button"
        className={`bf-select-trigger ${open ? 'bf-select-trigger--open' : ''}`}
        onClick={() => {
          setOpen((prev) => {
            if (prev) setSearch('');
            return !prev;
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
          {searchable && (
            <div className="bf-select-search">
              <input
                className="bf-select-search-input"
                value={search}
                placeholder={searchPlaceholder || 'Search...'}
                onChange={(e) => setSearch(e.target.value)}
              />
            </div>
          )}
          {filteredOptions.length > 0 ? (
            filteredOptions.map((option) => (
              <button
                key={option.value}
                type="button"
                className={`bf-select-option ${option.value === value ? 'bf-select-option--active' : ''}`}
                onClick={() => {
                  onChange(option.value);
                  setSearch('');
                  setOpen(false);
                }}
              >
                <span className="bf-select-option-label">{option.label}</span>
                {option.description && <span className="bf-select-option-desc">{option.description}</span>}
              </button>
            ))
          ) : (
            <div className="bf-select-empty">{emptyText || 'No results'}</div>
          )}
        </div>
      )}
    </div>
  );
}

export function ModelSetup({ options, setOptions, onSkip, onNext, onTestConnection }: ModelSetupProps) {
  const { t } = useTranslation();
  const providers = useMemo(() => getOrderedProviders(), []);
  const current = options.modelConfig;

  const [selectedProviderId, setSelectedProviderId] = useState(current?.provider || '');
  const [apiKey, setApiKey] = useState(current?.apiKey || '');
  const [baseUrl, setBaseUrl] = useState(current?.baseUrl || '');
  const [modelName, setModelName] = useState(current?.modelName || '');
  const [customFormat, setCustomFormat] = useState<ApiFormat>((current?.format as ApiFormat) || 'openai');
  const [forceCustomModelInput, setForceCustomModelInput] = useState(false);

  const [testStatus, setTestStatus] = useState<TestStatus>('idle');
  const [testMessage, setTestMessage] = useState('');
  const [isSubmitting, setIsSubmitting] = useState(false);

  const isCustomProvider = selectedProviderId === 'custom';
  const template = useMemo<ProviderTemplate | null>(() => {
    if (!selectedProviderId || selectedProviderId === 'custom') return null;
    return PROVIDER_TEMPLATES[selectedProviderId] || null;
  }, [selectedProviderId]);

  const effectiveBaseUrl = useMemo(() => {
    if (isCustomProvider) return baseUrl.trim();
    if (baseUrl.trim()) return baseUrl.trim();
    return template?.baseUrl || '';
  }, [isCustomProvider, baseUrl, template]);

  const effectiveModelName = useMemo(() => {
    if (modelName.trim()) return modelName.trim();
    return template?.models[0] || '';
  }, [modelName, template]);

  const effectiveFormat = useMemo<ApiFormat>(() => {
    if (isCustomProvider || !template) return customFormat;
    return resolveProviderFormat(template, effectiveBaseUrl);
  }, [isCustomProvider, template, customFormat, effectiveBaseUrl]);

  const draftModelConfig = useMemo<ModelConfig | null>(() => {
    if (!selectedProviderId) return null;

    const providerDisplayName = template
      ? t(template.nameKey, { defaultValue: template.id })
      : t('model.customProvider', { defaultValue: 'Custom' });
    const configName = `${providerDisplayName} - ${effectiveModelName}`.trim();

    return {
      provider: selectedProviderId,
      apiKey,
      baseUrl: effectiveBaseUrl,
      modelName: effectiveModelName,
      format: effectiveFormat,
      configName,
    };
  }, [
    selectedProviderId,
    template,
    apiKey,
    effectiveBaseUrl,
    effectiveModelName,
    effectiveFormat,
    t,
  ]);

  const canContinue = Boolean(selectedProviderId && apiKey.trim() && effectiveBaseUrl && effectiveModelName);

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

  const handleProviderSelect = useCallback((providerId: string) => {
    resetTestState();
    setSelectedProviderId(providerId);
    setForceCustomModelInput(false);
    if (providerId === 'custom') {
      setBaseUrl('');
      setModelName('');
      setCustomFormat('openai');
      return;
    }
    const nextTemplate = PROVIDER_TEMPLATES[providerId];
    if (!nextTemplate) return;
    const next = createModelConfigFromTemplate(nextTemplate, null);
    setBaseUrl(next.baseUrl);
    setModelName(next.modelName);
    setCustomFormat(next.format);
  }, [resetTestState]);

  const handleTestConnection = useCallback(async () => {
    if (!draftModelConfig || !canTestConnection) return;
    setTestStatus('testing');
    setTestMessage(t('model.testing', { defaultValue: 'Testing...' }));
    try {
      const result = await onTestConnection(draftModelConfig);
      if (result.success) {
        setTestStatus('success');
        setTestMessage(t('model.testSuccess', { defaultValue: 'Connection successful' }));
      } else {
        setTestStatus('error');
        setTestMessage(result.errorDetails || t('model.testFailed', { defaultValue: 'Connection failed' }));
      }
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setTestStatus('error');
      setTestMessage(message || t('model.testFailed', { defaultValue: 'Connection failed' }));
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
      { value: 'custom', label: t('model.customProvider', { defaultValue: 'Custom' }) },
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
      description: `${opt.format.toUpperCase()} / ${opt.noteKey ? t(opt.noteKey, { defaultValue: 'default' }) : 'default'}`,
    }));
  }, [template, t]);

  const modelOptions = useMemo<SelectOption[]>(() => {
    if (!template) return [];
    return [
      ...template.models.map((item) => ({ value: item, label: item })),
      {
        value: CUSTOM_MODEL_OPTION,
        label: t('model.customModel', { defaultValue: 'Use custom model name' }),
      },
    ];
  }, [template, t]);

  const modelSelectionValue = useMemo(() => {
    if (!template) return '';
    if (forceCustomModelInput) return CUSTOM_MODEL_OPTION;
    const trimmed = modelName.trim();
    if (!trimmed) return template.models[0] || '';
    if (template.models.includes(trimmed)) return trimmed;
    return CUSTOM_MODEL_OPTION;
  }, [template, modelName, forceCustomModelInput]);

  const customFormatOptions: SelectOption[] = [
    { value: 'openai', label: 'OpenAI Compatible' },
    { value: 'anthropic', label: 'Anthropic' },
  ];

  return (
    <div className="model-setup-page">
      <div className="model-setup-scroll">
        <div className="model-setup-container" style={{ animation: 'fadeIn 0.4s ease-out' }}>
          <div style={{ marginBottom: 2, fontSize: 12, color: 'var(--color-text-muted)' }}>
            {t('model.subtitle')}
          </div>
          <div style={{ marginBottom: 8, fontSize: 12, color: 'var(--color-text-secondary)' }}>
            {t('model.description', { defaultValue: 'Configure AI model provider and API key.' })}
          </div>

          <div className="section-label">{t('model.providerLabel', { defaultValue: 'Model Provider' })}</div>
          <SimpleSelect
            value={selectedProviderId}
            options={providerOptions}
            placeholder={t('model.selectProvider', { defaultValue: 'Select a provider...' })}
            onChange={handleProviderSelect}
          />

          {template && (
            <div style={{ fontSize: 11, color: 'var(--color-text-muted)', marginTop: 6 }}>
              {t(template.descriptionKey, { defaultValue: '' })}
            </div>
          )}

          {!!selectedProviderId && (
            <div className="model-setup-fields">
              {template ? (
                <>
                  <SimpleSelect
                    value={modelSelectionValue}
                    options={modelOptions}
                    placeholder={t('model.modelNameSelectPlaceholder', { defaultValue: 'Select a model...' })}
                    onChange={(next) => {
                      if (next === CUSTOM_MODEL_OPTION) {
                        setForceCustomModelInput(true);
                        if (template.models.includes(modelName.trim())) {
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
                  {(forceCustomModelInput || (modelName.trim() && !template.models.includes(modelName.trim()))) && (
                    <input
                      className="input"
                      placeholder={t('model.modelNamePlaceholder', {
                        defaultValue: 'Model name (for example: deepseek-chat)',
                      })}
                      value={modelName}
                      onChange={(e) => {
                        setModelName(e.target.value);
                        resetTestState();
                      }}
                    />
                  )}
                </>
              ) : (
                <input
                  className="input"
                  placeholder={t('model.modelNamePlaceholder', { defaultValue: 'Model name (for example: deepseek-chat)' })}
                  value={modelName}
                  onChange={(e) => {
                    setModelName(e.target.value);
                    resetTestState();
                  }}
                />
              )}

              {baseUrlOptions.length > 0 ? (
                <SimpleSelect
                  value={effectiveBaseUrl}
                  options={baseUrlOptions}
                  placeholder={t('model.baseUrlPlaceholder', { defaultValue: 'Base URL' })}
                  onChange={(next) => {
                    setBaseUrl(next);
                    resetTestState();
                  }}
                />
              ) : (
                <input
                  className="input"
                  placeholder={t('model.baseUrlPlaceholder', { defaultValue: 'Base URL' })}
                  value={effectiveBaseUrl}
                  onChange={(e) => {
                    setBaseUrl(e.target.value);
                    resetTestState();
                  }}
                />
              )}

              <input
                className="input"
                type="password"
                placeholder={t('model.apiKey')}
                value={apiKey}
                onChange={(e) => {
                  setApiKey(e.target.value);
                  resetTestState();
                }}
              />

              {isCustomProvider && (
                <SimpleSelect
                  value={customFormat}
                  options={customFormatOptions}
                  placeholder="OpenAI Compatible"
                  onChange={(next) => {
                    setCustomFormat((next as ApiFormat) || 'openai');
                    resetTestState();
                  }}
                />
              )}
            </div>
          )}

          {!!selectedProviderId && (
            <div className="model-setup-test-row">
              <button className="btn" disabled={!canTestConnection} onClick={handleTestConnection}>
                {testStatus === 'testing'
                  ? t('model.testing', { defaultValue: 'Testing...' })
                  : t('model.testConnection', { defaultValue: 'Test Connection' })}
              </button>
              {testStatus === 'success' && (
                <span style={{ fontSize: 12, color: 'var(--color-success)' }}>{testMessage}</span>
              )}
              {testStatus === 'error' && (
                <span style={{ fontSize: 12, color: 'var(--color-error)' }}>{testMessage}</span>
              )}
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
