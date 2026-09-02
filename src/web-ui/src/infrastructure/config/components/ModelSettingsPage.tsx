import {
  Button,
  Card,
  Combobox,
  Icon,
  IconButton,
  Input,
  MultiSelect,
  NumberInput,
  SearchField,
  Select,
  Switch,
  Textarea,
  Tooltip,
  ScrollArea,
  type ComboboxOption,
  Dialog,
  DialogBody,
  DialogClose,
  DialogFooter,
  DialogHeader,
  DialogHeading,
  DialogTitle,
} from '@bitfun/ui';
import React, { useState, useEffect, useMemo, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { Wifi, Loader, AlertTriangle, EyeOff, FolderOpen } from 'lucide-react';
import {
  AIModelConfig as AIModelConfigType, 
  ProxyConfig, 
  ModelCategory,
  ReasoningCatalogBinding,
  ReasoningCatalogProjection,
  ReasoningConfig,
} from '../types';
import { configManager } from '../services/ConfigManager';
import { getCapabilitiesByCategory, resolveModelCategory } from '../services/modelCategory';
import {
  allocateModelConfigId,
  getModelDisplayName,
  getProviderDisplayName,
  getProviderGroupKey,
  getProviderInstanceId,
  getProviderTemplateId,
  PROVIDER_INSTANCE_METADATA_KEY,
} from '../services/modelConfigs';
import { resolveProviderTemplates } from '../services/builtinProviderCatalog';
import { normalizeProviderBaseUrl } from '../services/providerCatalog';
import { supportsResponsesReasoning } from '../utils/reasoning';
import { canonicalReasoningConfig, validateReasoningConfig } from '../utils/reasoningPresets';
import { aiApi, systemAPI } from '@/infrastructure/api';
import type {
  SubscriptionAccount,
  SubscriptionApiOffering,
  SubscriptionLoginMethod,
} from '@/infrastructure/api/service-api/AIApi';
import type { ProviderRegion } from '@/shared/types';
import type { OpenCodePlan, SubscriptionProvider } from '../types';
import { useNotification } from '@/shared/notification-system';
import { ConfigPageHeader, ConfigPageLayout, ConfigPageContent, ConfigPageSection, ConfigPageRow, ConfigCollectionItem } from './common';
import DefaultModelConfig from './DefaultModelConfig';
import ReasoningConfigPanel, { type ReasoningConfigApplyResult } from './ReasoningConfigPanel';
import { createLogger } from '@/shared/utils/logger';
import { translateConnectionTestMessage } from '@/shared/utils/aiConnectionTestMessages';
import { i18nService } from '@/infrastructure/i18n';
import { isTauriRuntime } from '@/infrastructure/runtime';
import { isPeerDeviceModeActive } from '@/infrastructure/peer-device/peerModeFlag';
import { LONG_CONTEXT_WARNING_THRESHOLD_TOKENS } from '@/shared/constants/modelContext';
import {
  preferredSubscriptionLoginMethod,
  settleSubscriptionLoginStart,
  subscriptionLoginRequiresLocalDevice,
  SubscriptionLoginCoordinator,
  type SubscriptionLoginOperation,
} from './subscriptionLoginCoordinator';
import './ModelSettingsPage.scss';

const log = createLogger('ModelSettings');
const MODELS_DEV_DOWNLOAD_URL = 'https://models.dev/api.json';

/** Rows the preset picker shows before the user searches or expands the list. */
const COLLAPSED_PROVIDER_COUNT = 6;

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
  maxTokens?: number;
  reasoning: ReasoningConfig;
  reasoningProjectionCatalog?: ReasoningCatalogBinding;
  reasoningProjectionSnapshot?: {
    catalog: ReasoningCatalogBinding;
    projection?: ReasoningCatalogProjection | null;
  };
}

interface ProviderGroup {
  key: string;
  providerName: string;
  providerId?: string;
  models: AIModelConfigType[];
}

interface SubscriptionLoginPanelState {
  provider: SubscriptionProvider;
  method?: SubscriptionLoginMethod;
  authorizationUrl: string;
  userCode?: string | null;
  deadlineMs?: number;
  status: 'starting' | 'pending' | 'cancelling' | 'failed';
  error?: string;
}

interface SubscriptionLogoutRequest {
  account: SubscriptionAccount;
  affectedModels: AIModelConfigType[];
}

const SUBSCRIPTION_LOGIN_TIMEOUT_MS = 5 * 60 * 1000;
const SUBSCRIPTION_MIGRATION_NOTICE_KEY = 'bitfun.subscription-auth.secure-store-notice.v1';

function subscriptionLoginCancelledError(): Error {
  const error = new Error('Login cancelled');
  error.name = 'SubscriptionLoginCancelled';
  return error;
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
  const reasoning = overrides?.reasoning ?? canonicalReasoningConfig(baseConfig as AIModelConfigType);

  return {
    key: overrides?.key ?? overrides?.configId ?? baseConfig?.id ?? trimmedModelName,
    configId: overrides?.configId ?? baseConfig?.id,
    modelName: trimmedModelName,
    category: overrides?.category ?? baseConfig?.category ?? 'general_chat',
    contextWindow: overrides?.contextWindow ?? baseConfig?.context_window ?? 200000,
    maxTokens: overrides?.maxTokens ?? baseConfig?.max_tokens,
    reasoning,
    reasoningProjectionCatalog: overrides?.reasoningProjectionCatalog ?? reasoning.catalog,
  };
}

function reasoningCatalogBindingsEqual(
  left?: ReasoningCatalogBinding,
  right?: ReasoningCatalogBinding,
): boolean {
  return JSON.stringify(left ?? { source: 'auto' }) === JSON.stringify(right ?? { source: 'auto' });
}

function uniqModelNames(modelNames: string[]): string[] {
  return Array.from(new Set(modelNames.map(name => name.trim()).filter(Boolean)));
}

function modelNameLookupKey(name: string): string {
  return name.trim().toLowerCase();
}

/**
 * Trim, optionally collapse to single selection, then dedupe so one provider
 * instance cannot list the same logical model twice.
 */
function normalizeProviderModelNameList(
  modelNames: string[],
  singleSelection: boolean
): string[] {
  let list = uniqModelNames(modelNames);
  if (singleSelection) {
    list = list.slice(0, 1);
  }
  const seen = new Set<string>();
  const out: string[] = [];
  for (const raw of list) {
    const resolved = raw.trim();
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

function generateProviderInstanceId(): string {
  return `provider_${Date.now()}_${Math.random().toString(36).slice(2, 9)}`;
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

/**
 * Compute the stored request URL from a base URL and provider format.
 * For Gemini, stores the bare base without the /v1beta/models/... suffix;
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
    if (trimmed.endsWith('/messages')) return trimmed;
    return trimmed.endsWith('/v1') ? `${trimmed}/messages` : `${trimmed}/v1/messages`;
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

function hasHttpUrlScheme(value: string): boolean {
  return /^https?:\/\//i.test(value.trim());
}

function stableJson(value: unknown): string {
  if (Array.isArray(value)) {
    return `[${value.map(stableJson).join(',')}]`;
  }
  if (value && typeof value === 'object') {
    return `{${Object.entries(value as Record<string, unknown>)
      .sort(([a], [b]) => a.localeCompare(b))
      .map(([key, entryValue]) => `${JSON.stringify(key)}:${stableJson(entryValue)}`)
      .join(',')}}`;
  }
  return JSON.stringify(value);
}

function normalizeComparableString(value: string | undefined): string {
  return (value || '').trim();
}

function providerConnectionChanged(
  previous: AIModelConfigType | undefined,
  next: AIModelConfigType
): boolean {
  if (!previous) return true;

  return (
    normalizeComparableString(previous.provider) !== normalizeComparableString(next.provider) ||
    normalizeComparableString(previous.base_url) !== normalizeComparableString(next.base_url) ||
    normalizeComparableString(previous.api_key) !== normalizeComparableString(next.api_key) ||
    stableJson(previous.auth || { type: 'api_key' }) !== stableJson(next.auth || { type: 'api_key' }) ||
    stableJson(previous.custom_headers || {}) !== stableJson(next.custom_headers || {}) ||
    normalizeComparableString(previous.custom_headers_mode) !== normalizeComparableString(next.custom_headers_mode) ||
    normalizeComparableString(previous.custom_request_body) !== normalizeComparableString(next.custom_request_body) ||
    normalizeComparableString(previous.custom_request_body_mode) !== normalizeComparableString(next.custom_request_body_mode) ||
    (previous.skip_ssl_verify ?? false) !== (next.skip_ssl_verify ?? false)
  );
}

function modelRequestBehaviorChanged(
  previous: AIModelConfigType | undefined,
  next: AIModelConfigType
): boolean {
  if (!previous) return true;

  return (
    normalizeComparableString(previous.model_name) !== normalizeComparableString(next.model_name) ||
    normalizeComparableString(previous.request_url) !== normalizeComparableString(next.request_url) ||
    previous.context_window !== next.context_window ||
    previous.max_tokens !== next.max_tokens ||
    previous.category !== next.category ||
    stableJson(previous.capabilities || []) !== stableJson(next.capabilities || []) ||
    stableJson(canonicalReasoningConfig(previous)) !== stableJson(next.reasoning || canonicalReasoningConfig(next)) ||
    (previous.inline_think_in_text ?? true) !== (next.inline_think_in_text ?? true)
  );
}

function modelDraftHasUnsavedChanges(
  draft: SelectedModelDraft,
  persistedModels: AIModelConfigType[],
): boolean {
  const persisted = draft.configId
    ? persistedModels.find(model => model.id === draft.configId)
    : undefined;

  if (!persisted) return true;

  return (
    normalizeComparableString(draft.modelName) !== normalizeComparableString(persisted.model_name) ||
    draft.category !== (persisted.category ?? 'general_chat') ||
    draft.contextWindow !== (persisted.context_window || 200000) ||
    draft.maxTokens !== persisted.max_tokens ||
    stableJson(draft.reasoning) !== stableJson(canonicalReasoningConfig(persisted))
  );
}

function configsNeedingAutoTest(
  previousModels: AIModelConfigType[],
  nextConfigs: AIModelConfigType[],
  isProviderGroupEdit: boolean
): AIModelConfigType[] {
  const previousById = new Map(previousModels.map(model => [model.id, model]));
  const providerConnectionWasChanged = isProviderGroupEdit && nextConfigs.some(config =>
    providerConnectionChanged(previousById.get(config.id), config)
  );

  if (providerConnectionWasChanged) {
    return nextConfigs;
  }

  return nextConfigs.filter(config => {
    const previous = previousById.get(config.id);
    return (
      !previous ||
      providerConnectionChanged(previous, config) ||
      modelRequestBehaviorChanged(previous, config)
    );
  });
}

const ModelSettingsPage: React.FC = () => {
  const { t, i18n } = useTranslation('settings/models');
  const { t: tDefault } = useTranslation('settings/default-model');
  const { t: tComponents } = useTranslation('components');
  const [aiModels, setAiModels] = useState<AIModelConfigType[]>([]);
  const [modelCatalog, setModelCatalog] = useState<Awaited<ReturnType<typeof aiApi.getModelCatalog>> | null>(null);
  const [modelsDevStatus, setModelsDevStatus] = useState<Awaited<ReturnType<typeof aiApi.getModelsDevCatalogStatus>> | null>(null);
  const [modelsDevStatusAvailable, setModelsDevStatusAvailable] = useState(true);
  const [isRefreshingModelsDev, setIsRefreshingModelsDev] = useState(false);
  const [showModelsDevDetails, setShowModelsDevDetails] = useState(false);
  const [isEditing, setIsEditing] = useState(false);
  const [editingConfig, setEditingConfig] = useState<Partial<AIModelConfigType> | null>(null);
  const [showApiKey, setShowApiKey] = useState(false);
  const [testingConfigs, setTestingConfigs] = useState<Record<string, boolean>>({});
  const [testResults, setTestResults] = useState<Record<string, { success: boolean; message: string } | null>>({});
  const [expandedIds, setExpandedIds] = useState<Set<string>>(new Set());
  const notification = useNotification();
  
  const [showAdvancedSettings, setShowAdvancedSettings] = useState(false);

  const [creationMode, setCreationMode] = useState<'selection' | 'form' | null>(null);
  const [providerQuery, setProviderQuery] = useState('');
  const [showAllProviders, setShowAllProviders] = useState(false);

  const [selectedProviderId, setSelectedProviderId] = useState<string | null>(null);
  const [proxyConfig, setProxyConfig] = useState<ProxyConfig>({
    enabled: false,
    url: '',
    username: '',
    password: ''
  });
  const [streamIdleTimeoutInput, setStreamIdleTimeoutInput] = useState('');
  const [streamTtftTimeoutInput, setStreamTtftTimeoutInput] = useState('');
  const [isStreamTimeoutSaving, setIsStreamTimeoutSaving] = useState(false);
  const [isProxySaving, setIsProxySaving] = useState(false);
  const [remoteModelOptions, setRemoteModelOptions] = useState<RemoteModelOption[]>([]);
  const [isFetchingRemoteModels, setIsFetchingRemoteModels] = useState(false);
  const [remoteModelsError, setRemoteModelsError] = useState<string | null>(null);
  const [hasAttemptedRemoteFetch, setHasAttemptedRemoteFetch] = useState(false);
  const [selectedModelDrafts, setSelectedModelDrafts] = useState<SelectedModelDraft[]>([]);
  const [editingProviderModelIds, setEditingProviderModelIds] = useState<Set<string>>(new Set());
  const [manualModelInput, setManualModelInput] = useState('');
  const [expandedModelCards, setExpandedModelCards] = useState<Set<string>>(new Set());
  const [reasoningPanelDraftKey, setReasoningPanelDraftKey] = useState<string | null>(null);
  const [subscriptionAccounts, setSubscriptionAccounts] = useState<SubscriptionAccount[]>([]);
  const [isLoadingSubscriptions, setIsLoadingSubscriptions] = useState(false);
  const [loggingInProvider, setLoggingInProvider] = useState<SubscriptionProvider | null>(null);
  const [subscriptionLoginPanel, setSubscriptionLoginPanel] = useState<SubscriptionLoginPanelState | null>(null);
  const [subscriptionLoginClock, setSubscriptionLoginClock] = useState(() => Date.now());
  const [subscriptionLogoutRequest, setSubscriptionLogoutRequest] = useState<SubscriptionLogoutRequest | null>(null);
  const [showSubscriptionMigrationNotice, setShowSubscriptionMigrationNotice] = useState(() => {
    if (typeof window === 'undefined') return false;
    try {
      return window.localStorage.getItem(SUBSCRIPTION_MIGRATION_NOTICE_KEY) !== 'dismissed';
    } catch {
      return true;
    }
  });
  const lastRemoteFetchSignatureRef = React.useRef<string | null>(null);
  const activeRemoteFetchSignatureRef = React.useRef<string | null>(null);

  const requestFormatOptions = useMemo(
    () => [
      { label: 'OpenAI (chat/completions)', value: 'openai' },
      { label: 'OpenAI (responses)', value: 'responses' },
      { label: 'Anthropic (messages)', value: 'anthropic' },
      { label: 'Gemini (generateContent)', value: 'gemini' },
      { label: 'Gemini Code Assist (cloudcode-pa)', value: 'gemini-code-assist' },
    ],
    []
  );
  const requestFormatLabelMap = useMemo(
    () => Object.fromEntries(
      requestFormatOptions.map(option => [String(option.value), option.label])
    ) as Record<string, string>,
    [requestFormatOptions]
  );

  const categoryOptions = useMemo<ComboboxOption[]>(
    () => [
      { label: t('category.general_chat'), value: 'general_chat' },
      { label: t('category.multimodal'), value: 'multimodal' },
      { label: t('category.speech_recognition'), value: 'speech_recognition' },
    ],
    [t]
  );

  const categoryCompactLabels = useMemo<Record<ModelCategory, string>>(
    () => ({
      general_chat: t('categoryIcons.general_chat'),
      multimodal: t('categoryIcons.multimodal'),
      speech_recognition: t('categoryIcons.speech_recognition'),
    }),
    [t]
  );
  const parsedStreamIdleTimeout = useMemo(
    () => parseOptionalPositiveIntegerInput(streamIdleTimeoutInput),
    [streamIdleTimeoutInput]
  );
  const parsedStreamTtftTimeout = useMemo(
    () => parseOptionalPositiveIntegerInput(streamTtftTimeoutInput),
    [streamTtftTimeoutInput]
  );
  const isStreamIdleTimeoutInvalid = parsedStreamIdleTimeout === undefined;
  const isStreamTtftTimeoutInvalid = parsedStreamTtftTimeout === undefined;
  const isStreamTimeoutInvalid = isStreamIdleTimeoutInvalid || isStreamTtftTimeoutInvalid;

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

  const loadModelCatalog = useCallback(async () => {
    try {
      setModelCatalog(await aiApi.getModelCatalog());
    } catch (error) {
      setModelCatalog(null);
      log.warn('Failed to load model reasoning catalog', { error });
    }
  }, []);

  const loadModelsDevStatus = useCallback(async () => {
    try {
      setModelsDevStatus(await aiApi.getModelsDevCatalogStatus());
      setModelsDevStatusAvailable(true);
    } catch (error) {
      setModelsDevStatusAvailable(false);
      log.warn('Failed to load models.dev catalog status', { error });
    }
  }, []);

  const handleRefreshModelsDev = useCallback(async () => {
    setIsRefreshingModelsDev(true);
    try {
      const result = await aiApi.refreshModelsDevCatalogNow();
      setModelsDevStatus(result.status);
      await loadModelCatalog();
      notification.success(
        result.outcome === 'updated'
          ? t('modelsDevCatalog.refreshSuccess')
          : result.outcome === 'throttled'
            ? t('modelsDevCatalog.refreshThrottled')
            : t('modelsDevCatalog.alreadyCurrent'),
      );
    } catch (error) {
      log.warn('Failed to refresh models.dev catalog', { error });
      notification.error(t('modelsDevCatalog.refreshFailed'));
      await loadModelsDevStatus();
    } finally {
      setIsRefreshingModelsDev(false);
    }
  }, [loadModelCatalog, loadModelsDevStatus, notification, t]);

  const loadConfig = useCallback(async () => {
    try {
      const [models, proxy, streamIdleTimeoutSecs, streamTtftTimeoutSecs] = await Promise.all([
        configManager.getConfig<AIModelConfigType[]>('ai.models'),
        configManager.getConfig<ProxyConfig>('ai.proxy'),
        configManager.getConfig<number | null>('ai.stream_idle_timeout_secs'),
        configManager.getConfig<number | null>('ai.stream_ttft_timeout_secs'),
      ]);
      setAiModels(models);
      await loadModelCatalog();
      await loadModelsDevStatus();
      if (proxy) {
        setProxyConfig(proxy);
      }
      setStreamIdleTimeoutInput(
        streamIdleTimeoutSecs != null ? String(streamIdleTimeoutSecs) : ''
      );
      setStreamTtftTimeoutInput(
        streamTtftTimeoutSecs != null ? String(streamTtftTimeoutSecs) : ''
      );
    } catch (error) {
      log.error('Failed to load AI config', error);
    }
  }, [loadModelCatalog, loadModelsDevStatus]);

  useEffect(() => {
    const unsubscribeCatalog = aiApi.onModelCatalogUpdated(() => {
      void loadModelCatalog();
      void loadModelsDevStatus();
    });
    loadConfig();
    return unsubscribeCatalog;
  }, [loadConfig, loadModelCatalog, loadModelsDevStatus]);

  const refreshSubscriptionAccounts = useCallback(async () => {
    setIsLoadingSubscriptions(true);
    try {
      const items = await aiApi.listSubscriptionAccounts();
      setSubscriptionAccounts(items);
    } catch (e) {
      log.warn('list_subscription_accounts failed', { error: String(e) });
    } finally {
      setIsLoadingSubscriptions(false);
    }
  }, []);

  useEffect(() => {
    refreshSubscriptionAccounts();
  }, [refreshSubscriptionAccounts]);

  useEffect(() => {
    if (!subscriptionLoginPanel || subscriptionLoginPanel.status !== 'pending') return;
    setSubscriptionLoginClock(Date.now());
    const timer = window.setInterval(() => setSubscriptionLoginClock(Date.now()), 1000);
    return () => window.clearInterval(timer);
  }, [subscriptionLoginPanel]);
  
  // Provider options with translations (must be at top level, before any conditional returns)
  const providerTemplates = useMemo(
    () => resolveProviderTemplates(modelCatalog?.provider_catalog),
    [modelCatalog?.provider_catalog],
  );
  const providerOrder = useMemo(
    () => Object.values(providerTemplates)
      .sort((left, right) => (left.displayOrder ?? 999) - (right.displayOrder ?? 999))
      .map(provider => provider.id),
    [providerTemplates],
  );
  // A Chinese UI leads with mainland providers, every other UI leads with the
  // international ones. Both keep the full list, only the order changes.
  const preferredProviderRegion: ProviderRegion = i18n.language.toLowerCase().startsWith('zh') ? 'cn' : 'global';
  const providers = useMemo(() => {
    const regionRank = (region: ProviderRegion) => {
      if (region === 'any') return 0;
      return region === preferredProviderRegion ? 1 : 2;
    };

    // Dynamically get translated name and description
    return Object.values(providerTemplates)
      .map(provider => {
        const localizedName = t(`providers.${provider.id}.name`);
        const localizedDescription = t(`providers.${provider.id}.description`);
        return {
          ...provider,
          name: localizedName,
          description: localizedDescription,
          // Keeps the catalog's English name searchable while a CJK locale renders the localized one.
          searchText: [provider.id, provider.name, localizedName, localizedDescription, ...provider.models]
            .join(' ')
            .toLowerCase(),
        };
      })
      .sort((left, right) => (
        regionRank(left.region ?? 'any') - regionRank(right.region ?? 'any')
        || (left.displayOrder ?? 999) - (right.displayOrder ?? 999)
        || left.name.localeCompare(right.name)
      ));
  }, [preferredProviderRegion, providerTemplates, t]);

  const normalizedProviderQuery = providerQuery.trim().toLowerCase();
  const matchedProviders = useMemo(() => (
    normalizedProviderQuery
      ? providers.filter(provider => provider.searchText.includes(normalizedProviderQuery))
      : providers
  ), [normalizedProviderQuery, providers]);
  // Searching always reveals every hit; only the resting list stays short.
  const canToggleProviderList = !normalizedProviderQuery
    && matchedProviders.length > COLLAPSED_PROVIDER_COUNT;
  const isProviderListCollapsed = canToggleProviderList && !showAllProviders;
  const visibleProviders = isProviderListCollapsed
    ? matchedProviders.slice(0, COLLAPSED_PROVIDER_COUNT)
    : matchedProviders;

  // Current template with translations (must be at top level, before any conditional returns)
  const currentTemplate = useMemo(() => {
    if (!selectedProviderId) return null;
    const template = providerTemplates[selectedProviderId];
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
  }, [providerTemplates, selectedProviderId, t]);

  const createDraftsFromConfigs = (configs: AIModelConfigType[]) => (
    configs.map(config => createModelDraft(config.model_name, config, {
      configId: config.id,
      contextWindow: config.context_window || 200000,
      maxTokens: config.max_tokens,
      reasoning: canonicalReasoningConfig(config),
    }))
  );

  const resetRemoteModelDiscovery = useCallback(() => {
    setRemoteModelOptions([]);
    setIsFetchingRemoteModels(false);
    setRemoteModelsError(null);
    setHasAttemptedRemoteFetch(false);
    lastRemoteFetchSignatureRef.current = null;
    activeRemoteFetchSignatureRef.current = null;
  }, []);

  const getOpenCodePlanLabel = useCallback((plan: OpenCodePlan): string => (
    plan === 'go'
      ? t('subscriptionAuth.openCodePlans.go.label')
      : t('subscriptionAuth.openCodePlans.zen.label')
  ), [t]);

  const getOpenCodePlanDescription = useCallback((plan: OpenCodePlan): string => (
    plan === 'go'
      ? t('subscriptionAuth.openCodePlans.go.description')
      : t('subscriptionAuth.openCodePlans.zen.description')
  ), [t]);

  const getOpenCodeFormatLabel = useCallback((format: SubscriptionApiOffering['format']): string => {
    if (format === 'responses') return t('subscriptionAuth.openCodeFormats.responses');
    if (format === 'anthropic') return t('subscriptionAuth.openCodeFormats.messages');
    return t('subscriptionAuth.openCodeFormats.chatCompletions');
  }, [t]);

  const syncSelectedModelDrafts = (
    modelNames: string[],
    baseConfig?: Partial<AIModelConfigType>,
    singleSelection = false
  ) => {
    const nextModelNames = normalizeProviderModelNameList(
      modelNames,
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

        if (existingDraft) {
          const configId = pinnedRowId ?? existingDraft.configId;
          return {
            ...existingDraft,
            modelName,
            configId,
            key: configId ?? modelName,
          };
        }

        const draftBaseConfig = baseConfig
          ? { ...baseConfig, max_tokens: undefined }
          : undefined;

        const catalogModel = selectedProviderId
          ? modelCatalog?.provider_catalog?.providers
              .find(provider => provider.id === selectedProviderId)
              ?.models.find(model => model.id === modelName)
          : undefined;
        return createModelDraft(modelName, draftBaseConfig, {
          configId: pinnedRowId,
          contextWindow: catalogModel?.limits?.context,
          category: catalogModel?.capabilities.attachment ? 'multimodal' : undefined,
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

  const resolveDraftCatalogEntry = (draft: SelectedModelDraft) => (
    modelCatalog?.models.find(model => (
      model.id === draft.configId
      || model.id === editingConfig?.id
      || (
        model.model_name === draft.modelName
        && model.provider === (editingConfig?.provider || 'openai')
        && model.base_url === editingConfig?.base_url
      )
    ))
  );

  const resolveDraftReasoningProjection = (draft: SelectedModelDraft) => {
    const snapshot = draft.reasoningProjectionSnapshot;
    if (snapshot && reasoningCatalogBindingsEqual(draft.reasoning.catalog, snapshot.catalog)) {
      return snapshot.projection ?? undefined;
    }
    if (reasoningCatalogBindingsEqual(draft.reasoning.catalog, draft.reasoningProjectionCatalog)) {
      return resolveDraftCatalogEntry(draft)?.reasoning;
    }
    return undefined;
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

    const alreadyInDrafts = selectedModelDrafts.some(
      draft => modelNameLookupKey(draft.modelName) === modelNameLookupKey(trimmedModelName)
    );

    if (alreadyInDrafts) {
      notification.info(t('providerSelection.modelAlreadyInList'));
      setManualModelInput('');
      return;
    }

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
    const resolvedAuth = config.auth || { type: 'api_key' };
    const resolvedApiKey = (config.api_key || '').trim();
    const resolvedModelName = (
      config.model_name ||
      selectedModelDrafts[0]?.modelName ||
      currentTemplate?.models[0] ||
      'model-discovery'
    ).trim();

    // CLI-backed auth (Codex/Gemini) resolves the bearer token at request time
    // from `~/.codex` or `~/.gemini`, so we must NOT gate discovery on the
    // user pasting an API key. Only the legacy `api_key` mode requires it.
    const requiresApiKey = resolvedAuth.type === 'api_key';
    if (!resolvedBaseUrl || !resolvedProvider || (requiresApiKey && !resolvedApiKey)) {
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
      context_window: config.context_window || 200000,
      max_tokens: config.max_tokens,
      temperature: config.temperature,
      top_p: config.top_p,
      enabled: config.enabled ?? true,
      category: config.category || 'general_chat',
      capabilities: config.capabilities || ['text_chat'],
      recommended_for: config.recommended_for || [],
      metadata: config.metadata || {},
      inline_think_in_text: config.inline_think_in_text ?? true,
      reasoning: canonicalReasoningConfig(config),
      custom_headers: config.custom_headers,
      custom_headers_mode: config.custom_headers_mode,
      skip_ssl_verify: config.skip_ssl_verify ?? false,
      custom_request_body: config.custom_request_body,
      custom_request_body_mode: config.custom_request_body_mode,
      auth: resolvedAuth,
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
    auth: config.auth || { type: 'api_key' },
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
    const authType = editingConfig.auth?.type ?? 'api_key';
    if (authType === 'api_key' && !editingConfig.api_key?.trim()) return;
    if (hasAttemptedRemoteFetch) return;
    if (remoteModelOptions.length > 0) return;
    void fetchRemoteModels(editingConfig);
  };

  
  const handleCreateNew = () => {
    resetRemoteModelDiscovery();
    setSelectedModelDrafts([]);
    setEditingProviderModelIds(new Set());
    setManualModelInput('');
    setShowApiKey(false);
    setSelectedProviderId(null);
    setProviderQuery('');
    setShowAllProviders(false);
    setCreationMode('selection');
  };

  const handleImportFromSubscription = useCallback((
    account: SubscriptionAccount,
    offering?: SubscriptionApiOffering,
  ) => {
    resetRemoteModelDiscovery();
    const offeringModels = (offering?.models || []).map((model) => ({
      id: model.id,
      display_name: model.display_name || undefined,
    }));
    if (offeringModels.length > 0) {
      setRemoteModelOptions(offeringModels);
      setHasAttemptedRemoteFetch(true);
    }
    setManualModelInput('');
    setShowApiKey(false);
    setSelectedProviderId(null);
    setEditingConfig({
      name: offering
        ? getOpenCodePlanLabel(offering.plan)
        : account.display_label,
      provider: offering?.format || account.suggested_format,
      base_url: offering?.base_url || account.suggested_base_url,
      // Leave request_url + model_name empty so the user must pick a model
      // from the live list. We never inject a hard-coded default slug.
      request_url: '',
      api_key: '',
      model_name: '',
      enabled: true,
      context_window: 200000,
      category: 'general_chat',
      capabilities: ['text_chat', 'function_calling'],
      recommended_for: [],
      metadata: {},
      inline_think_in_text: true,
      auth: {
        type: 'subscription',
        provider: account.provider,
        ...(offering ? { plan: offering.plan } : {}),
      },
    });
    setSelectedModelDrafts([]);
    setEditingProviderModelIds(new Set());
    setShowAdvancedSettings(false);
    setCreationMode('form');
    setIsEditing(true);
  }, [getOpenCodePlanLabel, resetRemoteModelDiscovery]);

  const loginCoordinatorRef = React.useRef(new SubscriptionLoginCoordinator());
  const subscriptionLoginMountedRef = React.useRef(true);

  const pollSubscriptionLogin = useCallback(async (
    operation: SubscriptionLoginOperation,
    deadline: number,
  ) => {
    while (Date.now() < deadline) {
      if (!loginCoordinatorRef.current.isCurrent(operation)) {
        throw subscriptionLoginCancelledError();
      }
      const snapshot = await aiApi.getSubscriptionLoginStatus(
        operation.provider,
        operation.sessionId,
      );
      if (snapshot.session_id !== operation.sessionId) {
        throw new Error('Subscription login status returned a mismatched session');
      }
      if (snapshot.status === 'authorized') {
        return snapshot;
      }
      if (snapshot.status === 'failed' || snapshot.status === 'cancelled') {
        throw new Error(snapshot.error || `Login ${snapshot.status}`);
      }
      await new Promise((resolve) => setTimeout(resolve, 1500));
    }
    throw new Error('Login timed out');
  }, []);

  // Cancel any in-flight subscription login when the page unmounts so the
  // backend loopback server / device poll does not linger.
  useEffect(() => {
    const coordinator = loginCoordinatorRef.current;
    subscriptionLoginMountedRef.current = true;
    return () => {
      subscriptionLoginMountedRef.current = false;
      const pending = coordinator.current();
      if (pending && !pending.cancelled) {
        coordinator.requestCancel(pending.provider);
        // Cancel immediately when the backend placeholder already exists;
        // `settleSubscriptionLoginStart` retries after start returns to cover
        // the opposite command-order race.
        void aiApi.cancelSubscriptionLogin(pending.provider, pending.sessionId).catch(() => {});
      }
    };
  }, []);

  const handleSubscriptionLogin = useCallback(async (provider: SubscriptionProvider) => {
    if (isPeerDeviceModeActive() && subscriptionLoginRequiresLocalDevice(provider)) {
      notification.error(t('subscriptionAuth.peerLoginRequiresLocalDevice'));
      return;
    }
    // The settings surface intentionally permits one authorization flow at a
    // time. This prevents a stale provider poll/finally block from clearing a
    // newer provider's state or leaving an undiscoverable backend session.
    const operation = loginCoordinatorRef.current.begin(provider);
    if (!operation) return;
    const requestedMethod = preferredSubscriptionLoginMethod(
      provider,
      isTauriRuntime() && !isPeerDeviceModeActive(),
    );
    setLoggingInProvider(provider);
    setSubscriptionLoginPanel({
      provider,
      method: requestedMethod,
      authorizationUrl: '',
      status: 'starting',
    });
    try {
      const started = await aiApi.startSubscriptionLogin(
        provider,
        operation.sessionId,
        requestedMethod,
      );
      const settlement = await settleSubscriptionLoginStart(
        loginCoordinatorRef.current,
        operation,
        () => aiApi.cancelSubscriptionLogin(provider, operation.sessionId),
      );
      if (settlement.cleanupError) {
        log.warn('Failed to cancel subscription login after start settled', {
          provider,
          error: String(settlement.cleanupError),
        });
      }
      if (!settlement.shouldContinue) {
        throw subscriptionLoginCancelledError();
      }
      if (started.session_id !== operation.sessionId) {
        throw new Error('Subscription login start returned a mismatched session');
      }
      // Older backends ignore the new method request and omit `method` in the
      // response. Infer their actual flow from the returned device code so a
      // new UI never shows device instructions for a legacy browser login.
      const actualMethod = started.method || (started.user_code ? 'device' : 'browser');
      // Authorization time starts after the provider has returned its URL or
      // device code; callback binding/device-code acquisition does not consume
      // the user's five-minute completion window.
      const deadlineMs = Date.now() + SUBSCRIPTION_LOGIN_TIMEOUT_MS;
      setSubscriptionLoginPanel({
        provider,
        method: actualMethod,
        authorizationUrl: started.authorization_url,
        userCode: started.user_code,
        deadlineMs,
        status: 'pending',
      });
      if (started.authorization_url) {
        try {
          await systemAPI.openExternal(started.authorization_url);
        } catch (openError) {
          // Keep polling: the backend login session is already running. Surface
          // the URL so the user can open it manually (relative URLs / opener
          // policy failures must not abort an otherwise valid login).
          log.warn('Failed to open subscription authorization URL', {
            provider,
            url: started.authorization_url,
            error: String(openError),
          });
          notification.info(
            t('subscriptionAuth.openUrlManually', { url: started.authorization_url }),
          );
        }
      }
      if (!loginCoordinatorRef.current.isCurrent(operation)) {
        throw subscriptionLoginCancelledError();
      }
      if (started.user_code) {
        notification.info(t('subscriptionAuth.userCodeHint', { code: started.user_code }));
      }
      await pollSubscriptionLogin(operation, deadlineMs);
      if (!loginCoordinatorRef.current.isCurrent(operation)) {
        throw subscriptionLoginCancelledError();
      }
      await refreshSubscriptionAccounts();
      if (!loginCoordinatorRef.current.isCurrent(operation)) {
        throw subscriptionLoginCancelledError();
      }
      setSubscriptionLoginPanel(null);
      notification.success(t('subscriptionAuth.loginSuccess'));
    } catch (e) {
      if ((e as Error).name === 'SubscriptionLoginCancelled' || operation.cancelled) {
        // `startSubscriptionLogin` may reject instead of returning after an
        // early cancellation. Mark that invocation settled and retry the
        // idempotent backend cancellation so no placeholder/session survives.
        if (!operation.startSettled && loginCoordinatorRef.current.owns(operation)) {
          loginCoordinatorRef.current.markStartSettled(operation);
        }
        let authorizationAlreadyCompleted = false;
        try {
          await aiApi.cancelSubscriptionLogin(provider, operation.sessionId);
        } catch (cancelError) {
          log.warn('Failed to finish subscription login cancellation', {
            provider,
            error: String(cancelError),
          });
        }
        try {
          // The backend cancellation command is a commit barrier. Refreshing
          // now truthfully surfaces the narrow case where authorization had
          // already crossed its commit boundary before the user cancelled.
          const accounts = await aiApi.listSubscriptionAccounts();
          if (subscriptionLoginMountedRef.current) {
            setSubscriptionAccounts(accounts);
          }
          authorizationAlreadyCompleted = accounts.some((account) => (
            account.provider === provider && account.connected
          ));
        } catch (refreshError) {
          log.warn('Failed to refresh subscription accounts after cancellation', {
            provider,
            error: String(refreshError),
          });
        }
        if (
          loginCoordinatorRef.current.owns(operation)
          && subscriptionLoginMountedRef.current
        ) {
          setSubscriptionLoginPanel(null);
          notification.info(t(
            authorizationAlreadyCompleted
              ? 'subscriptionAuth.loginCompletedBeforeCancel'
              : 'subscriptionAuth.loginCancelled',
          ));
        }
      } else {
        // Status/start failures can occur while the backend runner is still
        // active. Await the session-scoped cancellation barrier before freeing
        // the coordinator slot or presenting retry UI.
        if (!operation.startSettled && loginCoordinatorRef.current.owns(operation)) {
          loginCoordinatorRef.current.markStartSettled(operation);
        }
        try {
          await aiApi.cancelSubscriptionLogin(provider, operation.sessionId);
        } catch (cancelError) {
          log.warn('Failed to stop subscription login after an operation error', {
            provider,
            sessionId: operation.sessionId,
            error: String(cancelError),
          });
        }
        if (
          loginCoordinatorRef.current.isCurrent(operation)
          && subscriptionLoginMountedRef.current
        ) {
          setSubscriptionLoginPanel({
            provider,
            method: requestedMethod,
            authorizationUrl: '',
            userCode: undefined,
            status: 'failed',
            error: String(e),
          });
          notification.error(t('subscriptionAuth.loginFailed', { error: String(e) }));
        }
      }
    } finally {
      if (loginCoordinatorRef.current.complete(operation)) {
        if (subscriptionLoginMountedRef.current) {
          setLoggingInProvider(null);
        }
      }
    }
  }, [notification, pollSubscriptionLogin, refreshSubscriptionAccounts, t]);

  const handleCancelSubscriptionLogin = useCallback(async (provider: SubscriptionProvider) => {
    const operation = loginCoordinatorRef.current.requestCancel(provider);
    if (!operation) return;
    // Keep the coordinator slot and loading state reserved until the start
    // command has settled and any backend session has been cancelled.
    setSubscriptionLoginPanel((current) => (
      current?.provider === provider
        ? { ...current, status: 'cancelling' }
        : current
    ));
    // This first attempt makes cancellation responsive if the backend has
    // installed its placeholder. The start-settlement path retries, because
    // desktop command scheduling can deliver this request first.
    try {
      await aiApi.cancelSubscriptionLogin(provider, operation.sessionId);
    } catch (e) {
      log.warn('cancel_subscription_login failed', { error: String(e) });
    }
  }, []);

  const handleOpenSubscriptionAuthorization = useCallback(async (url: string) => {
    if (!url) return;
    try {
      await systemAPI.openExternal(url);
    } catch (error) {
      log.warn('Failed to open subscription authorization URL from pending card', {
        url,
        error: String(error),
      });
      notification.info(t('subscriptionAuth.openUrlManually', { url }));
    }
  }, [notification, t]);

  const handleCopySubscriptionCode = useCallback(async (code: string) => {
    try {
      await systemAPI.setClipboard(code);
      notification.success(t('subscriptionAuth.codeCopied'));
    } catch (error) {
      log.warn('Failed to copy subscription device code', { error: String(error) });
      notification.error(t('subscriptionAuth.copyCodeFailed'));
    }
  }, [notification, t]);

  const requestSubscriptionLogout = useCallback((account: SubscriptionAccount) => {
    const affectedModels = aiModels.filter((model) => (
      model.auth?.type === 'subscription' && model.auth.provider === account.provider
    ));
    setSubscriptionLogoutRequest({ account, affectedModels });
  }, [aiModels]);

  const confirmSubscriptionLogout = useCallback(async () => {
    const request = subscriptionLogoutRequest;
    if (!request) return;
    try {
      const result = await aiApi.logoutSubscriptionAccount(request.account.provider);
      // Metadata removal is the source of truth for connection state. Reflect
      // it immediately, then refresh before presenting either outcome notice.
      setSubscriptionAccounts((current) => current.map((account) => (
        account.provider === request.account.provider
          ? {
              ...account,
              connected: false,
              account: null,
              expires_at: null,
              reauthentication_required: false,
              vault_unavailable: false,
            }
          : account
      )));
      await refreshSubscriptionAccounts();
      setSubscriptionLogoutRequest(null);
      if (result.cleanup_pending) {
        log.warn('Subscription logout completed with credential cleanup pending', {
          provider: request.account.provider,
          warning: result.warning,
        });
        notification.warning(t('subscriptionAuth.logoutCleanupPending'));
      } else {
        notification.success(t('subscriptionAuth.logoutSuccess'));
      }
    } catch (e) {
      notification.error(t('subscriptionAuth.logoutFailed', { error: String(e) }));
    }
  }, [notification, refreshSubscriptionAccounts, subscriptionLogoutRequest, t]);

  const dismissSubscriptionMigrationNotice = useCallback(() => {
    setShowSubscriptionMigrationNotice(false);
    try {
      window.localStorage.setItem(SUBSCRIPTION_MIGRATION_NOTICE_KEY, 'dismissed');
    } catch (error) {
      log.debug('Unable to persist subscription migration notice dismissal', { error: String(error) });
    }
  }, []);

  const handleSubscriptionRefresh = useCallback(async (provider: SubscriptionProvider) => {
    try {
      await aiApi.refreshSubscriptionAccount(provider);
      await refreshSubscriptionAccounts();
      notification.success(t('subscriptionAuth.refreshSuccess'));
    } catch (e) {
      notification.error(t('subscriptionAuth.refreshFailed', { error: String(e) }));
    }
  }, [notification, refreshSubscriptionAccounts, t]);

  
  const handleSelectProvider = (providerId: string) => {
    const template = providerTemplates[providerId];
    if (!template) return;
    resetRemoteModelDiscovery();
    setManualModelInput('');
    setShowApiKey(false);
    setSelectedProviderId(providerId);
    
    // Dynamically get translated name
    const providerName = t(`providers.${template.id}.name`);
    const defaultModel = template.models[0] || '';
    
    setEditingConfig({
      name: providerName,
      base_url: template.baseUrl,
      request_url: resolveRequestUrl(
        template.baseUrl,
        template.format,
        defaultModel
      ),
      api_key: '',
      model_name: defaultModel,
      provider: template.format,
      enabled: true,
      context_window: 200000,
      category: 'general_chat',
      capabilities: ['text_chat', 'function_calling'],
      recommended_for: [],
      metadata: {},
      inline_think_in_text: true,
    });
    setSelectedModelDrafts(
      defaultModel ? [createModelDraft(defaultModel, {
        context_window: 200000,
        reasoning: { catalog: { source: 'auto' }, presets: [] },
      })] : []
    );
    setEditingProviderModelIds(new Set());
    setShowAdvancedSettings(false);
    setCreationMode('form');
    setIsEditing(true);
  };

  
  const handleSelectCustom = () => {
    resetRemoteModelDiscovery();
    setManualModelInput('');
    setEditingProviderModelIds(new Set());
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
      context_window: 200000,
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
    const providerGroupKey = getProviderGroupKey(config);
    const configuredProviderModels = aiModels
      .filter(model => getProviderGroupKey(model) === providerGroupKey)
      .sort((a, b) => a.model_name.localeCompare(b.model_name));
    const providerTemplateId = getProviderTemplateId(config);
    setEditingProviderModelIds(new Set(
      configuredProviderModels
        .map(model => model.id)
        .filter((id): id is string => !!id)
    ));
    setSelectedProviderId(providerTemplateId || null);
    setEditingConfig({
      name: providerName,
      base_url: config.base_url,
      request_url: resolveRequestUrl(config.base_url, config.provider || 'openai'),
      api_key: config.api_key || '',
      model_name: '',
      provider: config.provider,
      enabled: true,
      context_window: config.context_window || 200000,
      max_tokens: config.max_tokens,
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
      auth: config.auth || { type: 'api_key' },
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
    setEditingProviderModelIds(new Set());
    setShowApiKey(false);
    setEditingConfig({ ...config, name: getProviderDisplayName(config) });
    setSelectedModelDrafts([
      createModelDraft(config.model_name, config, {
        contextWindow: config.context_window || 200000,
        maxTokens: config.max_tokens,
        reasoning: canonicalReasoningConfig(config),
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
      const baseUrl = editingConfig.base_url.trim();
      if (!providerName || !baseUrl) {
        notification.warning(t('messages.fillRequired'));
        return;
      }
      if (!hasHttpUrlScheme(baseUrl)) {
        notification.warning(t('messages.invalidBaseUrlScheme'));
        return;
      }
      const draftsToSave = dedupeSelectedModelDraftsByModelName(selectedModelDrafts);
      if (draftsToSave.some(draft => draft.contextWindow < 32000)) {
        notification.warning(t('messages.contextWindowTooSmall'));
        return;
      }
      const reasoningValidationResults = draftsToSave.map(draft => ({
        modelName: draft.modelName,
        reasoning: draft.reasoning,
        projectionCatalog: draft.reasoningProjectionCatalog,
        snapshotCatalog: draft.reasoningProjectionSnapshot?.catalog,
        generatedPresetIds: resolveDraftReasoningProjection(draft)?.presets
          ?.filter(preset => preset.source !== 'model_config')
          .map(preset => preset.id) ?? [],
      })).map(entry => ({
        ...entry,
        validationError: validateReasoningConfig(entry.reasoning, entry.generatedPresetIds),
      }));
      if (reasoningValidationResults.some(entry => entry.validationError !== null)) {
        notification.warning(t('messages.invalidReasoningPresets'));
        return;
      }
      const existingProviderInstanceId = getProviderInstanceId(editingConfig);
      const isProviderGroupEdit = !editingConfig.id && editingProviderModelIds.size > 0;
      const providerInstanceId = existingProviderInstanceId || generateProviderInstanceId();
      const providerGroupModelIds = isProviderGroupEdit
        ? editingProviderModelIds
        : new Set<string>();
      const allocatedConfigIds = new Set(
        aiModels
          .map(model => model.id?.trim())
          .filter((id): id is string => Boolean(id))
      );
      const configsToSave: AIModelConfigType[] = draftsToSave.map((draft) => {
        const id = editingConfig.id
          || draft.configId
          || allocateModelConfigId(draft.modelName, allocatedConfigIds);
        allocatedConfigIds.add(id);

        return {
          id,
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
          category: resolveModelCategory(
            draft.modelName,
            draft.category,
            editingConfig.provider || 'openai'
          ),
          capabilities: getCapabilitiesByCategory(
            resolveModelCategory(
              draft.modelName,
              draft.category,
              editingConfig.provider || 'openai'
            )
          ),
          recommended_for: editingConfig.recommended_for || [],
          metadata: {
            ...(editingConfig.metadata || {}),
            [PROVIDER_INSTANCE_METADATA_KEY]: providerInstanceId,
          },
          reasoning: draft.reasoning,
          inline_think_in_text: editingConfig.inline_think_in_text ?? true,
          custom_headers: editingConfig.custom_headers,
          custom_headers_mode: editingConfig.custom_headers_mode,
          skip_ssl_verify: editingConfig.skip_ssl_verify ?? false,
          custom_request_body: editingConfig.custom_request_body,
          custom_request_body_mode: editingConfig.custom_request_body_mode,
          auth: editingConfig.auth || { type: 'api_key' },
        };
      });
      const configsToAutoTest = configsNeedingAutoTest(
        aiModels,
        configsToSave,
        isProviderGroupEdit
      );

      const updatedModels = await configManager.updateConfig<AIModelConfigType[]>('ai.models', current => {
        if (editingConfig.id) {
          if (!current.some(model => model.id === editingConfig.id)) {
            throw new Error('The model was removed while it was being edited');
          }
          return current.map(model => model.id === editingConfig.id ? { ...model, ...configsToSave[0] } : model);
        }
        if (isProviderGroupEdit) {
          return [
            ...current.filter(model => !providerGroupModelIds.has(model.id || '')),
            ...configsToSave,
          ];
        }
        return [...current, ...configsToSave];
      });
      setAiModels(updatedModels);
      // The host reconciles default selectors using model capabilities.
      
      
      setIsEditing(false);
      setEditingConfig(null);
      setCreationMode(null);
      setSelectedProviderId(null);
      setEditingProviderModelIds(new Set());
      
      
      const autoTestConfigIds = configsToAutoTest.map(config => config.id).filter((id): id is string => !!id);
      if (autoTestConfigIds.length > 0) {
        setExpandedIds(prev => new Set([...prev, ...autoTestConfigIds]));
      }
      
      
      
      configsToAutoTest.forEach(config => {
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
      const updatedModels = await configManager.updateConfig<AIModelConfigType[]>(
        'ai.models', current => current.filter(model => model.id !== id)
      );
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
      const updatedModels = await configManager.updateConfig<AIModelConfigType[]>(
        'ai.models', current => current.map(model => model.id === config.id ? { ...model, enabled } : model)
      );
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

  const handleSaveStreamTimeouts = async () => {
    if (isStreamTimeoutInvalid) {
      notification.warning(t('streamIdleTimeout.invalid'));
      return;
    }

    setIsStreamTimeoutSaving(true);
    try {
      await Promise.all([
        configManager.setConfig(
          'ai.stream_idle_timeout_secs',
          parsedStreamIdleTimeout ?? null
        ),
        configManager.setConfig(
          'ai.stream_ttft_timeout_secs',
          parsedStreamTtftTimeout ?? null
        ),
      ]);
      setStreamIdleTimeoutInput(
        parsedStreamIdleTimeout != null ? String(parsedStreamIdleTimeout) : ''
      );
      setStreamTtftTimeoutInput(
        parsedStreamTtftTimeout != null ? String(parsedStreamTtftTimeout) : ''
      );
      notification.success(t('streamIdleTimeout.saveSuccess'));
    } catch (error) {
      log.error('Failed to save stream timeouts', error);
      notification.error(t('messages.saveFailed'));
    } finally {
      setIsStreamTimeoutSaving(false);
    }
  };

  const closeEditingModal = () => {
    resetRemoteModelDiscovery();
    setSelectedModelDrafts([]);
    setEditingProviderModelIds(new Set());
    setManualModelInput('');
    setShowApiKey(false);
    setIsEditing(false);
    setEditingConfig(null);
    setCreationMode(null);
    setSelectedProviderId(null);
    setProviderQuery('');
    setShowAllProviders(false);
    setReasoningPanelDraftKey(null);
  };

  const providerGroups = useMemo<ProviderGroup[]>(() => {
    const grouped = aiModels.reduce<Map<string, ProviderGroup>>((map, model) => {
      const groupKey = getProviderGroupKey(model);
      const providerName = getProviderDisplayName(model);
      const existingGroup = map.get(groupKey);
      if (existingGroup) {
        existingGroup.models.push(model);
        return map;
      }

      map.set(groupKey, {
        key: groupKey,
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
      <ConfigPageLayout className="bitfun-model-settings" data-bf-component="model-settings" data-bf-part="root" data-bf-view="selection">
        <ConfigPageHeader
          title={t('providerSelection.title')}
          subtitle={t('providerSelection.subtitle')}
        />

        <ConfigPageContent className="bitfun-model-settings__content bitfun-model-settings__content--selection">
          <div className="bitfun-model-settings__provider-selection" data-bf-component="model-settings" data-bf-part="providerSelection">
            
            <Card
              data-testid="settings-model-custom-config-btn"
              data-provider-id="custom"
              appearance="neutral"
              padding="sm"
              className="bitfun-model-settings__custom-option"
              onClick={handleSelectCustom}
            >
              <div className="bitfun-model-settings__custom-option-content" data-bf-component="model-settings" data-bf-part="customOption">
                <Icon name="settings" size="lg" />
                <div>
                  <div className="bitfun-model-settings__custom-option-title" data-bf-component="model-settings" data-bf-part="customOptionTitle">{t('providerSelection.customTitle')}</div>
                  <div className="bitfun-model-settings__custom-option-description" data-bf-component="model-settings" data-bf-part="customOptionDescription">{t('providerSelection.customDescription')}</div>
                </div>
              </div>
            </Card>

            
            <div className="bitfun-model-settings__selection-divider" data-bf-component="model-settings" data-bf-part="selectionDivider">
              <span>{t('providerSelection.orSelectProvider')}</span>
            </div>


            <SearchField
              leadingIcon={<Icon name="search" size="lg" aria-hidden />}
              size="sm"
              className="bitfun-model-settings__provider-search"
              data-testid="settings-model-provider-search"
              data-bf-component="model-settings"
              data-bf-part="providerSearch"
              value={providerQuery}
              placeholder={t('providerSelection.searchProviders')}
              aria-label={t('providerSelection.searchProviders')}
              onValueChange={setProviderQuery}
              onSearch={() => {
                const firstMatch = visibleProviders[0];
                if (normalizedProviderQuery && firstMatch) handleSelectProvider(firstMatch.id);
              }}
            />


            <div className="bitfun-model-settings__provider-list" data-bf-component="model-settings" data-bf-part="providerList">
              {visibleProviders.map(provider => (
                // The help link is a sibling of the select button, not a child:
                // a button may not contain interactive content.
                <div
                  key={provider.id}
                  className="bitfun-model-settings__provider-row"
                  data-bf-component="model-settings"
                  data-bf-part="providerRow"
                >
                  <button
                    type="button"
                    data-testid="settings-model-provider-option"
                    data-provider-id={provider.id}
                    className="bitfun-model-settings__provider-select"
                    data-bf-component="model-settings"
                    data-bf-part="providerSelect"
                    onClick={() => handleSelectProvider(provider.id)}
                  >
                    <span className="bitfun-model-settings__provider-name" data-bf-component="model-settings" data-bf-part="providerName">{provider.name}</span>
                    <span className="bitfun-model-settings__provider-description" data-bf-component="model-settings" data-bf-part="providerDescription">{provider.description}</span>
                  </button>
                  {provider.helpUrl && (
                    <a
                      href={provider.helpUrl}
                      target="_blank"
                      rel="noopener noreferrer"
                      className="bitfun-model-settings__provider-help-link"
                      onClick={async (e) => {
                        e.preventDefault();
                        e.stopPropagation();
                        try {
                          await systemAPI.openExternal(provider.helpUrl!);
                        } catch (error) {
                          console.error('[ModelSettings] Failed to open external URL:', error);
                        }
                      }}
                    >
                      <Icon name="arrow-up-right" size="xs" />
                      {t('providerSelection.getApiKey')}
                    </a>
                  )}
                  <Icon name="chevron-right" size="sm" className="bitfun-model-settings__provider-chevron" aria-hidden="true" />
                </div>
              ))}

              {visibleProviders.length === 0 && (
                <div className="bitfun-model-settings__provider-empty" data-bf-component="model-settings" data-bf-part="providerEmpty">
                  {t('providerSelection.noProviderMatches')}
                </div>
              )}

              {canToggleProviderList && (
                <button
                  type="button"
                  data-testid="settings-model-provider-expand-btn"
                  className="bitfun-model-settings__provider-more"
                  data-bf-component="model-settings"
                  data-bf-part="providerMore"
                  onClick={() => setShowAllProviders(previous => !previous)}
                >
                  {isProviderListCollapsed
                    ? t('providerSelection.showAllProviders', { count: matchedProviders.length })
                    : t('providerSelection.collapseProviders')}
                  {isProviderListCollapsed ? <Icon name="chevron-down" size="sm" /> : <Icon name="chevron-up" size="sm" />}
                </button>
              )}
            </div>


            <div className="bitfun-model-settings__selection-actions" data-bf-component="model-settings" data-bf-part="selectionActions">
              <Button variant="outline" onClick={() => setCreationMode(null)}>
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
    const catalogProvider = selectedProviderId
      ? modelCatalog?.provider_catalog?.providers.find(provider => provider.id === selectedProviderId)
      : undefined;
    const normalizedEditingBaseUrl = editingConfig.base_url
      ? normalizeProviderBaseUrl(editingConfig.base_url)
      : '';
    const selectedEndpointId = catalogProvider?.endpoints
      .filter(endpoint => !editingConfig.provider || endpoint.api_format === editingConfig.provider)
      .sort((left, right) => (
        normalizeProviderBaseUrl(right.base_url).length
        - normalizeProviderBaseUrl(left.base_url).length
      ))
      .find(endpoint => {
        const normalizedEndpoint = normalizeProviderBaseUrl(endpoint.base_url);
        return normalizedEndpoint === normalizedEditingBaseUrl
          || normalizedEndpoint.startsWith(`${normalizedEditingBaseUrl}/`)
          || normalizedEditingBaseUrl.startsWith(`${normalizedEndpoint}/`);
      })?.id;
    const catalogModelOptions: ComboboxOption[] = (catalogProvider?.models || [])
      .filter(model => (
        !selectedEndpointId
        || !model.endpoint_ids?.length
        || model.endpoint_ids.includes(selectedEndpointId)
      ))
      .map(model => ({
        label: model.display_name || model.id,
        value: model.id,
        description: model.display_name && model.display_name !== model.id ? model.id : undefined,
        testId: 'settings-model-option',
        testAttributes: {
          'data-model-id': model.id,
          'data-model-name': model.id,
          'data-model-source': model.source,
        },
      }));
    const fetchedOrPresetModelOptions: ComboboxOption[] = remoteModelOptions.length > 0
      ? remoteModelOptions.map(model => ({
          label: model.display_name || model.id,
          value: model.id,
          description: model.display_name && model.display_name !== model.id ? model.id : undefined,
          testId: 'settings-model-option',
          testAttributes: {
            'data-model-id': model.id,
            'data-model-name': model.id,
          },
        }))
      : catalogModelOptions.length > 0
        ? catalogModelOptions
        : (currentTemplate?.models || []).map(model => ({
          label: model,
          value: model,
          testId: 'settings-model-option',
          testAttributes: {
            'data-model-id': model,
            'data-model-name': model,
          },
        }));
    const selectedModelOptions: ComboboxOption[] = selectedModelDrafts.map(draft => ({
      label: draft.modelName,
      value: draft.modelName,
      testId: 'settings-model-option',
      testAttributes: {
        'data-model-id': draft.modelName,
        'data-model-name': draft.modelName,
      },
    }));
    const availableModelOptions: ComboboxOption[] = Array.from(
      new Map(
        [...fetchedOrPresetModelOptions, ...selectedModelOptions]
          .map(option => [String(option.value), option] as const)
      ).values()
    );
    const modelFetchHint = isFetchingRemoteModels
      ? t('providerSelection.fetchingModels')
      : remoteModelsError
        ? remoteModelsError
        : remoteModelOptions.length > 0
          ? null
          : fetchedOrPresetModelOptions.length > 0
            ? t('providerSelection.usingPresetModels')
            : hasAttemptedRemoteFetch
              ? t('providerSelection.noPresetModels')
              : null;
    const selectedModelValues = selectedModelDrafts.map(draft => draft.modelName);
    const apiKeyVisibilityLabel = showApiKey ? tComponents('hide') : tComponents('show');
    const apiKeySuffix = (
      <button
        type="button"
        className="bitfun-model-settings__input-visibility-toggle"
        onClick={() => setShowApiKey(prev => !prev)}
        aria-label={apiKeyVisibilityLabel}
        title={apiKeyVisibilityLabel}
      >
        {showApiKey ? <EyeOff size={14} /> : <Icon name="eye" size="sm" />}
      </button>
    );

    const formatReasoningSummary = (
      draft: SelectedModelDraft,
      generatedProjection?: ReasoningCatalogProjection,
    ) => {
      const presetCount = draft.reasoning.presets?.length ?? 0;
      const catalogSource = draft.reasoning.catalog?.source ?? 'auto';
      const catalogLabel = catalogSource === 'models_dev'
        ? t('reasoningPresets.catalogSummary.modelsDev')
        : catalogSource === 'disabled'
          ? t('reasoningPresets.catalogSummary.disabled')
          : t('reasoningPresets.catalogSummary.auto');
      const selected = draft.reasoning.presets?.find(
        preset => preset.id === draft.reasoning.default_preset,
      ) ?? generatedProjection?.presets?.find(
        preset => preset.id === draft.reasoning.default_preset,
      );
      const defaultLabel = draft.reasoning.default_preset
        ? selected?.label?.trim() || selected?.id || draft.reasoning.default_preset
        : t('reasoningPresets.autoShort');
      return presetCount > 0
        ? t('reasoningPresets.summaryWithCustom', {
            source: catalogLabel,
            default: defaultLabel,
            count: presetCount,
          })
        : t('reasoningPresets.summary', {
            source: catalogLabel,
            default: defaultLabel,
          });
    };

    const renderSelectedModelRows = () => {
      if (selectedModelDrafts.length === 0) {
        return (
          <div
            className="bitfun-model-settings__selected-models-empty"
            data-testid="settings-model-selected-list-empty"
            data-selected-count="0"
          >
            {t('providerSelection.noModelsSelected')}
          </div>
        );
      }

      return (
        <div
          className="bitfun-model-settings__selected-models-list"
          data-testid="settings-model-selected-list"
          data-selected-count={selectedModelDrafts.length}
        >
          {selectedModelDrafts.map(draft => {
            const isExpanded = expandedModelCards.has(draft.key) || selectedModelDrafts.length === 1;
            const hasUnsavedChanges = modelDraftHasUnsavedChanges(draft, aiModels);
            const categoryLabel = categoryCompactLabels[draft.category] ?? draft.category;
            const canToggleExpand = selectedModelDrafts.length > 1;
            const modelDisplayName = draft.modelName;
            const reasoningProjection = resolveDraftReasoningProjection(draft);

            return (
              <div
                key={draft.key}
                className="bitfun-model-settings__selected-model-row"
                data-testid="settings-model-selected-row"
                data-model-id={draft.modelName}
                data-model-name={draft.modelName}
                data-selected="true"
                data-expanded={isExpanded ? 'true' : 'false'}
                data-unsaved={hasUnsavedChanges ? 'true' : 'false'}
              >
                <div
                  className={[
                    'bitfun-model-settings__selected-model-head',
                    canToggleExpand && 'bitfun-model-settings__selected-model-head--toggleable',
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
                  <div className="bitfun-model-settings__selected-model-head-title">
                    <div className="bitfun-model-settings__selected-model-head-top">
                      <div className="bitfun-model-settings__selected-model-toggle">
                        {isExpanded ? <Icon name="chevron-down" size="sm" /> : <Icon name="chevron-right" size="sm" />}
                      </div>
                      <div className="bitfun-model-settings__selected-model-name">{modelDisplayName}</div>
                      {hasUnsavedChanges && (
                        <span
                          className="bitfun-model-settings__selected-model-unsaved"
                          title={t('providerSelection.unsavedModelHint')}
                          aria-label={t('providerSelection.unsavedModelHint')}
                          data-testid="settings-model-unsaved-badge"
                        >
                          {t('providerSelection.unsavedModel')}
                        </span>
                      )}
                    </div>
                    {!editingConfig.id && (
                      <Tooltip content={t('providerSelection.removeModel')}>
                        <IconButton
                          aria-label={t('providerSelection.removeModel')}
                          data-testid="settings-model-selected-remove-btn"
                          data-model-id={draft.modelName}
                          data-model-name={draft.modelName}
                          size="sm"
                          className="bitfun-model-settings__selected-model-remove"
                          onClick={(e) => {
                            e.stopPropagation();
                            removeSelectedModelDraft(draft.modelName);
                          }}
                          icon={<Icon name="xmark" size="sm" />}
                        />
                      </Tooltip>
                    )}
                  </div>
                  {!isExpanded && (
                    <div className="bitfun-model-settings__selected-model-head-bottom">
                      <span className="bitfun-model-settings__selected-model-summary">
                        {categoryLabel}
                        {' · '}
                        {formatTokenCountShort(draft.contextWindow)} ctx
                        {' · '}
                        {formatReasoningSummary(draft, reasoningProjection)}
                      </span>
                    </div>
                  )}
                </div>
                {isExpanded && (
                  <div className="bitfun-model-settings__selected-model-grid">
                    <div className="bitfun-model-settings__selected-model-field">
                      <span>{t('category.label')}</span>
                      <Combobox
                        value={draft.category}
                        onValueChange={(value) => updateModelDraft(draft.modelName, { category: value as ModelCategory })}
                        options={categoryOptions}
                        size="sm"
                        className="bitfun-model-settings__selected-model-category-select"
                      />
                    </div>
                    <div className="bitfun-model-settings__selected-model-field">
                      <span className="bitfun-model-settings__inline-header-main">
                        <span>{t('form.contextWindow')}</span>
                        <Tooltip content={t('form.contextWindowHint')} placement="top">
                          <span
                            className="bitfun-model-settings__inline-header-info"
                            role="button"
                            tabIndex={0}
                            aria-label={t('form.contextWindowHint')}
                          >
                            <Icon name="info" size="sm" aria-hidden="true" />
                          </span>
                        </Tooltip>
                      </span>
                      <NumberInput
                        className="bitfun-model-settings__selected-model-context-input"
                        value={draft.contextWindow}
                        onValueChange={(value) => updateModelDraft(draft.modelName, { contextWindow: value })}
                        min={32000}
                        max={2000000}
                        step={1000}
                        size="sm"
                        disableWheel
                      />
                    </div>
                    {draft.contextWindow > LONG_CONTEXT_WARNING_THRESHOLD_TOKENS && (
                      <div className="bitfun-model-settings__warning-inline bitfun-model-settings__context-window-warning">
                        <AlertTriangle size={14} />
                        <span>{t('form.contextWindowLongWarning')}</span>
                      </div>
                    )}
                    <button
                      type="button"
                      className="bitfun-model-settings__reasoning-summary"
                      onClick={() => setReasoningPanelDraftKey(draft.key)}
                      data-testid="settings-model-reasoning-edit"
                    >
                      <span className="bitfun-model-settings__reasoning-summary-icon">
                        <Icon name="thinking" size="md" aria-hidden="true" />
                      </span>
                      <span className="bitfun-model-settings__reasoning-summary-content">
                        <strong>{t('reasoningPresets.configTitle')}</strong>
                        <span>{formatReasoningSummary(draft, reasoningProjection)}</span>
                      </span>
                      <span className="bitfun-model-settings__reasoning-summary-action">
                        {t('actions.edit')}
                      </span>
                    </button>
                  </div>
                )}
              </div>
            );
          })}
        </div>
      );
    };

    const authType = editingConfig.auth?.type || 'api_key';
    const authIsSubscription = authType === 'subscription';
    const selectedSubscriptionProvider: SubscriptionProvider | undefined =
      editingConfig.auth?.type === 'subscription' ? editingConfig.auth.provider : undefined;
    const selectedOpenCodePlan: OpenCodePlan | undefined =
      editingConfig.auth?.type === 'subscription'
      && editingConfig.auth.provider === 'opencode'
        ? editingConfig.auth.plan || 'zen'
        : undefined;
    const authSelectValue = authIsSubscription
      ? selectedSubscriptionProvider === 'opencode'
        ? `subscription:opencode:${selectedOpenCodePlan || 'zen'}`
        : `subscription:${selectedSubscriptionProvider || 'codex'}`
      : 'api_key';
    const authOptions: ComboboxOption[] = [
      { value: 'api_key', label: t('subscriptionAuth.options.apiKey') },
      { value: 'subscription:codex', label: t('subscriptionAuth.options.codex') },
      { value: 'subscription:antigravity', label: t('subscriptionAuth.options.antigravity') },
      { value: 'subscription:grok', label: t('subscriptionAuth.options.grok') },
      { value: 'subscription:hermes', label: t('subscriptionAuth.options.hermes') },
      { value: 'subscription:opencode:zen', label: t('subscriptionAuth.options.opencodeZen') },
      { value: 'subscription:opencode:go', label: t('subscriptionAuth.options.opencodeGo') },
    ];
    const matchedSubscription = selectedSubscriptionProvider
      ? subscriptionAccounts.find((account) => account.provider === selectedSubscriptionProvider)
      : undefined;

    const renderAuthRow = () => (
      <ConfigPageRow label={t('subscriptionAuth.label')} align={authIsSubscription ? 'start' : 'center'} wide>
        <div className="bitfun-model-settings__control-stack">
          <Select
            value={authSelectValue}
            onValueChange={(value) => {
              const next = String(value);
              if (next === 'api_key') {
                setEditingConfig((prev) => ({ ...prev, auth: { type: 'api_key' } }));
                return;
              }
              const [, providerValue, planValue] = next.split(':');
              const provider = providerValue as SubscriptionProvider;
              const plan = provider === 'opencode'
                ? (planValue || 'zen') as OpenCodePlan
                : undefined;
              setEditingConfig((prev) => {
                if (!prev) return prev;
                if (provider !== 'opencode') {
                  return {
                    ...prev,
                    auth: { type: 'subscription', provider },
                  };
                }
                const format = ['openai', 'responses', 'anthropic'].includes(prev.provider || '')
                  ? prev.provider || 'openai'
                  : 'openai';
                const baseUrl = plan === 'go'
                  ? 'https://opencode.ai/zen/go/v1'
                  : 'https://opencode.ai/zen/v1';
                return {
                  ...prev,
                  provider: format,
                  base_url: baseUrl,
                  request_url: resolveRequestUrl(baseUrl, format, prev.model_name || ''),
                  auth: { type: 'subscription', provider, plan },
                };
              });
            }}
            options={authOptions}
            size="sm"
          />
          {authIsSubscription && (
            <small className={matchedSubscription?.connected
              ? 'resolved-url__hint bitfun-model-settings__cli-auth-hint'
              : 'resolved-url__hint bitfun-model-settings__cli-auth-hint bitfun-model-settings__json-status--error'}
            >
              {matchedSubscription?.connected
                ? t('subscriptionAuth.detected', {
                    label: matchedSubscription.display_label,
                    account: matchedSubscription.account || t('subscriptionAuth.unknownAccount'),
                  })
                : t('subscriptionAuth.notConnected', {
                    kind: selectedSubscriptionProvider || 'subscription',
                  })}
            </small>
          )}
        </div>
      </ConfigPageRow>
    );

    const renderApiKeyRow = (label: string) => (
      <ConfigPageRow label={label} align="center" wide>
        <Input
          data-testid="settings-model-api-key-input"
          type={showApiKey ? 'text' : 'password'}
          value={editingConfig.api_key || ''}
          onChange={(e) => {
            resetRemoteModelDiscovery();
            setEditingConfig(prev => ({ ...prev, api_key: e.target.value }));
          }}
          placeholder={t('form.apiKeyPlaceholder')}
          trailing={apiKeySuffix}
          size="sm"
        />
      </ConfigPageRow>
    );

    return (
      <>
        <div className="bitfun-model-settings__form bitfun-model-settings__form--modal" data-bf-component="model-settings" data-bf-part="form">
          <ScrollArea className="bitfun-model-settings__form-scrollable" data-bf-component="model-settings" data-bf-part="formBody">
            <ConfigPageSection
              title={isProviderScopedEditing ? t('editProviderSubtitle') : t('editSubtitle')}
              className="bitfun-model-settings__edit-section"
            >
            {isFromTemplate ? (
              <>
                <ConfigPageRow label={`${t('form.configName')} *`} align="center" wide>
                  <Input
                    data-testid="settings-model-provider-name-input"
                    value={editingConfig.name || ''}
                    onChange={(e) => setEditingConfig(prev => ({ ...prev, name: e.target.value }))}
                    placeholder={t('form.configNamePlaceholder')}
                    size="sm"
                  />
                </ConfigPageRow>
                {renderAuthRow()}
                {!authIsSubscription && renderApiKeyRow(`${t('form.apiKey')} *`)}
                <ConfigPageRow label={t('form.baseUrl')} align="center" wide>
                  <div className="bitfun-model-settings__control-stack">
                    {currentTemplate?.baseUrlOptions && currentTemplate.baseUrlOptions.length > 0 && (
                      <Combobox
                        value={currentTemplate.baseUrlOptions.some(opt => opt.url === editingConfig.base_url) ? editingConfig.base_url : ''}
                        onValueChange={(value) => {
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
                        size="sm"
                      />
                    )}
                    <Input
                      data-testid="settings-model-base-url-input"
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
                      size="sm"
                    />
                    {editingConfig.base_url && (
                      <div className="bitfun-model-settings__resolved-url">
                        <Input
                          value={previewRequestUrl(editingConfig.base_url, editingConfig.provider || 'openai')}
                          readOnly
                          onFocus={(e) => e.target.select()}
                          className="bitfun-model-settings__resolved-url-input"
                          size="sm"
                        />
                      </div>
                    )}
                  </div>
                </ConfigPageRow>
                <ConfigPageRow label={t('form.provider')} align="center" wide>
                  <Select
                    data-testid="settings-model-request-format-select"
                    value={editingConfig.provider || 'openai'}
                    onValueChange={(value) => {
                      const provider = value as string;
                      resetRemoteModelDiscovery();
                      setEditingConfig(prev => ({
                        ...prev,
                        provider,
                        request_url: resolveRequestUrl(prev?.base_url || '', provider, prev?.model_name || '')
                      }));
                    }}
                    placeholder={t('form.providerPlaceholder')}
                    options={requestFormatOptions}
                    size="sm"
                  />
                </ConfigPageRow>
                <ConfigPageRow label={`${t('form.modelSelection')} *`} wide multiline>
                  <div className="bitfun-model-settings__control-stack">
                    <div className="bitfun-model-settings__model-picker-row">
                      <MultiSelect
                        data-testid="settings-model-select"
                        value={selectedModelValues}
                        onValueChange={(value) => {
                          const nextModelNames = value.map(item => String(item));
                          syncSelectedModelDrafts(nextModelNames, editingConfig);
                        }}
                        placeholder={t('providerSelection.selectModel')}
                        options={availableModelOptions}
                        loading={isFetchingRemoteModels}
                        onCreateValue={value => value}
                        size="sm"
                        onOpenChange={handleModelSelectionOpenChange}
                      />
                    </div>
                    <div className="bitfun-model-settings__manual-model-entry">
                      <Input
                        data-testid="settings-model-manual-name-input"
                        value={manualModelInput}
                        onChange={(e) => setManualModelInput(e.target.value)}
                        onKeyDown={(e) => {
                          if (e.key === 'Enter') {
                            e.preventDefault();
                            addManualModelDraft();
                          }
                        }}
                        placeholder={t('providerSelection.inputModelName')}
                        size="sm"
                      />
                      <Button className="bitfun-model-settings__manual-model-add" data-testid="settings-model-add-custom-btn" variant="outline" size="sm" onClick={addManualModelDraft}>
                        {t('providerSelection.addCustomModel')}
                      </Button>
                    </div>
                    {modelFetchHint && (
                      <small className={`resolved-url__hint ${remoteModelsError ? 'bitfun-model-settings__json-status--error' : ''}`}>
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
                      <Input
                        data-testid="settings-model-provider-name-input"
                        value={editingConfig.name || ''}
                        onChange={(e) => setEditingConfig(prev => ({ ...prev, name: e.target.value }))}
                        placeholder={t('form.configNamePlaceholder')}
                        size="sm"
                      />
                    </ConfigPageRow>
                    {renderAuthRow()}
                    {!authIsSubscription && renderApiKeyRow(`${t('form.apiKey')} *`)}
                    <ConfigPageRow label={`${t('form.baseUrl')} *`} align="center" wide>
                      <div className="bitfun-model-settings__control-stack">
                        <Input
                          data-testid="settings-model-base-url-input"
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
                          size="sm"
                        />
                        {editingConfig.base_url && (
                          <div className="bitfun-model-settings__resolved-url">
                            <Input
                              value={previewRequestUrl(editingConfig.base_url, editingConfig.provider || 'openai')}
                              readOnly
                              onFocus={(e) => e.target.select()}
                              className="bitfun-model-settings__resolved-url-input"
                              size="sm"
                            />
                          </div>
                        )}
                      </div>
                    </ConfigPageRow>
                    <ConfigPageRow label={t('form.provider')} align="center" wide>
                      <Select data-testid="settings-model-request-format-select" value={editingConfig.provider || 'openai'} onValueChange={(value) => {
                        const provider = value as string;
                        resetRemoteModelDiscovery();
                        setEditingConfig(prev => ({
                          ...prev,
                          provider,
                          request_url: resolveRequestUrl(prev?.base_url || '', provider, prev?.model_name || ''),
                        }));
                      }} placeholder={t('form.providerPlaceholder')} options={requestFormatOptions} size="sm" />
                    </ConfigPageRow>
                  </>
                )}
              </>
            )}

            {!isFromTemplate && (
              <>
                <ConfigPageRow label={`${t('form.modelSelection')} *`} wide multiline>
                  <div className="bitfun-model-settings__control-stack">
                    <div className="bitfun-model-settings__model-picker-row">
                      {editingConfig.id ? (
                        <Combobox
                          data-testid="settings-model-select"
                          value={selectedModelValues[0] || ''}
                          onValueChange={(value) => {
                            syncSelectedModelDrafts([String(value)], editingConfig, true);
                          }}
                          placeholder="glm-5.2"
                          options={availableModelOptions}
                          loading={isFetchingRemoteModels}
                          onCreateValue={value => value}
                          size="sm"
                          onOpenChange={handleModelSelectionOpenChange}
                        />
                      ) : (
                        <MultiSelect
                          data-testid="settings-model-select"
                          value={selectedModelValues}
                          onValueChange={(value) => {
                            syncSelectedModelDrafts(value.map(item => String(item)), editingConfig, false);
                          }}
                          placeholder="glm-5.2"
                          options={availableModelOptions}
                          loading={isFetchingRemoteModels}
                          onCreateValue={value => value}
                          size="sm"
                          onOpenChange={handleModelSelectionOpenChange}
                        />
                      )}
                    </div>
                    <div className="bitfun-model-settings__manual-model-entry">
                      <Input
                        data-testid="settings-model-manual-name-input"
                        value={manualModelInput}
                        onChange={(e) => setManualModelInput(e.target.value)}
                        onKeyDown={(e) => {
                          if (e.key === 'Enter') {
                            e.preventDefault();
                            addManualModelDraft();
                          }
                        }}
                        placeholder={t('providerSelection.inputModelName')}
                        size="sm"
                      />
                      <Button className="bitfun-model-settings__manual-model-add" data-testid="settings-model-add-custom-btn" variant="outline" size="sm" onClick={addManualModelDraft}>
                        {t('providerSelection.addCustomModel')}
                      </Button>
                    </div>
                    {modelFetchHint && (
                      <small className={`resolved-url__hint ${remoteModelsError ? 'bitfun-model-settings__json-status--error' : ''}`}>
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
            className="bitfun-model-settings__edit-section"
          >
            <ConfigPageRow className="bitfun-model-settings__toggle-row" label={t('advancedSettings.title')} align="center">
              <Switch checked={showAdvancedSettings} onChange={(e) => setShowAdvancedSettings(e.target.checked)} />
            </ConfigPageRow>

            {showAdvancedSettings && (
              <>
                {(editingConfig.provider === 'openai' || editingConfig.provider === 'anthropic') && (
                  <ConfigPageRow
                    label={t('advancedSettings.inlineThinkInText.label')}
                    description={t('advancedSettings.inlineThinkInText.hint')}
                    align="center"
                    className="bitfun-model-settings__toggle-row"
                  >
                    <Switch
                      checked={editingConfig.inline_think_in_text ?? true}
                      onChange={(e) => setEditingConfig(prev => ({ ...prev, inline_think_in_text: e.target.checked }))}
                    />
                  </ConfigPageRow>
                )}
                <ConfigPageRow
                  label={t('advancedSettings.skipSslVerify.label')}
                  description={editingConfig.skip_ssl_verify ? (
                    <span className="bitfun-model-settings__warning-inline">
                      <AlertTriangle size={14} />
                      <span>{t('advancedSettings.skipSslVerify.warning')}</span>
                    </span>
                  ) : undefined}
                  align="center"
                  className="bitfun-model-settings__toggle-row"
                >
                  <Switch
                    checked={editingConfig.skip_ssl_verify || false}
                    onChange={(e) => setEditingConfig(prev => ({ ...prev, skip_ssl_verify: e.target.checked }))}
                  />
                </ConfigPageRow>
                <ConfigPageRow
                  label={(
                    <span className="bitfun-model-settings__inline-header">
                      <span className="bitfun-model-settings__inline-header-main">
                        <span>{t('advancedSettings.customHeaders.label')}</span>
                        <Tooltip
                          content={(
                            <span className="bitfun-model-settings__header-tooltip">
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
                            className="bitfun-model-settings__inline-header-info"
                            role="button"
                            tabIndex={0}
                            aria-label={t('advancedSettings.customHeaders.hint')}
                          >
                            <Icon name="info" size="sm" />
                          </span>
                        </Tooltip>
                      </span>
                      <span className="bitfun-model-settings__inline-header-actions">
                        <Tooltip content={t('advancedSettings.customHeaders.modeMergeHint')} placement="top">
                          <Button
                            type="button"
                            variant={(editingConfig.custom_headers_mode || 'merge') === 'merge' ? 'fill' : 'outline'}
                            size="sm"
                            className="bitfun-model-settings__mode-button"
                            onClick={() => setEditingConfig(prev => ({ ...prev, custom_headers_mode: 'merge' }))}
                          >
                            {t('advancedSettings.customHeaders.modeMerge')}
                          </Button>
                        </Tooltip>
                        <Tooltip content={t('advancedSettings.customHeaders.modeReplaceHint')} placement="top">
                          <Button
                            type="button"
                            variant={editingConfig.custom_headers_mode === 'replace' ? 'fill' : 'outline'}
                            size="sm"
                            className="bitfun-model-settings__mode-button"
                            onClick={() => setEditingConfig(prev => ({ ...prev, custom_headers_mode: 'replace' }))}
                          >
                            {t('advancedSettings.customHeaders.modeReplace')}
                          </Button>
                        </Tooltip>
                      </span>
                    </span>
                  )}
                  multiline
                  className="bitfun-model-settings__custom-headers-row"
                >
                  <div className="bitfun-model-settings__row-control--stack">
                    <div className="bitfun-model-settings__custom-headers">
                      {Object.entries(editingConfig.custom_headers || {}).map(([key, value], index) => (
                        <div key={index} className="bitfun-model-settings__header-row">
                          <Input
                            value={key}
                            onChange={(e) => { const nh = { ...editingConfig.custom_headers }; const ov = nh[key]; delete nh[key]; if (e.target.value) nh[e.target.value] = ov; setEditingConfig(prev => ({ ...prev, custom_headers: nh })); }}
                            placeholder={t('advancedSettings.customHeaders.keyPlaceholder')}
                            className="bitfun-model-settings__header-key"
                            size="sm"
                          />
                          <Input
                            value={value}
                            onChange={(e) => { const nh = { ...editingConfig.custom_headers }; nh[key] = e.target.value; setEditingConfig(prev => ({ ...prev, custom_headers: nh })); }}
                            placeholder={t('advancedSettings.customHeaders.valuePlaceholder')}
                            className="bitfun-model-settings__header-value"
                            size="sm"
                          />
                          <Tooltip content={t('actions.delete')}>
                            <IconButton
                              aria-label={t('actions.delete')}
                              size="sm"
                              onClick={() => { const nh = { ...editingConfig.custom_headers }; delete nh[key]; setEditingConfig(prev => ({ ...prev, custom_headers: Object.keys(nh).length > 0 ? nh : undefined })); }}
                              icon={<Icon name="xmark" size="sm" />}
                            />
                          </Tooltip>
                        </div>
                      ))}
                      <Button type="button" variant="outline" size="sm" onClick={() => setEditingConfig(prev => ({ ...prev, custom_headers: { ...prev?.custom_headers, '': '' } }))} className="bitfun-model-settings__add-header-btn" leadingIcon={<Icon name="plus" size="sm" />}>{t('advancedSettings.customHeaders.addHeader')}</Button>
                    </div>
                  </div>
                </ConfigPageRow>
                <ConfigPageRow
                  label={(
                    <span className="bitfun-model-settings__inline-header">
                      <span className="bitfun-model-settings__inline-header-main">
                        <span>{t('advancedSettings.customRequestBody.label')}</span>
                        <Tooltip
                          content={(
                            <span className="bitfun-model-settings__header-tooltip">
                              <span>{t('advancedSettings.customRequestBody.hint')}</span>
                              <span>{getCustomRequestBodyModeHint(editingConfig.provider, editingConfig.custom_request_body_mode)}</span>
                            </span>
                          )}
                          placement="top"
                        >
                          <span
                            className="bitfun-model-settings__inline-header-info"
                            role="button"
                            tabIndex={0}
                            aria-label={t('advancedSettings.customRequestBody.hint')}
                          >
                            <Icon name="info" size="sm" />
                          </span>
                        </Tooltip>
                      </span>
                      <span className="bitfun-model-settings__inline-header-actions">
                        <Tooltip content={t('advancedSettings.customRequestBody.modeMergeHint')} placement="top">
                          <Button
                            type="button"
                            variant={(editingConfig.custom_request_body_mode || 'merge') === 'merge' ? 'fill' : 'outline'}
                            size="sm"
                            className="bitfun-model-settings__mode-button"
                            onClick={() => setEditingConfig(prev => ({ ...prev, custom_request_body_mode: 'merge' }))}
                          >
                            {t('advancedSettings.customRequestBody.modeMerge')}
                          </Button>
                        </Tooltip>
                        <Tooltip content={getCustomRequestBodyTrimHint(editingConfig.provider)} placement="top">
                          <Button
                            type="button"
                            variant={editingConfig.custom_request_body_mode === 'trim' ? 'fill' : 'outline'}
                            size="sm"
                            className="bitfun-model-settings__mode-button"
                            onClick={() => setEditingConfig(prev => ({ ...prev, custom_request_body_mode: 'trim' }))}
                          >
                            {t('advancedSettings.customRequestBody.modeTrim')}
                          </Button>
                        </Tooltip>
                      </span>
                    </span>
                  )}
                  multiline
                  className="bitfun-model-settings__custom-request-body-row"
                >
                  <div className="bitfun-model-settings__row-control--stack">
                    <Textarea value={editingConfig.custom_request_body || ''} onChange={(e) => setEditingConfig(prev => ({ ...prev, custom_request_body: e.target.value }))} placeholder={t('advancedSettings.customRequestBody.placeholder')} rows={8} style={{ fontFamily: 'var(--bf-type-code-md-font-family)', fontSize: 'var(--bf-type-code-md-font-size)' }} />
                    {editingConfig.custom_request_body && editingConfig.custom_request_body.trim() !== '' && (() => {
                      try { JSON.parse(editingConfig.custom_request_body); return <small className="bitfun-model-settings__json-status bitfun-model-settings__json-status--success">{t('advancedSettings.customRequestBody.validJson')}</small>; }
                      catch { return <small className="bitfun-model-settings__json-status bitfun-model-settings__json-status--error">{t('advancedSettings.customRequestBody.invalidJson')}</small>; }
                    })()}
                  </div>
                </ConfigPageRow>
              </>
            )}
          </ConfigPageSection>
          </ScrollArea>

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
        <span
          className="bitfun-model-settings__meta-tag"
          data-bf-component="model-settings"
          data-bf-part="modelMeta"
        >
          {t(`category.${config.category}`)}
        </span>
        {testResult && (
          <span
            data-testid="settings-model-test-status"
            data-config-id={config.id || ''}
            data-model-id={config.model_name}
            data-model-name={config.model_name}
            data-status={testResult.success ? 'success' : 'error'}
            className={`bitfun-model-settings__status-dot ${testResult.success ? 'is-success' : 'is-error'}`}
            title={testResult.message}
          />
        )}
      </>
    );

    const details = (
      <div
        className="bitfun-model-settings__details"
        data-bf-component="model-settings"
        data-bf-part="modelDetails"
      >
        <div className="bitfun-model-settings__details-section">
          <div className="bitfun-model-settings__details-section-title">
            {t('details.basicInfo')}
          </div>
          <div className="bitfun-model-settings__details-grid">
            <div className="bitfun-model-settings__details-item">
              <span className="bitfun-model-settings__details-label">{t('form.configName')}</span>
              <span className="bitfun-model-settings__details-value">{providerDisplayName}</span>
            </div>
            <div className="bitfun-model-settings__details-item">
              <span className="bitfun-model-settings__details-label">{t('details.modelName')}</span>
              <span className="bitfun-model-settings__details-value">{config.model_name}</span>
            </div>
            <div className="bitfun-model-settings__details-item">
              <span className="bitfun-model-settings__details-label">{t('details.contextWindow')}</span>
              <span className="bitfun-model-settings__details-value">{config.context_window != null ? i18nService.formatNumber(config.context_window) : '128,000'}</span>
            </div>
            <div className="bitfun-model-settings__details-item bitfun-model-settings__details-item--wide">
              <span className="bitfun-model-settings__details-label">{t('details.apiUrl')}</span>
              <span className="bitfun-model-settings__details-value">{config.base_url}</span>
            </div>
            {config.capabilities && config.capabilities.length > 0 && (
              <div className="bitfun-model-settings__details-item bitfun-model-settings__details-item--wide">
                <span className="bitfun-model-settings__details-label">{t('details.capabilities')}</span>
                <div className="bitfun-model-settings__details-tags">
                  {config.capabilities.map(capability => (
                    <span key={capability} className="bitfun-model-settings__details-tag">
                      {t(`capabilities.${capability}`, { defaultValue: capability })}
                    </span>
                  ))}
                </div>
              </div>
            )}
          </div>
        </div>
        {testResult && (
          <div className="bitfun-model-settings__details-section">
            <div className="bitfun-model-settings__details-section-title">
              {t('actions.test')}
            </div>
            <div className={`bitfun-model-settings__test-result ${testResult.success ? 'success' : 'error'}`}>
              {testResult.message}
            </div>
          </div>
        )}
      </div>
    );

    const control = (
      <div
        className="bitfun-model-settings__model-actions"
        data-bf-component="model-settings"
        data-bf-part="modelActions"
      >
        <Switch
          checked={config.enabled}
          onChange={(e) => {
            void handleToggleEnabled(config, e.target.checked);
          }}
        />
        <Tooltip content={t('actions.test')}>
          <IconButton
            aria-label={t('actions.test')}
            size="sm"
            loading={isTesting}
            onClick={() => void handleTest(config)}
            icon={isTesting ? <Loader size={14} /> : <Wifi size={14} />}
          />
        </Tooltip>
        <Tooltip content={t('actions.edit')}>
          <IconButton
            aria-label={t('actions.edit')}
            size="sm"
            onClick={() => handleEdit(config)}
            icon={<Icon name="edit" size="sm" />}
          />
        </Tooltip>
        <Tooltip content={t('actions.delete')}>
          <IconButton
            aria-label={t('actions.delete')}
            tone="danger"
            size="sm"
            onClick={() => void handleDelete(config.id!)}
            icon={<Icon name="delete" size="sm" />}
          />
        </Tooltip>
      </div>
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
        data-testid="settings-model-row"
        data-config-id={config.id || ''}
        data-model-id={config.model_name}
        data-model-name={config.model_name}
        data-bf-component="model-settings"
        data-bf-part="modelItem"
        data-bf-state={[isExpanded && 'expanded', !config.enabled && 'disabled'].filter(Boolean).join(' ') || undefined}
      />
    );
  };

  const streamTtftTimeoutLabel = (
    <span className="bitfun-model-settings__inline-header-main">
      <span>{t('streamTtftTimeout.label')}</span>
      <Tooltip content={t('streamTtftTimeout.hint')} placement="top">
        <span
          className="bitfun-model-settings__inline-header-info"
          role="button"
          tabIndex={0}
          aria-label={t('streamTtftTimeout.hint')}
        >
          <Icon name="info" size="sm" />
        </span>
      </Tooltip>
    </span>
  );

  const streamIdleTimeoutLabel = (
    <span className="bitfun-model-settings__inline-header-main">
      <span>{t('streamIdleTimeout.label')}</span>
      <Tooltip content={t('streamIdleTimeout.hint')} placement="top">
        <span
          className="bitfun-model-settings__inline-header-info"
          role="button"
          tabIndex={0}
          aria-label={t('streamIdleTimeout.hint')}
        >
          <Icon name="info" size="sm" />
        </span>
      </Tooltip>
    </span>
  );
  const reasoningPanelDraft = reasoningPanelDraftKey
    ? selectedModelDrafts.find(draft => draft.key === reasoningPanelDraftKey)
    : undefined;
  const reasoningPanelProjection = reasoningPanelDraft
    ? resolveDraftReasoningProjection(reasoningPanelDraft)
    : undefined;
  const reasoningPanelProjectionRequest = reasoningPanelDraft && editingConfig
    ? {
        provider: editingConfig.provider || 'openai',
        modelName: reasoningPanelDraft.modelName,
        baseUrl: editingConfig.base_url || '',
        contextWindow: reasoningPanelDraft.contextWindow,
        maxTokens: reasoningPanelDraft.maxTokens,
      }
    : undefined;
  const modelsDevSourceLabel = modelsDevStatus
    ? t(`modelsDevCatalog.source.${modelsDevStatus.active_source}`)
    : t('modelsDevCatalog.loading');
  const modelsDevUpdatedAt = modelsDevStatus?.cache_updated_at_ms
    ? i18nService.formatDate(new Date(modelsDevStatus.cache_updated_at_ms), {
        dateStyle: 'medium',
        timeStyle: 'short',
      })
    : t('modelsDevCatalog.noCache');

  
  return (
    <ConfigPageLayout className="bitfun-model-settings" data-bf-component="model-settings" data-bf-part="root" data-bf-view="settings">
      <ConfigPageHeader
        title={t('title')}
        subtitle={t('subtitle')}
      />

      <ConfigPageContent className="bitfun-model-settings__content">
        <ConfigPageSection
          title={tDefault('sections.defaults')}
          description={tDefault('subtitle')}
        >
          <DefaultModelConfig />
        </ConfigPageSection>

        <ConfigPageSection
          title={t('subscriptionAuth.sectionTitle')}
          description={t('subscriptionAuth.sectionDescription')}
          extra={(
            <Tooltip content={t('subscriptionAuth.rescan')}>
              <IconButton
                size="sm"
                onClick={refreshSubscriptionAccounts}
                aria-label={t('subscriptionAuth.rescan')}
                disabled={isLoadingSubscriptions}
                icon={<Icon name="refresh" size="md" className={isLoadingSubscriptions ? 'bitfun-model-settings__spin' : ''} />}
              />
            </Tooltip>
          )}
        >
          <div className="bitfun-model-settings__cli-discovery" data-bf-component="model-settings" data-bf-part="subscriptionArea">
            {showSubscriptionMigrationNotice && (
              <div className="bitfun-model-settings__subscription-migration-notice" data-bf-component="model-settings" data-bf-part="subscriptionNotice" role="status">
                <Icon name="info" size="md" aria-hidden="true" />
                <span>{t('subscriptionAuth.secureStoreMigrationNotice')}</span>
                <Button
                  size="sm"
                  variant="outline"
                  onClick={dismissSubscriptionMigrationNotice}
                >
                  {t('subscriptionAuth.dismissMigrationNotice')}
                </Button>
              </div>
            )}
            {subscriptionAccounts.map((account) => {
              const descriptionParts: string[] = [];
              if (account.connected && account.account) {
                descriptionParts.push(account.account);
              }
              if (account.connected && account.expires_at) {
                descriptionParts.push(
                  t('subscriptionAuth.expiresAt', {
                    time: i18nService.formatDate(new Date(account.expires_at * 1000), {
                      dateStyle: 'medium',
                      timeStyle: 'short',
                    }),
                  }),
                );
              } else if (account.connected) {
                descriptionParts.push(t('subscriptionAuth.tokenValid'));
              } else if (account.vault_unavailable) {
                descriptionParts.push(t('subscriptionAuth.vaultUnavailable'));
              } else if (account.reauthentication_required) {
                descriptionParts.push(t('subscriptionAuth.reauthenticationRequired'));
              } else {
                descriptionParts.push(t('subscriptionAuth.notSignedIn'));
              }
              const isLoggingIn = loggingInProvider === account.provider;
              const anyLoginInProgress = loggingInProvider !== null;
              const loginPanel = subscriptionLoginPanel?.provider === account.provider
                ? subscriptionLoginPanel
                : null;
              const remainingSeconds = loginPanel?.deadlineMs
                ? Math.max(0, Math.ceil((loginPanel.deadlineMs - subscriptionLoginClock) / 1000))
                : 0;
              const countdown = `${Math.floor(remainingSeconds / 60)}:${String(remainingSeconds % 60).padStart(2, '0')}`;
              const openCodePlanRows = account.provider === 'opencode'
                ? (['zen', 'go'] as const).map((plan) => {
                    const planOfferings = (account.api_offerings || [])
                      .filter((offering) => offering.plan === plan);
                    const populatedOfferings = planOfferings
                      .filter((offering) => offering.models.length > 0);
                    return {
                      plan,
                      offerings: populatedOfferings.length > 0
                        ? populatedOfferings
                        : planOfferings,
                    };
                  }).filter((row) => row.offerings.length > 0)
                : [];
              const hasOpenCodeOfferings = openCodePlanRows.length > 0;
              return (
                <React.Fragment key={account.provider}>
                  <ConfigPageRow
                    label={account.display_label}
                    description={descriptionParts.map((part) => (
                      <span
                        key={part}
                        className="bitfun-model-settings__cli-description-line"
                      >
                        {part}
                      </span>
                    ))}
                    className="bitfun-model-settings__cli-account"
                    align="center"
                  >
                    <div className="bitfun-model-settings__cli-actions">
                      {account.connected ? (
                        <>
                          <Button
                            size="sm"
                            variant="outline"
                            disabled={anyLoginInProgress}
                            onClick={() => void handleSubscriptionRefresh(account.provider)}
                          >
                            {t('subscriptionAuth.refresh')}
                          </Button>
                          <Button
                            size="sm"
                            variant="outline"
                            disabled={anyLoginInProgress}
                            onClick={() => requestSubscriptionLogout(account)}
                          >
                            {t('subscriptionAuth.logout')}
                          </Button>
                          {(account.provider !== 'opencode' || !hasOpenCodeOfferings) && (
                            <Button
                              size="sm"
                              variant="fill"
                              disabled={anyLoginInProgress}
                              onClick={() => handleImportFromSubscription(account)}
                            >
                              {t('subscriptionAuth.import')}
                            </Button>
                          )}
                        </>
                      ) : account.vault_unavailable ? (
                        <Button
                          size="sm"
                          variant="outline"
                          disabled={anyLoginInProgress}
                          onClick={() => void handleSubscriptionRefresh(account.provider)}
                        >
                          {t('subscriptionAuth.retryVault')}
                        </Button>
                      ) : (
                        <Button
                          size="sm"
                          variant="fill"
                          loading={isLoggingIn}
                          disabled={anyLoginInProgress}
                          onClick={() => void handleSubscriptionLogin(account.provider)}
                        >
                          {t(loginPanel?.status === 'failed'
                            ? 'subscriptionAuth.retryLogin'
                            : 'subscriptionAuth.login')}
                        </Button>
                      )}
                      {isLoggingIn && (
                        <Button
                          size="sm"
                          variant="outline"
                          disabled={loginPanel?.status === 'cancelling'}
                          onClick={() => void handleCancelSubscriptionLogin(account.provider)}
                        >
                          {t('subscriptionAuth.cancel')}
                        </Button>
                      )}
                    </div>
                  </ConfigPageRow>

                  {account.connected && openCodePlanRows.map(({ plan, offerings }) => (
                    <ConfigPageRow
                      key={`${account.provider}:${plan}`}
                      label={getOpenCodePlanLabel(plan)}
                      description={getOpenCodePlanDescription(plan)}
                      className="bitfun-model-settings__opencode-plan"
                      align="center"
                    >
                      <div className="bitfun-model-settings__cli-actions bitfun-model-settings__opencode-plan-actions">
                        {offerings.map((offering) => {
                          const formatLabel = getOpenCodeFormatLabel(offering.format);
                          const label = offering.models.length > 0
                            ? t('subscriptionAuth.useFormatWithCount', {
                                format: formatLabel,
                                modelCount: i18nService.formatNumber(offering.models.length),
                              })
                            : t('subscriptionAuth.useFormat', { format: formatLabel });
                          return (
                            <Button
                              key={`${offering.plan}:${offering.format}`}
                              size="sm"
                              variant="outline"
                              disabled={anyLoginInProgress}
                              onClick={() => handleImportFromSubscription(account, offering)}
                            >
                              {label}
                            </Button>
                          );
                        })}
                      </div>
                    </ConfigPageRow>
                  ))}

                  {loginPanel && (
                    <div
                      className={`bitfun-model-settings__subscription-login-panel bitfun-model-settings__subscription-login-panel--${loginPanel.status}`}
                      data-bf-component="model-settings"
                      data-bf-part="subscriptionPanel"
                      data-bf-status={loginPanel.status}
                      role={loginPanel.status === 'failed' ? 'alert' : undefined}
                    >
                      <div className="bitfun-model-settings__subscription-login-summary" data-bf-component="model-settings" data-bf-part="subscriptionSummary">
                        <strong>
                          {loginPanel.status === 'failed'
                            ? t('subscriptionAuth.loginNeedsRetry')
                            : loginPanel.status === 'cancelling'
                              ? t('subscriptionAuth.loginCancelling')
                              : t('subscriptionAuth.loginPending')}
                        </strong>
                        {loginPanel.status === 'pending' && (
                          <>
                            <span>
                              {t(loginPanel.method === 'browser'
                                ? 'subscriptionAuth.browserInstructions'
                                : 'subscriptionAuth.deviceInstructions')}
                            </span>
                            <span>{t('subscriptionAuth.timeRemaining', { time: countdown })}</span>
                          </>
                        )}
                        {loginPanel.status === 'failed' && loginPanel.error && (
                          <span>{t('subscriptionAuth.loginFailedInline', { error: loginPanel.error })}</span>
                        )}
                      </div>

                      {loginPanel.status === 'pending' && loginPanel.userCode && (
                        <div className="bitfun-model-settings__subscription-code" data-bf-component="model-settings" data-bf-part="subscriptionCode">
                          <span>{t('subscriptionAuth.verificationCode')}</span>
                          <code>{loginPanel.userCode}</code>
                        </div>
                      )}

                      {loginPanel.status === 'pending' && (
                        <div className="bitfun-model-settings__subscription-login-actions" data-bf-component="model-settings" data-bf-part="subscriptionActions">
                          {loginPanel.userCode && (
                            <Button
                              size="sm"
                              variant="outline"
                              onClick={() => void handleCopySubscriptionCode(loginPanel.userCode!)}
                            >
                              {t('subscriptionAuth.copyCode')}
                            </Button>
                          )}
                          {loginPanel.authorizationUrl && (
                            <Button
                              size="sm"
                              variant="outline"
                              onClick={() => void handleOpenSubscriptionAuthorization(loginPanel.authorizationUrl)}
                              leadingIcon={<Icon name="arrow-up-right" size="sm" aria-hidden="true" />}
                            >
                              {t('subscriptionAuth.openAuthorization')}
                            </Button>
                          )}
                        </div>
                      )}
                    </div>
                  )}
                </React.Fragment>
              );
            })}
          </div>
        </ConfigPageSection>

        <ConfigPageSection
          className="bitfun-model-settings__models-section"
          bodySurface={false}
          title={tDefault('sections.providers')}
          description={t('subtitle')}
          extra={(
            <Tooltip content={t('actions.addProvider')}>
              <IconButton
                aria-label={t('actions.addProvider')}
                size="sm"
                onClick={handleCreateNew}
                icon={<Icon name="plus" size="md" />}
              />
            </Tooltip>
          )}
        >
          {aiModels.length === 0 ? (
            <div className="bitfun-model-settings__empty" data-bf-component="model-settings" data-bf-part="empty">
              <Wifi size={36} />
              <p>{t('empty.noModels')}</p>
              <Button data-testid="settings-model-create-first-config-btn" variant="fill" size="sm" onClick={handleCreateNew} leadingIcon={<Icon name="plus" size="sm" />}>

                {t('actions.createFirst')}
              </Button>
            </div>
          ) : (
            <div className="bitfun-model-settings__collection" data-bf-component="model-settings" data-bf-part="collection" data-testid="settings-model-list">
              {providerGroups.map(group => (
                <div key={group.key} className="bitfun-model-settings__provider-group" data-bf-component="model-settings" data-bf-part="providerGroup">
                  <div className="bitfun-model-settings__provider-group-header" data-bf-component="model-settings" data-bf-part="providerGroupHeader">
                    <div className="bitfun-model-settings__provider-group-title" data-bf-component="model-settings" data-bf-part="providerGroupTitle">
                      <span>{group.providerName}</span>
                      <span className="bitfun-model-settings__provider-group-count">{group.models.length}</span>
                      <span className="bitfun-model-settings__meta-tag">
                        {requestFormatLabelMap[group.models[0]?.provider || 'openai'] || (group.models[0]?.provider || 'openai')}
                      </span>
                    </div>
                    <div className="bitfun-model-settings__provider-group-actions" data-bf-component="model-settings" data-bf-part="providerGroupActions">
                      <Tooltip content={t('actions.edit')}>
                        <IconButton
                          aria-label={t('actions.edit')}
                          size="sm"
                          onClick={() => handleEditProvider(group.models[0])}
                          icon={<Icon name="edit" size="sm" />}
                        />
                      </Tooltip>
                    </div>
                  </div>
                  <div className="bitfun-model-settings__provider-group-list" data-bf-component="model-settings" data-bf-part="providerGroupList">
                    {group.models.map(config => renderModelCollectionItem(config))}
                  </div>
                </div>
              ))}
            </div>
          )}
        </ConfigPageSection>

        {modelsDevStatusAvailable && <ConfigPageSection
          title={t('modelsDevCatalog.title')}
          description={t('modelsDevCatalog.description')}
          bodySurface={false}
          extra={(
            <div className="bitfun-model-settings__catalog-actions">
              <Tooltip content={t('modelsDevCatalog.viewDetails')}>
                <IconButton
                  aria-label={t('modelsDevCatalog.viewDetails')}
                  size="sm"
                  onClick={() => setShowModelsDevDetails(true)}
                  icon={<Icon name="eye" size="sm" aria-hidden="true" />}
                />
              </Tooltip>
              <Tooltip content={t(isRefreshingModelsDev
                  ? 'modelsDevCatalog.refreshing'
                  : 'modelsDevCatalog.refreshNow')}>
                <IconButton
                  aria-label={t(isRefreshingModelsDev
                    ? 'modelsDevCatalog.refreshing'
                    : 'modelsDevCatalog.refreshNow')}
                  size="sm"
                  onClick={() => void handleRefreshModelsDev()}
                  disabled={isRefreshingModelsDev}
                  icon={<Icon name="refresh" size="sm" className={isRefreshingModelsDev ? 'bitfun-model-settings__spin' : ''} />}
                />
              </Tooltip>
            </div>
          )}
        >
          <span />
        </ConfigPageSection>}

        <ConfigPageSection
          title={t('streamIdleTimeout.title')}
          description={t('streamIdleTimeout.effectiveNextRound')}
          extra={(
            <Button
              variant="fill"
              size="sm"
              onClick={handleSaveStreamTimeouts}
              disabled={isStreamTimeoutSaving || isStreamTimeoutInvalid}
            >
              {isStreamTimeoutSaving ? (
                <Loader size={16} className="spinning" />
              ) : (
                t('streamIdleTimeout.save')
              )}
            </Button>
          )}
        >
          <ConfigPageRow
            label={streamTtftTimeoutLabel}
            align="center"
          >
            <Input
              value={streamTtftTimeoutInput}
              onChange={(e) => setStreamTtftTimeoutInput(e.target.value)}
              placeholder={t('streamTtftTimeout.placeholder')}
              size="sm"
            />
          </ConfigPageRow>
          <ConfigPageRow
            label={streamIdleTimeoutLabel}
            align="center"
          >
            <Input
              value={streamIdleTimeoutInput}
              onChange={(e) => setStreamIdleTimeoutInput(e.target.value)}
              placeholder={t('streamIdleTimeout.placeholder')}
              size="sm"
            />
          </ConfigPageRow>
        </ConfigPageSection>

        <ConfigPageSection
          title={tDefault('sections.proxy')}
          description={t('proxy.enableHint')}
          extra={(
            <Button
              variant="fill"
              size="sm"
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
            />
          </ConfigPageRow>
          <ConfigPageRow label={t('proxy.url')} description={t('proxy.urlHint')} align="center">
            <Input
              value={proxyConfig.url}
              onChange={(e) => setProxyConfig(prev => ({ ...prev, url: e.target.value }))}
              placeholder={t('proxy.urlPlaceholder')}
              disabled={!proxyConfig.enabled}
              size="sm"
            />
          </ConfigPageRow>
          <ConfigPageRow label={t('proxy.username')} align="center">
            <Input
              value={proxyConfig.username || ''}
              onChange={(e) => setProxyConfig(prev => ({ ...prev, username: e.target.value }))}
              placeholder={t('proxy.usernamePlaceholder')}
              disabled={!proxyConfig.enabled}
              size="sm"
            />
          </ConfigPageRow>
          <ConfigPageRow label={t('proxy.password')} align="center">
            <Input
              type="password"
              value={proxyConfig.password || ''}
              onChange={(e) => setProxyConfig(prev => ({ ...prev, password: e.target.value }))}
              placeholder={t('proxy.passwordPlaceholder')}
              disabled={!proxyConfig.enabled}
              size="sm"
            />
          </ConfigPageRow>
        </ConfigPageSection>
      </ConfigPageContent>

      <Dialog
        open={showModelsDevDetails}
        onOpenChange={(nextOpen) => { if (!nextOpen) setShowModelsDevDetails(false); }}
        size="sm"
      >
        <DialogHeader>
          <DialogHeading>
            <DialogTitle>{t('modelsDevCatalog.detailsTitle')}</DialogTitle>
          </DialogHeading>
          <DialogClose />
        </DialogHeader>
        <DialogBody inset="none">
        <div className="bitfun-model-settings__catalog-details">
          <ConfigPageRow label={t('modelsDevCatalog.activeSource')} align="center">
            <span className="bitfun-model-settings__catalog-status-value">{modelsDevSourceLabel}</span>
          </ConfigPageRow>
          <ConfigPageRow label={t('modelsDevCatalog.catalogSize')} align="center">
            <span className="bitfun-model-settings__catalog-status-value">
              {modelsDevStatus
                ? t('modelsDevCatalog.catalogSizeValue', {
                    providers: i18nService.formatNumber(modelsDevStatus.provider_count),
                    models: i18nService.formatNumber(modelsDevStatus.reasoning_model_count),
                  })
                : t('modelsDevCatalog.loading')}
            </span>
          </ConfigPageRow>
          <ConfigPageRow label={t('modelsDevCatalog.cacheUpdatedAt')} align="center">
            <span className="bitfun-model-settings__catalog-status-value">{modelsDevUpdatedAt}</span>
          </ConfigPageRow>
          <ConfigPageRow label={t('modelsDevCatalog.cachePath')} align="center" wide>
            <div className="bitfun-model-settings__catalog-path">
              <code title={modelsDevStatus?.cache_path}>{modelsDevStatus?.cache_path || '—'}</code>
              <Tooltip content={t('modelsDevCatalog.reveal')}>
                <IconButton
                  aria-label={t('modelsDevCatalog.reveal')}
                  size="sm"
                  onClick={() => {
                    void aiApi.revealModelsDevCacheDirectory().catch((error) => {
                      log.warn('Failed to reveal models.dev cache', { error });
                      notification.error(t('modelsDevCatalog.revealFailed'));
                    });
                  }}
                  icon={<FolderOpen size={14} aria-hidden="true" />}
                />
              </Tooltip>
            </div>
          </ConfigPageRow>
          <ConfigPageRow label={t('modelsDevCatalog.revision')} align="center">
            <code className="bitfun-model-settings__catalog-revision" title={modelsDevStatus?.revision}>
              {modelsDevStatus?.revision ? `${modelsDevStatus.revision.slice(0, 12)}…` : '—'}
            </code>
          </ConfigPageRow>
          <div className="bitfun-model-settings__catalog-offline-help" role="note">
            <Icon name="info" size="sm" aria-hidden="true" />
            <div>
              <strong>{t('modelsDevCatalog.offlineTitle')}</strong>
              <p>{t('modelsDevCatalog.offlineDescription')}</p>
              <div className="bitfun-model-settings__catalog-offline-actions">
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => void systemAPI.openExternal(MODELS_DEV_DOWNLOAD_URL)}
                  leadingIcon={<Icon name="arrow-up-right" size="sm" aria-hidden="true" />}
                >

                  {t('modelsDevCatalog.downloadOriginal')}
                </Button>
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => {
                    void aiApi.revealModelsDevCacheDirectory().catch((error) => {
                      log.warn('Failed to reveal models.dev cache directory', { error });
                      notification.error(t('modelsDevCatalog.revealFailed'));
                    });
                  }}
                  leadingIcon={<FolderOpen size={14} aria-hidden="true" />}
                >

                  {t('modelsDevCatalog.openCacheDirectory')}
                </Button>
              </div>
            </div>
          </div>
        </div>
              </DialogBody>
      </Dialog>

      <Dialog
        open={!!subscriptionLogoutRequest}
        onOpenChange={(nextOpen) => { if (!nextOpen) setSubscriptionLogoutRequest(null); }}
        size="sm"
        closeOnPointerOutside={false}
      >
        <DialogHeader>
          <DialogHeading>
            <DialogTitle>{t('subscriptionAuth.logoutConfirmTitle')}</DialogTitle>
          </DialogHeading>
          <DialogClose />
        </DialogHeader>
        <DialogBody inset="none">
        <div className="bitfun-model-settings__subscription-logout-confirm" data-bf-component="model-settings" data-bf-part="logoutConfirm">
          <p>
            {subscriptionLogoutRequest?.affectedModels.length
              ? t('subscriptionAuth.logoutAffectedModels', {
                  count: subscriptionLogoutRequest.affectedModels.length,
                })
              : t('subscriptionAuth.logoutNoAffectedModels')}
          </p>
          {!!subscriptionLogoutRequest?.affectedModels.length && (
            <ScrollArea className="bitfun-model-settings__subscription-logout-list">
              <ul>
                {subscriptionLogoutRequest.affectedModels.map((model) => (
                  <li key={model.id}>{model.name} · {model.model_name}</li>
                ))}
              </ul>
            </ScrollArea>
          )}
          <p>{t('subscriptionAuth.logoutConsequence')}</p>
        </div>
              </DialogBody>
        <DialogFooter>{(
          <>
            <Button
              size="sm"
              variant="fill"
              onClick={() => setSubscriptionLogoutRequest(null)}
            >
              {t('subscriptionAuth.cancel')}
            </Button>
            <Button
              size="sm"
              variant="fill"
              tone="danger"
              onClick={() => void confirmSubscriptionLogout()}
            >
              {t('subscriptionAuth.confirmLogout')}
            </Button>
          </>
        )}</DialogFooter>
      </Dialog>

      <Dialog
        open={isEditing && !!editingConfig}
        onOpenChange={(nextOpen) => {
          if (!nextOpen) {
            if (reasoningPanelDraft) setReasoningPanelDraftKey(null);
            else closeEditingModal();
          }
        }}
        size="2xl"
      >
        <DialogHeader>
          <DialogHeading>
            <DialogTitle>{reasoningPanelDraft
          ? t('reasoningPresets.dialogTitle', {
              provider: editingConfig?.name?.trim()
                || currentTemplate?.name
                || editingConfig?.provider
                || '',
              model: reasoningPanelDraft.modelName,
            })
          : editingConfig?.id
            ? t('editModel')
            : (getProviderInstanceId(editingConfig)
              ? t('editProvider')
              : (currentTemplate ? `${t('newProvider')} - ${currentTemplate.name}` : t('newProvider')))}</DialogTitle>
          </DialogHeading>
          <DialogClose />
        </DialogHeader>
        <DialogBody inset="none">
        {reasoningPanelDraft ? (
          <ReasoningConfigPanel
            key={reasoningPanelDraft.key}
            value={reasoningPanelDraft.reasoning}
            generatedProjection={reasoningPanelProjection}
            modelsDevReasoningCatalog={modelCatalog?.models_dev_reasoning_catalog}
            projectionRequest={reasoningPanelProjectionRequest}
            requestFormatLabel={reasoningPanelProjectionRequest
              ? requestFormatLabelMap[reasoningPanelProjectionRequest.provider]
                || reasoningPanelProjectionRequest.provider
              : undefined}
            onCancel={() => setReasoningPanelDraftKey(null)}
            onApply={(result: ReasoningConfigApplyResult) => {
              updateModelDraft(reasoningPanelDraft.modelName, {
                reasoning: result.reasoning,
                reasoningProjectionCatalog: result.projectionCatalog,
                reasoningProjectionSnapshot: {
                  catalog: result.projectionCatalog,
                  projection: result.projection,
                },
              });
              setReasoningPanelDraftKey(null);
            }}
          />
        ) : renderEditingForm()}
              </DialogBody>
        <DialogFooter>{reasoningPanelDraft ? undefined : (
          <>
            <Button variant="fill" onClick={closeEditingModal}>{t('actions.cancel')}</Button>
            <Button data-testid="settings-model-save-btn" variant="primary" onClick={handleSave}>
              {t('actions.save')}
            </Button>
          </>
        )}</DialogFooter>
      </Dialog>
    </ConfigPageLayout>
  );
};

export default ModelSettingsPage;
