import React, { useState, useEffect, useMemo, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { Plus, SquarePen, Trash2, Wifi, Loader, AlertTriangle, X, Settings, ExternalLink, Eye, EyeOff, ChevronDown, ChevronRight, Info } from 'lucide-react';
import { Button, Switch, Select, IconButton, NumberInput, Card, Modal, Input, Textarea, Tooltip, type SelectOption } from '@/component-library';
import { 
  AIModelConfig as AIModelConfigType, 
  ProxyConfig, 
  ModelCategory,
  ModelCapability,
  ReasoningMode
} from '../types';
import { configManager } from '../services/ConfigManager';
import { PROVIDER_TEMPLATES, getModelDisplayName, getProviderDisplayName, getProviderTemplateId } from '../services/modelConfigs';
import { DEFAULT_REASONING_MODE, getEffectiveReasoningMode, supportsAnthropicAdaptive, supportsAnthropicReasoning, supportsAnthropicThinkingBudget, supportsResponsesReasoning } from '../utils/reasoning';
import { aiApi, systemAPI } from '@/infrastructure/api';
import { useNotification } from '@/shared/notification-system';
import { ConfigPageHeader, ConfigPageLayout, ConfigPageContent, ConfigPageSection, ConfigPageRow, ConfigCollectionItem } from './common';
import DefaultModelConfig from './DefaultModelConfig';
import { createLogger } from '@/shared/utils/logger';
import { translateConnectionTestMessage } from '@/shared/utils/aiConnectionTestMessages';
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
  reasoningMode: ReasoningMode;
  reasoningEffort?: string;
  thinkingBudgetTokens?: number;
}

interface ProviderGroup {
  providerName: string;
  providerId?: string;
  models: AIModelConfigType[];
}

function isResponsesProvider(provider?: string): boolean {
  return supportsResponsesReasoning(provider);
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
    reasoningMode: overrides?.reasoningMode ?? getEffectiveReasoningMode(baseConfig),
    reasoningEffort: overrides?.reasoningEffort ?? baseConfig?.reasoning_effort,
    thinkingBudgetTokens: overrides?.thinkingBudgetTokens ?? baseConfig?.thinking_budget_tokens,
  };
}

function normalizeDraftReasoningForProvider(
  draft: SelectedModelDraft,
  provider?: string
): SelectedModelDraft {
  let reasoningMode = draft.reasoningMode;

  if (supportsResponsesReasoning(provider)) {
    reasoningMode = DEFAULT_REASONING_MODE;
  } else if (!supportsAnthropicReasoning(provider) && reasoningMode === 'adaptive') {
    reasoningMode = 'enabled';
  } else if (supportsAnthropicReasoning(provider)
    && reasoningMode === 'adaptive'
    && !supportsAnthropicAdaptive(draft.modelName)) {
    reasoningMode = 'enabled';
  }

  const keepReasoningEffort = supportsResponsesReasoning(provider)
    || (supportsAnthropicReasoning(provider) && reasoningMode === 'adaptive');
  const keepThinkingBudget = supportsAnthropicReasoning(provider)
    && reasoningMode === 'enabled'
    && supportsAnthropicThinkingBudget(draft.modelName);

  return {
    ...draft,
    reasoningMode,
    reasoningEffort: keepReasoningEffort ? draft.reasoningEffort : undefined,
    thinkingBudgetTokens: keepThinkingBudget ? draft.thinkingBudgetTokens : undefined,
  };
}

function uniqModelNames(modelNames: string[]): string[] {
  return Array.from(new Set(modelNames.map(name => name.trim()).filter(Boolean)));
}

function modelNameLookupKey(name: string): string {
  return name.trim().toLowerCase();
}

/** Map lowercased model_name -> config (first wins if duplicates exist in storage). */
function buildConfiguredModelsByLowerName(models: AIModelConfigType[]): Map<string, AIModelConfigType> {
  const map = new Map<string, AIModelConfigType>();
  for (const model of models) {
    const key = modelNameLookupKey(model.model_name);
    if (!map.has(key)) {
      map.set(key, model);
    }
  }
  return map;
}

function resolveModelNameWithExisting(
  rawName: string,
  configuredByLower: Map<string, AIModelConfigType>
): string {
  const trimmed = rawName.trim();
  if (!trimmed) return trimmed;
  const configured = configuredByLower.get(modelNameLookupKey(trimmed));
  return configured?.model_name.trim() ?? trimmed;
}

/**
 * Trim, optionally collapse to single selection, resolve each name against existing provider models
 * (case-insensitive), then dedupe so one provider cannot list the same logical model twice.
 */
function normalizeProviderModelNameList(
  modelNames: string[],
  configuredByLower: Map<string, AIModelConfigType>,
  singleSelection: boolean
): string[] {
  let list = uniqModelNames(modelNames);
  if (singleSelection) {
    list = list.slice(0, 1);
  }
  const seen = new Set<string>();
  const out: string[] = [];
  for (const raw of list) {
    const resolved = resolveModelNameWithExisting(raw, configuredByLower);
    if (!resolved) continue;
    const key = modelNameLookupKey(resolved);
    if (seen.has(key)) continue;
    seen.add(key);
    out.push(resolved);
  }
  return out;
}

/** Compact display for context/output token counts (e.g. 200000 -> "200K", 1000000 -> "1M"). */
function formatTokenCountShort(n: number): string {
  if (!Number.isFinite(n) || n < 0) {
    return String(n);
  }
  if (n >= 1_000_000) {
    const m = n / 1_000_000;
    const s = m % 1 === 0 ? `${m}` : m.toFixed(1).replace(/\.0$/, '');
    return `${s}M`;
  }
  if (n >= 1_000) {
    const k = n / 1_000;
    const s = k % 1 === 0 ? `${k}` : k.toFixed(1).replace(/\.0$/, '');
    return `${s}K`;
  }
  return String(n);
}

function parseOptionalPositiveIntegerInput(value: string): number | null | undefined {
  const trimmed = value.trim();
  if (trimmed === '') {
    return null;
  }

  if (!/^\d+$/.test(trimmed)) {
    return undefined;
  }

  const parsed = Number(trimmed);
  if (!Number.isSafeInteger(parsed) || parsed <= 0) {
    return undefined;
  }

  return parsed;
}

/** Last line of defense: same logical model name once per save; prefer draft tied to an existing config id. */
function dedupeSelectedModelDraftsByModelName(drafts: SelectedModelDraft[]): SelectedModelDraft[] {
  const out: SelectedModelDraft[] = [];
  for (const draft of drafts) {
    const k = modelNameLookupKey(draft.modelName);
    const i = out.findIndex(d => modelNameLookupKey(d.modelName) === k);
    if (i < 0) {
      out.push(draft);
      continue;
    }
    const prev = out[i];
    out[i] = !prev.configId && draft.configId ? draft : prev;
  }
  return out;
}

function getCapabilitiesByCategory(category: ModelCategory): ModelCapability[] {
  switch (category) {
    case 'general_chat':
    case 'multimodal':
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

  const [creationMode, setCreationMode] = useState<'selection' | 'form' | null>(null);
  
  const [selectedProviderId, setSelectedProviderId] = useState<string | null>(null);
  const [proxyConfig, setProxyConfig] = useState<ProxyConfig>({
    enabled: false,
    url: '',
    username: '',
    password: ''
  });
  const [streamIdleTimeoutInput, setStreamIdleTimeoutInput] = useState('');
  const [isStreamIdleTimeoutSaving, setIsStreamIdleTimeoutSaving] = useState(false);
  const [isProxySaving, setIsProxySaving] = useState(false);
  const [remoteModelOptions, setRemoteModelOptions] = useState<RemoteModelOption[]>([]);
  const [isFetchingRemoteModels, setIsFetchingRemoteModels] = useState(false);
  const [remoteModelsError, setRemoteModelsError] = useState<string | null>(null);
  const [hasAttemptedRemoteFetch, setHasAttemptedRemoteFetch] = useState(false);
  const [selectedModelDrafts, setSelectedModelDrafts] = useState<SelectedModelDraft[]>([]);
  const [manualModelInput, setManualModelInput] = useState('');
  const [expandedModelCards, setExpandedModelCards] = useState<Set<string>>(new Set());
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

  const responsesReasoningEffortOptions = useMemo(
    () => [
      { label: 'None', value: 'none' },
      { label: 'Minimal', value: 'minimal' },
      { label: 'Low', value: 'low' },
      { label: 'Medium', value: 'medium' },
      { label: 'High', value: 'high' },
      { label: 'Extra High', value: 'xhigh' },
    ],
    []
  );

  const anthropicReasoningEffortOptions = useMemo(
    () => [
      { label: 'Low', value: 'low' },
      { label: 'Medium', value: 'medium' },
      { label: 'High', value: 'high' },
      { label: 'Max', value: 'max' },
    ],
    []
  );

  const buildReasoningModeOptions = useCallback((provider?: string, modelName?: string, currentMode?: ReasoningMode): SelectOption[] => {
    const options: SelectOption[] = [
      { label: t('thinking.optionDefault'), value: DEFAULT_REASONING_MODE },
      { label: t('thinking.optionEnabled'), value: 'enabled' },
      { label: t('thinking.optionDisabled'), value: 'disabled' },
    ];

    if (
      supportsAnthropicReasoning(provider)
      && (supportsAnthropicAdaptive(modelName) || currentMode === 'adaptive')
    ) {
      options.push({ label: t('thinking.optionAdaptive'), value: 'adaptive' });
    }

    return options;
  }, [t]);

  const categoryOptions = useMemo<SelectOption[]>(
    () => [
      { label: t('category.general_chat'), value: 'general_chat' },
      { label: t('category.multimodal'), value: 'multimodal' },
    ],
    [t]
  );

  const categoryCompactLabels = useMemo<Record<ModelCategory, string>>(
    () => ({
      general_chat: t('categoryIcons.general_chat'),
      multimodal: t('categoryIcons.multimodal'),
    }),
    [t]
  );
  const parsedStreamIdleTimeout = useMemo(
    () => parseOptionalPositiveIntegerInput(streamIdleTimeoutInput),
    [streamIdleTimeoutInput]
  );
  const isStreamIdleTimeoutInvalid = parsedStreamIdleTimeout === undefined;

  const getCustomRequestBodyTrimHint = useCallback((provider?: string): string => {
    switch (provider) {
      case 'responses':
        return t('advancedSettings.customRequestBody.trimHintResponses');
      case 'anthropic':
        return t('advancedSettings.customRequestBody.trimHintAnthropic');
      case 'gemini':
        return t('advancedSettings.customRequestBody.trimHintGemini');
      case 'openai':
      default:
        return t('advancedSettings.customRequestBody.trimHintOpenAI');
    }
  }, [t]);

  const getCustomRequestBodyModeHint = useCallback((provider?: string, mode?: string | null): string => {
    return mode === 'trim'
      ? getCustomRequestBodyTrimHint(provider)
      : t('advancedSettings.customRequestBody.modeMergeHint');
  }, [getCustomRequestBodyTrimHint, t]);

  
  const loadConfig = useCallback(async () => {
    try {
      const [models, proxy, streamIdleTimeoutSecs] = await Promise.all([
        configManager.getConfig<AIModelConfigType[]>('ai.models'),
        configManager.getConfig<ProxyConfig>('ai.proxy'),
        configManager.getConfig<number | null>('ai.stream_idle_timeout_secs'),
      ]);
      setAiModels(models);
      if (proxy) {
        setProxyConfig(proxy);
      }
      setStreamIdleTimeoutInput(
        streamIdleTimeoutSecs != null ? String(streamIdleTimeoutSecs) : ''
      );
    } catch (error) {
      log.error('Failed to load AI config', error);
    }
  }, []);

  useEffect(() => {
    loadConfig();
  }, [loadConfig]);
  
  // Provider options with translations (must be at top level, before any conditional returns)
  const providerOrder = useMemo(
    () => ['openbitfun', 'zhipu', 'qwen', 'deepseek', 'volcengine', 'minimax', 'moonshot', 'gemini', 'anthropic'],
    []
  );
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
  }, [providerOrder, t]);

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
      reasoningMode: getEffectiveReasoningMode(config),
      reasoningEffort: config.reasoning_effort,
      thinkingBudgetTokens: config.thinking_budget_tokens,
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
    const providerName = (
      baseConfig?.name ||
      editingConfig?.name ||
      currentTemplate?.name ||
      ''
    ).trim();
    const configuredByLower = buildConfiguredModelsByLowerName(
      getConfiguredModelsForProvider(providerName)
    );
    const nextModelNames = normalizeProviderModelNameList(
      modelNames,
      configuredByLower,
      singleSelection
    );

    const pinnedRowId =
      singleSelection && baseConfig?.id ? String(baseConfig.id) : undefined;

    setSelectedModelDrafts(prevDrafts =>
      nextModelNames.map(modelName => {
        const lookupKey = modelNameLookupKey(modelName);
        const existingDraft = prevDrafts.find(
          draft => modelNameLookupKey(draft.modelName) === lookupKey
        );
        const configuredModel = configuredByLower.get(lookupKey);

        if (existingDraft) {
          const configId = pinnedRowId ?? configuredModel?.id ?? existingDraft.configId;
          return {
            ...existingDraft,
            modelName,
            configId,
            key: configId ?? modelName,
          };
        }

        return createModelDraft(modelName, configuredModel || baseConfig, {
          configId: pinnedRowId ?? configuredModel?.id,
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

  const toggleSelectedModelCardExpanded = useCallback((draftKey: string) => {
    setExpandedModelCards(prev => {
      const next = new Set(prev);
      if (next.has(draftKey)) next.delete(draftKey);
      else next.add(draftKey);
      return next;
    });
  }, []);

  const onSelectedModelHeadKeyDown = useCallback(
    (e: React.KeyboardEvent, draftKey: string) => {
      if (e.key !== 'Enter' && e.key !== ' ') return;
      e.preventDefault();
      toggleSelectedModelCardExpanded(draftKey);
    },
    [toggleSelectedModelCardExpanded]
  );

  const removeSelectedModelDraft = (modelName: string) => {
    const removed = selectedModelDrafts.find(d => d.modelName === modelName);
    if (removed) {
      setExpandedModelCards(prev => {
        const next = new Set(prev);
        next.delete(removed.key);
        return next;
      });
    }

    const remainingModelNames = selectedModelDrafts
      .filter(draft => draft.modelName !== modelName)
      .map(draft => draft.modelName);

    syncSelectedModelDrafts(remainingModelNames, editingConfig || undefined, !!editingConfig?.id);
  };

  const addManualModelDraft = () => {
    const trimmedModelName = manualModelInput.trim();
    if (!trimmedModelName) return;

    const providerName = (
      editingConfig?.name ||
      currentTemplate?.name ||
      ''
    ).trim();
    const configuredByLower = buildConfiguredModelsByLowerName(
      getConfiguredModelsForProvider(providerName)
    );
    const resolvedName = resolveModelNameWithExisting(trimmedModelName, configuredByLower);
    const matchesExistingSaved = configuredByLower.has(modelNameLookupKey(trimmedModelName));
    const alreadyInDrafts = selectedModelDrafts.some(
      draft => modelNameLookupKey(draft.modelName) === modelNameLookupKey(trimmedModelName)
    );

    if (alreadyInDrafts) {
      notification.info(t('providerSelection.modelAlreadyInList'));
      setManualModelInput('');
      return;
    }

    const nextModelNames = editingConfig?.id
      ? [resolvedName]
      : uniqModelNames([
          ...selectedModelDrafts.map(draft => draft.modelName),
          resolvedName,
        ]);

    syncSelectedModelDrafts(nextModelNames, editingConfig || undefined, !!editingConfig?.id);
    setManualModelInput('');
    if (matchesExistingSaved) {
      notification.info(t('providerSelection.reusedExistingModel'));
    }
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
      context_window: config.context_window || 128000,
      max_tokens: config.max_tokens || 8192,
      temperature: config.temperature,
      top_p: config.top_p,
      enabled: config.enabled ?? true,
      category: config.category || 'general_chat',
      capabilities: config.capabilities || ['text_chat'],
      recommended_for: config.recommended_for || [],
      metadata: config.metadata || {},
      reasoning_mode: config.reasoning_mode ?? getEffectiveReasoningMode(config),
      inline_think_in_text: config.inline_think_in_text ?? true,
      reasoning_effort: config.reasoning_effort,
      thinking_budget_tokens: config.thinking_budget_tokens,
      custom_headers: config.custom_headers,
      custom_headers_mode: config.custom_headers_mode,
      skip_ssl_verify: config.skip_ssl_verify ?? false,
      custom_request_body: config.custom_request_body,
      custom_request_body_mode: config.custom_request_body_mode,
    };
  };

  const buildModelDiscoverySignature = (config: AIModelConfigType): string => JSON.stringify({
    provider: config.provider,
    base_url: config.base_url,
    api_key: config.api_key,
    model_name: config.model_name,
    inline_think_in_text: config.inline_think_in_text ?? true,
    skip_ssl_verify: config.skip_ssl_verify ?? false,
    custom_headers_mode: config.custom_headers_mode || null,
    custom_headers: config.custom_headers || null,
    custom_request_body: config.custom_request_body || null,
    custom_request_body_mode: config.custom_request_body_mode || null,
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
      metadata: {},
      inline_think_in_text: true,
    });
    setSelectedModelDrafts(
      configuredProviderModels.length > 0
        ? createDraftsFromConfigs(configuredProviderModels)
        : (defaultModel ? [createModelDraft(defaultModel, {
            context_window: 128000,
            max_tokens: 8192,
            reasoning_mode: DEFAULT_REASONING_MODE,
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
      metadata: {},
      inline_think_in_text: true,
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
      context_window: config.context_window || 128000,
      max_tokens: config.max_tokens || 8192,
      category: config.category || 'general_chat',
      capabilities: config.capabilities || getCapabilitiesByCategory(config.category || 'general_chat'),
      recommended_for: config.recommended_for || [],
      metadata: config.metadata || {},
      inline_think_in_text: config.inline_think_in_text ?? true,
      custom_headers: config.custom_headers,
      custom_headers_mode: config.custom_headers_mode,
      skip_ssl_verify: config.skip_ssl_verify ?? false,
      custom_request_body: config.custom_request_body,
      custom_request_body_mode: config.custom_request_body_mode,
    });
    setSelectedModelDrafts(createDraftsFromConfigs(configuredProviderModels));
    setShowAdvancedSettings(
      !!config.skip_ssl_verify ||
      config.custom_request_body_mode === 'trim' ||
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
        reasoningMode: getEffectiveReasoningMode(config),
        reasoningEffort: config.reasoning_effort,
        thinkingBudgetTokens: config.thinking_budget_tokens,
      })
    ]);
    
    const hasCustomHeaders = !!config.custom_headers && Object.keys(config.custom_headers).length > 0;
    const hasCustomBody = !!config.custom_request_body && config.custom_request_body.trim() !== '';
    setShowAdvancedSettings(
      hasCustomHeaders ||
      hasCustomBody ||
      config.custom_request_body_mode === 'trim' ||
      !!config.skip_ssl_verify
    );
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
      const draftsToSave = dedupeSelectedModelDraftsByModelName(selectedModelDrafts);
      const configsToSave: AIModelConfigType[] = draftsToSave.map((draft, index) => {
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
          context_window: draft.contextWindow,
          max_tokens: draft.maxTokens,
          category: draft.category,
          capabilities: getCapabilitiesByCategory(draft.category),
          recommended_for: editingConfig.recommended_for || [],
          metadata: editingConfig.metadata,
          reasoning_mode: draft.reasoningMode,
          inline_think_in_text: editingConfig.inline_think_in_text ?? true,
          reasoning_effort: draft.reasoningEffort,
          thinking_budget_tokens: draft.thinkingBudgetTokens,
          custom_headers: editingConfig.custom_headers,
          custom_headers_mode: editingConfig.custom_headers_mode,
          skip_ssl_verify: editingConfig.skip_ssl_verify ?? false,
          custom_request_body: editingConfig.custom_request_body,
          custom_request_body_mode: editingConfig.custom_request_body_mode,
        };
      });

      if (editingConfig.id && configsToSave[0]) {
        const dupKey = modelNameLookupKey(configsToSave[0].model_name);
        const nameConflict = aiModels.some(
          m =>
            m.id !== editingConfig.id &&
            getProviderDisplayName(m) === providerName &&
            modelNameLookupKey(m.model_name) === dupKey
        );
        if (nameConflict) {
          notification.warning(t('messages.duplicateModelNameUnderProvider'));
          return;
        }
      }

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
            const localizedMessage = translateConnectionTestMessage(result.message_code, t);

            if (localizedMessage) {
              message += `\n${localizedMessage}`;
            }

            if (result.error_details) {
              message += result.success
                ? `\n${result.error_details}`
                : `\n${t('messages.errorDetails')}: ${result.error_details}`;
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
      const localizedMessage = translateConnectionTestMessage(result.message_code, t);
      
      if (localizedMessage) {
        message += `\n${localizedMessage}`;
      }

      if (result.error_details) {
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

  const handleSaveStreamIdleTimeout = async () => {
    if (isStreamIdleTimeoutInvalid) {
      notification.warning(t('streamIdleTimeout.invalid'));
      return;
    }

    setIsStreamIdleTimeoutSaving(true);
    try {
      await configManager.setConfig(
        'ai.stream_idle_timeout_secs',
        parsedStreamIdleTimeout ?? null
      );
      setStreamIdleTimeoutInput(
        parsedStreamIdleTimeout != null ? String(parsedStreamIdleTimeout) : ''
      );
      notification.success(t('streamIdleTimeout.saveSuccess'));
    } catch (error) {
      log.error('Failed to save stream idle timeout', error);
      notification.error(t('messages.saveFailed'));
    } finally {
      setIsStreamIdleTimeoutSaving(false);
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

    const formatReasoningSummary = (draft: SelectedModelDraft) => {
      const parts: string[] = [];

      switch (draft.reasoningMode) {
        case 'enabled':
          parts.push(t('thinking.summaryEnabled'));
          break;
        case 'disabled':
          parts.push(t('thinking.summaryDisabled'));
          break;
        case 'adaptive':
          parts.push(t('thinking.summaryAdaptive'));
          break;
        default:
          parts.push(t('thinking.summaryDefault'));
          break;
      }

      if (draft.reasoningEffort) {
        parts.push(draft.reasoningEffort);
      }

      return parts.join(' · ');
    };

    const getDraftReasoningEffortOptions = (provider?: string) => {
      if (supportsResponsesReasoning(provider)) {
        return responsesReasoningEffortOptions;
      }

      if (supportsAnthropicReasoning(provider)) {
        return anthropicReasoningEffortOptions;
      }

      return [];
    };

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
          {selectedModelDrafts.map(draft => {
            const isExpanded = expandedModelCards.has(draft.key) || selectedModelDrafts.length === 1;
            const categoryLabel = categoryCompactLabels[draft.category] ?? draft.category;
            const canToggleExpand = selectedModelDrafts.length > 1;
            const modelDisplayName = draft.modelName;
            const reasoningModeOptions = buildReasoningModeOptions(editingConfig.provider, draft.modelName, draft.reasoningMode);
            const reasoningEffortOptions = getDraftReasoningEffortOptions(editingConfig.provider);
            const showReasoningModeControl = !supportsResponsesReasoning(editingConfig.provider);
            const showReasoningEffortControl = reasoningEffortOptions.length > 0
              && (
                supportsResponsesReasoning(editingConfig.provider)
                || (supportsAnthropicReasoning(editingConfig.provider) && draft.reasoningMode === 'adaptive')
              );
            const showThinkingBudgetControl = supportsAnthropicReasoning(editingConfig.provider)
              && draft.reasoningMode === 'enabled'
              && supportsAnthropicThinkingBudget(draft.modelName);
            const displayedThinkingBudget = draft.thinkingBudgetTokens
              ?? Math.min(Math.floor(draft.maxTokens * 0.75), 10000);

            return (
              <div key={draft.key} className="bitfun-ai-model-config__selected-model-row">
                <div
                  className={[
                    'bitfun-ai-model-config__selected-model-head',
                    canToggleExpand && 'bitfun-ai-model-config__selected-model-head--toggleable',
                  ].filter(Boolean).join(' ')}
                  onClick={canToggleExpand ? () => toggleSelectedModelCardExpanded(draft.key) : undefined}
                  onKeyDown={canToggleExpand ? (e) => onSelectedModelHeadKeyDown(e, draft.key) : undefined}
                  role={canToggleExpand ? 'button' : undefined}
                  tabIndex={canToggleExpand ? 0 : undefined}
                  aria-expanded={canToggleExpand ? isExpanded : undefined}
                  aria-label={
                    canToggleExpand
                      ? t(
                          isExpanded
                            ? 'providerSelection.collapseModelSettings'
                            : 'providerSelection.expandModelSettings',
                          { name: modelDisplayName }
                        )
                      : undefined
                  }
                >
                  <div className="bitfun-ai-model-config__selected-model-head-title">
                    <div className="bitfun-ai-model-config__selected-model-head-top">
                      <div className="bitfun-ai-model-config__selected-model-toggle">
                        {isExpanded ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
                      </div>
                      <div className="bitfun-ai-model-config__selected-model-name">{modelDisplayName}</div>
                    </div>
                    {!editingConfig.id && (
                      <IconButton
                        variant="ghost"
                        size="small"
                        className="bitfun-ai-model-config__selected-model-remove"
                        onClick={(e) => {
                          e.stopPropagation();
                          removeSelectedModelDraft(draft.modelName);
                        }}
                        tooltip={t('providerSelection.removeModel')}
                      >
                        <X size={14} />
                      </IconButton>
                    )}
                  </div>
                  {!isExpanded && (
                    <div className="bitfun-ai-model-config__selected-model-head-bottom">
                      <span className="bitfun-ai-model-config__selected-model-summary">
                        {categoryLabel}
                        {' · '}
                        {formatTokenCountShort(draft.contextWindow)} ctx
                        {' · '}
                        {formatTokenCountShort(draft.maxTokens)} out
                        {' · '}
                        {formatReasoningSummary(draft)}
                      </span>
                    </div>
                  )}
                </div>
                {isExpanded && (
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
                    {showReasoningModeControl && (
                      <div className="bitfun-ai-model-config__selected-model-field">
                        <span>{t('thinking.mode')}</span>
                        <Select
                          value={draft.reasoningMode}
                          onChange={(value) => updateModelDraft(draft.modelName, { reasoningMode: value as ReasoningMode })}
                          options={reasoningModeOptions}
                          size="small"
                        />
                      </div>
                    )}
                    {showReasoningEffortControl && (
                      <div className="bitfun-ai-model-config__selected-model-field">
                        <span>{t('reasoningEffort.label')}</span>
                        <Select
                          value={draft.reasoningEffort || ''}
                          onChange={(value) => updateModelDraft(draft.modelName, { reasoningEffort: (value as string) || undefined })}
                          placeholder={t('reasoningEffort.placeholder')}
                          options={reasoningEffortOptions}
                          size="small"
                        />
                      </div>
                    )}
                    {showThinkingBudgetControl && (
                      <div className="bitfun-ai-model-config__selected-model-field">
                        <span>{t('thinking.budgetTokens')}</span>
                        <NumberInput
                          value={displayedThinkingBudget}
                          onChange={(value) => updateModelDraft(draft.modelName, { thinkingBudgetTokens: value || undefined })}
                          min={1024}
                          max={50000}
                          step={1024}
                          size="small"
                          disableWheel
                        />
                      </div>
                    )}
                  </div>
                )}
              </div>
            );
          })}
        </div>
      );
    };

    return (
      <>
        <div className="bitfun-ai-model-config__form bitfun-ai-model-config__form--modal">
          <div className="bitfun-ai-model-config__form-scrollable">
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
                      const provider = value as string;
                      resetRemoteModelDiscovery();
                      setSelectedModelDrafts(prevDrafts =>
                        prevDrafts.map(draft => normalizeDraftReasoningForProvider(draft, provider))
                      );
                      setEditingConfig(prev => ({
                        ...prev,
                        provider,
                        request_url: resolveRequestUrl(prev?.base_url || '', provider, prev?.model_name || '')
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
                        setSelectedModelDrafts(prevDrafts =>
                          prevDrafts.map(draft => normalizeDraftReasoningForProvider(draft, provider))
                        );
                        setEditingConfig(prev => ({
                          ...prev,
                          provider,
                          request_url: resolveRequestUrl(prev?.base_url || '', provider, prev?.model_name || ''),
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
                {(editingConfig.provider === 'openai' || editingConfig.provider === 'anthropic') && (
                  <ConfigPageRow
                    label={t('advancedSettings.inlineThinkInText.label')}
                    description={t('advancedSettings.inlineThinkInText.hint')}
                    align="center"
                    className="bitfun-ai-model-config__toggle-row"
                  >
                    <Switch
                      checked={editingConfig.inline_think_in_text ?? true}
                      onChange={(e) => setEditingConfig(prev => ({ ...prev, inline_think_in_text: e.target.checked }))}
                      size="small"
                    />
                  </ConfigPageRow>
                )}
                <ConfigPageRow
                  label={t('advancedSettings.skipSslVerify.label')}
                  description={editingConfig.skip_ssl_verify ? (
                    <span className="bitfun-ai-model-config__warning-inline">
                      <AlertTriangle size={14} />
                      <span>{t('advancedSettings.skipSslVerify.warning')}</span>
                    </span>
                  ) : undefined}
                  align="center"
                  className="bitfun-ai-model-config__toggle-row"
                >
                  <Switch
                    checked={editingConfig.skip_ssl_verify || false}
                    onChange={(e) => setEditingConfig(prev => ({ ...prev, skip_ssl_verify: e.target.checked }))}
                    size="small"
                  />
                </ConfigPageRow>
                <ConfigPageRow
                  label={(
                    <span className="bitfun-ai-model-config__inline-header">
                      <span className="bitfun-ai-model-config__inline-header-main">
                        <span>{t('advancedSettings.customHeaders.label')}</span>
                        <Tooltip
                          content={(
                            <span className="bitfun-ai-model-config__header-tooltip">
                              <span>{t('advancedSettings.customHeaders.hint')}</span>
                              <span>
                                {(editingConfig.custom_headers_mode || 'merge') === 'replace'
                                  ? t('advancedSettings.customHeaders.modeReplaceHint')
                                  : t('advancedSettings.customHeaders.modeMergeHint')}
                              </span>
                            </span>
                          )}
                          placement="top"
                        >
                          <span
                            className="bitfun-ai-model-config__inline-header-info"
                            role="button"
                            tabIndex={0}
                            aria-label={t('advancedSettings.customHeaders.hint')}
                          >
                            <Info size={14} />
                          </span>
                        </Tooltip>
                      </span>
                      <span className="bitfun-ai-model-config__inline-header-actions">
                        <Tooltip content={t('advancedSettings.customHeaders.modeMergeHint')} placement="top">
                          <Button
                            type="button"
                            variant={(editingConfig.custom_headers_mode || 'merge') === 'merge' ? 'primary' : 'ghost'}
                            size="small"
                            className="bitfun-ai-model-config__mode-button"
                            onClick={() => setEditingConfig(prev => ({ ...prev, custom_headers_mode: 'merge' }))}
                          >
                            {t('advancedSettings.customHeaders.modeMerge')}
                          </Button>
                        </Tooltip>
                        <Tooltip content={t('advancedSettings.customHeaders.modeReplaceHint')} placement="top">
                          <Button
                            type="button"
                            variant={editingConfig.custom_headers_mode === 'replace' ? 'primary' : 'ghost'}
                            size="small"
                            className="bitfun-ai-model-config__mode-button"
                            onClick={() => setEditingConfig(prev => ({ ...prev, custom_headers_mode: 'replace' }))}
                          >
                            {t('advancedSettings.customHeaders.modeReplace')}
                          </Button>
                        </Tooltip>
                      </span>
                    </span>
                  )}
                  multiline
                  className="bitfun-ai-model-config__custom-headers-row"
                >
                  <div className="bitfun-ai-model-config__row-control--stack">
                    <div className="bitfun-ai-model-config__custom-headers">
                      {Object.entries(editingConfig.custom_headers || {}).map(([key, value], index) => (
                        <div key={index} className="bitfun-ai-model-config__header-row">
                          <Input value={key} onChange={(e) => { const nh = { ...editingConfig.custom_headers }; const ov = nh[key]; delete nh[key]; if (e.target.value) nh[e.target.value] = ov; setEditingConfig(prev => ({ ...prev, custom_headers: nh })); }} placeholder={t('advancedSettings.customHeaders.keyPlaceholder')} inputSize="small" className="bitfun-ai-model-config__header-key" />
                          <Input value={value} onChange={(e) => { const nh = { ...editingConfig.custom_headers }; nh[key] = e.target.value; setEditingConfig(prev => ({ ...prev, custom_headers: nh })); }} placeholder={t('advancedSettings.customHeaders.valuePlaceholder')} inputSize="small" className="bitfun-ai-model-config__header-value" />
                          <IconButton variant="ghost" size="small" onClick={() => { const nh = { ...editingConfig.custom_headers }; delete nh[key]; setEditingConfig(prev => ({ ...prev, custom_headers: Object.keys(nh).length > 0 ? nh : undefined })); }} tooltip={t('actions.delete')}><X size={14} /></IconButton>
                        </div>
                      ))}
                      <Button type="button" variant="secondary" size="small" onClick={() => setEditingConfig(prev => ({ ...prev, custom_headers: { ...prev?.custom_headers, '': '' } }))} className="bitfun-ai-model-config__add-header-btn"><Plus size={14} />{t('advancedSettings.customHeaders.addHeader')}</Button>
                    </div>
                  </div>
                </ConfigPageRow>
                <ConfigPageRow
                  label={(
                    <span className="bitfun-ai-model-config__inline-header">
                      <span className="bitfun-ai-model-config__inline-header-main">
                        <span>{t('advancedSettings.customRequestBody.label')}</span>
                        <Tooltip
                          content={(
                            <span className="bitfun-ai-model-config__header-tooltip">
                              <span>{t('advancedSettings.customRequestBody.hint')}</span>
                              <span>{getCustomRequestBodyModeHint(editingConfig.provider, editingConfig.custom_request_body_mode)}</span>
                            </span>
                          )}
                          placement="top"
                        >
                          <span
                            className="bitfun-ai-model-config__inline-header-info"
                            role="button"
                            tabIndex={0}
                            aria-label={t('advancedSettings.customRequestBody.hint')}
                          >
                            <Info size={14} />
                          </span>
                        </Tooltip>
                      </span>
                      <span className="bitfun-ai-model-config__inline-header-actions">
                        <Tooltip content={t('advancedSettings.customRequestBody.modeMergeHint')} placement="top">
                          <Button
                            type="button"
                            variant={(editingConfig.custom_request_body_mode || 'merge') === 'merge' ? 'primary' : 'ghost'}
                            size="small"
                            className="bitfun-ai-model-config__mode-button"
                            onClick={() => setEditingConfig(prev => ({ ...prev, custom_request_body_mode: 'merge' }))}
                          >
                            {t('advancedSettings.customRequestBody.modeMerge')}
                          </Button>
                        </Tooltip>
                        <Tooltip content={getCustomRequestBodyTrimHint(editingConfig.provider)} placement="top">
                          <Button
                            type="button"
                            variant={editingConfig.custom_request_body_mode === 'trim' ? 'primary' : 'ghost'}
                            size="small"
                            className="bitfun-ai-model-config__mode-button"
                            onClick={() => setEditingConfig(prev => ({ ...prev, custom_request_body_mode: 'trim' }))}
                          >
                            {t('advancedSettings.customRequestBody.modeTrim')}
                          </Button>
                        </Tooltip>
                      </span>
                    </span>
                  )}
                  multiline
                  className="bitfun-ai-model-config__custom-request-body-row"
                >
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
          </div>

          <div className="bitfun-ai-model-config__form-actions bitfun-ai-model-config__form-actions--sticky">
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
        <IconButton
          variant="ghost"
          size="small"
          isLoading={isTesting}
          onClick={() => void handleTest(config)}
          tooltip={t('actions.test')}
        >
          {isTesting ? <Loader size={14} /> : <Wifi size={14} />}
        </IconButton>
        <IconButton
          variant="ghost"
          size="small"
          onClick={() => handleEdit(config)}
          tooltip={t('actions.edit')}
        >
          <SquarePen size={14} />
        </IconButton>
        <IconButton
          variant="danger"
          size="small"
          onClick={() => void handleDelete(config.id!)}
          tooltip={t('actions.delete')}
        >
          <Trash2 size={14} />
        </IconButton>
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
              variant="ghost"
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
                        <SquarePen size={14} />
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
          title={t('streamIdleTimeout.title')}
          description={t('streamIdleTimeout.hint')}
          extra={(
            <Button
              variant="primary"
              size="small"
              onClick={handleSaveStreamIdleTimeout}
              disabled={isStreamIdleTimeoutSaving || isStreamIdleTimeoutInvalid}
            >
              {isStreamIdleTimeoutSaving ? (
                <Loader size={16} className="spinning" />
              ) : (
                t('streamIdleTimeout.save')
              )}
            </Button>
          )}
        >
          <ConfigPageRow
            label={t('streamIdleTimeout.label')}
            align="center"
          >
            <Input
              value={streamIdleTimeoutInput}
              onChange={(e) => setStreamIdleTimeoutInput(e.target.value)}
              placeholder={t('streamIdleTimeout.placeholder')}
              inputSize="small"
            />
          </ConfigPageRow>
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
        contentClassName="modal__content--fill-flex bitfun-ai-model-config__form--modal"
      >
        {renderEditingForm()}
      </Modal>
    </ConfigPageLayout>
  );
};

export default AIModelConfig;
