import React, { useState, useEffect, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { Plus, Edit2, Trash2, Wifi, Loader, Search, ChevronDown, ChevronUp, AlertTriangle, X, Settings, ArrowLeft, ExternalLink } from 'lucide-react';
import { Button, Switch, Select, IconButton, NumberInput, Card, CardBody, Checkbox } from '@/component-library';
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
import { ConfigPageHeader, ConfigPageLayout, ConfigPageContent } from './common';
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
  
  
  const [mainTab, setMainTab] = useState<'default' | 'models' | 'proxy'>('default');
  
  
  const [showAdvancedSettings, setShowAdvancedSettings] = useState(false);
  
  
  const [creationMode, setCreationMode] = useState<'selection' | 'form' | null>(null);
  
  const [selectedProviderId, setSelectedProviderId] = useState<string | null>(null);
  
  const [selectedCategoryTab, setSelectedCategoryTab] = useState<'all' | 'text' | 'multimodal' | 'other'>('all');
  const [searchQuery, setSearchQuery] = useState('');
  
  
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
  
  
  const filteredModels = useMemo(() => {
    let filtered = aiModels;
    
    
    if (selectedCategoryTab !== 'all') {
      if (selectedCategoryTab === 'text') {
        
        filtered = filtered.filter(m => 
          m.category === 'general_chat'
        );
      } else if (selectedCategoryTab === 'multimodal') {
        
        filtered = filtered.filter(m => 
          m.category === 'multimodal' || 
          m.category === 'image_generation' || 
          m.category === 'speech_recognition'
        );
      } else if (selectedCategoryTab === 'other') {
        
        filtered = filtered.filter(m => 
          m.category === 'search_enhanced'
        );
      }
    }
    
    
    if (searchQuery.trim()) {
      const query = searchQuery.toLowerCase();
      filtered = filtered.filter(m => 
        m.name.toLowerCase().includes(query) ||
        m.model_name.toLowerCase().includes(query) ||
        m.provider.toLowerCase().includes(query)
      );
    }
    
    return filtered;
  }, [aiModels, selectedCategoryTab, searchQuery]);

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

  const handleTest = async (config: AIModelConfigType, e: React.MouseEvent) => {
    e.stopPropagation(); 
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

  
  if (isEditing && editingConfig) {
    const isFromTemplate = !editingConfig.id && currentTemplate;
    
    return (
      <ConfigPageLayout className="bitfun-ai-model-config">
        <ConfigPageHeader
          title={editingConfig.id ? t('editModel') : (isFromTemplate ? `${t('newModel')} - ${currentTemplate.name}` : t('newModel'))}
          subtitle={t('editSubtitle')}
        />

        <ConfigPageContent className="bitfun-ai-model-config__content bitfun-ai-model-config__content--form">
          
          {!editingConfig.id && (
            <button 
              className="bitfun-ai-model-config__back-button"
              onClick={handleBackToSelection}
            >
              <ArrowLeft size={16} />
              {t('providerSelection.backToSelection')}
            </button>
          )}

          <div className="bitfun-ai-model-config__form">
            
            {isFromTemplate ? (
              <>
                
                <div className="bitfun-ai-model-config__form-field">
                  <label>{t('form.modelName')} *</label>
                  <Select
                    value={editingConfig.model_name || ''}
                    onChange={(value) => {
                      const newModelName = value as string;
                      setEditingConfig(prev => {
                        
                        const oldAutoName = prev?.model_name ? `${currentTemplate.name} - ${prev.model_name}` : '';
                        const isAutoGenerated = !prev?.name || prev.name === oldAutoName;
                        
                        return { 
                          ...prev, 
                          model_name: newModelName,
                          
                          name: isAutoGenerated ? `${currentTemplate.name} - ${newModelName}` : prev?.name
                        };
                      });
                    }}
                    placeholder={t('providerSelection.selectModel')}
                    options={currentTemplate.models.map(model => ({
                      label: model,
                      value: model
                    }))}
                    searchable
                    allowCustomValue
                    searchPlaceholder={t('providerSelection.inputModelName')}
                    customValueHint={t('providerSelection.useCustomModel')}
                  />
                </div>

                
                <div className="bitfun-ai-model-config__form-field">
                  <label>{t('form.configName')} *</label>
                  <input
                    type="text"
                    value={editingConfig.name || ''}
                    onChange={(e) => setEditingConfig(prev => ({ ...prev, name: e.target.value }))}
                    placeholder={t('form.configNamePlaceholder')}
                  />
                </div>

                
                <div className="bitfun-ai-model-config__form-field">
                  <label>{t('form.apiKey')} *</label>
                  <input
                    type="password"
                    value={editingConfig.api_key || ''}
                    onChange={(e) => setEditingConfig(prev => ({ ...prev, api_key: e.target.value }))}
                    placeholder={t('form.apiKeyPlaceholder')}
                  />
                  {currentTemplate.helpUrl && (
                    <small style={{ color: 'var(--color-text-secondary)', fontSize: '12px' }}>
                      <a 
                        href={currentTemplate.helpUrl} 
                        target="_blank" 
                        rel="noopener noreferrer"
                        style={{ color: 'var(--color-primary)', cursor: 'pointer' }}
                        onClick={async (e) => {
                          e.preventDefault();
                          try {
                            await systemAPI.openExternal(currentTemplate.helpUrl!);
                          } catch (error) {
                            console.error('[AIModelConfig] Failed to open external URL:', error);
                          }
                        }}
                      >
                        {t('providerSelection.getApiKey')}
                      </a>
                    </small>
                  )}
                </div>

                
                <div className="bitfun-ai-model-config__form-field">
                  <label>{t('form.baseUrl')}</label>
                  {currentTemplate.baseUrlOptions && currentTemplate.baseUrlOptions.length > 0 ? (
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
                      options={currentTemplate.baseUrlOptions.map(opt => ({
                        label: opt.url,
                        value: opt.url,
                        description: `${opt.format.toUpperCase()} · ${opt.note}`
                      }))}
                    />
                  ) : (
                    <input
                      type="url"
                      value={editingConfig.base_url || ''}
                      onChange={(e) => setEditingConfig(prev => ({
                        ...prev,
                        base_url: e.target.value,
                        request_url: resolveRequestUrl(e.target.value, prev?.provider || 'openai')
                      }))}
                      onFocus={(e) => e.target.select()}
                      placeholder={currentTemplate.baseUrl}
                    />
                  )}
                  {editingConfig.base_url && (
                    <div className="bitfun-ai-model-config__resolved-url">
                      <span className="resolved-url__label">{t('form.resolvedUrlLabel')}</span>
                      <code className="resolved-url__value">
                        {resolveRequestUrl(editingConfig.base_url, editingConfig.provider || 'openai')}
                      </code>
                      {!(currentTemplate.baseUrlOptions && currentTemplate.baseUrlOptions.length > 0) && (
                        <small className="resolved-url__hint">{t('form.forceUrlHint')}</small>
                      )}
                    </div>
                  )}
                </div>

                
                <div className="bitfun-ai-model-config__form-field">
                  <label>{t('form.provider')}</label>
                  <Select
                    value={editingConfig.provider || 'openai'}
                    onChange={(value) => setEditingConfig(prev => ({
                      ...prev,
                      provider: value as string,
                      request_url: resolveRequestUrl(prev?.base_url || '', value as string)
                    }))}
                    placeholder={t('form.providerPlaceholder')}
                    options={[
                      { label: 'OpenAI', value: 'openai' },
                      { label: 'Anthropic', value: 'anthropic' }
                    ]}
                  />
                  <small style={{ color: 'var(--color-text-secondary)', fontSize: '12px' }}>
                    {t('providerSelection.formatHint')}
                  </small>
                </div>

                {/* Context Window */}
                <div className="bitfun-ai-model-config__form-field">
                  <label>{t('form.contextWindow')}</label>
                  <NumberInput
                    value={editingConfig.context_window || 128000}
                    onChange={(v) => setEditingConfig(prev => ({ ...prev, context_window: v }))}
                    min={1000}
                    max={2000000}
                    step={1000}
                    size="small"
                  />
                  <small style={{ color: 'var(--color-text-secondary)', fontSize: '12px' }}>
                    {t('form.contextWindowHint')}
                  </small>
                </div>

                {/* Max Tokens */}
                <div className="bitfun-ai-model-config__form-field">
                  <label>{t('form.maxTokens')}</label>
                  <NumberInput
                    value={editingConfig.max_tokens || 8192}
                    onChange={(v) => setEditingConfig(prev => ({ ...prev, max_tokens: v }))}
                    min={1000}
                    max={1000000}
                    step={1000}
                    size="small"
                  />
                  <small style={{ color: 'var(--color-text-secondary)', fontSize: '12px' }}>
                    {t('form.maxTokensHint')}
                  </small>
                </div>

                {/* Enable Thinking */}
                <div className="bitfun-ai-model-config__form-field">
                  <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
                    <div>
                      <label style={{ marginBottom: '4px' }}>{t('thinking.enable')}</label>
                      <small style={{ display: 'block', color: 'var(--color-text-secondary)', fontSize: '12px' }}>
                        {t('thinking.enableHint')}
                      </small>
                    </div>
                    <Switch
                      checked={editingConfig.enable_thinking_process ?? false}
                      onChange={(e) => setEditingConfig(prev => ({ ...prev, enable_thinking_process: e.target.checked }))}
                    />
                  </div>
                </div>

              </>
            ) : (
              <>
                
                
                <div className="bitfun-ai-model-config__form-field">
                  <label>{t('category.label')} *</label>
                  <Select
                    value={editingConfig.category || 'general_chat'}
                    onChange={(value) => {
                      const category = value as ModelCategory;
                      setEditingConfig(prev => {
                        
                        let defaultCapabilities: ModelCapability[] = ['text_chat'];
                        let updates: Partial<AIModelConfigType> = { category, capabilities: defaultCapabilities };
                        
                        switch (category) {
                          case 'general_chat':
                            defaultCapabilities = ['text_chat', 'function_calling'];
                            updates.base_url = 'https://open.bigmodel.cn/api/paas/v4/chat/completions';
                            updates.request_url = resolveRequestUrl(updates.base_url, prev?.provider || 'openai');
                            break;
                          case 'multimodal':
                            defaultCapabilities = ['text_chat', 'image_understanding', 'function_calling'];
                            updates.base_url = 'https://open.bigmodel.cn/api/paas/v4/chat/completions';
                            updates.request_url = resolveRequestUrl(updates.base_url, prev?.provider || 'openai');
                            break;
                          case 'image_generation':
                            defaultCapabilities = ['image_generation'];
                            updates.base_url = 'https://open.bigmodel.cn/api/paas/v4/images/generations';
                            updates.request_url = resolveRequestUrl(updates.base_url, prev?.provider || 'openai');
                            break;
                          case 'search_enhanced':
                            defaultCapabilities = ['search'];
                            updates.base_url = 'https://open.bigmodel.cn/api/paas/v4/web_search';
                            updates.request_url = resolveRequestUrl(updates.base_url, 'openai');
                            updates.model_name = 'search-api';
                            updates.provider = 'openai';
                            updates.context_window = 128000;
                            updates.max_tokens = 8192;
                            break;
                          case 'speech_recognition':
                            defaultCapabilities = ['speech_recognition'];
                            updates.base_url = 'https://open.bigmodel.cn/api/paas/v4/chat/completions';
                            updates.request_url = resolveRequestUrl(updates.base_url, prev?.provider || 'openai');
                            break;
                        }
                        updates.capabilities = defaultCapabilities;
                        return { ...prev, ...updates };
                      });
                    }}
                    placeholder={t('category.placeholder')}
                    options={[
                      { label: t('category.general_chat'), value: 'general_chat' },
                      { label: t('category.multimodal'), value: 'multimodal' },
                      { label: t('category.image_generation'), value: 'image_generation' },
                      { label: t('category.search_enhanced'), value: 'search_enhanced' },
                      { label: t('category.speech_recognition'), value: 'speech_recognition' }
                    ]}
                  />
                  <small style={{ color: 'var(--color-text-secondary)', fontSize: '12px' }}>
                    {editingConfig.category && t(`categoryHints.${editingConfig.category}`)}
                  </small>
                </div>

                
                <div className="bitfun-ai-model-config__form-field">
                  <label>{t('form.configName')} *</label>
                  <input
                    type="text"
                    value={editingConfig.name || ''}
                    onChange={(e) => setEditingConfig(prev => ({ ...prev, name: e.target.value }))}
                    placeholder={t('form.configNamePlaceholder')}
                  />
                </div>

                
                <div className="bitfun-ai-model-config__form-field">
                  <label>{t('form.baseUrl')} *</label>
                  <input
                    type="url"
                    value={editingConfig.base_url || ''}
                    onChange={(e) => setEditingConfig(prev => ({
                      ...prev,
                      base_url: e.target.value,
                      request_url: resolveRequestUrl(e.target.value, prev?.provider || 'openai')
                    }))}
                    onFocus={(e) => e.target.select()}
                    placeholder={
                      editingConfig.category === 'search_enhanced'
                        ? 'https://open.bigmodel.cn/api/paas/v4/web_search'
                        : 'https://open.bigmodel.cn/api/paas/v4/chat/completions'
                    }
                  />
                  {editingConfig.category === 'search_enhanced' && (
                    <small style={{ color: 'var(--color-text-secondary)', fontSize: '12px' }}>
                      {t('form.searchApiHint')}
                    </small>
                  )}
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

                
                <div className="bitfun-ai-model-config__form-field">
                  <label>{t('form.apiKey')} *</label>
                  <input
                    type="password"
                    value={editingConfig.api_key || ''}
                    onChange={(e) => setEditingConfig(prev => ({ ...prev, api_key: e.target.value }))}
                    placeholder={t('form.apiKeyPlaceholder')}
                  />
                </div>
              </>
            )}

            
            {!isFromTemplate && editingConfig.category !== 'search_enhanced' && (
              <>
                <div className="bitfun-ai-model-config__form-field">
                  <label>{t('form.modelName')} *</label>
                  <input
                    type="text"
                    value={editingConfig.model_name || ''}
                    onChange={(e) => setEditingConfig(prev => ({ ...prev, model_name: e.target.value }))}
                    placeholder={editingConfig.category === 'speech_recognition' ? 'glm-asr' : 'glm-4.7'}
                  />
                  {editingConfig.category === 'speech_recognition' && (
                    <small style={{ color: 'var(--color-text-secondary)', fontSize: '12px' }}>
                      {t('form.modelNameHint')}
                    </small>
                  )}
                </div>

                <div className="bitfun-ai-model-config__form-field">
                  <label>{t('form.provider')}</label>
                  <Select
                    value={editingConfig.provider || 'openai'}
                    onChange={(value) => setEditingConfig(prev => ({ ...prev, provider: value as string }))}
                    placeholder={t('form.providerPlaceholder')}
                    options={[
                      { label: 'OpenAI', value: 'openai' },
                      { label: 'Anthropic', value: 'anthropic' }
                    ]}
                  />
                </div>

                
                {editingConfig.category !== 'speech_recognition' && (
                  <>
                    <div className="bitfun-ai-model-config__form-field">
                      <label>{t('form.contextWindow')}</label>
                      <NumberInput
                        value={editingConfig.context_window || 128000}
                        onChange={(v) => setEditingConfig(prev => ({ ...prev, context_window: v }))}
                        min={1000}
                        max={2000000}
                        step={1000}
                        size="small"
                      />
                      <small style={{ color: 'var(--color-text-secondary)', fontSize: '12px' }}>
                        {t('form.contextWindowHint')}
                      </small>
                    </div>

                    
                    {editingConfig.category !== 'multimodal' && (
                      <div className="bitfun-ai-model-config__form-field">
                        <label>{t('form.maxTokens')}</label>
                        <NumberInput
                          value={editingConfig.max_tokens || 65536}
                          onChange={(v) => setEditingConfig(prev => ({ ...prev, max_tokens: v }))}
                          min={1000}
                          max={1000000}
                          step={1000}
                          size="small"
                        />
                        <small style={{ color: 'var(--color-text-secondary)', fontSize: '12px' }}>
                          {t('form.maxTokensHint')}
                        </small>
                      </div>
                    )}
                  </>
                )}

                
                <div className="bitfun-ai-model-config__form-field">
                  <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
                    <div>
                      <label style={{ marginBottom: '4px' }}>{t('thinking.enable')}</label>
                      <small style={{ display: 'block', color: 'var(--color-text-secondary)', fontSize: '12px' }}>
                        {t('thinking.enableHint')}
                      </small>
                    </div>
                    <Switch
                      checked={editingConfig.enable_thinking_process ?? false}
                      onChange={(e) => setEditingConfig(prev => ({ ...prev, enable_thinking_process: e.target.checked }))}
                    />
                  </div>
                </div>

                
                <div className="bitfun-ai-model-config__form-field">
                  <label>{t('form.description')}</label>
                  <textarea
                    value={editingConfig.description || ''}
                    onChange={(e) => setEditingConfig(prev => ({ ...prev, description: e.target.value }))}
                    placeholder={t('form.descriptionPlaceholder')}
                    rows={2}
                  />
                </div>
              </>
            )}

            
            <div className="bitfun-ai-model-config__advanced-settings">
              <button
                type="button"
                className="bitfun-ai-model-config__advanced-toggle"
                onClick={() => setShowAdvancedSettings(!showAdvancedSettings)}
              >
                {showAdvancedSettings ? <ChevronUp size={16} /> : <ChevronDown size={16} />}
                <span>{t('advancedSettings.title')}</span>
              </button>
              
              {showAdvancedSettings && (
                <div className="bitfun-ai-model-config__advanced-content">
                  
                  {editingConfig.enable_thinking_process && (
                    <div className="bitfun-ai-model-config__form-field">
                      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
                        <div>
                          <label style={{ marginBottom: '4px' }}>{t('thinking.preserve')}</label>
                          <small style={{ display: 'block', color: 'var(--color-text-secondary)', fontSize: '12px' }}>
                            {t('thinking.preserveHint')}
                          </small>
                        </div>
                        <Switch
                          checked={editingConfig.support_preserved_thinking ?? false}
                          onChange={(e) => setEditingConfig(prev => ({ ...prev, support_preserved_thinking: e.target.checked }))}
                        />
                      </div>
                    </div>
                  )}

                  <div className="bitfun-ai-model-config__form-field">
                    <Checkbox
                      label={t('advancedSettings.skipSslVerify.label')}
                      checked={editingConfig.skip_ssl_verify || false}
                      onChange={(e) => setEditingConfig(prev => ({ ...prev, skip_ssl_verify: e.target.checked }))}
                    />
                    {editingConfig.skip_ssl_verify && (
                      <div className="bitfun-ai-model-config__warning">
                        <AlertTriangle size={16} />
                        <span>{t('advancedSettings.skipSslVerify.warning')}</span>
                      </div>
                    )}
                  </div>
                  
                  
                  <div className="bitfun-ai-model-config__form-field">
                    <label>{t('advancedSettings.customHeaders.label')}</label>
                    <small className="bitfun-ai-model-config__field-hint">
                      {t('advancedSettings.customHeaders.hint')}
                    </small>
                    
                    
                    <div className="bitfun-ai-model-config__header-mode">
                      <label>
                        {t('advancedSettings.customHeaders.modeLabel')}
                      </label>
                      <div>
                        <label className="bitfun-ai-model-config__radio-label">
                          <input
                            type="radio"
                            name="custom_headers_mode"
                            value="merge"
                            checked={(editingConfig.custom_headers_mode || 'merge') === 'merge'}
                            onChange={() => setEditingConfig(prev => ({ ...prev, custom_headers_mode: 'merge' }))}
                          />
                          <span>{t('advancedSettings.customHeaders.modeMerge')}</span>
                        </label>
                        <label className="bitfun-ai-model-config__radio-label">
                          <input
                            type="radio"
                            name="custom_headers_mode"
                            value="replace"
                            checked={editingConfig.custom_headers_mode === 'replace'}
                            onChange={() => setEditingConfig(prev => ({ ...prev, custom_headers_mode: 'replace' }))}
                          />
                          <span>{t('advancedSettings.customHeaders.modeReplace')}</span>
                        </label>
                      </div>
                      <small>
                        {editingConfig.custom_headers_mode === 'replace' 
                          ? t('advancedSettings.customHeaders.modeReplaceHint')
                          : t('advancedSettings.customHeaders.modeMergeHint')
                        }
                      </small>
                    </div>
                    
                    
                    <div className="bitfun-ai-model-config__custom-headers">
                      {Object.entries(editingConfig.custom_headers || {}).map(([key, value], index) => (
                        <div key={index} className="bitfun-ai-model-config__header-row">
                          <input
                            type="text"
                            value={key}
                            onChange={(e) => {
                              const newHeaders = { ...editingConfig.custom_headers };
                              const oldValue = newHeaders[key];
                              delete newHeaders[key];
                              if (e.target.value) {
                                newHeaders[e.target.value] = oldValue;
                              }
                              setEditingConfig(prev => ({ ...prev, custom_headers: newHeaders }));
                            }}
                            placeholder={t('advancedSettings.customHeaders.keyPlaceholder')}
                            className="bitfun-ai-model-config__header-key"
                          />
                          <input
                            type="text"
                            value={value}
                            onChange={(e) => {
                              const newHeaders = { ...editingConfig.custom_headers };
                              newHeaders[key] = e.target.value;
                              setEditingConfig(prev => ({ ...prev, custom_headers: newHeaders }));
                            }}
                            placeholder={t('advancedSettings.customHeaders.valuePlaceholder')}
                            className="bitfun-ai-model-config__header-value"
                          />
                          <IconButton
                            variant="ghost"
                            size="small"
                            onClick={() => {
                              const newHeaders = { ...editingConfig.custom_headers };
                              delete newHeaders[key];
                              setEditingConfig(prev => ({ 
                                ...prev, 
                                custom_headers: Object.keys(newHeaders).length > 0 ? newHeaders : undefined 
                              }));
                            }}
                            tooltip={t('actions.delete')}
                          >
                            <X size={14} />
                          </IconButton>
                        </div>
                      ))}
                      
                      
                      <Button
                        variant="secondary"
                        size="small"
                        onClick={() => {
                          const newHeaders = { ...editingConfig.custom_headers, '': '' };
                          setEditingConfig(prev => ({ ...prev, custom_headers: newHeaders }));
                        }}
                        className="bitfun-ai-model-config__add-header-btn"
                      >
                        <Plus size={14} />
                        {t('advancedSettings.customHeaders.addHeader')}
                      </Button>
                    </div>
                  </div>

                  
                  <div className="bitfun-ai-model-config__form-field">
                    <label>{t('advancedSettings.customRequestBody.label')}</label>
                    <small className="bitfun-ai-model-config__field-hint">
                      {t('advancedSettings.customRequestBody.hint')}
                    </small>
                    <small
                      className="bitfun-ai-model-config__field-hint"
                      style={{ color: 'var(--color-primary)', marginTop: '4px' }}
                    >
                      {t('advancedSettings.customRequestBody.mergeHint')}
                    </small>
                    <textarea
                      value={editingConfig.custom_request_body || ''}
                      onChange={(e) => {
                        setEditingConfig(prev => ({ ...prev, custom_request_body: e.target.value }));
                      }}
                      placeholder={t('advancedSettings.customRequestBody.placeholder')}
                      rows={8}
                      style={{
                        fontFamily: 'monospace',
                        fontSize: '13px',
                        lineHeight: '1.5',
                        padding: '12px',
                        border: '1px solid var(--color-border)',
                        borderRadius: '4px',
                        resize: 'vertical',
                        minHeight: '120px'
                      }}
                    />
                    
                    {editingConfig.custom_request_body && editingConfig.custom_request_body.trim() !== '' && (() => {
                      try {
                        JSON.parse(editingConfig.custom_request_body);
                        return (
                          <small style={{ color: 'var(--color-success)', fontSize: '12px', marginTop: '4px' }}>
                            {t('advancedSettings.customRequestBody.validJson')}
                          </small>
                        );
                      } catch {
                        return (
                          <small style={{ color: 'var(--color-error)', fontSize: '12px', marginTop: '4px' }}>
                            {t('advancedSettings.customRequestBody.invalidJson')}
                          </small>
                        );
                      }
                    })()}
                  </div>
                </div>
              )}
            </div>

            <div className="bitfun-ai-model-config__form-actions">
              <Button variant="secondary" onClick={() => { setIsEditing(false); setEditingConfig(null); setCreationMode(null); setSelectedProviderId(null); }}>
                {t('actions.cancel')}
              </Button>
              <Button variant="primary" onClick={handleSave}>
                {t('actions.save')}
              </Button>
            </div>
          </div>
        </ConfigPageContent>
      </ConfigPageLayout>
    );
  }

  
  return (
    <ConfigPageLayout className="bitfun-ai-model-config">
      <ConfigPageHeader
        title={t('title')}
        subtitle={t('subtitle')}
      />

      <ConfigPageContent className="bitfun-ai-model-config__content">
        
        <div className="bitfun-ai-model-config__tabs bitfun-ai-model-config__tabs--main">
          <button
            className={`bitfun-ai-model-config__tab ${mainTab === 'default' ? 'bitfun-ai-model-config__tab--active' : ''}`}
            onClick={() => setMainTab('default')}
          >
            <span>{tDefault('tabs.default')}</span>
          </button>
          <button
            className={`bitfun-ai-model-config__tab ${mainTab === 'models' ? 'bitfun-ai-model-config__tab--active' : ''}`}
            onClick={() => setMainTab('models')}
          >
            <span>{tDefault('tabs.models')}</span>
          </button>
          <button
            className={`bitfun-ai-model-config__tab ${mainTab === 'proxy' ? 'bitfun-ai-model-config__tab--active' : ''}`}
            onClick={() => setMainTab('proxy')}
          >
            <span>{tDefault('tabs.proxy')}</span>
          </button>
        </div>

        
        <div className="bitfun-ai-model-config__tab-content">
          
          {mainTab === 'default' && (
            <DefaultModelConfig />
          )}

          
          {mainTab === 'models' && (
            <>
              
        <div className="bitfun-ai-model-config__filters">
          {aiModels.length > 0 && (
            <div className="bitfun-ai-model-config__search">
              <Search size={18} />
              <input
                type="text"
                placeholder={t('search.placeholder')}
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
              />
            </div>
          )}
          <IconButton 
            variant="primary" 
            size="small"
            onClick={handleCreateNew}
            tooltip={t('actions.newConfig')}
          >
            <Plus size={16} />
          </IconButton>
        </div>

        
        {aiModels.length > 0 && (
          <div className="bitfun-ai-model-config__category-tabs">
            <button
              className={`bitfun-ai-model-config__category-tab ${selectedCategoryTab === 'all' ? 'bitfun-ai-model-config__category-tab--active' : ''}`}
              onClick={() => setSelectedCategoryTab('all')}
            >
              {t('categories.all')}
            </button>
            <button
              className={`bitfun-ai-model-config__category-tab ${selectedCategoryTab === 'text' ? 'bitfun-ai-model-config__category-tab--active' : ''}`}
              onClick={() => setSelectedCategoryTab('text')}
            >
              {t('categories.text')}
            </button>
            <button
              className={`bitfun-ai-model-config__category-tab ${selectedCategoryTab === 'multimodal' ? 'bitfun-ai-model-config__category-tab--active' : ''}`}
              onClick={() => setSelectedCategoryTab('multimodal')}
            >
              {t('categories.multimodal')}
            </button>
            <button
              className={`bitfun-ai-model-config__category-tab ${selectedCategoryTab === 'other' ? 'bitfun-ai-model-config__category-tab--active' : ''}`}
              onClick={() => setSelectedCategoryTab('other')}
            >
              {t('categories.other')}
            </button>
          </div>
        )}

        
        {aiModels.length === 0 ? (
          <div className="bitfun-ai-model-config__empty">
            <Wifi size={48} />
            <p>{t('empty.noModels')}</p>
            <Button variant="primary" onClick={handleCreateNew}>
              <Plus size={16} />
              {t('actions.createFirst')}
            </Button>
          </div>
        ) : filteredModels.length === 0 ? (
          <div className="bitfun-ai-model-config__empty">
            <Search size={48} />
            <p>{t('empty.noMatchingModels')}</p>
          </div>
        ) : (
          <div className="bitfun-ai-model-config__list">
            {filteredModels.map(config => {
              const isExpanded = expandedIds.has(config.id || '');

              return (
                <Card
                  key={config.id}
                  variant="default"
                  padding="none"
                  interactive
                  className={`bitfun-ai-model-config__item ${isExpanded ? 'is-expanded' : ''}`}
                >
                  
                  <div
                    className="bitfun-ai-model-config__item-header"
                    onClick={() => config.id && toggleExpanded(config.id)}
                  >
                    <div className="bitfun-ai-model-config__item-main">
                      <div className="bitfun-ai-model-config__item-icon">
                        <Wifi size={18} />
                      </div>
                      <div className="bitfun-ai-model-config__item-info">
                        <div className="bitfun-ai-model-config__item-name">{config.name}</div>
                        <div className="bitfun-ai-model-config__item-description">{config.model_name}</div>
                      </div>
                    </div>
                    <div className="bitfun-ai-model-config__item-badges">
                      
                      {config.capabilities?.slice(0, 3).map(capability => (
                        <span 
                          key={capability}
                          className={`bitfun-ai-model-config__capability-tag bitfun-ai-model-config__capability-tag--${capability}`}
                        >
                          {t(`capabilities.${capability}`)}
                        </span>
                      ))}
                      {(config.capabilities?.length || 0) > 3 && (
                        <span className="bitfun-ai-model-config__capability-tag bitfun-ai-model-config__capability-tag--more">
                          +{(config.capabilities?.length || 0) - 3}
                        </span>
                      )}
                    </div>
                  </div>

                  
                  {isExpanded && (
                    <CardBody className="bitfun-ai-model-config__item-details">
                      <div className="bitfun-ai-model-config__detail-section">
                        <div className="bitfun-ai-model-config__detail-label">{t('details.basicInfo')}</div>
                        <div className="bitfun-ai-model-config__properties">
                          <div className="bitfun-ai-model-config__property">
                            <span className="bitfun-ai-model-config__property-label">{t('details.modelName')}:</span>
                            <span className="bitfun-ai-model-config__property-value">{config.model_name}</span>
                          </div>
                          <div className="bitfun-ai-model-config__property">
                            <span className="bitfun-ai-model-config__property-label">{t('details.provider')}:</span>
                            <span className="bitfun-ai-model-config__property-value">{config.provider}</span>
                          </div>
                          <div className="bitfun-ai-model-config__property">
                            <span className="bitfun-ai-model-config__property-label">{t('details.apiUrl')}:</span>
                            <span className="bitfun-ai-model-config__property-value">{config.base_url}</span>
                          </div>
                          <div className="bitfun-ai-model-config__property">
                            <span className="bitfun-ai-model-config__property-label">{t('details.contextWindow')}:</span>
                            <span className="bitfun-ai-model-config__property-value">{config.context_window?.toLocaleString() || '128,000'} {t('details.tokens')}</span>
                          </div>
                          <div className="bitfun-ai-model-config__property">
                            <span className="bitfun-ai-model-config__property-label">{t('details.maxOutput')}:</span>
                            <span className="bitfun-ai-model-config__property-value">{config.max_tokens?.toLocaleString() || '65,536'} {t('details.tokens')}</span>
                          </div>
                          {config.description && (
                            <div className="bitfun-ai-model-config__property">
                              <span className="bitfun-ai-model-config__property-label">{t('details.description')}:</span>
                              <span className="bitfun-ai-model-config__property-value">{config.description}</span>
                            </div>
                          )}
                        </div>
                      </div>

                      
                      {config.id && testResults[config.id] && (
                        <div className={`bitfun-ai-model-config__test-result ${testResults[config.id]!.success ? 'success' : 'error'}`}>
                          {testResults[config.id]!.message}
                        </div>
                      )}

                      
                      <div className="bitfun-ai-model-config__item-actions">
                        <button
                          className="bitfun-ai-model-config__action-btn"
                          onClick={(e) => handleTest(config, e)}
                          disabled={config.id ? testingConfigs[config.id] : false}
                        >
                          {config.id && testingConfigs[config.id] ? (
                            <Loader size={16} className="spinning" />
                          ) : (
                            <Wifi size={16} />
                          )}
                          {t('actions.test')}
                        </button>
                        <button
                          className="bitfun-ai-model-config__action-btn"
                          onClick={(e) => {
                            e.stopPropagation();
                            handleEdit(config);
                          }}
                        >
                          <Edit2 size={16} />
                          {t('actions.edit')}
                        </button>
                        <button
                          className="bitfun-ai-model-config__action-btn bitfun-ai-model-config__action-btn--danger"
                          onClick={(e) => {
                            e.stopPropagation();
                            handleDelete(config.id!);
                          }}
                        >
                          <Trash2 size={16} />
                          {t('actions.delete')}
                        </button>
                      </div>
                    </CardBody>
                  )}
                </Card>
              );
            })}
          </div>
        )}
            </>
          )}

          
          {mainTab === 'proxy' && (
            <div className="bitfun-ai-model-config__proxy-panel">
              
              <div className="bitfun-ai-model-config__proxy-switch-wrapper">
                <Switch
                  checked={proxyConfig.enabled}
                  onChange={(e) => setProxyConfig(prev => ({ ...prev, enabled: e.target.checked }))}
                  label={t('proxy.enable')}
                  description={t('proxy.enableHint')}
                  size="medium"
                />
              </div>

              
              <div className="bitfun-ai-model-config__form-field">
                <label>{t('proxy.url')} *</label>
                <input
                  type="text"
                  value={proxyConfig.url}
                  onChange={(e) => setProxyConfig(prev => ({ ...prev, url: e.target.value }))}
                  placeholder={t('proxy.urlPlaceholder')}
                  disabled={!proxyConfig.enabled}
                />
                <small style={{ color: 'var(--color-text-secondary)', fontSize: '12px' }}>
                  {t('proxy.urlHint')}
                </small>
              </div>

              <div className="bitfun-ai-model-config__form-field">
                <label>{t('proxy.username')}</label>
                <input
                  type="text"
                  value={proxyConfig.username || ''}
                  onChange={(e) => setProxyConfig(prev => ({ ...prev, username: e.target.value }))}
                  placeholder={t('proxy.usernamePlaceholder')}
                  disabled={!proxyConfig.enabled}
                />
              </div>

              <div className="bitfun-ai-model-config__form-field">
                <label>{t('proxy.password')}</label>
                <input
                  type="password"
                  value={proxyConfig.password || ''}
                  onChange={(e) => setProxyConfig(prev => ({ ...prev, password: e.target.value }))}
                  placeholder={t('proxy.passwordPlaceholder')}
                  disabled={!proxyConfig.enabled}
                />
              </div>

              <div className="bitfun-ai-model-config__proxy-actions">
                <Button 
                  variant="primary" 
                  onClick={handleSaveProxy}
                  disabled={isProxySaving || (proxyConfig.enabled && !proxyConfig.url)}
                >
                  {isProxySaving ? <Loader size={16} className="spinning" /> : t('proxy.save')}
                </Button>
              </div>
            </div>
          )}
        </div>
      </ConfigPageContent>
    </ConfigPageLayout>
  );
};

export default AIModelConfig;

