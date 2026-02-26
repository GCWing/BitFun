import React, { useState, useEffect, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { Plus, Edit2, Trash2, Wifi, Loader, AlertTriangle, X, Settings, ArrowLeft, ExternalLink } from 'lucide-react';
import { Button, Switch, Select, IconButton, NumberInput, Card, Checkbox, Modal, Input, Textarea } from '@/component-library';
import { 
  AIModelConfig as AIModelConfigType, 
  ProxyConfig, 
  ModelCategory,
  ModelCapability
} from '../types';
import { configManager } from '../services/ConfigManager';
import { PROVIDER_TEMPLATES } from '../services/modelConfigs';
import { aiApi, systemAPI } from '@/infrastructure/api';
import { useNotification } from '@/shared/notification-system';
import { ConfigPageHeader, ConfigPageLayout, ConfigPageContent, ConfigPageSection, ConfigPageRow, ConfigCollectionItem } from './common';
import DefaultModelConfig from './DefaultModelConfig';
import { createLogger } from '@/shared/utils/logger';
import './AIModelConfig.scss';

const log = createLogger('AIModelConfig');

/**
 * Compute the actual request URL from a base URL and provider format.
 * Rules:
 *   - Ends with '#'  → strip '#', use as-is (force override)
 *   - openai         → append '/chat/completions' unless already present
 *   - anthropic      → append '/v1/messages' unless already present
 *   - other          → use base_url as-is
 */
function resolveRequestUrl(baseUrl: string, provider: string): string {
  const trimmed = baseUrl.trim().replace(/\/+$/, '');
  if (trimmed.endsWith('#')) {
    return trimmed.slice(0, -1).replace(/\/+$/, '');
  }
  if (provider === 'openai') {
    return trimmed.endsWith('chat/completions') ? trimmed : `${trimmed}/chat/completions`;
  }
  if (provider === 'anthropic') {
    return trimmed.endsWith('v1/messages') ? trimmed : `${trimmed}/v1/messages`;
  }
  return trimmed;
}

const AIModelConfig: React.FC = () => {
  const { t } = useTranslation('settings/ai-model');
  const { t: tDefault } = useTranslation('settings/default-model');
  const [aiModels, setAiModels] = useState<AIModelConfigType[]>([]);
  const [isEditing, setIsEditing] = useState(false);
  const [editingConfig, setEditingConfig] = useState<Partial<AIModelConfigType> | null>(null);
  const [testingConfigs, setTestingConfigs] = useState<Record<string, boolean>>({});
  const [testResults, setTestResults] = useState<Record<string, { success: boolean; message: string } | null>>({});
  const [expandedIds, setExpandedIds] = useState<Set<string>>(new Set());
  const notification = useNotification();
  
  const [showAdvancedSettings, setShowAdvancedSettings] = useState(false);
  
  
  const [creationMode, setCreationMode] = useState<'selection' | 'form' | null>(null);
  
  const [selectedProviderId, setSelectedProviderId] = useState<string | null>(null);
  const [proxyConfig, setProxyConfig] = useState<ProxyConfig>({
    enabled: false,
    url: '',
    username: '',
    password: ''
  });
  const [isProxySaving, setIsProxySaving] = useState(false);

  
  useEffect(() => {
    loadConfig();
  }, []);

  const loadConfig = async () => {
    try {
      const models = await configManager.getConfig<AIModelConfigType[]>('ai.models') || [];
      const proxy = await configManager.getConfig<ProxyConfig>('ai.proxy');
      setAiModels(models);
      if (proxy) {
        setProxyConfig(proxy);
      }
    } catch (error) {
      log.error('Failed to load AI config', error);
    }
  };
  
  // Provider options with translations (must be at top level, before any conditional returns)
  const providerOrder = ['zhipu', 'qwen', 'deepseek', 'volcengine', 'minimax', 'moonshot', 'anthropic'];
  const providers = useMemo(() => {
    const sorted = Object.values(PROVIDER_TEMPLATES).sort((a, b) => {
      const indexA = providerOrder.indexOf(a.id);
      const indexB = providerOrder.indexOf(b.id);
      return (indexA === -1 ? 999 : indexA) - (indexB === -1 ? 999 : indexB);
    });
    
    // Dynamically get translated name and description
    return sorted.map(provider => ({
      ...provider,
      name: t(`providers.${provider.id}.name`),
      description: t(`providers.${provider.id}.description`)
    }));
  }, [t]);

  // Current template with translations (must be at top level, before any conditional returns)
  const currentTemplate = useMemo(() => {
    if (!selectedProviderId) return null;
    const template = PROVIDER_TEMPLATES[selectedProviderId];
    if (!template) return null;
    // Dynamically get translated name, description, and baseUrlOptions notes
    return {
      ...template,
      name: t(`providers.${template.id}.name`),
      description: t(`providers.${template.id}.description`),
      baseUrlOptions: template.baseUrlOptions?.map(opt => ({
        ...opt,
        note: t(`providers.${template.id}.urlOptions.${opt.note}`, { defaultValue: opt.note })
      }))
    };
  }, [selectedProviderId, t]);

  
  const handleCreateNew = () => {
    setSelectedProviderId(null);
    setCreationMode('selection');
  };

  
  const handleSelectProvider = (providerId: string) => {
    const template = PROVIDER_TEMPLATES[providerId];
    if (!template) return;
    
    const defaultModel = template.models[0] || '';
    setSelectedProviderId(providerId);
    
    // Dynamically get translated name
    const providerName = t(`providers.${template.id}.name`);
    
    setEditingConfig({
      name: defaultModel ? `${providerName} - ${defaultModel}` : '',
      base_url: template.baseUrl,
      request_url: resolveRequestUrl(template.baseUrl, template.format),
      api_key: '',
      model_name: defaultModel,
      provider: template.format,  
      enabled: true,
      context_window: 128000,
      max_tokens: 8192,
      category: 'general_chat',
      capabilities: ['text_chat', 'function_calling'],
      recommended_for: [],
      metadata: {}
    });
    setShowAdvancedSettings(false);
    setCreationMode('form');
    setIsEditing(true);
  };

  
  const handleSelectCustom = () => {
    setSelectedProviderId(null);
    setEditingConfig({
      name: '',
      base_url: 'https://open.bigmodel.cn/api/paas/v4',
      request_url: resolveRequestUrl('https://open.bigmodel.cn/api/paas/v4', 'openai'),
      api_key: '',
      model_name: '',
      provider: 'openai',  
      enabled: true,
      context_window: 128000,
      max_tokens: 8192,  
      
      category: 'general_chat',
      capabilities: ['text_chat'],
      recommended_for: [],
      metadata: {}
    });
    setShowAdvancedSettings(false);  
    setCreationMode('form');
    setIsEditing(true);
  };

  
  const handleBackToSelection = () => {
    setCreationMode('selection');
    setIsEditing(false);
    setEditingConfig(null);
  };

  const handleEdit = (config: AIModelConfigType) => {
    setEditingConfig({ ...config });
    
    const hasCustomHeaders = !!config.custom_headers && Object.keys(config.custom_headers).length > 0;
    const hasCustomBody = !!config.custom_request_body && config.custom_request_body.trim() !== '';
    setShowAdvancedSettings(hasCustomHeaders || hasCustomBody || !!config.skip_ssl_verify);
    setIsEditing(true);
  };

  const handleSave = async () => {
    
    if (!editingConfig || !editingConfig.name || !editingConfig.base_url) {
      notification.warning(t('messages.fillRequired'));
      return;
    }
    
    
    if (editingConfig.category !== 'search_enhanced' && !editingConfig.model_name) {
      notification.warning(t('messages.fillModelName'));
      return;
    }

    try {
      const newConfig: AIModelConfigType = {
        id: editingConfig.id || `model_${Date.now()}`,
        name: editingConfig.name,
        base_url: editingConfig.base_url,
        request_url: editingConfig.request_url || resolveRequestUrl(editingConfig.base_url, editingConfig.provider || 'openai'),
        api_key: editingConfig.api_key || '',
        model_name: editingConfig.model_name || 'search-api', 
        provider: editingConfig.provider || 'openai',
        enabled: editingConfig.enabled ?? true,
        description: editingConfig.description,
        context_window: editingConfig.context_window || 128000,
        
        max_tokens: editingConfig.category === 'multimodal' ? undefined : (editingConfig.max_tokens || 8192),
        
        category: editingConfig.category || 'general_chat',
        capabilities: editingConfig.capabilities || ['text_chat'],
        recommended_for: editingConfig.recommended_for || [],
        metadata: editingConfig.metadata,
        
        enable_thinking_process: editingConfig.enable_thinking_process ?? false,
        
        support_preserved_thinking: editingConfig.support_preserved_thinking ?? false,
        
        custom_headers: editingConfig.custom_headers,
        
        custom_headers_mode: editingConfig.custom_headers_mode,
        
        skip_ssl_verify: editingConfig.skip_ssl_verify ?? false,
        
        custom_request_body: editingConfig.custom_request_body
      };

      let updatedModels: AIModelConfigType[];
      if (editingConfig.id) {
        updatedModels = aiModels.map(m => m.id === editingConfig.id ? newConfig : m);
      } else {
        updatedModels = [...aiModels, newConfig];
      }

      
      await configManager.setConfig('ai.models', updatedModels);
      setAiModels(updatedModels);

      // Auto-set as primary model if no primary model is configured and this is a new model
      if (!editingConfig.id) {
        try {
          const currentDefaultModels = await configManager.getConfig<Record<string, unknown>>('ai.default_models') || {};
          const primaryModelExists = currentDefaultModels.primary && updatedModels.some(m => m.id === currentDefaultModels.primary);
          if (!primaryModelExists) {
            await configManager.setConfig('ai.default_models', {
              ...currentDefaultModels,
              primary: newConfig.id,
            });
            log.info('Auto-set primary model for first configured model', { modelId: newConfig.id });
            notification.success(t('messages.autoSetPrimary'));
          }
        } catch (error) {
          log.warn('Failed to auto-set primary model', { error });
        }
      }
      
      
      const configId = newConfig.id;
      if (!configId) {
        
        setIsEditing(false);
        setEditingConfig(null);
        setCreationMode(null);
        setSelectedProviderId(null);
        return;
      }
      
      setIsEditing(false);
      setEditingConfig(null);
      setCreationMode(null);
      setSelectedProviderId(null);
      
      
      setExpandedIds(prev => new Set([...prev, configId]));
      
      
      
      (async () => {
        
        setTestingConfigs(prev => ({ ...prev, [configId]: true }));
        setTestResults(prev => ({ ...prev, [configId]: null }));
        
        try {
          
          const result = await aiApi.testAIConfigConnection(newConfig);
          
          
          const baseMessage = result.success ? t('messages.testSuccess') : t('messages.testFailed');
          let message = baseMessage + (result.response_time_ms ? ` (${result.response_time_ms}ms)` : '');
          
          if (!result.success && result.error_details) {
            message += `\n${t('messages.errorDetails')}: ${result.error_details}`;
          }
          
          setTestResults(prev => ({
            ...prev,
            [configId]: { 
              success: result.success, 
              message
            }
          }));
        } catch (error) {
          
          const message = `${t('messages.testFailed')}\n${t('messages.errorDetails')}: ${error}`;
          setTestResults(prev => ({
            ...prev,
            [configId]: { success: false, message }
          }));
          log.warn('Auto test failed after save', { configId, error });
        } finally {
          setTestingConfigs(prev => ({ ...prev, [configId]: false }));
        }
      })();
    } catch (error) {
      log.error('Failed to save config', error);
      notification.error(t('messages.saveFailed'));
    }
  };

  const handleDelete = async (id: string) => {
    
    if (!(await confirm(t('confirmDelete')))) return;

    try {
      const updatedModels = aiModels.filter(m => m.id !== id);
      await configManager.setConfig('ai.models', updatedModels);
      setAiModels(updatedModels);
    } catch (error) {
      log.error('Failed to delete config', { configId: id, error });
    }
  };

  const toggleExpanded = (id: string) => {
    setExpandedIds(prev => {
      const next = new Set(prev);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      return next;
    });
  };

  const handleTest = async (config: AIModelConfigType) => {
    if (!config.id) return;
    
    const configId = config.id;
    setTestingConfigs(prev => ({ ...prev, [configId]: true }));
    setTestResults(prev => ({ ...prev, [configId]: null }));

    try {
      
      const result = await aiApi.testAIConfigConnection(config);
      
      
      const baseMessage = result.success ? t('messages.testSuccess') : t('messages.testFailed');
      let message = baseMessage + (result.response_time_ms ? ` (${result.response_time_ms}ms)` : '');
      
      if (!result.success && result.error_details) {
        message += `\n${t('messages.errorDetails')}: ${result.error_details}`;
      }
      
      setTestResults(prev => ({
        ...prev,
        [configId]: { 
          success: result.success, 
          message
        }
      }));
    } catch (error) {
      const message = `${t('messages.testFailed')}\n${t('messages.errorDetails')}: ${error}`;
      setTestResults(prev => ({
        ...prev,
        [configId]: { success: false, message }
      }));
    } finally {
      setTestingConfigs(prev => ({ ...prev, [configId]: false }));
    }
  };

  const handleToggleEnabled = async (config: AIModelConfigType, enabled: boolean) => {
    if (!config.id) return;

    try {
      const updatedModels = aiModels.map(model =>
        model.id === config.id ? { ...model, enabled } : model
      );
      await configManager.setConfig('ai.models', updatedModels);
      setAiModels(updatedModels);
    } catch (error) {
      log.error('Failed to toggle model status', { configId: config.id, enabled, error });
      notification.error(t('messages.saveFailed'));
    }
  };

  
  const handleSaveProxy = async () => {
    setIsProxySaving(true);
    try {
      await configManager.setConfig('ai.proxy', proxyConfig);
      notification.success(t('proxy.saveSuccess'));
    } catch (error) {
      log.error('Failed to save proxy config', error);
      notification.error(t('messages.saveFailed'));
    } finally {
      setIsProxySaving(false);
    }
  };

  const closeEditingModal = () => {
    setIsEditing(false);
    setEditingConfig(null);
    setCreationMode(null);
    setSelectedProviderId(null);
  };

  
  if (creationMode === 'selection') {
    return (
      <ConfigPageLayout className="bitfun-ai-model-config">
        <ConfigPageHeader
          title={t('providerSelection.title')}
          subtitle={t('providerSelection.subtitle')}
        />

        <ConfigPageContent className="bitfun-ai-model-config__content bitfun-ai-model-config__content--selection">
          <div className="bitfun-ai-model-config__provider-selection">
            
            <Card
              variant="default"
              padding="medium"
              interactive
              className="bitfun-ai-model-config__custom-option"
              onClick={handleSelectCustom}
            >
              <div className="bitfun-ai-model-config__custom-option-content">
                <Settings size={24} />
                <div>
                  <div className="bitfun-ai-model-config__custom-option-title">{t('providerSelection.customTitle')}</div>
                  <div className="bitfun-ai-model-config__custom-option-description">{t('providerSelection.customDescription')}</div>
                </div>
              </div>
            </Card>

            
            <div className="bitfun-ai-model-config__selection-divider">
              <span>{t('providerSelection.orSelectProvider')}</span>
            </div>

            
            <div className="bitfun-ai-model-config__provider-grid">
              {providers.map(provider => (
                <Card
                  key={provider.id}
                  variant="default"
                  padding="medium"
                  interactive
                  className="bitfun-ai-model-config__provider-card"
                  onClick={() => handleSelectProvider(provider.id)}
                >
                  <div className="bitfun-ai-model-config__provider-card-content">
                    <div className="bitfun-ai-model-config__provider-name">{provider.name}</div>
                    <div className="bitfun-ai-model-config__provider-description">{provider.description}</div>
                    <div className="bitfun-ai-model-config__provider-models">
                      {provider.models.slice(0, 3).map(model => (
                        <span key={model} className="bitfun-ai-model-config__provider-model-tag">{model}</span>
                      ))}
                      {provider.models.length > 3 && (
                        <span className="bitfun-ai-model-config__provider-model-tag bitfun-ai-model-config__provider-model-tag--more">
                          +{provider.models.length - 3}
                        </span>
                      )}
                    </div>
                    {provider.helpUrl && (
                      <a
                        href={provider.helpUrl}
                        target="_blank"
                        rel="noopener noreferrer"
                        className="bitfun-ai-model-config__provider-help-link"
                        onClick={async (e) => {
                          e.preventDefault();
                          e.stopPropagation();
                          try {
                            await systemAPI.openExternal(provider.helpUrl!);
                          } catch (error) {
                            console.error('[AIModelConfig] Failed to open external URL:', error);
                          }
                        }}
                      >
                        <ExternalLink size={12} />
                        {t('providerSelection.getApiKey')}
                      </a>
                    )}
                  </div>
                </Card>
              ))}
            </div>

            
            <div className="bitfun-ai-model-config__selection-actions">
              <Button variant="secondary" onClick={() => setCreationMode(null)}>
                {t('actions.cancel')}
              </Button>
            </div>
          </div>
        </ConfigPageContent>
      </ConfigPageLayout>
    );
  }

  
  const renderEditingForm = () => {
    if (!isEditing || !editingConfig) return null;
    const isFromTemplate = !editingConfig.id && !!currentTemplate;

    const handleCategoryChange = (value: string | number | (string | number)[]) => {
      const category = value as ModelCategory;
      setEditingConfig(prev => {
        let defaultCapabilities: ModelCapability[] = ['text_chat'];
        const updates: Partial<AIModelConfigType> = { category, capabilities: defaultCapabilities };
        switch (category) {
          case 'general_chat':
            defaultCapabilities = ['text_chat', 'function_calling'];
            updates.base_url = 'https://open.bigmodel.cn/api/paas/v4/chat/completions';
            updates.request_url = resolveRequestUrl(updates.base_url!, prev?.provider || 'openai');
            break;
          case 'multimodal':
            defaultCapabilities = ['text_chat', 'image_understanding', 'function_calling'];
            updates.base_url = 'https://open.bigmodel.cn/api/paas/v4/chat/completions';
            updates.request_url = resolveRequestUrl(updates.base_url!, prev?.provider || 'openai');
            break;
          case 'image_generation':
            defaultCapabilities = ['image_generation'];
            updates.base_url = 'https://open.bigmodel.cn/api/paas/v4/images/generations';
            updates.request_url = resolveRequestUrl(updates.base_url!, prev?.provider || 'openai');
            break;
          case 'search_enhanced':
            defaultCapabilities = ['search'];
            updates.base_url = 'https://open.bigmodel.cn/api/paas/v4/web_search';
            updates.request_url = resolveRequestUrl(updates.base_url!, 'openai');
            updates.model_name = 'search-api';
            updates.provider = 'openai';
            updates.context_window = 128000;
            updates.max_tokens = 8192;
            break;
          case 'speech_recognition':
            defaultCapabilities = ['speech_recognition'];
            updates.base_url = 'https://open.bigmodel.cn/api/paas/v4/chat/completions';
            updates.request_url = resolveRequestUrl(updates.base_url!, prev?.provider || 'openai');
            break;
        }
        updates.capabilities = defaultCapabilities;
        return { ...prev, ...updates };
      });
    };

    return (
      <>
        {!editingConfig.id && (
          <button className="bitfun-ai-model-config__back-button" onClick={handleBackToSelection}>
            <ArrowLeft size={16} />
            {t('providerSelection.backToSelection')}
          </button>
        )}

        <div className="bitfun-ai-model-config__form bitfun-ai-model-config__form--modal">
          <ConfigPageSection
            title={t('editSubtitle', { defaultValue: '基础模型参数配置' })}
            className="bitfun-ai-model-config__edit-section"
          >
            {isFromTemplate ? (
              <>
                <ConfigPageRow label={`${t('form.modelName')} *`} align="center" wide>
                  <Select
                    value={editingConfig.model_name || ''}
                    onChange={(value) => {
                      const newModelName = value as string;
                      setEditingConfig(prev => {
                        const oldAutoName = prev?.model_name ? `${currentTemplate?.name} - ${prev.model_name}` : '';
                        const isAutoGenerated = !prev?.name || prev.name === oldAutoName;
                        return {
                          ...prev,
                          model_name: newModelName,
                          name: isAutoGenerated && currentTemplate ? `${currentTemplate.name} - ${newModelName}` : prev?.name
                        };
                      });
                    }}
                    placeholder={t('providerSelection.selectModel')}
                    options={(currentTemplate?.models || []).map(model => ({ label: model, value: model }))}
                    searchable
                    allowCustomValue
                    searchPlaceholder={t('providerSelection.inputModelName')}
                    customValueHint={t('providerSelection.useCustomModel')}
                  />
                </ConfigPageRow>
                <ConfigPageRow label={`${t('form.configName')} *`} align="center" wide>
                  <Input value={editingConfig.name || ''} onChange={(e) => setEditingConfig(prev => ({ ...prev, name: e.target.value }))} placeholder={t('form.configNamePlaceholder')} inputSize="small" />
                </ConfigPageRow>
                <ConfigPageRow label={`${t('form.apiKey')} *`} align="center" wide>
                  <Input type="password" value={editingConfig.api_key || ''} onChange={(e) => setEditingConfig(prev => ({ ...prev, api_key: e.target.value }))} placeholder={t('form.apiKeyPlaceholder')} inputSize="small" />
                </ConfigPageRow>
                <ConfigPageRow label={t('form.baseUrl')} align="center" wide>
                  {currentTemplate?.baseUrlOptions && currentTemplate.baseUrlOptions.length > 0 ? (
                    <Select
                      value={editingConfig.base_url || currentTemplate.baseUrl}
                      onChange={(value) => {
                        const selectedOption = currentTemplate.baseUrlOptions!.find(opt => opt.url === value);
                        const newProvider = selectedOption?.format || editingConfig.provider || 'openai';
                        setEditingConfig(prev => ({
                          ...prev,
                          base_url: value as string,
                          request_url: resolveRequestUrl(value as string, newProvider),
                          provider: newProvider
                        }));
                      }}
                      placeholder={t('form.baseUrl')}
                      options={currentTemplate.baseUrlOptions.map(opt => ({ label: opt.url, value: opt.url, description: `${opt.format.toUpperCase()} · ${opt.note}` }))}
                    />
                  ) : (
                    <div className="bitfun-ai-model-config__control-stack">
                      <Input
                        type="url"
                        value={editingConfig.base_url || ''}
                        onChange={(e) => setEditingConfig(prev => ({
                          ...prev,
                          base_url: e.target.value,
                          request_url: resolveRequestUrl(e.target.value, prev?.provider || 'openai')
                        }))}
                        onFocus={(e) => e.target.select()}
                        placeholder={currentTemplate?.baseUrl}
                        inputSize="small"
                      />
                      {editingConfig.base_url && (
                        <div className="bitfun-ai-model-config__resolved-url">
                          <span className="resolved-url__label">{t('form.resolvedUrlLabel')}</span>
                          <code className="resolved-url__value">
                            {resolveRequestUrl(editingConfig.base_url, editingConfig.provider || 'openai')}
                          </code>
                          <small className="resolved-url__hint">{t('form.forceUrlHint')}</small>
                        </div>
                      )}
                    </div>
                  )}
                </ConfigPageRow>
                <ConfigPageRow label={t('form.provider')} description={t('providerSelection.formatHint')} align="center">
                  <Select
                    value={editingConfig.provider || 'openai'}
                    onChange={(value) => setEditingConfig(prev => ({
                      ...prev,
                      provider: value as string,
                      request_url: resolveRequestUrl(prev?.base_url || '', value as string)
                    }))}
                    placeholder={t('form.providerPlaceholder')}
                    options={[{ label: 'OpenAI', value: 'openai' }, { label: 'Anthropic', value: 'anthropic' }]}
                  />
                </ConfigPageRow>
                <ConfigPageRow label={t('form.contextWindow')} description={t('form.contextWindowHint')} align="center">
                  <NumberInput value={editingConfig.context_window || 128000} onChange={(v) => setEditingConfig(prev => ({ ...prev, context_window: v }))} min={1000} max={2000000} step={1000} size="small" />
                </ConfigPageRow>
                <ConfigPageRow label={t('form.maxTokens')} description={t('form.maxTokensHint')} align="center">
                  <NumberInput value={editingConfig.max_tokens || 8192} onChange={(v) => setEditingConfig(prev => ({ ...prev, max_tokens: v }))} min={1000} max={1000000} step={1000} size="small" />
                </ConfigPageRow>
                <ConfigPageRow label={t('thinking.enable')} description={t('thinking.enableHint')} align="center">
                  <Switch checked={editingConfig.enable_thinking_process ?? false} onChange={(e) => setEditingConfig(prev => ({ ...prev, enable_thinking_process: e.target.checked }))} size="small" />
                </ConfigPageRow>
              </>
            ) : (
              <>
                <ConfigPageRow label={`${t('category.label')} *`} description={editingConfig.category ? t(`categoryHints.${editingConfig.category}`) : undefined} align="center">
                  <Select value={editingConfig.category || 'general_chat'} onChange={handleCategoryChange} placeholder={t('category.placeholder')} options={[
                    { label: t('category.general_chat'), value: 'general_chat' },
                    { label: t('category.multimodal'), value: 'multimodal' },
                    { label: t('category.image_generation'), value: 'image_generation' },
                    { label: t('category.search_enhanced'), value: 'search_enhanced' },
                    { label: t('category.speech_recognition'), value: 'speech_recognition' },
                  ]} />
                </ConfigPageRow>
                <ConfigPageRow label={`${t('form.configName')} *`} align="center" wide>
                  <Input value={editingConfig.name || ''} onChange={(e) => setEditingConfig(prev => ({ ...prev, name: e.target.value }))} placeholder={t('form.configNamePlaceholder')} inputSize="small" />
                </ConfigPageRow>
                <ConfigPageRow label={`${t('form.baseUrl')} *`} description={editingConfig.category === 'search_enhanced' ? t('form.searchApiHint') : undefined} align="center" wide>
                  <div className="bitfun-ai-model-config__control-stack">
                    <Input
                      type="url"
                      value={editingConfig.base_url || ''}
                      onChange={(e) => setEditingConfig(prev => ({
                        ...prev,
                        base_url: e.target.value,
                        request_url: resolveRequestUrl(e.target.value, prev?.provider || 'openai')
                      }))}
                      onFocus={(e) => e.target.select()}
                      placeholder={editingConfig.category === 'search_enhanced' ? 'https://open.bigmodel.cn/api/paas/v4/web_search' : 'https://open.bigmodel.cn/api/paas/v4/chat/completions'}
                      inputSize="small"
                    />
                    {editingConfig.base_url && (
                      <div className="bitfun-ai-model-config__resolved-url">
                        <span className="resolved-url__label">{t('form.resolvedUrlLabel')}</span>
                        <code className="resolved-url__value">
                          {resolveRequestUrl(editingConfig.base_url, editingConfig.provider || 'openai')}
                        </code>
                        <small className="resolved-url__hint">{t('form.forceUrlHint')}</small>
                      </div>
                    )}
                  </div>
                </ConfigPageRow>
                <ConfigPageRow label={`${t('form.apiKey')} *`} align="center" wide>
                  <Input type="password" value={editingConfig.api_key || ''} onChange={(e) => setEditingConfig(prev => ({ ...prev, api_key: e.target.value }))} placeholder={t('form.apiKeyPlaceholder')} inputSize="small" />
                </ConfigPageRow>
              </>
            )}

            {!isFromTemplate && editingConfig.category !== 'search_enhanced' && (
              <>
                <ConfigPageRow label={`${t('form.modelName')} *`} description={editingConfig.category === 'speech_recognition' ? t('form.modelNameHint') : undefined} align="center" wide>
                  <Input value={editingConfig.model_name || ''} onChange={(e) => setEditingConfig(prev => ({ ...prev, model_name: e.target.value }))} placeholder={editingConfig.category === 'speech_recognition' ? 'glm-asr' : 'glm-4.7'} inputSize="small" />
                </ConfigPageRow>
                <ConfigPageRow label={t('form.provider')} align="center">
                  <Select value={editingConfig.provider || 'openai'} onChange={(value) => setEditingConfig(prev => ({ ...prev, provider: value as string }))} placeholder={t('form.providerPlaceholder')} options={[{ label: 'OpenAI', value: 'openai' }, { label: 'Anthropic', value: 'anthropic' }]} />
                </ConfigPageRow>
                {editingConfig.category !== 'speech_recognition' && (
                  <>
                    <ConfigPageRow label={t('form.contextWindow')} description={t('form.contextWindowHint')} align="center">
                      <NumberInput value={editingConfig.context_window || 128000} onChange={(v) => setEditingConfig(prev => ({ ...prev, context_window: v }))} min={1000} max={2000000} step={1000} size="small" />
                    </ConfigPageRow>
                    {editingConfig.category !== 'multimodal' && (
                      <ConfigPageRow label={t('form.maxTokens')} description={t('form.maxTokensHint')} align="center">
                        <NumberInput value={editingConfig.max_tokens || 65536} onChange={(v) => setEditingConfig(prev => ({ ...prev, max_tokens: v }))} min={1000} max={1000000} step={1000} size="small" />
                      </ConfigPageRow>
                    )}
                  </>
                )}
                <ConfigPageRow label={t('thinking.enable')} description={t('thinking.enableHint')} align="center">
                  <Switch checked={editingConfig.enable_thinking_process ?? false} onChange={(e) => setEditingConfig(prev => ({ ...prev, enable_thinking_process: e.target.checked }))} size="small" />
                </ConfigPageRow>
                <ConfigPageRow label={t('form.description')} multiline>
                  <Textarea value={editingConfig.description || ''} onChange={(e) => setEditingConfig(prev => ({ ...prev, description: e.target.value }))} placeholder={t('form.descriptionPlaceholder')} rows={2} />
                </ConfigPageRow>
              </>
            )}
          </ConfigPageSection>

          <ConfigPageSection
            title={t('advancedSettings.title')}
            className="bitfun-ai-model-config__edit-section"
          >
            <ConfigPageRow label={t('advancedSettings.title')} align="center">
              <Switch checked={showAdvancedSettings} onChange={(e) => setShowAdvancedSettings(e.target.checked)} size="small" />
            </ConfigPageRow>

            {showAdvancedSettings && (
              <>
                {editingConfig.enable_thinking_process && (
                  <ConfigPageRow label={t('thinking.preserve')} description={t('thinking.preserveHint')} align="center">
                    <Switch checked={editingConfig.support_preserved_thinking ?? false} onChange={(e) => setEditingConfig(prev => ({ ...prev, support_preserved_thinking: e.target.checked }))} size="small" />
                  </ConfigPageRow>
                )}
                <ConfigPageRow label={t('advancedSettings.skipSslVerify.label')} align="center">
                  <div className="bitfun-ai-model-config__row-control--stack">
                    <Checkbox label={t('advancedSettings.skipSslVerify.label')} checked={editingConfig.skip_ssl_verify || false} onChange={(e) => setEditingConfig(prev => ({ ...prev, skip_ssl_verify: e.target.checked }))} />
                    {editingConfig.skip_ssl_verify && (
                      <div className="bitfun-ai-model-config__warning">
                        <AlertTriangle size={16} />
                        <span>{t('advancedSettings.skipSslVerify.warning')}</span>
                      </div>
                    )}
                  </div>
                </ConfigPageRow>
                <ConfigPageRow label={t('advancedSettings.customHeaders.label')} description={t('advancedSettings.customHeaders.hint')} multiline>
                  <div className="bitfun-ai-model-config__row-control--stack">
                    <div className="bitfun-ai-model-config__header-mode">
                      <label>{t('advancedSettings.customHeaders.modeLabel')}</label>
                      <div>
                        <label className="bitfun-ai-model-config__radio-label">
                          <input type="radio" name="custom_headers_mode" value="merge" checked={(editingConfig.custom_headers_mode || 'merge') === 'merge'} onChange={() => setEditingConfig(prev => ({ ...prev, custom_headers_mode: 'merge' }))} />
                          <span>{t('advancedSettings.customHeaders.modeMerge')}</span>
                        </label>
                        <label className="bitfun-ai-model-config__radio-label">
                          <input type="radio" name="custom_headers_mode" value="replace" checked={editingConfig.custom_headers_mode === 'replace'} onChange={() => setEditingConfig(prev => ({ ...prev, custom_headers_mode: 'replace' }))} />
                          <span>{t('advancedSettings.customHeaders.modeReplace')}</span>
                        </label>
                      </div>
                      <small>{editingConfig.custom_headers_mode === 'replace' ? t('advancedSettings.customHeaders.modeReplaceHint') : t('advancedSettings.customHeaders.modeMergeHint')}</small>
                    </div>
                    <div className="bitfun-ai-model-config__custom-headers">
                      {Object.entries(editingConfig.custom_headers || {}).map(([key, value], index) => (
                        <div key={index} className="bitfun-ai-model-config__header-row">
                          <Input value={key} onChange={(e) => { const nh = { ...editingConfig.custom_headers }; const ov = nh[key]; delete nh[key]; if (e.target.value) nh[e.target.value] = ov; setEditingConfig(prev => ({ ...prev, custom_headers: nh })); }} placeholder={t('advancedSettings.customHeaders.keyPlaceholder')} inputSize="small" className="bitfun-ai-model-config__header-key" />
                          <Input value={value} onChange={(e) => { const nh = { ...editingConfig.custom_headers }; nh[key] = e.target.value; setEditingConfig(prev => ({ ...prev, custom_headers: nh })); }} placeholder={t('advancedSettings.customHeaders.valuePlaceholder')} inputSize="small" className="bitfun-ai-model-config__header-value" />
                          <IconButton variant="ghost" size="small" onClick={() => { const nh = { ...editingConfig.custom_headers }; delete nh[key]; setEditingConfig(prev => ({ ...prev, custom_headers: Object.keys(nh).length > 0 ? nh : undefined })); }} tooltip={t('actions.delete')}><X size={14} /></IconButton>
                        </div>
                      ))}
                      <Button variant="secondary" size="small" onClick={() => setEditingConfig(prev => ({ ...prev, custom_headers: { ...prev?.custom_headers, '': '' } }))} className="bitfun-ai-model-config__add-header-btn"><Plus size={14} />{t('advancedSettings.customHeaders.addHeader')}</Button>
                    </div>
                  </div>
                </ConfigPageRow>
                <ConfigPageRow label={t('advancedSettings.customRequestBody.label')} description={t('advancedSettings.customRequestBody.hint')} multiline>
                  <div className="bitfun-ai-model-config__row-control--stack">
                    <Textarea value={editingConfig.custom_request_body || ''} onChange={(e) => setEditingConfig(prev => ({ ...prev, custom_request_body: e.target.value }))} placeholder={t('advancedSettings.customRequestBody.placeholder')} rows={8} style={{ fontFamily: 'var(--font-family-mono)', fontSize: '13px' }} />
                    {editingConfig.custom_request_body && editingConfig.custom_request_body.trim() !== '' && (() => {
                      try { JSON.parse(editingConfig.custom_request_body); return <small className="bitfun-ai-model-config__json-status bitfun-ai-model-config__json-status--success">{t('advancedSettings.customRequestBody.validJson')}</small>; }
                      catch { return <small className="bitfun-ai-model-config__json-status bitfun-ai-model-config__json-status--error">{t('advancedSettings.customRequestBody.invalidJson')}</small>; }
                    })()}
                  </div>
                </ConfigPageRow>
              </>
            )}
          </ConfigPageSection>

          <div className="bitfun-ai-model-config__form-actions">
            <Button variant="secondary" onClick={closeEditingModal}>{t('actions.cancel')}</Button>
            <Button variant="primary" onClick={handleSave}>{t('actions.save')}</Button>
          </div>
        </div>
      </>
    );
  };

  
  return (
    <ConfigPageLayout className="bitfun-ai-model-config">
      <ConfigPageHeader
        title={t('title')}
        subtitle={t('subtitle')}
      />

      <ConfigPageContent className="bitfun-ai-model-config__content">
        <ConfigPageSection
          title={tDefault('tabs.default', { defaultValue: '默认模型分配' })}
          description={tDefault('subtitle', { defaultValue: '为不同能力场景指定默认模型。' })}
        >
          <DefaultModelConfig />
        </ConfigPageSection>

        <ConfigPageSection
          title={tDefault('tabs.models', { defaultValue: '已配置模型' })}
          description={t('subtitle')}
          extra={(
            <IconButton
              variant="primary"
              size="small"
              onClick={handleCreateNew}
              tooltip={t('actions.newConfig')}
            >
              <Plus size={16} />
            </IconButton>
          )}
        >
          {aiModels.length === 0 ? (
            <div className="bitfun-ai-model-config__empty">
              <Wifi size={36} />
              <p>{t('empty.noModels')}</p>
              <Button variant="primary" size="small" onClick={handleCreateNew}>
                <Plus size={14} />
                {t('actions.createFirst')}
              </Button>
            </div>
          ) : (
            <div className="bitfun-ai-model-config__collection">
              {aiModels.map(config => {
                const isExpanded = expandedIds.has(config.id || '');
                const testResult = config.id ? testResults[config.id] : null;
                const isTesting = config.id ? !!testingConfigs[config.id] : false;

                const badge = (
                  <>
                    <span className="bitfun-ai-model-config__meta-tag">
                      {t(`category.${config.category}`)}
                    </span>
                    <span className="bitfun-ai-model-config__meta-tag">
                      {config.provider}
                    </span>
                    {testResult && (
                      <span
                        className={`bitfun-ai-model-config__status-dot ${testResult.success ? 'is-success' : 'is-error'}`}
                        title={testResult.message}
                      />
                    )}
                  </>
                );

                const details = (
                  <div className="bitfun-ai-model-config__details">
                    <div className="bitfun-ai-model-config__details-section">
                      <div className="bitfun-ai-model-config__details-section-title">
                        {t('details.basicInfo', { defaultValue: '基础信息' })}
                      </div>
                      <div className="bitfun-ai-model-config__details-grid">
                        <div className="bitfun-ai-model-config__details-item">
                          <span className="bitfun-ai-model-config__details-label">{t('details.modelName')}</span>
                          <span className="bitfun-ai-model-config__details-value">{config.model_name}</span>
                        </div>
                        <div className="bitfun-ai-model-config__details-item">
                          <span className="bitfun-ai-model-config__details-label">{t('details.provider')}</span>
                          <span className="bitfun-ai-model-config__details-value">{config.provider}</span>
                        </div>
                        <div className="bitfun-ai-model-config__details-item">
                          <span className="bitfun-ai-model-config__details-label">{t('details.contextWindow')}</span>
                          <span className="bitfun-ai-model-config__details-value">{config.context_window?.toLocaleString() || '128,000'}</span>
                        </div>
                        <div className="bitfun-ai-model-config__details-item">
                          <span className="bitfun-ai-model-config__details-label">{t('details.maxOutput')}</span>
                          <span className="bitfun-ai-model-config__details-value">{config.max_tokens?.toLocaleString() || '-'}</span>
                        </div>
                        <div className="bitfun-ai-model-config__details-item bitfun-ai-model-config__details-item--wide">
                          <span className="bitfun-ai-model-config__details-label">{t('details.apiUrl')}</span>
                          <span className="bitfun-ai-model-config__details-value">{config.base_url}</span>
                        </div>
                        {config.capabilities && config.capabilities.length > 0 && (
                          <div className="bitfun-ai-model-config__details-item bitfun-ai-model-config__details-item--wide">
                            <span className="bitfun-ai-model-config__details-label">{t('details.capabilities', { defaultValue: '能力' })}</span>
                            <div className="bitfun-ai-model-config__details-tags">
                              {config.capabilities.map(capability => (
                                <span key={capability} className="bitfun-ai-model-config__details-tag">
                                  {t(`capabilities.${capability}`, { defaultValue: capability })}
                                </span>
                              ))}
                            </div>
                          </div>
                        )}
                        {config.description && (
                          <div className="bitfun-ai-model-config__details-item bitfun-ai-model-config__details-item--wide">
                            <span className="bitfun-ai-model-config__details-label">{t('details.description')}</span>
                            <span className="bitfun-ai-model-config__details-value bitfun-ai-model-config__details-value--text">
                              {config.description}
                            </span>
                          </div>
                        )}
                      </div>
                    </div>
                    {testResult && (
                      <div className="bitfun-ai-model-config__details-section">
                        <div className="bitfun-ai-model-config__details-section-title">
                          {t('actions.test')}
                        </div>
                        <div className={`bitfun-ai-model-config__test-result ${testResult.success ? 'success' : 'error'}`}>
                          {testResult.message}
                        </div>
                      </div>
                    )}
                  </div>
                );

                const control = (
                  <>
                    <Switch
                      checked={config.enabled}
                      onChange={(e) => {
                        void handleToggleEnabled(config, e.target.checked);
                      }}
                      size="small"
                    />
                    <button
                      type="button"
                      className="bitfun-collection-btn"
                      onClick={() => void handleTest(config)}
                      disabled={isTesting}
                      title={t('actions.test')}
                    >
                      {isTesting ? <Loader size={14} className="spinning" /> : <Wifi size={14} />}
                    </button>
                    <button
                      type="button"
                      className="bitfun-collection-btn"
                      onClick={() => handleEdit(config)}
                      title={t('actions.edit')}
                    >
                      <Edit2 size={14} />
                    </button>
                    <button
                      type="button"
                      className="bitfun-collection-btn bitfun-collection-btn--danger"
                      onClick={() => void handleDelete(config.id!)}
                      title={t('actions.delete')}
                    >
                      <Trash2 size={14} />
                    </button>
                  </>
                );

                return (
                  <ConfigCollectionItem
                    key={config.id}
                    label={config.name}
                    badge={badge}
                    control={control}
                    details={details}
                    expanded={isExpanded}
                    onToggle={() => config.id && toggleExpanded(config.id)}
                    disabled={!config.enabled}
                  />
                );
              })}
            </div>
          )}
        </ConfigPageSection>

        <ConfigPageSection
          title={tDefault('tabs.proxy', { defaultValue: '网络代理' })}
          description={t('proxy.enableHint')}
          extra={(
            <Button
              variant="primary"
              size="small"
              onClick={handleSaveProxy}
              disabled={isProxySaving || (proxyConfig.enabled && !proxyConfig.url)}
            >
              {isProxySaving ? <Loader size={16} className="spinning" /> : t('proxy.save')}
            </Button>
          )}
        >
          <ConfigPageRow label={t('proxy.enable')} align="center">
            <Switch
              checked={proxyConfig.enabled}
              onChange={(e) => setProxyConfig(prev => ({ ...prev, enabled: e.target.checked }))}
              size="small"
            />
          </ConfigPageRow>
          <ConfigPageRow label={t('proxy.url')} description={t('proxy.urlHint')} align="center">
            <Input
              value={proxyConfig.url}
              onChange={(e) => setProxyConfig(prev => ({ ...prev, url: e.target.value }))}
              placeholder={t('proxy.urlPlaceholder')}
              disabled={!proxyConfig.enabled}
              inputSize="small"
            />
          </ConfigPageRow>
          <ConfigPageRow label={t('proxy.username')} align="center">
            <Input
              value={proxyConfig.username || ''}
              onChange={(e) => setProxyConfig(prev => ({ ...prev, username: e.target.value }))}
              placeholder={t('proxy.usernamePlaceholder')}
              disabled={!proxyConfig.enabled}
              inputSize="small"
            />
          </ConfigPageRow>
          <ConfigPageRow label={t('proxy.password')} align="center">
            <Input
              type="password"
              value={proxyConfig.password || ''}
              onChange={(e) => setProxyConfig(prev => ({ ...prev, password: e.target.value }))}
              placeholder={t('proxy.passwordPlaceholder')}
              disabled={!proxyConfig.enabled}
              inputSize="small"
            />
          </ConfigPageRow>
        </ConfigPageSection>
      </ConfigPageContent>

      <Modal
        isOpen={isEditing && !!editingConfig}
        onClose={closeEditingModal}
        title={editingConfig?.id
          ? t('editModel')
          : (currentTemplate ? `${t('newModel')} - ${currentTemplate.name}` : t('newModel'))}
        size="large"
      >
        {renderEditingForm()}
      </Modal>
    </ConfigPageLayout>
  );
};

export default AIModelConfig;

