import React, { useState, useEffect, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { Plus, Edit2, Trash2, Wifi, Loader, AlertTriangle, X, Settings, ExternalLink, BarChart3, Eye, EyeOff } from 'lucide-react';
import { Button, Switch, Select, IconButton, NumberInput, Card, Checkbox, Modal, Input, Textarea, type SelectOption } from '@/component-library';
import { 
  AIModelConfig as AIModelConfigType, 
  ProxyConfig, 
  ModelCategory,
  ModelCapability
} from '../types';
import { configManager } from '../services/ConfigManager';
import { PROVIDER_TEMPLATES, getModelDisplayName, getProviderDisplayName, getProviderTemplateId } from '../services/modelConfigs';
import { aiApi, systemAPI } from '@/infrastructure/api';
import { useNotification } from '@/shared/notification-system';
import { ConfigPageHeader, ConfigPageLayout, ConfigPageContent, ConfigPageSection, ConfigPageRow, ConfigCollectionItem } from './common';
import DefaultModelConfig from './DefaultModelConfig';
import TokenStatsModal from './TokenStatsModal';
import { createLogger } from '@/shared/utils/logger';
import './AIModelConfig.scss';

const log = createLogger('AIModelConfig');

interface RemoteModelOption {
  id: string;
  display_name?: string;
}

interface SelectedModelDraft {
  key: string;
  configId?: string;
  modelName: string;
  category: ModelCategory;
  contextWindow: number;
  maxTokens: number;
  enableThinking: boolean;
}

interface ProviderGroup {
  providerName: string;
  providerId?: string;
  models: AIModelConfigType[];
}

function isResponsesProvider(provider?: string): boolean {
  return provider === 'response' || provider === 'responses';
}

function createModelDraft(
  modelName: string,
  baseConfig?: Partial<AIModelConfigType>,
  overrides?: Partial<SelectedModelDraft>
): SelectedModelDraft {
  const trimmedModelName = modelName.trim();

  return {
    key: overrides?.key ?? overrides?.configId ?? baseConfig?.id ?? trimmedModelName,
    configId: overrides?.configId ?? baseConfig?.id,
    modelName: trimmedModelName,
    category: overrides?.category ?? baseConfig?.category ?? 'general_chat',
    contextWindow: overrides?.contextWindow ?? baseConfig?.context_window ?? 128000,
    maxTokens: overrides?.maxTokens ?? baseConfig?.max_tokens ?? 8192,
    enableThinking: overrides?.enableThinking ?? baseConfig?.enable_thinking_process ?? false,
  };
}

function uniqModelNames(modelNames: string[]): string[] {
  return Array.from(new Set(modelNames.map(name => name.trim()).filter(Boolean)));
}

function getCapabilitiesByCategory(category: ModelCategory): ModelCapability[] {
  switch (category) {
    case 'general_chat':
    default:
      return ['text_chat', 'function_calling'];
  }
}

/**
 * Compute the stored request URL from a base URL and provider format.
 * For gemini, stores the bare base (no /v1beta/models/... suffix) —
 * the backend dynamically appends /v1beta/models/{model}:streamGenerateContent?alt=sse.
 */
function resolveRequestUrl(baseUrl: string, provider: string, _modelName = ''): string {
  const trimmed = baseUrl.trim().replace(/\/+$/, '');
  if (trimmed.endsWith('#')) {
    return trimmed.slice(0, -1).replace(/\/+$/, '');
  }
  if (provider === 'openai') {
    return trimmed.endsWith('chat/completions') ? trimmed : `${trimmed}/chat/completions`;
  }
  if (isResponsesProvider(provider)) {
    return trimmed.endsWith('responses') ? trimmed : `${trimmed}/responses`;
  }
  if (provider === 'anthropic') {
    return trimmed.endsWith('v1/messages') ? trimmed : `${trimmed}/v1/messages`;
  }
  if (provider === 'gemini') {
    return geminiBaseUrl(trimmed);
  }
  return trimmed;
}

/** Strip /v1beta/models/... or /models/... suffix from a gemini URL to get the bare host+path root. */
function geminiBaseUrl(url: string): string {
  return url
    .replace(/\/v1beta(?:\/models(?:\/[^/?#]*(?::(?:stream)?[Gg]enerateContent)?(?:\?[^]*)?)?)?$/, '')
    .replace(/\/models(?:\/[^/?#]*(?::(?:stream)?[Gg]enerateContent)?(?:\?[^]*)?)?$/, '')
    .replace(/\/+$/, '');
}

/**
 * Build a human-readable preview URL for display in the UI.
 * For gemini: always shows {base}/v1beta/models/...
 */
function previewRequestUrl(baseUrl: string, provider: string): string {
  if (provider === 'gemini') {
    return `${geminiBaseUrl(baseUrl.trim().replace(/\/+$/, ''))}/v1beta/models/...`;
  }
  return resolveRequestUrl(baseUrl, provider);
}

const AIModelConfig: React.FC = () => {
  const { t } = useTranslation('settings/ai-model');
  const { t: tDefault } = useTranslation('settings/default-model');
  const { t: tComponents } = useTranslation('components');
  const [aiModels, setAiModels] = useState<AIModelConfigType[]>([]);
  const [isEditing, setIsEditing] = useState(false);
  const [editingConfig, setEditingConfig] = useState<Partial<AIModelConfigType> | null>(null);
  const [showApiKey, setShowApiKey] = useState(false);
  const [testingConfigs, setTestingConfigs] = useState<Record<string, boolean>>({});
  const [testResults, setTestResults] = useState<Record<string, { success: boolean; message: string } | null>>({});
  const [expandedIds, setExpandedIds] = useState<Set<string>>(new Set());
  const notification = useNotification();
  
  const [showAdvancedSettings, setShowAdvancedSettings] = useState(false);
  
  const [showTokenStats, setShowTokenStats] = useState(false);
  const [selectedModelForStats, setSelectedModelForStats] = useState<{ id: string; name: string } | null>(null);
  
  const [creationMode, setCreationMode] = useState<'selection' | 'form' | null>(null);
  
  const [selectedProviderId, setSelectedProviderId] = useState<string | null>(null);
  const [proxyConfig, setProxyConfig] = useState<ProxyConfig>({
    enabled: false,
    url: '',
    username: '',
    password: ''
  });
  const [isProxySaving, setIsProxySaving] = useState(false);
  const [remoteModelOptions, setRemoteModelOptions] = useState<RemoteModelOption[]>([]);
  const [isFetchingRemoteModels, setIsFetchingRemoteModels] = useState(false);
  const [remoteModelsError, setRemoteModelsError] = useState<string | null>(null);
  const [hasAttemptedRemoteFetch, setHasAttemptedRemoteFetch] = useState(false);
  const [selectedModelDrafts, setSelectedModelDrafts] = useState<SelectedModelDraft[]>([]);
  const [manualModelInput, setManualModelInput] = useState('');
  const lastRemoteFetchSignatureRef = React.useRef<string | null>(null);
  const activeRemoteFetchSignatureRef = React.useRef<string | null>(null);

  const requestFormatOptions = useMemo(
    () => [
      { label: 'OpenAI (chat/completions)', value: 'openai' },
      { label: 'OpenAI (responses)', value: 'responses' },
      { label: 'Anthropic (messages)', value: 'anthropic' },
      { label: 'Gemini (generateContent)', value: 'gemini' },
    ],
    []
  );
  const requestFormatLabelMap = useMemo(
    () => Object.fromEntries(
      requestFormatOptions.map(option => [String(option.value), option.label])
    ) as Record<string, string>,
    [requestFormatOptions]
  );

  const reasoningEffortOptions = useMemo(
    () => [
      { label: 'Low', value: 'low' },
      { label: 'Medium', value: 'medium' },
      { label: 'High', value: 'high' },
      { label: 'Extra High', value: 'xhigh' },
    ],
    []
  );

  const thinkingModeOptions = useMemo(
    () => [
      { label: t('thinking.optionEnabled'), value: 'enabled' },
      { label: t('thinking.optionDisabled'), value: 'disabled' },
    ],
    [t]
  );

  const categoryOptions = useMemo<SelectOption[]>(
    () => [
      { label: t('category.general_chat'), value: 'general_chat' },
    ],
    [t]
  );

  const categoryCompactLabels = useMemo<Record<ModelCategory, string>>(
    () => ({
      general_chat: t('categoryIcons.general_chat'),
    }),
    [t]
  );

  
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
  const providerOrder = ['openbitfun', 'zhipu', 'qwen', 'deepseek', 'volcengine', 'minimax', 'moonshot', 'gemini', 'anthropic'];
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

  const getConfiguredModelsForProvider = (providerName: string) => {
    const normalizedProviderName = providerName.trim();
    if (!normalizedProviderName) {
      return [];
    }

    return aiModels
      .filter(model => getProviderDisplayName(model) === normalizedProviderName)
      .sort((a, b) => a.model_name.localeCompare(b.model_name));
  };

  const createDraftsFromConfigs = (configs: AIModelConfigType[]) => (
    configs.map(config => createModelDraft(config.model_name, config, {
      configId: config.id,
      contextWindow: config.context_window || 128000,
      maxTokens: config.max_tokens || 8192,
      enableThinking: config.enable_thinking_process ?? false,
    }))
  );

  const resetRemoteModelDiscovery = () => {
    setRemoteModelOptions([]);
    setIsFetchingRemoteModels(false);
    setRemoteModelsError(null);
    setHasAttemptedRemoteFetch(false);
    lastRemoteFetchSignatureRef.current = null;
    activeRemoteFetchSignatureRef.current = null;
  };

  const syncSelectedModelDrafts = (
    modelNames: string[],
    baseConfig?: Partial<AIModelConfigType>,
    singleSelection = false
  ) => {
    const nextModelNames = singleSelection
      ? uniqModelNames(modelNames).slice(0, 1)
      : uniqModelNames(modelNames);

    const providerName = (
      baseConfig?.name ||
      editingConfig?.name ||
      currentTemplate?.name ||
      ''
    ).trim();
    const configuredModelsByName = new Map(
      getConfiguredModelsForProvider(providerName).map(model => [model.model_name, model])
    );

    setSelectedModelDrafts(prevDrafts =>
      nextModelNames.map(modelName => {
        const existingDraft = prevDrafts.find(draft => draft.modelName === modelName);
        if (existingDraft) {
          return existingDraft;
        }

        const configuredModel = configuredModelsByName.get(modelName);
        return createModelDraft(modelName, configuredModel || baseConfig, {
          configId: configuredModel?.id,
        });
      })
    );

    setEditingConfig(prev => {
      if (!prev) return prev;

      const nextPrimaryModel = nextModelNames[0] || '';
      const providerName = currentTemplate?.name || prev.name || '';
      const oldAutoName = prev.model_name ? `${providerName} - ${prev.model_name}` : '';
      const isAutoGenerated = !prev.name || prev.name === oldAutoName || prev.name === providerName;

      return {
        ...prev,
        model_name: nextPrimaryModel,
        request_url: resolveRequestUrl(
          prev.base_url || currentTemplate?.baseUrl || '',
          prev.provider || currentTemplate?.format || 'openai',
          nextPrimaryModel
        ),
        name: isAutoGenerated ? providerName : prev.name
      };
    });
  };

  const updateModelDraft = (modelName: string, updates: Partial<SelectedModelDraft>) => {
    setSelectedModelDrafts(prevDrafts => prevDrafts.map(draft => (
      draft.modelName === modelName ? { ...draft, ...updates } : draft
    )));
  };

  const removeSelectedModelDraft = (modelName: string) => {
    const remainingModelNames = selectedModelDrafts
      .filter(draft => draft.modelName !== modelName)
      .map(draft => draft.modelName);

    syncSelectedModelDrafts(remainingModelNames, editingConfig || undefined, !!editingConfig?.id);
  };

  const addManualModelDraft = () => {
    const trimmedModelName = manualModelInput.trim();
    if (!trimmedModelName) return;

    const nextModelNames = editingConfig?.id
      ? [trimmedModelName]
      : uniqModelNames([
          ...selectedModelDrafts.map(draft => draft.modelName),
          trimmedModelName,
        ]);

    syncSelectedModelDrafts(nextModelNames, editingConfig || undefined, !!editingConfig?.id);
    setManualModelInput('');
  };

  const buildModelDiscoveryConfig = (config: Partial<AIModelConfigType>): AIModelConfigType | null => {
    const resolvedBaseUrl = (config.base_url || currentTemplate?.baseUrl || '').trim();
    const resolvedProvider = (config.provider || currentTemplate?.format || 'openai').trim();
    const resolvedApiKey = (config.api_key || '').trim();
    const resolvedModelName = (
      config.model_name ||
      selectedModelDrafts[0]?.modelName ||
      currentTemplate?.models[0] ||
      'model-discovery'
    ).trim();

    if (!resolvedBaseUrl || !resolvedProvider || !resolvedApiKey) {
      return null;
    }

    return {
      id: config.id || 'model_discovery',
      name: config.name || 'Model Discovery',
      provider: resolvedProvider,
      api_key: resolvedApiKey,
      base_url: resolvedBaseUrl,
      request_url: config.request_url || resolveRequestUrl(resolvedBaseUrl, resolvedProvider, resolvedModelName),
      model_name: resolvedModelName,
      description: config.description,
      context_window: config.context_window || 128000,
      max_tokens: config.max_tokens || 8192,
      temperature: config.temperature,
      top_p: config.top_p,
      enabled: config.enabled ?? true,
      category: config.category || 'general_chat',
      capabilities: config.capabilities || ['text_chat'],
      recommended_for: config.recommended_for || [],
      metadata: config.metadata || {},
      enable_thinking_process: config.enable_thinking_process ?? false,
      support_preserved_thinking: config.support_preserved_thinking ?? false,
      reasoning_effort: config.reasoning_effort,
      custom_headers: config.custom_headers,
      custom_headers_mode: config.custom_headers_mode,
      skip_ssl_verify: config.skip_ssl_verify ?? false,
      custom_request_body: config.custom_request_body
    };
  };

  const buildModelDiscoverySignature = (config: AIModelConfigType): string => JSON.stringify({
    provider: config.provider,
    base_url: config.base_url,
    api_key: config.api_key,
    model_name: config.model_name,
    skip_ssl_verify: config.skip_ssl_verify ?? false,
    custom_headers_mode: config.custom_headers_mode || null,
    custom_headers: config.custom_headers || null,
    custom_request_body: config.custom_request_body || null,
  });

  const fetchRemoteModels = async (config: Partial<AIModelConfigType> | null) => {
    if (!config) return;

    const discoveryConfig = buildModelDiscoveryConfig(config);
    if (!discoveryConfig) {
      setRemoteModelOptions([]);
      setRemoteModelsError(t('providerSelection.fillApiKeyBeforeFetch'));
      setHasAttemptedRemoteFetch(true);
      return;
    }

    const requestSignature = buildModelDiscoverySignature(discoveryConfig);
    if (activeRemoteFetchSignatureRef.current === requestSignature) {
      return;
    }
    if (lastRemoteFetchSignatureRef.current === requestSignature) {
      return;
    }

    setIsFetchingRemoteModels(true);
    setRemoteModelsError(null);
    setHasAttemptedRemoteFetch(true);
    lastRemoteFetchSignatureRef.current = requestSignature;
    activeRemoteFetchSignatureRef.current = requestSignature;

    try {
      const remoteModels = await aiApi.listModelsByConfig(discoveryConfig);
      const dedupedModels = remoteModels.filter((model, index, arr) => (
        !!model.id && arr.findIndex(item => item.id === model.id) === index
      ));

      if (dedupedModels.length === 0) {
        setRemoteModelOptions([]);
        setRemoteModelsError(t('providerSelection.fetchEmptyFallback'));
        return;
      }

      setRemoteModelOptions(dedupedModels);
      setRemoteModelsError(null);
    } catch (error) {
      log.warn('Failed to fetch remote model list, falling back to presets', { error });
      setRemoteModelOptions([]);
      setRemoteModelsError(t('providerSelection.fetchFailedFallback'));
    } finally {
      setIsFetchingRemoteModels(false);
      if (activeRemoteFetchSignatureRef.current === requestSignature) {
        activeRemoteFetchSignatureRef.current = null;
      }
    }
  };

  const handleModelSelectionOpenChange = (isOpen: boolean) => {
    if (!isOpen || !editingConfig || isFetchingRemoteModels) return;
    if (!editingConfig.api_key?.trim()) return;
    if (hasAttemptedRemoteFetch) return;
    if (remoteModelOptions.length > 0) return;
    void fetchRemoteModels(editingConfig);
  };

  
  const handleCreateNew = () => {
    resetRemoteModelDiscovery();
    setSelectedModelDrafts([]);
    setManualModelInput('');
    setShowApiKey(false);
    setSelectedProviderId(null);
    setCreationMode('selection');
  };

  
  const handleSelectProvider = (providerId: string) => {
    const template = PROVIDER_TEMPLATES[providerId];
    if (!template) return;
    resetRemoteModelDiscovery();
    setManualModelInput('');
    setShowApiKey(false);
    setSelectedProviderId(providerId);
    
    // Dynamically get translated name
    const providerName = t(`providers.${template.id}.name`);
    const configuredProviderModels = getConfiguredModelsForProvider(providerName);
    const primaryConfiguredModel = configuredProviderModels[0];
    const defaultModel = primaryConfiguredModel?.model_name || template.models[0] || '';
    
    setEditingConfig({
      name: providerName,
      base_url: primaryConfiguredModel?.base_url || template.baseUrl,
      request_url: resolveRequestUrl(
        primaryConfiguredModel?.base_url || template.baseUrl,
        primaryConfiguredModel?.provider || template.format,
        defaultModel
      ),
      api_key: primaryConfiguredModel?.api_key || '',
      model_name: defaultModel,
      provider: primaryConfiguredModel?.provider || template.format,
      enabled: true,
      context_window: 128000,
      max_tokens: 8192,
      category: 'general_chat',
      capabilities: ['text_chat', 'function_calling'],
      recommended_for: [],
      metadata: {}
    });
    setSelectedModelDrafts(
      configuredProviderModels.length > 0
        ? createDraftsFromConfigs(configuredProviderModels)
        : (defaultModel ? [createModelDraft(defaultModel, {
            context_window: 128000,
            max_tokens: 8192,
            enable_thinking_process: false,
          })] : [])
    );
    setShowAdvancedSettings(false);
    setCreationMode('form');
    setIsEditing(true);
  };

  
  const handleSelectCustom = () => {
    resetRemoteModelDiscovery();
    setManualModelInput('');
    setShowApiKey(false);
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
    setSelectedModelDrafts([]);
    setShowAdvancedSettings(false);  
    setCreationMode('form');
    setIsEditing(true);
  };

  const handleEditProvider = (config: AIModelConfigType) => {
    resetRemoteModelDiscovery();
    setManualModelInput('');
    setShowApiKey(false);

    const providerName = getProviderDisplayName(config);
    const configuredProviderModels = getConfiguredModelsForProvider(providerName);
    const providerTemplateId = getProviderTemplateId(config);
    setSelectedProviderId(providerTemplateId || null);
    setEditingConfig({
      name: providerName,
      base_url: config.base_url,
      request_url: resolveRequestUrl(config.base_url, config.provider || 'openai'),
      api_key: config.api_key || '',
      model_name: '',
      provider: config.provider,
      enabled: true,
      description: config.description,
      context_window: config.context_window || 128000,
      max_tokens: config.max_tokens || 8192,
      category: config.category || 'general_chat',
      capabilities: config.capabilities || getCapabilitiesByCategory(config.category || 'general_chat'),
      recommended_for: config.recommended_for || [],
      metadata: config.metadata || {},
      enable_thinking_process: config.enable_thinking_process ?? false,
      support_preserved_thinking: config.support_preserved_thinking ?? false,
      reasoning_effort: config.reasoning_effort,
      custom_headers: config.custom_headers,
      custom_headers_mode: config.custom_headers_mode,
      skip_ssl_verify: config.skip_ssl_verify ?? false,
      custom_request_body: config.custom_request_body,
    });
    setSelectedModelDrafts(createDraftsFromConfigs(configuredProviderModels));
    setShowAdvancedSettings(
      !!config.skip_ssl_verify ||
      (!!config.custom_request_body && config.custom_request_body.trim() !== '') ||
      (!!config.custom_headers && Object.keys(config.custom_headers).length > 0)
    );
    setCreationMode('form');
    setIsEditing(true);
  };

  const handleEdit = (config: AIModelConfigType) => {
    resetRemoteModelDiscovery();
    setManualModelInput('');
    setShowApiKey(false);
    setEditingConfig({ ...config, name: getProviderDisplayName(config) });
    setSelectedModelDrafts([
      createModelDraft(config.model_name, config, {
        contextWindow: config.context_window || 128000,
        maxTokens: config.max_tokens || 8192,
        enableThinking: config.enable_thinking_process ?? false,
      })
    ]);
    
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
    
    if (selectedModelDrafts.length === 0) {
      notification.warning(t('messages.fillModelName'));
      return;
    }

    try {
      const providerName = editingConfig.name.trim();
      const baseUrl = editingConfig.base_url;
      if (!providerName || !baseUrl) {
        notification.warning(t('messages.fillRequired'));
        return;
      }
      const configuredProviderModels = getConfiguredModelsForProvider(providerName);
      const configuredProviderModelIds = new Set(
        configuredProviderModels
          .map(model => model.id)
          .filter((id): id is string => !!id)
      );
      const configsToSave: AIModelConfigType[] = selectedModelDrafts.map((draft, index) => {
        return {
          id: editingConfig.id || draft.configId || `model_${Date.now()}_${index}`,
          name: providerName,
          base_url: baseUrl,
          request_url: resolveRequestUrl(
            baseUrl,
            editingConfig.provider || 'openai',
            draft.modelName
          ),
          api_key: editingConfig.api_key || '',
          model_name: draft.modelName,
          provider: editingConfig.provider || 'openai',
          enabled: editingConfig.enabled ?? true,
          description: editingConfig.description,
          context_window: draft.contextWindow,
          max_tokens: draft.maxTokens,
          category: draft.category,
          capabilities: getCapabilitiesByCategory(draft.category),
          recommended_for: editingConfig.recommended_for || [],
          metadata: editingConfig.metadata,
          enable_thinking_process: draft.enableThinking,
          support_preserved_thinking: editingConfig.support_preserved_thinking ?? false,
          reasoning_effort: editingConfig.reasoning_effort,
          custom_headers: editingConfig.custom_headers,
          custom_headers_mode: editingConfig.custom_headers_mode,
          skip_ssl_verify: editingConfig.skip_ssl_verify ?? false,
          custom_request_body: editingConfig.custom_request_body
        };
      });

      let updatedModels: AIModelConfigType[];
      if (editingConfig.id) {
        updatedModels = aiModels.map(m => m.id === editingConfig.id ? configsToSave[0] : m);
      } else {
        updatedModels = [
          ...aiModels.filter(model => !configuredProviderModelIds.has(model.id || '')),
          ...configsToSave,
        ];
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
              primary: configsToSave[0]?.id,
            });
            log.info('Auto-set primary model for first configured model', { modelId: configsToSave[0]?.id });
            notification.success(t('messages.autoSetPrimary'));
          }
        } catch (error) {
          log.warn('Failed to auto-set primary model', { error });
        }
      }
      
      
      const createdConfigIds = configsToSave.map(config => config.id).filter((id): id is string => !!id);
      if (createdConfigIds.length === 0) {
        
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
      
      
      setExpandedIds(prev => new Set([...prev, ...createdConfigIds]));
      
      
      
      configsToSave.forEach(config => {
        const configId = config.id;
        if (!configId) return;

        void (async () => {
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
            log.warn('Auto test failed after save', { configId, error });
          } finally {
            setTestingConfigs(prev => ({ ...prev, [configId]: false }));
          }
        })();
      });
    } catch (error) {
      log.error('Failed to save config', error);
      notification.error(t('messages.saveFailed'));
    }
  };

  const handleDelete = async (id: string) => {
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
    resetRemoteModelDiscovery();
    setSelectedModelDrafts([]);
    setManualModelInput('');
    setShowApiKey(false);
    setIsEditing(false);
    setEditingConfig(null);
    setCreationMode(null);
    setSelectedProviderId(null);
  };

  const providerGroups = useMemo<ProviderGroup[]>(() => {
    const grouped = aiModels.reduce<Map<string, ProviderGroup>>((map, model) => {
      const providerName = getProviderDisplayName(model);
      const existingGroup = map.get(providerName);
      if (existingGroup) {
        existingGroup.models.push(model);
        return map;
      }

      map.set(providerName, {
        providerName,
        providerId: getProviderTemplateId(model),
        models: [model],
      });
      return map;
    }, new Map());

    return Array.from(grouped.values()).sort((a, b) => {
      const indexA = a.providerId ? providerOrder.indexOf(a.providerId) : -1;
      const indexB = b.providerId ? providerOrder.indexOf(b.providerId) : -1;

      if (indexA !== indexB) {
        return (indexA === -1 ? 999 : indexA) - (indexB === -1 ? 999 : indexB);
      }

      return a.providerName.localeCompare(b.providerName);
    });
  }, [aiModels, providerOrder]);

  
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
    const isProviderScopedEditing = !editingConfig.id;
    const currentProviderLabel = (editingConfig.name || currentTemplate?.name || t('providerSelection.customTitle')).trim() || t('providerSelection.customTitle');
    const configuredProviderModels = getConfiguredModelsForProvider(currentProviderLabel);
    const configuredProviderModelOptions: SelectOption[] = configuredProviderModels.map(model => ({
      label: model.model_name,
      value: model.model_name,
    }));
    const fetchedOrPresetModelOptions: SelectOption[] = remoteModelOptions.length > 0
      ? remoteModelOptions.map(model => ({
          label: model.display_name || model.id,
          value: model.id,
          description: model.display_name && model.display_name !== model.id ? model.id : undefined
        }))
      : (currentTemplate?.models || []).map(model => ({
          label: model,
          value: model
        }));
    const availableModelOptions: SelectOption[] = Array.from(
      new Map(
        [...configuredProviderModelOptions, ...fetchedOrPresetModelOptions]
          .map(option => [String(option.value), option] as const)
      ).values()
    );
    const modelFetchHint = isFetchingRemoteModels
      ? t('providerSelection.fetchingModels')
      : remoteModelsError
        ? remoteModelsError
        : remoteModelOptions.length > 0
          ? null
          : currentTemplate?.models?.length
            ? t('providerSelection.usingPresetModels')
            : hasAttemptedRemoteFetch
              ? t('providerSelection.noPresetModels')
              : null;
    const selectedModelValues = selectedModelDrafts.map(draft => draft.modelName);
    const renderModelPickerValue = (option?: SelectOption | SelectOption[]) => {
      const selectedOptions = Array.isArray(option) ? option : option ? [option] : [];

      if (selectedOptions.length === 0) {
        return <span className="select__placeholder">{t('providerSelection.selectModel')}</span>;
      }
      const summaryText = selectedOptions
        .map(item => String(item.label))
        .join(', ');

      return (
        <span className="select__value bitfun-ai-model-config__model-picker-value">
          <span className="select__value-label bitfun-ai-model-config__model-picker-value-text">
            {summaryText}
          </span>
        </span>
      );
    };
    const apiKeyVisibilityLabel = showApiKey ? tComponents('hide') : tComponents('show');
    const apiKeySuffix = (
      <button
        type="button"
        className="bitfun-ai-model-config__input-visibility-toggle"
        onClick={() => setShowApiKey(prev => !prev)}
        aria-label={apiKeyVisibilityLabel}
        title={apiKeyVisibilityLabel}
      >
        {showApiKey ? <EyeOff size={14} /> : <Eye size={14} />}
      </button>
    );

    const renderSelectedModelRows = () => {
      if (selectedModelDrafts.length === 0) {
        return (
          <div className="bitfun-ai-model-config__selected-models-empty">
            {t('providerSelection.noModelsSelected')}
          </div>
        );
      }

      return (
        <div className="bitfun-ai-model-config__selected-models-list">
          {selectedModelDrafts.map(draft => (
            <div key={draft.key} className="bitfun-ai-model-config__selected-model-row">
              <div className="bitfun-ai-model-config__selected-model-head">
                <div className="bitfun-ai-model-config__selected-model-name">{`${currentProviderLabel}/${draft.modelName}`}</div>
                {!editingConfig.id && (
                  <IconButton
                    variant="ghost"
                    size="small"
                    onClick={() => removeSelectedModelDraft(draft.modelName)}
                    tooltip={t('providerSelection.removeModel')}
                  >
                    <X size={14} />
                  </IconButton>
                )}
              </div>
              <div className="bitfun-ai-model-config__selected-model-grid">
                <div className="bitfun-ai-model-config__selected-model-field">
                  <span>{t('category.label')}</span>
                  <Select
                    value={draft.category}
                    onChange={(value) => updateModelDraft(draft.modelName, { category: value as ModelCategory })}
                    options={categoryOptions}
                    size="small"
                    className="bitfun-ai-model-config__selected-model-category-select"
                    renderValue={(option) => {
                      if (!option || Array.isArray(option)) {
                        return null;
                      }

                      const compactLabel = categoryCompactLabels[option.value as ModelCategory] ?? option.label;

                      return (
                        <span className="select__value">
                          <span className="select__value-label">{compactLabel}</span>
                        </span>
                      );
                    }}
                  />
                </div>
                <div className="bitfun-ai-model-config__selected-model-field">
                  <span>{t('form.contextWindow')}</span>
                  <NumberInput
                    value={draft.contextWindow}
                    onChange={(value) => updateModelDraft(draft.modelName, { contextWindow: value })}
                    min={1000}
                    max={2000000}
                    step={1000}
                    size="small"
                    disableWheel
                  />
                </div>
                <div className="bitfun-ai-model-config__selected-model-field">
                  <span>{t('form.maxTokens')}</span>
                  <NumberInput
                    value={draft.maxTokens}
                    onChange={(value) => updateModelDraft(draft.modelName, { maxTokens: value })}
                    min={1000}
                    max={1000000}
                    step={1000}
                    size="small"
                    disableWheel
                  />
                </div>
                <div className="bitfun-ai-model-config__selected-model-field">
                  <span>{t('thinking.enable')}</span>
                  <Select
                    value={draft.enableThinking ? 'enabled' : 'disabled'}
                    onChange={(value) => updateModelDraft(draft.modelName, { enableThinking: value === 'enabled' })}
                    options={thinkingModeOptions}
                    size="small"
                  />
                </div>
              </div>
            </div>
          ))}
        </div>
      );
    };

    return (
      <>
        <div className="bitfun-ai-model-config__form bitfun-ai-model-config__form--modal">
          <ConfigPageSection
            title={isProviderScopedEditing ? t('editProviderSubtitle') : t('editSubtitle')}
            className="bitfun-ai-model-config__edit-section"
          >
            {isFromTemplate ? (
              <>
                <ConfigPageRow label={`${t('form.configName')} *`} align="center" wide>
                  <Input value={editingConfig.name || ''} onChange={(e) => setEditingConfig(prev => ({ ...prev, name: e.target.value }))} placeholder={t('form.configNamePlaceholder')} inputSize="small" />
                </ConfigPageRow>
                <ConfigPageRow label={`${t('form.apiKey')} *`} align="center" wide>
                  <Input
                    type={showApiKey ? 'text' : 'password'}
                    value={editingConfig.api_key || ''}
                    onChange={(e) => {
                      resetRemoteModelDiscovery();
                      setEditingConfig(prev => ({ ...prev, api_key: e.target.value }));
                    }}
                    placeholder={t('form.apiKeyPlaceholder')}
                    inputSize="small"
                    suffix={apiKeySuffix}
                  />
                </ConfigPageRow>
                <ConfigPageRow label={t('form.baseUrl')} align="center" wide>
                  <div className="bitfun-ai-model-config__control-stack">
                    {currentTemplate?.baseUrlOptions && currentTemplate.baseUrlOptions.length > 0 && (
                      <Select
                        value={currentTemplate.baseUrlOptions.some(opt => opt.url === editingConfig.base_url) ? editingConfig.base_url : ''}
                        onChange={(value) => {
                          const selectedOption = currentTemplate.baseUrlOptions!.find(opt => opt.url === value);
                          const newProvider = selectedOption?.format || editingConfig.provider || 'openai';
                          resetRemoteModelDiscovery();
                          setEditingConfig(prev => ({
                            ...prev,
                            base_url: value as string,
                            request_url: resolveRequestUrl(value as string, newProvider, editingConfig.model_name || ''),
                            provider: newProvider
                          }));
                        }}
                        placeholder={t('form.baseUrl')}
                        options={currentTemplate.baseUrlOptions.map(opt => ({ label: opt.url, value: opt.url, description: `${opt.format.toUpperCase()} · ${opt.note}` }))}
                        size="small"
                      />
                    )}
                    <Input
                      type="url"
                      value={editingConfig.base_url || ''}
                      onChange={(e) => {
                        resetRemoteModelDiscovery();
                        setEditingConfig(prev => ({
                          ...prev,
                          base_url: e.target.value,
                          request_url: resolveRequestUrl(e.target.value, prev?.provider || 'openai', prev?.model_name || '')
                        }));
                      }}
                      onFocus={(e) => e.target.select()}
                      placeholder={currentTemplate?.baseUrl}
                      inputSize="small"
                    />
                    {editingConfig.base_url && (
                      <div className="bitfun-ai-model-config__resolved-url">
                        <Input
                          value={previewRequestUrl(editingConfig.base_url, editingConfig.provider || 'openai')}
                          readOnly
                          onFocus={(e) => e.target.select()}
                          inputSize="small"
                          className="bitfun-ai-model-config__resolved-url-input"
                        />
                      </div>
                    )}
                  </div>
                </ConfigPageRow>
                <ConfigPageRow label={t('form.provider')} align="center" wide>
                  <Select
                    value={editingConfig.provider || 'openai'}
                    onChange={(value) => {
                      resetRemoteModelDiscovery();
                      setEditingConfig(prev => ({
                        ...prev,
                        provider: value as string,
                        request_url: resolveRequestUrl(prev?.base_url || '', value as string, prev?.model_name || '')
                      }));
                    }}
                    placeholder={t('form.providerPlaceholder')}
                    options={requestFormatOptions}
                    size="small"
                  />
                </ConfigPageRow>
                <ConfigPageRow label={`${t('form.modelSelection')} *`} wide multiline>
                  <div className="bitfun-ai-model-config__control-stack">
                    <div className="bitfun-ai-model-config__model-picker-row">
                      <Select
                        value={selectedModelValues}
                        onChange={(value) => {
                          const nextModelNames = Array.isArray(value) ? value.map(item => String(item)) : [String(value)];
                          syncSelectedModelDrafts(nextModelNames, editingConfig);
                        }}
                        placeholder={t('providerSelection.selectModel')}
                        options={availableModelOptions}
                        searchable
                        multiple
                        loading={isFetchingRemoteModels}
                        emptyText={t('providerSelection.noPresetModels')}
                        searchPlaceholder={t('providerSelection.inputModelName')}
                        size="small"
                        onOpenChange={handleModelSelectionOpenChange}
                        renderValue={renderModelPickerValue}
                        className={selectedModelValues.length > 0 ? 'bitfun-ai-model-config__model-picker-select bitfun-ai-model-config__model-picker-select--has-value' : 'bitfun-ai-model-config__model-picker-select'}
                      />
                    </div>
                    <div className="bitfun-ai-model-config__manual-model-entry">
                      <Input
                        value={manualModelInput}
                        onChange={(e) => setManualModelInput(e.target.value)}
                        onKeyDown={(e) => {
                          if (e.key === 'Enter') {
                            e.preventDefault();
                            addManualModelDraft();
                          }
                        }}
                        placeholder={t('providerSelection.inputModelName')}
                        inputSize="small"
                      />
                      <Button variant="secondary" size="small" onClick={addManualModelDraft}>
                        {t('providerSelection.addCustomModel')}
                      </Button>
                    </div>
                    {modelFetchHint && (
                      <small className={`resolved-url__hint ${remoteModelsError ? 'bitfun-ai-model-config__json-status--error' : ''}`}>
                        {modelFetchHint}
                      </small>
                    )}
                    {renderSelectedModelRows()}
                  </div>
                </ConfigPageRow>
                {isResponsesProvider(editingConfig.provider) && (
                  <ConfigPageRow label={t('reasoningEffort.label')} description={t('reasoningEffort.hint')} align="center">
                    <Select value={editingConfig.reasoning_effort || ''} onChange={(v) => setEditingConfig(prev => ({ ...prev, reasoning_effort: (v as string) || undefined }))} placeholder={t('reasoningEffort.placeholder')} options={reasoningEffortOptions} size="small" />
                  </ConfigPageRow>
                )}
              </>
            ) : (
              <>
                {isProviderScopedEditing && (
                  <>
                    <ConfigPageRow label={`${t('form.configName')} *`} align="center" wide>
                      <Input value={editingConfig.name || ''} onChange={(e) => setEditingConfig(prev => ({ ...prev, name: e.target.value }))} placeholder={t('form.configNamePlaceholder')} inputSize="small" />
                    </ConfigPageRow>
                    <ConfigPageRow label={`${t('form.baseUrl')} *`} align="center" wide>
                      <div className="bitfun-ai-model-config__control-stack">
                        <Input
                          type="url"
                          value={editingConfig.base_url || ''}
                          onChange={(e) => {
                            resetRemoteModelDiscovery();
                            setEditingConfig(prev => ({
                              ...prev,
                              base_url: e.target.value,
                              request_url: resolveRequestUrl(e.target.value, prev?.provider || 'openai', prev?.model_name || '')
                            }));
                          }}
                          onFocus={(e) => e.target.select()}
                          placeholder={'https://open.bigmodel.cn/api/paas/v4/chat/completions'}
                          inputSize="small"
                        />
                        {editingConfig.base_url && (
                          <div className="bitfun-ai-model-config__resolved-url">
                            <Input
                              value={previewRequestUrl(editingConfig.base_url, editingConfig.provider || 'openai')}
                              readOnly
                              onFocus={(e) => e.target.select()}
                              inputSize="small"
                              className="bitfun-ai-model-config__resolved-url-input"
                            />
                          </div>
                        )}
                      </div>
                    </ConfigPageRow>
                    <ConfigPageRow label={`${t('form.apiKey')} *`} align="center" wide>
                      <Input
                        type={showApiKey ? 'text' : 'password'}
                        value={editingConfig.api_key || ''}
                        onChange={(e) => {
                          resetRemoteModelDiscovery();
                          setEditingConfig(prev => ({ ...prev, api_key: e.target.value }));
                        }}
                        placeholder={t('form.apiKeyPlaceholder')}
                        inputSize="small"
                        suffix={apiKeySuffix}
                      />
                    </ConfigPageRow>
                    <ConfigPageRow label={t('form.provider')} align="center" wide>
                      <Select value={editingConfig.provider || 'openai'} onChange={(value) => {
                        const provider = value as string;
                        resetRemoteModelDiscovery();
                        setEditingConfig(prev => ({
                          ...prev,
                          provider,
                          request_url: resolveRequestUrl(prev?.base_url || '', provider, prev?.model_name || ''),
                          reasoning_effort: isResponsesProvider(provider) ? (prev?.reasoning_effort || 'medium') : undefined,
                        }));
                      }} placeholder={t('form.providerPlaceholder')} options={requestFormatOptions} size="small" />
                    </ConfigPageRow>
                  </>
                )}
              </>
            )}

            {!isFromTemplate && (
              <>
                <ConfigPageRow label={`${t('form.modelSelection')} *`} wide multiline>
                  <div className="bitfun-ai-model-config__control-stack">
                    <div className="bitfun-ai-model-config__model-picker-row">
                      <Select
                        value={editingConfig.id ? (selectedModelValues[0] || '') : selectedModelValues}
                        onChange={(value) => {
                          const nextModelNames = Array.isArray(value)
                            ? value.map(item => String(item))
                            : [String(value)];
                          syncSelectedModelDrafts(nextModelNames, editingConfig, !!editingConfig.id);
                        }}
                        placeholder="glm-4.7"
                        options={availableModelOptions}
                        searchable
                        multiple={!editingConfig.id}
                        loading={isFetchingRemoteModels}
                        emptyText={t('providerSelection.noPresetModels')}
                        searchPlaceholder={t('providerSelection.inputModelName')}
                        size="small"
                        onOpenChange={handleModelSelectionOpenChange}
                      />
                    </div>
                    <div className="bitfun-ai-model-config__manual-model-entry">
                      <Input
                        value={manualModelInput}
                        onChange={(e) => setManualModelInput(e.target.value)}
                        onKeyDown={(e) => {
                          if (e.key === 'Enter') {
                            e.preventDefault();
                            addManualModelDraft();
                          }
                        }}
                        placeholder={t('providerSelection.inputModelName')}
                        inputSize="small"
                      />
                      <Button variant="secondary" size="small" onClick={addManualModelDraft}>
                        {t('providerSelection.addCustomModel')}
                      </Button>
                    </div>
                    {modelFetchHint && (
                      <small className={`resolved-url__hint ${remoteModelsError ? 'bitfun-ai-model-config__json-status--error' : ''}`}>
                        {modelFetchHint}
                      </small>
                    )}
                    {renderSelectedModelRows()}
                  </div>
                </ConfigPageRow>
                {isResponsesProvider(editingConfig.provider) && (
                  <ConfigPageRow label={t('reasoningEffort.label')} description={t('reasoningEffort.hint')} align="center">
                    <Select value={editingConfig.reasoning_effort || ''} onChange={(v) => setEditingConfig(prev => ({ ...prev, reasoning_effort: (v as string) || undefined }))} placeholder={t('reasoningEffort.placeholder')} options={reasoningEffortOptions} size="small" />
                  </ConfigPageRow>
                )}
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

  const renderModelCollectionItem = (config: AIModelConfigType) => {
    const isExpanded = expandedIds.has(config.id || '');
    const testResult = config.id ? testResults[config.id] : null;
    const isTesting = config.id ? !!testingConfigs[config.id] : false;
    const providerDisplayName = getProviderDisplayName(config);
    const modelDisplayName = getModelDisplayName(config);
    const modelLabel = config.model_name || modelDisplayName;

    const badge = (
      <>
        <span className="bitfun-ai-model-config__meta-tag">
          {t(`category.${config.category}`)}
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
            {t('details.basicInfo')}
          </div>
          <div className="bitfun-ai-model-config__details-grid">
            <div className="bitfun-ai-model-config__details-item">
              <span className="bitfun-ai-model-config__details-label">{t('form.configName')}</span>
              <span className="bitfun-ai-model-config__details-value">{providerDisplayName}</span>
            </div>
            <div className="bitfun-ai-model-config__details-item">
              <span className="bitfun-ai-model-config__details-label">{t('details.modelName')}</span>
              <span className="bitfun-ai-model-config__details-value">{config.model_name}</span>
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
                <span className="bitfun-ai-model-config__details-label">{t('details.capabilities')}</span>
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
          onClick={() => {
            setSelectedModelForStats({ id: config.id!, name: modelLabel });
            setShowTokenStats(true);
          }}
          title={t('actions.viewStats')}
        >
          <BarChart3 size={14} />
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
        label={modelLabel}
        badge={badge}
        control={control}
        details={details}
        expanded={isExpanded}
        onToggle={() => config.id && toggleExpanded(config.id)}
        disabled={!config.enabled}
      />
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
          title={tDefault('tabs.default')}
          description={tDefault('subtitle')}
        >
          <DefaultModelConfig />
        </ConfigPageSection>

        <ConfigPageSection
          title={tDefault('tabs.models')}
          description={t('subtitle')}
          extra={(
            <IconButton
              variant="primary"
              size="small"
              onClick={handleCreateNew}
              tooltip={t('actions.addProvider')}
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
              {providerGroups.map(group => (
                <div key={group.providerName} className="bitfun-ai-model-config__provider-group">
                  <div className="bitfun-ai-model-config__provider-group-header">
                    <div className="bitfun-ai-model-config__provider-group-title">
                      <span>{group.providerName}</span>
                      <span className="bitfun-ai-model-config__provider-group-count">{group.models.length}</span>
                      <span className="bitfun-ai-model-config__meta-tag">
                        {requestFormatLabelMap[group.models[0]?.provider || 'openai'] || (group.models[0]?.provider || 'openai')}
                      </span>
                    </div>
                    <div className="bitfun-ai-model-config__provider-group-actions">
                      <IconButton
                        variant="ghost"
                        size="small"
                        onClick={() => handleEditProvider(group.models[0])}
                        tooltip={t('actions.edit')}
                      >
                        <Edit2 size={14} />
                      </IconButton>
                    </div>
                  </div>
                  <div className="bitfun-ai-model-config__provider-group-list">
                    {group.models.map(config => renderModelCollectionItem(config))}
                  </div>
                </div>
              ))}
            </div>
          )}
        </ConfigPageSection>

        <ConfigPageSection
          title={tDefault('tabs.proxy')}
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
          : (selectedModelDrafts.some(draft => !!draft.configId)
            ? t('editProvider')
            : (currentTemplate ? `${t('newProvider')} - ${currentTemplate.name}` : t('newProvider')))}
        size="xlarge"
      >
        {renderEditingForm()}
      </Modal>

      {selectedModelForStats && (
        <TokenStatsModal
          isOpen={showTokenStats}
          onClose={() => {
            setShowTokenStats(false);
            setSelectedModelForStats(null);
          }}
          modelId={selectedModelForStats.id}
          modelName={selectedModelForStats.name}
        />
      )}
    </ConfigPageLayout>
  );
};

export default AIModelConfig;
