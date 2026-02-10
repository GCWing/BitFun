/**
 * ModelConfigStep
 */

import React, { useState, useCallback, useMemo, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { Settings, Loader, Check, X, ExternalLink, ChevronDown, ChevronUp, AlertTriangle, Plus } from 'lucide-react';
import { useOnboardingStore } from '../../store/onboardingStore';
import { aiApi } from '@/infrastructure/api';
import { systemAPI } from '@/infrastructure/api';
import { Select, Checkbox, Button, IconButton } from '@/component-library';
import { PROVIDER_TEMPLATES } from '@/infrastructure/config/services/modelConfigs';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('ModelConfigStep');

interface ModelConfigStepProps {
  onSkipForNow: () => void;
}

/** Provider display order */
const PROVIDER_ORDER = ['zhipu', 'qwen', 'deepseek', 'volcengine', 'minimax', 'moonshot', 'anthropic'];

type TestStatus = 'idle' | 'testing' | 'success' | 'error';

export const ModelConfigStep: React.FC<ModelConfigStepProps> = ({ onSkipForNow }) => {
  const { t } = useTranslation('onboarding');
  const { t: tAiModel } = useTranslation('settings/ai-model');
  const { modelConfig, setModelConfig } = useOnboardingStore();
  
  // Basic fields
  const [selectedProviderId, setSelectedProviderId] = useState(modelConfig?.provider || '');
  const [apiKey, setApiKey] = useState(modelConfig?.apiKey || '');
  const [baseUrl, setBaseUrl] = useState(modelConfig?.baseUrl || '');
  const [modelName, setModelName] = useState(modelConfig?.modelName || '');
  const [customFormat, setCustomFormat] = useState<'openai' | 'anthropic'>(
    (modelConfig?.format as 'openai' | 'anthropic') || 'openai'
  );
  const [testStatus, setTestStatus] = useState<TestStatus>('idle');
  const [testError, setTestError] = useState<string>('');

  // Advanced settings - restore from store so state survives unmount/remount
  const [showAdvancedSettings, setShowAdvancedSettings] = useState(
    Boolean(modelConfig?.customRequestBody || modelConfig?.skipSslVerify || modelConfig?.customHeaders)
  );
  const [customRequestBody, setCustomRequestBody] = useState(modelConfig?.customRequestBody || '');
  const [skipSslVerify, setSkipSslVerify] = useState(modelConfig?.skipSslVerify || false);
  const [customHeaders, setCustomHeaders] = useState<Record<string, string>>(modelConfig?.customHeaders || {});
  const [customHeadersMode, setCustomHeadersMode] = useState<'merge' | 'replace'>(
    modelConfig?.customHeadersMode || 'merge'
  );

  // Build sorted provider options from PROVIDER_TEMPLATES
  const providerOptions = useMemo(() => {
    const sorted = PROVIDER_ORDER
      .filter(id => PROVIDER_TEMPLATES[id])
      .map(id => PROVIDER_TEMPLATES[id]);
    
    // Add any templates not in the explicit order
    Object.values(PROVIDER_TEMPLATES).forEach(template => {
      if (!PROVIDER_ORDER.includes(template.id)) {
        sorted.push(template);
      }
    });

    // Dynamically get translated name and description
    return sorted.map(provider => ({
      ...provider,
      name: tAiModel(`providers.${provider.id}.name`),
      description: tAiModel(`providers.${provider.id}.description`)
    }));
  }, [tAiModel]);

  // Build select options: custom first, then preset providers
  const selectOptions = useMemo(() => {
    const options: Array<{ label: string; value: string; description: string }> = [{
      label: t('model.provider.options.custom'),
      value: 'custom',
      description: t('model.provider.customDescription')
    }];
    providerOptions.forEach(p => {
      options.push({
        label: p.name,
        value: p.id,
        description: p.description
      });
    });
    return options;
  }, [providerOptions, t]);

  // Current template (null if custom or not selected)
  const currentTemplate = useMemo(() => {
    if (!selectedProviderId || selectedProviderId === 'custom') return null;
    const template = PROVIDER_TEMPLATES[selectedProviderId];
    if (!template) return null;
    // Dynamically get translated name, description, and baseUrlOptions notes
    return {
      ...template,
      name: tAiModel(`providers.${template.id}.name`),
      description: tAiModel(`providers.${template.id}.description`),
      baseUrlOptions: template.baseUrlOptions?.map(opt => ({
        ...opt,
        note: tAiModel(`providers.${template.id}.urlOptions.${opt.note}`, { defaultValue: opt.note })
      }))
    };
  }, [selectedProviderId, tAiModel]);

  // Stable JSON representation of customHeaders for useEffect dependency
  const customHeadersJson = useMemo(() => JSON.stringify(customHeaders), [customHeaders]);

  // Sync form state to onboarding store whenever fields change
  useEffect(() => {
    if (!selectedProviderId) {
      setModelConfig(null);
      return;
    }

    const template = selectedProviderId !== 'custom' ? PROVIDER_TEMPLATES[selectedProviderId] : null;
    const effectiveBaseUrl = baseUrl || (template?.baseUrl || '');
    const effectiveModelName = modelName || (template?.models[0] || '');

    // Derive format
    let format: 'openai' | 'anthropic' = customFormat;
    if (template) {
      if (template.baseUrlOptions?.length) {
        const effectiveUrl = baseUrl || template.baseUrl;
        const matched = template.baseUrlOptions.find(opt => opt.url === effectiveUrl);
        format = matched ? matched.format : template.format;
      } else {
        format = template.format;
      }
    }

    const translatedName = template ? tAiModel(`providers.${template.id}.name`) : null;
    const customLabel = t('model.provider.options.custom');
    const configName = translatedName
      ? `${translatedName} - ${effectiveModelName}`
      : effectiveModelName ? `${customLabel}-${effectiveModelName}` : customLabel;

    const parsedHeaders = JSON.parse(customHeadersJson) as Record<string, string>;

    setModelConfig({
      provider: selectedProviderId,
      apiKey,
      baseUrl: effectiveBaseUrl,
      modelName: effectiveModelName,
      format,
      configName,
      customRequestBody: customRequestBody.trim() || undefined,
      skipSslVerify: skipSslVerify || undefined,
      customHeaders: Object.keys(parsedHeaders).length > 0 ? parsedHeaders : undefined,
      customHeadersMode: Object.keys(parsedHeaders).length > 0 ? customHeadersMode : undefined,
    });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectedProviderId, apiKey, baseUrl, modelName, customFormat, customRequestBody, skipSslVerify, customHeadersJson, customHeadersMode]);

  // Handle provider change
  const handleProviderChange = useCallback((newProviderId: string) => {
    setSelectedProviderId(newProviderId);
    setTestStatus('idle');
    setTestError('');

    if (newProviderId === 'custom') {
      setBaseUrl('');
      setModelName('');
    } else {
      const template = PROVIDER_TEMPLATES[newProviderId];
      if (template) {
        setBaseUrl(template.baseUrl);
        setModelName(template.models[0] || '');
      }
    }
  }, []);

  // Handle "skip for now": clear config and proceed
  const handleSkipForNow = useCallback(() => {
    setModelConfig(null);
    onSkipForNow();
  }, [setModelConfig, onSkipForNow]);

  // Handle model name change from template select
  const handleModelNameChange = useCallback((value: string) => {
    setModelName(value);
    setTestStatus('idle');
    setTestError('');
  }, []);

  // Open help URL
  const handleOpenHelpUrl = useCallback(async () => {
    if (currentTemplate?.helpUrl) {
      try {
        await systemAPI.openExternal(currentTemplate.helpUrl);
      } catch {
        window.open(currentTemplate.helpUrl, '_blank');
      }
    }
  }, [currentTemplate]);

  // Get effective format
  const getEffectiveFormat = useCallback(() => {
    if (currentTemplate) {
      // If the template has baseUrlOptions, derive format from the selected URL
      if (currentTemplate.baseUrlOptions && currentTemplate.baseUrlOptions.length > 0) {
        const effectiveUrl = baseUrl || currentTemplate.baseUrl;
        const matchedOption = currentTemplate.baseUrlOptions.find(opt => opt.url === effectiveUrl);
        if (matchedOption) {
          return matchedOption.format;
        }
      }
      return currentTemplate.format;
    }
    return customFormat;
  }, [currentTemplate, customFormat, baseUrl]);

  // Test connection (purely for connectivity validation, does not affect saving)
  const handleTestConnection = useCallback(async () => {
    if (!apiKey || !selectedProviderId) return;

    setTestStatus('testing');
    setTestError('');

    try {
      const effectiveBaseUrl = baseUrl || (currentTemplate?.baseUrl || '');
      const effectiveModelName = modelName || (currentTemplate?.models[0] || '');
      const format = getEffectiveFormat();

      const result = await aiApi.testConfigConnection({
        base_url: effectiveBaseUrl,
        api_key: apiKey,
        model_name: effectiveModelName,
        provider: format
      });

      if (result.success) {
        setTestStatus('success');
        log.info('Connection test passed', { 
          provider: selectedProviderId, 
          modelName: effectiveModelName
        });
      } else {
        setTestStatus('error');
        const errorMsg = result.error_details
          ? `${t('model.testFailed')}\n${result.error_details}`
          : t('model.testFailed');
        setTestError(errorMsg);
      }
    } catch (error) {
      log.error('Connection test failed', error);
      setTestStatus('error');
      const rawMsg = error instanceof Error ? error.message : String(error);
      // Tauri command errors often have "Connection test failed: " prefix, extract the actual cause
      const cleanMsg = rawMsg.replace(/^Connection test failed:\s*/i, '');
      setTestError(cleanMsg ? `${t('model.testFailed')}\n${cleanMsg}` : t('model.testFailed'));
    }
  }, [apiKey, selectedProviderId, baseUrl, modelName, currentTemplate, getEffectiveFormat, t]);

  // Render test button
  const renderTestButton = () => {
    const isDisabled = !apiKey || !selectedProviderId || testStatus === 'testing';
    
    let buttonClass = 'bitfun-onboarding-model__test-btn';
    let content: React.ReactNode = t('model.testConnection');

    switch (testStatus) {
      case 'testing':
        content = (
          <>
            <Loader className="animate-spin" size={16} />
            {t('model.testing')}
          </>
        );
        break;
      case 'success':
        buttonClass += ' bitfun-onboarding-model__test-btn--success';
        content = (
          <>
            <Check size={16} />
            {t('model.testSuccess')}
          </>
        );
        break;
      case 'error':
        buttonClass += ' bitfun-onboarding-model__test-btn--error';
        content = (
          <>
            <X size={16} />
            {t('model.testFailed')}
          </>
        );
        break;
    }

    return (
      <button
        className={buttonClass}
        onClick={handleTestConnection}
        disabled={isDisabled}
      >
        {content}
      </button>
    );
  };

  // Validate custom request body JSON
  const customRequestBodyValidation = useMemo(() => {
    if (!customRequestBody || !customRequestBody.trim()) return null;
    try {
      JSON.parse(customRequestBody);
      return 'valid';
    } catch {
      return 'invalid';
    }
  }, [customRequestBody]);

  // Whether a provider is selected (to show the form)
  const isProviderSelected = !!selectedProviderId;

  return (
    <div className="bitfun-onboarding-step bitfun-onboarding-model">
      {/* Icon */}
      <div className="bitfun-onboarding-step__icon">
        <Settings />
      </div>

      {/* Header */}
      <div className="bitfun-onboarding-step__header">
        <h1 className="bitfun-onboarding-step__title">
          {t('model.title')}
        </h1>
        <p className="bitfun-onboarding-step__description">
          {t('model.description')}
        </p>
      </div>

      {/* Config Form */}
      <div className="bitfun-onboarding-model__form">
        {/* Provider Select */}
        <div className="bitfun-onboarding-model__field">
          <Select
            label={t('model.provider.label')}
            options={selectOptions}
            value={selectedProviderId}
            onChange={(val) => handleProviderChange(val as string)}
            placeholder={t('model.provider.placeholder')}
          />
        </div>

        {/* Template provider form */}
        {isProviderSelected && currentTemplate && (
          <>
            {/* Model Select from template */}
            <div className="bitfun-onboarding-model__field">
              <label className="bitfun-onboarding-model__label">
                {t('model.modelName.label')}
              </label>
              <Select
                value={modelName}
                onChange={(value) => handleModelNameChange(value as string)}
                placeholder={t('model.modelName.selectPlaceholder')}
                options={currentTemplate.models.map(model => ({
                  label: model,
                  value: model
                }))}
                searchable
                allowCustomValue
                searchPlaceholder={t('model.modelName.inputPlaceholder')}
                customValueHint={t('model.modelName.customHint')}
              />
            </div>

            {/* API Key */}
            <div className="bitfun-onboarding-model__field">
              <label className="bitfun-onboarding-model__label">
                {t('model.apiKey.label')}
              </label>
              <input
                type="password"
                className="bitfun-onboarding-model__input"
                placeholder={t('model.apiKey.placeholder')}
                value={apiKey}
                onChange={(e) => {
                  setApiKey(e.target.value);
                  setTestStatus('idle');
                  setTestError('');
                }}
              />
              {currentTemplate.helpUrl && (
                <button 
                  className="bitfun-onboarding-model__help"
                  onClick={handleOpenHelpUrl}
                  type="button"
                >
                  {t('model.apiKey.help')}
                  <ExternalLink size={12} />
                </button>
              )}
            </div>

            {/* Base URL (pre-filled, editable) */}
            <div className="bitfun-onboarding-model__field">
              <label className="bitfun-onboarding-model__label">
                {t('model.baseUrl.label')}
              </label>
              {currentTemplate.baseUrlOptions && currentTemplate.baseUrlOptions.length > 0 ? (
                <Select
                  value={baseUrl || currentTemplate.baseUrl}
                  onChange={(value) => {
                    const selectedOption = currentTemplate.baseUrlOptions!.find(opt => opt.url === value);
                    setBaseUrl(value as string);
                    if (selectedOption) {
                      setCustomFormat(selectedOption.format);
                    }
                    setTestStatus('idle');
                    setTestError('');
                  }}
                  placeholder={t('model.baseUrl.placeholder')}
                  options={currentTemplate.baseUrlOptions.map(opt => ({
                    label: opt.url,
                    value: opt.url,
                    description: `${opt.format.toUpperCase()} · ${opt.note}`
                  }))}
                />
              ) : (
                <input
                  type="text"
                  className="bitfun-onboarding-model__input"
                  placeholder={currentTemplate.baseUrl}
                  value={baseUrl}
                  onChange={(e) => {
                    setBaseUrl(e.target.value);
                    setTestStatus('idle');
                    setTestError('');
                  }}
                  onFocus={(e) => e.target.select()}
                />
              )}
            </div>
          </>
        )}

        {/* Custom provider form */}
        {isProviderSelected && selectedProviderId === 'custom' && (
          <>
            {/* Base URL */}
            <div className="bitfun-onboarding-model__field">
              <label className="bitfun-onboarding-model__label">
                {t('model.baseUrl.label')}
              </label>
              <input
                type="text"
                className="bitfun-onboarding-model__input"
                placeholder={t('model.baseUrl.placeholder')}
                value={baseUrl}
                onChange={(e) => {
                  setBaseUrl(e.target.value);
                  setTestStatus('idle');
                  setTestError('');
                }}
              />
            </div>

            {/* Model Name (text input) */}
            <div className="bitfun-onboarding-model__field">
              <label className="bitfun-onboarding-model__label">
                {t('model.modelName.label')}
              </label>
              <input
                type="text"
                className="bitfun-onboarding-model__input"
                placeholder={t('model.modelName.placeholder')}
                value={modelName}
                onChange={(e) => {
                  setModelName(e.target.value);
                  setTestStatus('idle');
                  setTestError('');
                }}
              />
            </div>

            {/* API Key */}
            <div className="bitfun-onboarding-model__field">
              <label className="bitfun-onboarding-model__label">
                {t('model.apiKey.label')}
              </label>
              <input
                type="password"
                className="bitfun-onboarding-model__input"
                placeholder={t('model.apiKey.placeholder')}
                value={apiKey}
                onChange={(e) => {
                  setApiKey(e.target.value);
                  setTestStatus('idle');
                  setTestError('');
                }}
              />
            </div>

            {/* API Format */}
            <div className="bitfun-onboarding-model__field">
              <Select
                label={t('model.format.label')}
                options={[
                  { label: 'OpenAI', value: 'openai' },
                  { label: 'Anthropic', value: 'anthropic' }
                ]}
                value={customFormat}
                onChange={(val) => setCustomFormat(val as 'openai' | 'anthropic')}
                placeholder={t('model.format.placeholder')}
              />
            </div>
          </>
        )}

        {/* Advanced Settings (collapsed by default, only for custom or moonshot) */}
        {isProviderSelected && (selectedProviderId === 'custom' || selectedProviderId === 'moonshot') && (
          <div className="bitfun-onboarding-model__advanced">
            <button
              type="button"
              className="bitfun-onboarding-model__advanced-toggle"
              onClick={() => setShowAdvancedSettings(!showAdvancedSettings)}
            >
              {showAdvancedSettings ? <ChevronUp size={16} /> : <ChevronDown size={16} />}
              <span>{t('model.advanced.title')}</span>
            </button>

            {showAdvancedSettings && (
              <div className="bitfun-onboarding-model__advanced-content">
                {/* Skip SSL Verify (custom only) */}
                {selectedProviderId === 'custom' && (
                  <div className="bitfun-onboarding-model__field">
                    <Checkbox
                      label={tAiModel('advancedSettings.skipSslVerify.label')}
                      checked={skipSslVerify}
                      onChange={(e) => setSkipSslVerify(e.target.checked)}
                    />
                    {skipSslVerify && (
                      <div className="bitfun-onboarding-model__hint bitfun-onboarding-model__hint--error" style={{ display: 'flex', alignItems: 'center', gap: '4px' }}>
                        <AlertTriangle size={14} />
                        <span>{tAiModel('advancedSettings.skipSslVerify.warning')}</span>
                      </div>
                    )}
                  </div>
                )}

                {/* Custom Headers (custom only) */}
                {selectedProviderId === 'custom' && (
                  <div className="bitfun-onboarding-model__field">
                    <label className="bitfun-onboarding-model__label">
                      {tAiModel('advancedSettings.customHeaders.label')}
                    </label>
                    <span className="bitfun-onboarding-model__hint">
                      {tAiModel('advancedSettings.customHeaders.hint')}
                    </span>

                    <div style={{ display: 'flex', gap: '12px', margin: '8px 0' }}>
                      <label style={{ display: 'flex', alignItems: 'center', gap: '4px', fontSize: '13px', cursor: 'pointer' }}>
                        <input
                          type="radio"
                          name="onboarding_headers_mode"
                          value="merge"
                          checked={customHeadersMode === 'merge'}
                          onChange={() => setCustomHeadersMode('merge')}
                        />
                        <span>{tAiModel('advancedSettings.customHeaders.modeMerge')}</span>
                      </label>
                      <label style={{ display: 'flex', alignItems: 'center', gap: '4px', fontSize: '13px', cursor: 'pointer' }}>
                        <input
                          type="radio"
                          name="onboarding_headers_mode"
                          value="replace"
                          checked={customHeadersMode === 'replace'}
                          onChange={() => setCustomHeadersMode('replace')}
                        />
                        <span>{tAiModel('advancedSettings.customHeaders.modeReplace')}</span>
                      </label>
                    </div>

                    {Object.entries(customHeaders).map(([key, value], index) => (
                      <div key={index} style={{ display: 'flex', gap: '8px', marginBottom: '6px', alignItems: 'center' }}>
                        <input
                          type="text"
                          className="bitfun-onboarding-model__input"
                          value={key}
                          onChange={(e) => {
                            const newHeaders = { ...customHeaders };
                            const oldValue = newHeaders[key];
                            delete newHeaders[key];
                            if (e.target.value) {
                              newHeaders[e.target.value] = oldValue;
                            }
                            setCustomHeaders(newHeaders);
                          }}
                          placeholder={tAiModel('advancedSettings.customHeaders.keyPlaceholder')}
                          style={{ flex: 1 }}
                        />
                        <input
                          type="text"
                          className="bitfun-onboarding-model__input"
                          value={value}
                          onChange={(e) => {
                            setCustomHeaders(prev => ({ ...prev, [key]: e.target.value }));
                          }}
                          placeholder={tAiModel('advancedSettings.customHeaders.valuePlaceholder')}
                          style={{ flex: 1 }}
                        />
                        <IconButton
                          variant="ghost"
                          size="small"
                          onClick={() => {
                            const newHeaders = { ...customHeaders };
                            delete newHeaders[key];
                            setCustomHeaders(newHeaders);
                          }}
                          tooltip={tAiModel('actions.delete')}
                        >
                          <X size={14} />
                        </IconButton>
                      </div>
                    ))}
                    <Button
                      variant="secondary"
                      size="small"
                      onClick={() => setCustomHeaders(prev => ({ ...prev, '': '' }))}
                    >
                      <Plus size={14} />
                      {tAiModel('advancedSettings.customHeaders.addHeader')}
                    </Button>
                  </div>
                )}

                {/* Custom Request Body */}
                <div className="bitfun-onboarding-model__field">
                  <label className="bitfun-onboarding-model__label">
                    {t('model.advanced.customRequestBody')}
                  </label>
                  <span className="bitfun-onboarding-model__hint">
                    {t('model.advanced.customRequestBodyHint')}
                  </span>
                  <textarea
                    className="bitfun-onboarding-model__textarea"
                    value={customRequestBody}
                    onChange={(e) => setCustomRequestBody(e.target.value)}
                    placeholder={t('model.advanced.customRequestBodyPlaceholder')}
                    rows={4}
                  />
                  {customRequestBodyValidation === 'valid' && (
                    <span className="bitfun-onboarding-model__hint bitfun-onboarding-model__hint--success">
                      {t('model.advanced.jsonValid')}
                    </span>
                  )}
                  {customRequestBodyValidation === 'invalid' && (
                    <span className="bitfun-onboarding-model__hint bitfun-onboarding-model__hint--error">
                      {t('model.advanced.jsonInvalid')}
                    </span>
                  )}
                </div>
              </div>
            )}
          </div>
        )}

        {/* Test Connection Button */}
        {isProviderSelected && (
          <>
            <div className="bitfun-onboarding-model__actions">
              {renderTestButton()}
            </div>

            {/* Error message */}
            {testStatus === 'error' && testError && (
              <div className="bitfun-onboarding-model__error">
                {testError}
              </div>
            )}
          </>
        )}

        {/* Skip for now */}
        <button
          className="bitfun-onboarding-model__skip-link"
          onClick={handleSkipForNow}
        >
          {t('model.skipForNow')}
        </button>
      </div>
    </div>
  );
};

export default ModelConfigStep;
