/**
 * Model selector component.
 * Shows the active model and allows quick switching.
 *
 * Config linkage:
 * - Model selection is shared across all future mode sessions through
 *   ai.agent_model_defaults.mode. Delegated subagents keep separate defaults.
 * - Supports 'auto' | 'primary' | 'fast' | specific model IDs
 */

import React, { useState, useEffect, useId, useRef, useCallback, useMemo, useSyncExternalStore } from 'react';
import { createPortal } from 'react-dom';
import { getAppearanceOverlayHost } from '@/infrastructure/appearance/runtime/AppearanceOverlayHost';
import { ChevronDown, Check, Zap } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { configManager } from '@/infrastructure/config/services/ConfigManager';
import { agentAPI } from '@/infrastructure/api/service-api/AgentAPI';
import {
  aiApi,
  type AIModelCatalog,
  type ReasoningCatalogProjection,
} from '@/infrastructure/api/service-api/AIApi';
import { ACPClientAPI, type AcpSessionOptions } from '@/infrastructure/api/service-api/ACPClientAPI';
import { getProviderDisplayName } from '@/infrastructure/config/services/modelConfigs';
import { globalEventBus } from '@/infrastructure/event-bus';
import type { AIModelConfig, AgentModelDefaultsConfig, DefaultModelsConfig } from '@/infrastructure/config/types';
import { Switch, Tooltip } from '@/component-library';
import { PresenceBoundary } from '@/component-library/components/PresenceBoundary';
import { notificationService } from '@/shared/notification-system';
import { FlowChatStore } from '../store/FlowChatStore';
import { getModelMaxTokens } from '../services/flow-chat-manager/SessionModule';
import { acpClientIdFromAgentType } from '../utils/acpSession';
import {
  buildAcpFastModeValue,
  getAcpModelProviderName,
  resolveAcpFastModeState,
  resolveAcpModeState,
  resolveAcpReasoningState,
} from '../utils/acpSessionConfig';
import { sessionProjectWorkspacePath } from '../utils/sessionWorkspace';
import {
  buildContextUsageTooltip,
  type ContextUsageSource,
} from '../utils/tokenUsageDisplay';
import { createLogger } from '@/shared/utils/logger';
import { getModelSelectorDropdownLayout } from './modelSelectorDropdownPosition';
import { ReasoningPresetSelector } from './ReasoningPresetSelector';
import {
  getRecentReasoningPreset,
  setRecentReasoningPreset,
} from '../utils/reasoningPresets';
import {
  shouldIncludeInternalModelSession,
  shouldSyncSessionModelSelection,
} from '../utils/modelSelectionTarget';
import './ModelSelector.scss';

const log = createLogger('ModelSelector');
const ACP_SESSION_OPTIONS_TIMEOUT_MS = 65_000;

export interface ExternalModelSelection {
  models: string[];
  selectedModelId?: string;
  defaultModelId?: string;
  reasoningCatalog?: AIModelCatalog;
  selectedReasoningPreset?: string;
  providerLabel: string;
  disabled?: boolean;
  /**
   * Also offer this device's own enabled models, and fall back to its catalog
   * for reasoning presets.
   *
   * For a transport that only relays a session elsewhere, `models` is a probe
   * snapshot rather than the set of choices the user has: the executing side
   * is brought up to whatever is picked. Leave this off for a transport that
   * owns a genuinely foreign model list.
   */
  includeLocalCatalog?: boolean;
  onSelect: (modelId: string) => void | Promise<void>;
  onSelectReasoningPreset?: (presetId: string | null) => void | Promise<void>;
}

interface ModelSelectorProps {
  /** Current target agent type. */
  currentMode: string;
  /** Custom class name. */
  className?: string;
  /** Preferred dropdown placement relative to the trigger. */
  dropdownPlacement?: 'top' | 'bottom';
  /** Current session ID (used to update the selected session model). */
  sessionId?: string;
  /** Whether the active input target is a Task subagent session. */
  isSubagentSession?: boolean;
  /** Current token count. */
  currentTokens?: number;
  /** Max token capacity. */
  maxTokens?: number;
  /** Semantic source for the context usage number. */
  contextUsageSource?: ContextUsageSource;
  /** Called when model switching starts or completes, so the parent can gate sending. */
  onLoadingChange?: (loading: boolean) => void;
  /** Target-owned model catalog for transports that do not have a local backend session. */
  externalSelection?: ExternalModelSelection;
  /** Agent-profile model used only when the session has no explicit selection. */
  modeDefaultModelId?: string;
  /** Whether a selection also changes BitFun's shared built-in mode default. */
  persistSharedModeDefault?: boolean;
  /** Whether lifecycle ownership currently prevents Session setting changes. */
  disabled?: boolean;
}

interface ModelInfo {
  id: string;
  /** User-defined configuration name (AIModelConfig.name). */
  configName: string;
  /** Actual model identifier (AIModelConfig.model_name). */
  modelName: string;
  providerName: string;
  provider: string;
  contextWindow?: number;
}

// Helper: identify special model IDs.
const isSpecialModel = (value: string): value is 'auto' | 'primary' | 'fast' => {
  return value === 'auto' || value === 'primary' || value === 'fast';
};

function resolveConcreteModelId(
  modelId: string,
  defaultModels: DefaultModelsConfig,
): string | undefined {
  if (modelId === 'auto' || modelId === 'primary') return defaultModels.primary ?? undefined;
  if (modelId === 'fast') return defaultModels.fast ?? defaultModels.primary ?? undefined;
  return modelId || undefined;
}

const formatContextWindow = (contextWindow?: number): string | null => {
  if (!contextWindow) return null;
  return `${Math.round(contextWindow / 1000)}k`;
};

const buildModelMetaText = (model: Pick<ModelInfo, 'providerName' | 'contextWindow'>): string => {
  const parts = [model.providerName];
  const contextWindow = formatContextWindow(model.contextWindow);

  if (contextWindow) {
    parts.push(contextWindow);
  }

  return parts.join(' · ');
};

const buildResolvedModelTooltipText = (
  modelName: string | undefined,
  model: Pick<ModelInfo, 'providerName' | 'contextWindow'> | null | undefined,
  fallback: string
): string => {
  if (!model) return fallback;

  const parts = [];
  if (modelName) {
    parts.push(modelName);
  }

  const metaText = buildModelMetaText(model);
  if (metaText) {
    parts.push(metaText);
  }

  return parts.join(' · ') || fallback;
};

const getModelDisplayLabel = (model: ModelInfo | null, fallback: string): string => {
  if (!model) return fallback;
  if (isSpecialModel(model.id)) return model.configName;
  return model.modelName || model.configName || fallback;
};

const getModelTooltipText = (model: ModelInfo | null, fallback: string): string => {
  if (!model) return fallback;
  if (model.id === 'auto') return model.providerName;
  if (isSpecialModel(model.id)) {
    return buildResolvedModelTooltipText(model.modelName, model, fallback);
  }
  return buildModelMetaText(model);
};

const buildAutoModelInfo = (
  t: (key: string) => string,
): ModelInfo => ({
  id: 'auto',
  configName: t('modelSelector.autoModel'),
  modelName: t('modelSelector.autoModel'),
  providerName: t('modelSelector.autoModelDesc'),
  provider: 'auto',
});

function withTimeout<T>(promise: Promise<T>, timeoutMs: number, message: string): Promise<T> {
  return new Promise((resolve, reject) => {
    const timeoutId = window.setTimeout(() => reject(new Error(message)), timeoutMs);
    promise.then(
      value => {
        window.clearTimeout(timeoutId);
        resolve(value);
      },
      error => {
        window.clearTimeout(timeoutId);
        reject(error);
      },
    );
  });
}

const syncAcpContextUsageToStore = (
  sessionId: string | undefined,
  options: AcpSessionOptions,
): void => {
  if (!sessionId || !options.contextUsage) {
    return;
  }

  FlowChatStore.getInstance().updateAcpContextUsage(sessionId, options.contextUsage);
};

export const ModelSelector: React.FC<ModelSelectorProps> = ({
  currentMode,
  className = '',
  dropdownPlacement = 'top',
  sessionId,
  isSubagentSession = false,
  currentTokens = 0,
  maxTokens = 0,
  contextUsageSource,
  onLoadingChange,
  externalSelection,
  modeDefaultModelId,
  persistSharedModeDefault = true,
  disabled = false,
}) => {
  const { t } = useTranslation('flow-chat');
  const [allModels, setAllModels] = useState<AIModelConfig[]>([]);
  const [modelCatalog, setModelCatalog] = useState<AIModelCatalog | null>(null);
  const [defaultModels, setDefaultModels] = useState<DefaultModelsConfig>({});
  const [modeModel, setModeModel] = useState('auto');
  const [acpOptions, setAcpOptions] = useState<AcpSessionOptions | null>(null);
  const [dropdownOpen, setDropdownOpen] = useState(false);
  const [keyboardNavigationOpen, setKeyboardNavigationOpen] = useState(false);
  const [loading, setLoading] = useState(false);
  const [reasoningLoading, setReasoningLoading] = useState(false);
  const acpRestoreToastShownRef = useRef<string | null>(null);
  const acpOptionsRef = useRef<AcpSessionOptions | null>(null);

  const dropdownRef = useRef<HTMLDivElement>(null);
  const portalDropdownRef = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const menuId = useId();

  useEffect(() => {
    onLoadingChange?.(loading || reasoningLoading);
  }, [loading, onLoadingChange, reasoningLoading]);

  const [dropdownStyle, setDropdownStyle] = useState<React.CSSProperties>({
    position: 'fixed',
    visibility: 'hidden',
  });
  const [resolvedDropdownPlacement, setResolvedDropdownPlacement] = useState(dropdownPlacement);
  const activeSession = sessionId ? FlowChatStore.getInstance().getState().sessions.get(sessionId) : undefined;
  const sessionReasoningPreset = useSyncExternalStore(
    useCallback(
      (callback) => FlowChatStore.getInstance().subscribe(() => callback()),
      [],
    ),
    useCallback(
      () => sessionId
        ? FlowChatStore.getInstance().getState().sessions.get(sessionId)?.config.reasoningPreset
        : undefined,
      [sessionId],
    ),
    () => undefined,
  );
  const acpClientId =
    acpClientIdFromAgentType(activeSession?.config.agentType) ??
    acpClientIdFromAgentType(activeSession?.mode);
  const isAcpSession = Boolean(acpClientId && sessionId);
  const targetIsSubagent = isSubagentSession || activeSession?.sessionKind === 'subagent';

  const loadModelCatalog = useCallback(async () => {
    try {
      setModelCatalog(await aiApi.getModelCatalog());
    } catch (error) {
      setModelCatalog(null);
      log.warn('Failed to load AI model catalog', { error });
    }
  }, []);

  // Load configuration data.
  const loadConfigData = useCallback(async () => {
    try {
      const configData = await configManager.getConfigs([
        'ai.models',
        'ai.default_models',
        'ai.agent_model_defaults',
      ]);
      const models = (configData['ai.models'] as AIModelConfig[] | undefined) || [];
      const defaultModelsData = (configData['ai.default_models'] as DefaultModelsConfig | undefined) || {};
      const agentModelDefaults = configData['ai.agent_model_defaults'] as AgentModelDefaultsConfig | undefined;

      setAllModels(models);
      setDefaultModels(defaultModelsData);
      setModeModel(agentModelDefaults?.mode?.trim() || 'auto');
      await loadModelCatalog();

      log.debug('Configuration loaded', {
        modelsCount: models.length
      });
    } catch (error) {
      log.error('Failed to load configuration', error);
    }
  }, [loadModelCatalog]);
  
  useEffect(() => {
    const unsubscribeCatalog = aiApi.onModelCatalogUpdated(() => {
      void loadModelCatalog();
    });
    loadConfigData();
    
    const handleConfigUpdate = () => {
      log.debug('Configuration update detected, reloading');
      loadConfigData();
    };
    
    globalEventBus.on('mode:config:updated', handleConfigUpdate);
    
    const unsubscribe = configManager.onConfigChange((path) => {
      if (path.startsWith('ai.')) {
        log.debug('AI configuration changed', { path });
        loadConfigData();
      }
    });
    
    return () => {
      globalEventBus.off('mode:config:updated', handleConfigUpdate);
      unsubscribe();
      unsubscribeCatalog();
    };
  }, [loadConfigData, loadModelCatalog]);

  const loadAcpOptions = useCallback(async () => {
    if (!isAcpSession || !acpClientId || !sessionId) {
      setAcpOptions(null);
      return;
    }

    const shouldShowRestoreToast = !acpOptionsRef.current && acpRestoreToastShownRef.current !== sessionId;
    const restoreRequestId = `acp-options:${sessionId}:${acpClientId}`;
    if (shouldShowRestoreToast) {
      acpRestoreToastShownRef.current = sessionId;
      window.dispatchEvent(new CustomEvent('bitfun:acp-session-creation', {
        detail: { phase: 'start', clientId: acpClientId, action: 'restore', requestId: restoreRequestId },
      }));
    }

    let succeeded = false;
    try {
      const options = await withTimeout(
        ACPClientAPI.getSessionOptions({
          sessionId,
          clientId: acpClientId,
          workspacePath: activeSession?.workspacePath || activeSession?.config.workspacePath,
          remoteConnectionId: activeSession?.remoteConnectionId,
          remoteSshHost: activeSession?.remoteSshHost,
        }),
        ACP_SESSION_OPTIONS_TIMEOUT_MS,
        `Timed out restoring ACP session options for ${acpClientId}`,
      );
      setAcpOptions(options);
      syncAcpContextUsageToStore(sessionId, options);
      succeeded = true;
    } catch (error) {
      log.warn('Failed to load ACP session model options', { sessionId, acpClientId, error });
      setAcpOptions(null);
    } finally {
      if (shouldShowRestoreToast) {
        window.dispatchEvent(new CustomEvent('bitfun:acp-session-creation', {
          detail: {
            phase: 'finish',
            clientId: acpClientId,
            action: 'restore',
            requestId: restoreRequestId,
            succeeded,
          },
        }));
      }
    }
  }, [
    activeSession?.config.workspacePath,
    activeSession?.remoteConnectionId,
    activeSession?.remoteSshHost,
    activeSession?.workspacePath,
    acpClientId,
    isAcpSession,
    sessionId,
  ]);

  useEffect(() => {
    acpOptionsRef.current = null;
    acpRestoreToastShownRef.current = null;
    setAcpOptions(null);
  }, [sessionId]);

  useEffect(() => {
    acpOptionsRef.current = acpOptions;
  }, [acpOptions]);

  useEffect(() => {
    loadAcpOptions();
  }, [loadAcpOptions]);

  useEffect(() => {
    if (!isAcpSession || !sessionId || !acpClientId) return;

    return ACPClientAPI.onSessionOptionsChanged((event) => {
      if (event.sessionId === sessionId && event.clientId === acpClientId) {
        loadAcpOptions();
      }
    });
  }, [acpClientId, isAcpSession, loadAcpOptions, sessionId]);
  
  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      const target = event.target as Node;
      if (dropdownRef.current && !dropdownRef.current.contains(target)
          && portalDropdownRef.current && !portalDropdownRef.current.contains(target)) {
        setDropdownOpen(false);
        setKeyboardNavigationOpen(false);
      }
    };

    if (dropdownOpen) {
      document.addEventListener('mousedown', handleClickOutside);
    }

    return () => {
      document.removeEventListener('mousedown', handleClickOutside);
    };
  }, [dropdownOpen]);

  // Calculate portal dropdown position relative to the trigger container.
  useEffect(() => {
    if (!dropdownOpen || !dropdownRef.current) return;

    const updatePosition = () => {
      if (!dropdownRef.current || !portalDropdownRef.current) return;
      const anchorRect = dropdownRef.current.getBoundingClientRect();
      const dropdownRect = portalDropdownRef.current.getBoundingClientRect();
      const layout = getModelSelectorDropdownLayout(
        anchorRect,
        dropdownRect,
        dropdownPlacement,
        { width: window.innerWidth, height: window.innerHeight },
      );
      setDropdownStyle(layout.style);
      setResolvedDropdownPlacement(layout.placement);
    };

    updatePosition();

    const resizeObserver = new ResizeObserver(updatePosition);
    if (portalDropdownRef.current) {
      resizeObserver.observe(portalDropdownRef.current);
    }

    window.addEventListener('scroll', updatePosition, true);
    window.addEventListener('resize', updatePosition);

    return () => {
      resizeObserver.disconnect();
      window.removeEventListener('scroll', updatePosition, true);
      window.removeEventListener('resize', updatePosition);
    };
  }, [dropdownOpen, dropdownPlacement]);

  const acpAvailableModels = useMemo((): ModelInfo[] => {
    if (!isAcpSession || !acpOptions) return [];
    return acpOptions.availableModels.map(model => ({
      id: model.id,
      configName: model.name,
      modelName: model.name,
      providerName: getAcpModelProviderName(model) ?? (acpClientId ? `${acpClientId} ACP` : 'ACP'),
      provider: 'acp',
    }));
  }, [acpClientId, acpOptions, isAcpSession]);

  const acpCurrentModel = useMemo((): ModelInfo | null => {
    if (!isAcpSession || !acpOptions?.currentModelId) return null;
    return acpAvailableModels.find(model => model.id === acpOptions.currentModelId) || {
      id: acpOptions.currentModelId,
      configName: acpOptions.currentModelId,
      modelName: acpOptions.currentModelId,
      providerName: acpClientId ? `${acpClientId} ACP` : 'ACP',
      provider: 'acp',
    };
  }, [acpAvailableModels, acpClientId, acpOptions?.currentModelId, isAcpSession]);

  const externalAvailableModels = useMemo((): ModelInfo[] => {
    if (!externalSelection) return [];
    const localSelectable = externalSelection.includeLocalCatalog
      ? allModels.filter(model => model.enabled).map(model => model.id)
      : [];
    return Array.from(new Set([
      ...externalSelection.models,
      ...localSelectable,
      externalSelection.defaultModelId,
      externalSelection.selectedModelId,
    ].filter((model): model is string => !!model?.trim())))
      .map(modelId => {
        // A synced target reports stable config ids because those are what the
        // worker must execute. Reuse the controller catalog for presentation
        // so generated ids never leak into the normal model-picker UI.
        const localModel = allModels.find(model => model.id === modelId);
        return localModel
          ? {
              id: modelId,
              configName: localModel.name,
              modelName: localModel.model_name,
              providerName: getProviderDisplayName(localModel),
              provider: localModel.provider,
              contextWindow: localModel.context_window,
            }
          : {
              id: modelId,
              configName: modelId,
              modelName: modelId,
              providerName: externalSelection.providerLabel,
              provider: 'external',
            };
      });
  }, [allModels, externalSelection]);

  /**
   * This device's own default, resolved to the concrete id the executing side
   * needs. Only consulted when the target reported no default of its own, so a
   * session that has never been given a model still shows what a local session
   * here would run rather than whichever id happens to sort first.
   */
  const externalLocalDefaultModelId = useMemo((): string | undefined => {
    if (!externalSelection?.includeLocalCatalog) return undefined;
    const configured = modeDefaultModelId?.trim() || modeModel;
    const concrete = resolveConcreteModelId(configured, defaultModels);
    return concrete && allModels.some(model => model.id === concrete && model.enabled)
      ? concrete
      : undefined;
  }, [
    allModels,
    defaultModels,
    externalSelection?.includeLocalCatalog,
    modeDefaultModelId,
    modeModel,
  ]);
  const externalCurrentModelId =
    externalSelection?.selectedModelId?.trim()
    || externalSelection?.defaultModelId?.trim()
    || externalLocalDefaultModelId
    || externalAvailableModels[0]?.id
    || '';
  const externalCurrentModel = externalAvailableModels.find(
    model => model.id === externalCurrentModelId,
  ) ?? null;
  const externalReasoningProjection = useMemo((): ReasoningCatalogProjection | null => {
    if (!externalSelection || !externalCurrentModelId) return null;
    // The target's catalog wins where it has an entry: it is what the worker
    // will actually execute. The local catalog covers a model this device just
    // offered, and a projection restored without a probe snapshot at all.
    const catalogs = [
      externalSelection.reasoningCatalog,
      ...(externalSelection.includeLocalCatalog && modelCatalog ? [modelCatalog] : []),
    ];
    for (const catalog of catalogs) {
      const reasoning = catalog?.models.find(
        model => model.id === externalCurrentModelId,
      )?.reasoning;
      if (reasoning) return reasoning;
    }
    return null;
  }, [externalCurrentModelId, externalSelection, modelCatalog]);

  const acpFastMode = useMemo(
    () => resolveAcpFastModeState(acpOptions?.configOptions ?? []),
    [acpOptions?.configOptions],
  );
  const acpReasoning = useMemo(
    () => resolveAcpReasoningState(acpOptions?.configOptions ?? []),
    [acpOptions?.configOptions],
  );
  const acpMode = useMemo(
    () => resolveAcpModeState(acpOptions?.configOptions ?? []),
    [acpOptions?.configOptions],
  );
  
  const getCurrentModelId = useCallback((): string => {
    // Session-owned model takes priority so that each session remembers
    // its own model selection independently.
    const sessionModelName = activeSession?.config.modelName?.trim();
    if (sessionModelName) {
      if (isSpecialModel(sessionModelName)) {
        if (sessionModelName === 'auto') {
          return 'auto';
        }
        const actualModelId = defaultModels[sessionModelName];
        return allModels.some(model => model.id === actualModelId)
          ? sessionModelName
          : 'auto';
      }
      return allModels.some(model => model.id === sessionModelName)
        ? sessionModelName
        : 'auto';
    }

    if (targetIsSubagent) {
      return 'auto';
    }

    // Legacy sessions created without a model selector fall back to the current
    // mode default until they are migrated by the send path.
    const configuredModelId = modeDefaultModelId?.trim() || modeModel;
    if (configuredModelId === 'auto') return 'auto';
    if (configuredModelId === 'primary' || configuredModelId === 'fast') {
      const actualModelId = defaultModels[configuredModelId];
      const model = allModels.find(m => m.id === actualModelId);
      return model ? configuredModelId : 'auto';
    }
    const model = allModels.find(m => m.id === configuredModelId);
    return model ? configuredModelId : 'auto';
  }, [allModels, modeDefaultModelId, modeModel, defaultModels, activeSession?.config.modelName, targetIsSubagent]);

  const currentModel = useMemo((): ModelInfo | null => {
    const modelId = getCurrentModelId();

    if (modelId === 'auto') {
      return buildAutoModelInfo(t);
    }

    if (modelId === 'primary' || modelId === 'fast') {
      const actualModelId = defaultModels[modelId];
      if (!actualModelId) return buildAutoModelInfo(t);

      const model = allModels.find(m => m.id === actualModelId);
      if (!model) return buildAutoModelInfo(t);

      return {
        id: modelId,
        configName: modelId === 'primary' ? t('modelSelector.primaryModel') : t('modelSelector.fastModel'),
        modelName: model.model_name,
        providerName: getProviderDisplayName(model),
        provider: model.provider,
        contextWindow: model.context_window,
      };
    }

    const model = allModels.find(m => m.id === modelId);
    if (!model) return buildAutoModelInfo(t);

    return {
      id: model.id || '',
      configName: model.name,
      modelName: model.model_name,
      providerName: getProviderDisplayName(model),
      provider: model.provider,
      contextWindow: model.context_window,
    };
  }, [getCurrentModelId, allModels, defaultModels, t]);
  
  const availableModels = useMemo((): ModelInfo[] => {
    return allModels
      .filter(m => {
        if (!m.enabled) return false;
        // Only show chat-capable models (exclude embeddings / image-gen / speech, etc.).
        const capabilities = Array.isArray(m.capabilities) ? m.capabilities : [];
        return capabilities.includes('text_chat');
      })
      .map(m => ({
        id: m.id || '',
        configName: m.name,
        modelName: m.model_name,
        providerName: getProviderDisplayName(m),
        provider: m.provider,
        contextWindow: m.context_window,
      }));
  }, [allModels]);

  const currentNativeModelId = getCurrentModelId();
  const concreteModelId = resolveConcreteModelId(currentNativeModelId, defaultModels);
  const currentReasoningProjection = useMemo((): ReasoningCatalogProjection | null => {
    if (!concreteModelId) return null;
    return modelCatalog?.models.find(model => model.id === concreteModelId)?.reasoning ?? null;
  }, [concreteModelId, modelCatalog]);
  const selectedReasoningPreset = currentReasoningProjection?.status === 'known'
    && currentReasoningProjection.presets?.some(preset => preset.id === sessionReasoningPreset)
    ? sessionReasoningPreset
    : undefined;

  useEffect(() => {
    if (
      !targetIsSubagent
      && concreteModelId
      && selectedReasoningPreset
    ) {
      setRecentReasoningPreset(concreteModelId, selectedReasoningPreset);
    }
  }, [concreteModelId, selectedReasoningPreset, targetIsSubagent]);

  const recentPresetForModel = useCallback((modelId: string): string | undefined => {
    const resolvedModelId = resolveConcreteModelId(modelId, defaultModels);
    if (!resolvedModelId) return undefined;
    const projection = modelCatalog?.models.find(model => model.id === resolvedModelId)?.reasoning;
    if (projection?.status !== 'known') return undefined;
    const recentPreset = getRecentReasoningPreset(resolvedModelId);
    return projection.presets?.some(preset => preset.id === recentPreset)
      ? recentPreset
      : undefined;
  }, [defaultModels, modelCatalog]);
  
  const handleSelectModel = useCallback(async (modelId: string) => {
    if (disabled || loading || reasoningLoading) return;

    if (portalDropdownRef.current?.contains(document.activeElement)) {
      triggerRef.current?.focus();
    }
    setLoading(true);
    setDropdownOpen(false);

    // The optimistic session write below must be undone when the backend
    // rejects the switch; otherwise the selector keeps showing a model the
    // session never adopted, and the next send pushes it to the backend.
    const store = FlowChatStore.getInstance();
    const previousSessionModelName = sessionId
      ? store.getState().sessions.get(sessionId)?.config.modelName
      : undefined;
    const previousReasoningPreset = sessionId
      ? store.getState().sessions.get(sessionId)?.config.reasoningPreset
      : undefined;
    const nextReasoningPreset = recentPresetForModel(modelId);
    let sessionModelWrittenOptimistically = false;

    try {
      if (externalSelection) {
        await externalSelection.onSelect(modelId);
        return;
      }
      if (isAcpSession && acpClientId && sessionId) {
        const options = await ACPClientAPI.setSessionModel({
          sessionId,
          clientId: acpClientId,
          workspacePath: activeSession?.workspacePath || activeSession?.config.workspacePath,
          remoteConnectionId: activeSession?.remoteConnectionId,
          remoteSshHost: activeSession?.remoteSshHost,
          modelId,
        });
        setAcpOptions(options);
        syncAcpContextUsageToStore(sessionId, options);
        store.updateSessionModelName(sessionId, modelId);
        log.info('ACP session model updated', { sessionId, acpClientId, modelId });
        return;
      }

      const updateTargetSessionModel = async () => {
        if (!sessionId) return;

        // Update the frontend session model immediately so the UI reflects the
        // switch without waiting for the backend IPC round-trip.
        store.updateSessionModelName(sessionId, modelId);
        store.updateSessionReasoningPreset(sessionId, nextReasoningPreset);
        sessionModelWrittenOptimistically = true;
        const maxContextTokens = await getModelMaxTokens(modelId, currentMode);
        store.updateSessionMaxContextTokens(sessionId, maxContextTokens);
        const session = store.getState().sessions.get(sessionId);
        if (shouldSyncSessionModelSelection(session)) {
          await agentAPI.updateSessionModel({
            sessionId,
            modelName: modelId,
            reasoningPreset: nextReasoningPreset ?? null,
            workspacePath: sessionProjectWorkspacePath(session),
            remoteConnectionId: session.remoteConnectionId,
            remoteSshHost: session.remoteSshHost,
            includeInternal: shouldIncludeInternalModelSession(session),
          });
        }
      };

      if (targetIsSubagent) {
        await updateTargetSessionModel();
        log.info('Subagent session model updated', { sessionId, modelId });
        return;
      }

      if (persistSharedModeDefault) {
        await configManager.setConfig('ai.agent_model_defaults.mode', modelId);
        setModeModel(modelId);
        globalEventBus.emit('mode:config:updated');
      }
      await updateTargetSessionModel();
      if (sessionId) {
        setRecentReasoningPreset(resolveConcreteModelId(modelId, defaultModels) ?? modelId, nextReasoningPreset);
      }

      log.info('Mode model updated', { mode: currentMode, modelId });
    } catch (error) {
      log.error('Failed to switch model', error);
      // Only a previously pinned selection can be restored: the store has no
      // way to express "never pinned", and forcing 'auto' there would claim a
      // binding the session does not have either.
      if (sessionId && sessionModelWrittenOptimistically && previousSessionModelName) {
        store.updateSessionModelName(sessionId, previousSessionModelName);
      }
      if (sessionId && sessionModelWrittenOptimistically) {
        store.updateSessionReasoningPreset(sessionId, previousReasoningPreset);
      }
      notificationService.error(t('modelSelector.switchFailed'));
    } finally {
      setLoading(false);
    }
  }, [
    activeSession?.config.workspacePath,
    activeSession?.remoteConnectionId,
    activeSession?.remoteSshHost,
    activeSession?.workspacePath,
    acpClientId,
    currentMode,
    defaultModels,
    disabled,
    externalSelection,
    isAcpSession,
    loading,
    persistSharedModeDefault,
    reasoningLoading,
    recentPresetForModel,
    sessionId,
    t,
    targetIsSubagent,
  ]);

  const handleSelectReasoningPreset = useCallback(async (presetId: string | null) => {
    if (
      disabled
      || loading
      || reasoningLoading
      || !sessionId
      || !concreteModelId
      || currentReasoningProjection?.status !== 'known'
    ) {
      return;
    }
    const normalizedPreset = presetId?.trim() || undefined;
    if (
      normalizedPreset
      && !currentReasoningProjection.presets?.some(preset => preset.id === normalizedPreset)
    ) {
      return;
    }

    const store = FlowChatStore.getInstance();
    const session = store.getState().sessions.get(sessionId);
    if (!session) return;
    const previousPreset = session.config.reasoningPreset;
    if (previousPreset === normalizedPreset) return;

    setReasoningLoading(true);
    store.updateSessionReasoningPreset(sessionId, normalizedPreset);
    try {
      if (shouldSyncSessionModelSelection(session)) {
        await agentAPI.updateSessionModel({
          sessionId,
          modelName: currentNativeModelId,
          reasoningPreset: normalizedPreset ?? null,
          workspacePath: sessionProjectWorkspacePath(session),
          remoteConnectionId: session.remoteConnectionId,
          remoteSshHost: session.remoteSshHost,
          includeInternal: shouldIncludeInternalModelSession(session),
        });
      }
      if (!targetIsSubagent) {
        setRecentReasoningPreset(concreteModelId, normalizedPreset);
      }
      log.info('Session reasoning preset updated', {
        sessionId,
        modelId: concreteModelId,
        presetId: normalizedPreset ?? 'auto',
      });
    } catch (error) {
      store.updateSessionReasoningPreset(sessionId, previousPreset);
      log.error('Failed to update session reasoning preset', error);
      notificationService.error(t('reasoningSelector.updateFailed'));
    } finally {
      setReasoningLoading(false);
    }
  }, [
    concreteModelId,
    currentNativeModelId,
    currentReasoningProjection,
    disabled,
    loading,
    reasoningLoading,
    sessionId,
    t,
    targetIsSubagent,
  ]);

  const handleSetAcpFastMode = useCallback(async (enabled: boolean) => {
    if (disabled || loading || !acpFastMode || !acpClientId || !sessionId) return;
    const value = buildAcpFastModeValue(acpFastMode.option, enabled);
    if (!value) return;

    setLoading(true);
    try {
      const options = await ACPClientAPI.setSessionConfigOption({
        sessionId,
        clientId: acpClientId,
        workspacePath: activeSession?.workspacePath || activeSession?.config.workspacePath,
        remoteConnectionId: activeSession?.remoteConnectionId,
        remoteSshHost: activeSession?.remoteSshHost,
        configId: acpFastMode.option.id,
        value,
      });
      setAcpOptions(options);
      syncAcpContextUsageToStore(sessionId, options);
      log.info('ACP Fast mode updated', { sessionId, acpClientId, enabled });
    } catch (error) {
      log.error('Failed to update ACP Fast mode', error);
    } finally {
      setLoading(false);
    }
  }, [
    activeSession?.config.workspacePath,
    activeSession?.remoteConnectionId,
    activeSession?.remoteSshHost,
    activeSession?.workspacePath,
    acpClientId,
    acpFastMode,
    disabled,
    loading,
    sessionId,
  ]);

  const handleSelectAcpReasoning = useCallback(async (presetId: string | null) => {
    if (disabled || loading || !presetId || !acpReasoning || !acpClientId || !sessionId) return;
    setReasoningLoading(true);
    try {
      const options = await ACPClientAPI.setSessionConfigOption({
        sessionId,
        clientId: acpClientId,
        workspacePath: activeSession?.workspacePath || activeSession?.config.workspacePath,
        remoteConnectionId: activeSession?.remoteConnectionId,
        remoteSshHost: activeSession?.remoteSshHost,
        configId: acpReasoning.option.id,
        value: { type: 'select', value: presetId },
      });
      setAcpOptions(options);
      syncAcpContextUsageToStore(sessionId, options);
      log.info('ACP reasoning level updated', { sessionId, acpClientId, presetId });
    } catch (error) {
      log.error('Failed to update ACP reasoning level', error);
      notificationService.error(t('reasoningSelector.updateFailed'));
    } finally {
      setReasoningLoading(false);
    }
  }, [
    activeSession?.config.workspacePath,
    activeSession?.remoteConnectionId,
    activeSession?.remoteSshHost,
    activeSession?.workspacePath,
    acpClientId,
    acpReasoning,
    disabled,
    loading,
    sessionId,
    t,
  ]);

  const handleSelectAcpMode = useCallback(async (value: string) => {
    if (loading || !acpMode || !acpClientId || !sessionId) return;
    // A locked picker is disabled in the UI; refusing here too keeps a stray
    // keyboard activation from asking the agent for something it will refuse.
    if (acpMode.locked || acpMode.currentValue === value) return;

    setLoading(true);
    try {
      const options = await ACPClientAPI.setSessionConfigOption({
        sessionId,
        clientId: acpClientId,
        workspacePath: activeSession?.workspacePath || activeSession?.config.workspacePath,
        remoteConnectionId: activeSession?.remoteConnectionId,
        remoteSshHost: activeSession?.remoteSshHost,
        configId: acpMode.option.id,
        value: { type: 'select', value },
      });
      setAcpOptions(options);
      syncAcpContextUsageToStore(sessionId, options);
      log.info('ACP session mode updated', { sessionId, acpClientId, value });
    } catch (error) {
      log.error('Failed to update ACP session mode', error);
      notificationService.error(t('modelSelector.acpModeFailed'));
    } finally {
      setLoading(false);
    }
  }, [
    activeSession?.config.workspacePath,
    activeSession?.remoteConnectionId,
    activeSession?.remoteSshHost,
    activeSession?.workspacePath,
    acpClientId,
    acpMode,
    loading,
    sessionId,
    t,
  ]);

  const handleTriggerKeyDown = useCallback((event: React.KeyboardEvent<HTMLButtonElement>) => {
    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      event.preventDefault();
      setKeyboardNavigationOpen(true);
      setDropdownOpen(true);
      if (isAcpSession) {
        void loadAcpOptions();
      }
      return;
    }

    if (event.key === 'Escape' && dropdownOpen) {
      event.preventDefault();
      setDropdownOpen(false);
    }
  }, [dropdownOpen, isAcpSession, loadAcpOptions]);

  const handleDropdownKeyDown = useCallback((event: React.KeyboardEvent<HTMLDivElement>) => {
    if (event.key === 'Escape') {
      event.preventDefault();
      triggerRef.current?.focus();
      setDropdownOpen(false);
      return;
    }

    if (!['ArrowDown', 'ArrowUp', 'Home', 'End'].includes(event.key)) {
      return;
    }

    const items = Array.from(
      event.currentTarget.querySelectorAll<HTMLButtonElement>(
        'button[role="menuitemradio"]:not(:disabled)',
      ),
    );
    if (items.length === 0) return;

    event.preventDefault();
    const activeIndex = items.indexOf(document.activeElement as HTMLButtonElement);
    let nextIndex = activeIndex;
    if (event.key === 'Home') nextIndex = 0;
    if (event.key === 'End') nextIndex = items.length - 1;
    if (event.key === 'ArrowDown') nextIndex = activeIndex < 0 ? 0 : (activeIndex + 1) % items.length;
    if (event.key === 'ArrowUp') nextIndex = activeIndex < 0 ? items.length - 1 : (activeIndex - 1 + items.length) % items.length;
    items[nextIndex]?.focus();
  }, []);

  useEffect(() => {
    if (!dropdownOpen || !keyboardNavigationOpen) return;

    const frameId = window.requestAnimationFrame(() => {
      const menu = portalDropdownRef.current;
      const selectedItem = menu?.querySelector<HTMLButtonElement>(
        'button[role="menuitemradio"][aria-checked="true"]',
      );
      const firstItem = menu?.querySelector<HTMLButtonElement>(
        'button[role="menuitemradio"]:not(:disabled)',
      );
      (selectedItem ?? firstItem)?.focus();
    });

    return () => window.cancelAnimationFrame(frameId);
  }, [dropdownOpen, keyboardNavigationOpen]);

  useEffect(() => {
    if (!dropdownOpen && keyboardNavigationOpen) {
      triggerRef.current?.focus();
    }
  }, [dropdownOpen, keyboardNavigationOpen]);
  
  const tokenPercentage = useMemo(() => {
    if (!maxTokens || maxTokens <= 0 || !currentTokens) return 0;
    return Math.min(Math.round((currentTokens / maxTokens) * 100), 100);
  }, [currentTokens, maxTokens]);

  const tokenStatusClass = useMemo(() => {
    if (tokenPercentage >= 90) return 'critical';
    if (tokenPercentage >= 70) return 'warning';
    return '';
  }, [tokenPercentage]);

  const resolvedContextUsageSource: ContextUsageSource =
    contextUsageSource ?? (isAcpSession ? 'acp_context' : 'agent_prompt');

  if (externalSelection) {
    if (externalAvailableModels.length === 0) {
      return null;
    }

    return (
      <div
        ref={dropdownRef}
        className={`bitfun-model-selector ${className}`}
      >
        <Tooltip disabled={dropdownOpen} content={getModelTooltipText(
          externalCurrentModel,
          externalSelection.providerLabel,
        )}>
          <button
            ref={triggerRef}
            data-testid="chat-model-selector-btn"
            className={`bitfun-model-selector__trigger ${dropdownOpen ? 'bitfun-model-selector__trigger--open' : ''}`}
            type="button"
            aria-haspopup="menu"
            aria-expanded={dropdownOpen}
            aria-controls={dropdownOpen ? menuId : undefined}
            onKeyDown={handleTriggerKeyDown}
            onClick={(event) => {
              const nextOpen = !dropdownOpen;
              if (nextOpen) {
                setKeyboardNavigationOpen(event.detail === 0);
              } else if (event.detail !== 0) {
                setKeyboardNavigationOpen(false);
              }
              setDropdownOpen(nextOpen);
            }}
            disabled={disabled || loading || externalSelection.disabled}
          >
            <span className="bitfun-model-selector__name">
              {getModelDisplayLabel(externalCurrentModel, externalCurrentModelId)}
            </span>
            <ChevronDown size={10} className="bitfun-model-selector__chevron" />
          </button>
        </Tooltip>

        {externalSelection.onSelectReasoningPreset && (
          <ReasoningPresetSelector
            projection={externalReasoningProjection}
            selectedPreset={externalSelection.selectedReasoningPreset === 'auto'
              ? undefined
              : externalSelection.selectedReasoningPreset}
            disabled={disabled || externalSelection.disabled}
            loading={false}
            dropdownPlacement={dropdownPlacement}
            onSelect={externalSelection.onSelectReasoningPreset}
          />
        )}

        <PresenceBoundary active={dropdownOpen}>
          {createPortal(
            <div
            id={menuId}
            className="bitfun-model-selector__dropdown"
            ref={portalDropdownRef}
            style={dropdownStyle}
            data-testid="chat-model-selector-menu"
            data-keyboard-open={keyboardNavigationOpen ? 'true' : 'false'}
            data-placement={resolvedDropdownPlacement}
            data-open={dropdownOpen ? 'true' : 'false'}
            aria-hidden={!dropdownOpen}
            {...(!dropdownOpen ? { inert: '' } : {})}
            role="menu"
            aria-label={t('modelSelector.modelSelection')}
            onKeyDown={handleDropdownKeyDown}
          >
            <div className="bitfun-model-selector__dropdown-header">
              <span>{t('modelSelector.modelSelection')}</span>
              <span className="bitfun-model-selector__dropdown-hint">
                {externalSelection.providerLabel}
              </span>
            </div>
            <div className="bitfun-model-selector__list">
              {externalAvailableModels.map(model => {
                const isSelected = externalCurrentModelId === model.id;
                return (
                  <Tooltip key={model.id} content={model.providerName} placement="right">
                    <button
                      type="button"
                      role="menuitemradio"
                      aria-checked={isSelected}
                      data-testid="chat-model-selector-option"
                      data-model-id={model.id}
                      data-model-name={model.modelName}
                      data-selected={isSelected ? 'true' : 'false'}
                      className={`bitfun-model-selector__option ${isSelected ? 'bitfun-model-selector__option--selected' : ''}`}
                      onClick={() => handleSelectModel(model.id)}
                    >
                      <div className="bitfun-model-selector__option-main">
                        <span className="bitfun-model-selector__option-name">
                          {model.modelName}
                        </span>
                      </div>
                      {isSelected ? (
                        <Check size={14} className="bitfun-model-selector__option-check" />
                      ) : null}
                    </button>
                  </Tooltip>
                );
              })}
            </div>
            </div>,
            document.body,
          )}
        </PresenceBoundary>
      </div>
    );
  }

  if (isAcpSession) {
    // An agent may offer models, a mode, or both. dsh-acp offers only a mode,
    // so returning early on an empty model list would hide its picker entirely.
    if (acpAvailableModels.length === 0 && !acpMode) {
      return null;
    }

    const currentAcpModelId = acpOptions?.currentModelId || acpAvailableModels[0]?.id || '';
    const acpModeLabel = acpMode
      ? (acpMode.option.options.find(candidate => candidate.value === acpMode.currentValue)?.name
        ?? acpMode.currentValue)
      : '';
    const acpModeOnly = acpAvailableModels.length === 0 && acpMode !== null;
    const acpBaseTooltip = acpModeOnly
      ? (acpMode?.option.description ?? `${acpMode?.option.name ?? ''}: ${acpModeLabel}`)
      : getModelTooltipText(acpCurrentModel, acpClientId ? `${acpClientId} ACP` : 'ACP');
    const acpTooltip = buildContextUsageTooltip({
      baseTooltip: acpBaseTooltip,
      usage: {
        current: currentTokens,
        max: maxTokens,
        source: resolvedContextUsageSource,
      },
      t,
    });

    return (
      <div data-bf-component="model-selector" data-bf-part="root" data-bf-state={dropdownOpen ? 'open' : undefined}
        ref={dropdownRef}
        className={`bitfun-model-selector ${className}`}
      >
        <Tooltip content={acpTooltip} disabled={dropdownOpen}>
          <button
            ref={triggerRef}
            data-testid="chat-model-selector-btn"
            className={`bitfun-model-selector__trigger ${dropdownOpen ? 'bitfun-model-selector__trigger--open' : ''}`}
            type="button"
            aria-haspopup="menu"
            aria-expanded={dropdownOpen}
            aria-controls={dropdownOpen ? menuId : undefined}
            onKeyDown={handleTriggerKeyDown}
            onClick={(event) => {
              const nextOpen = !dropdownOpen;
              if (nextOpen) {
                setKeyboardNavigationOpen(event.detail === 0);
              } else if (event.detail !== 0) {
                setKeyboardNavigationOpen(false);
              }
              setDropdownOpen(nextOpen);
              if (nextOpen) {
                void loadAcpOptions();
              }
            }}
            disabled={disabled || loading}
           data-bf-component="model-selector" data-bf-part="trigger" data-bf-state={dropdownOpen ? 'open' : undefined}>
            <span className="bitfun-model-selector__name" data-bf-component="model-selector" data-bf-part="name">
              {acpModeOnly ? acpModeLabel : getModelDisplayLabel(acpCurrentModel, currentAcpModelId)}
            </span>
            {acpFastMode?.enabled && (
              <Zap size={9} className="bitfun-model-selector__fast-icon" />
            )}
            {tokenPercentage > 0 && (
              <span className={`bitfun-model-selector__ctx-usage${tokenStatusClass ? ` bitfun-model-selector__ctx-usage--${tokenStatusClass}` : ''}`} data-bf-component="model-selector" data-bf-part="contextUsage">
                · {tokenPercentage}%
              </span>
            )}
            <ChevronDown size={10} className="bitfun-model-selector__chevron" />
          </button>
        </Tooltip>

        {acpReasoning && (
          <ReasoningPresetSelector
            projection={acpReasoning.projection}
            selectedPreset={acpReasoning.selectedPreset}
            disabled={disabled || loading}
            loading={reasoningLoading}
            dropdownPlacement={dropdownPlacement}
            onSelect={handleSelectAcpReasoning}
          />
        )}

        <PresenceBoundary active={dropdownOpen}>
          {createPortal(
            <div
            id={menuId}
            className="bitfun-model-selector__dropdown"
            data-bf-component="model-selector"
            data-bf-part="dropdown"
            ref={portalDropdownRef}
            style={dropdownStyle}
            data-testid="chat-model-selector-menu"
            data-keyboard-open={keyboardNavigationOpen ? 'true' : 'false'}
            data-placement={resolvedDropdownPlacement}
            data-open={dropdownOpen ? 'true' : 'false'}
            aria-hidden={!dropdownOpen}
            {...(!dropdownOpen ? { inert: '' } : {})}
            role="menu"
            aria-label={acpModeOnly ? t('modelSelector.acpMode') : 'ACP model'}
            onKeyDown={handleDropdownKeyDown}
          >
            <div className="bitfun-model-selector__dropdown-header" data-bf-component="model-selector" data-bf-part="dropdownHeader">
              <span>{acpModeOnly ? t('modelSelector.acpMode') : 'ACP model'}</span>
              <span className="bitfun-model-selector__dropdown-hint">
                {acpClientId}
              </span>
            </div>

            {acpAvailableModels.length > 0 && (
            <div className="bitfun-model-selector__list" data-bf-component="model-selector" data-bf-part="list">
              {acpAvailableModels.map(model => {
                const isSelected = currentAcpModelId === model.id;

                return (
                  <Tooltip key={model.id} content={buildModelMetaText(model)} placement="right">
                    <button
                      type="button"
                      role="menuitemradio"
                      aria-checked={isSelected}
                      data-testid="chat-model-selector-option"
                      data-model-id={model.id}
                      data-model-name={model.modelName}
                      data-selected={isSelected ? 'true' : 'false'}
                      className={`bitfun-model-selector__option ${isSelected ? 'bitfun-model-selector__option--selected' : ''}`}
                      data-bf-component="model-selector"
                      data-bf-part="option"
                      data-bf-state={isSelected ? 'selected' : undefined}
                      onClick={() => handleSelectModel(model.id)}
                    >
                      <div className="bitfun-model-selector__option-main" data-bf-component="model-selector" data-bf-part="optionMain">
                        <span className="bitfun-model-selector__option-name">
                          {model.modelName}
                        </span>
                        <span className="bitfun-model-selector__option-provider">
                          {model.providerName}
                        </span>
                      </div>
                      {isSelected && (
                        <Check size={14} className="bitfun-model-selector__option-check" />
                      )}
                    </button>
                  </Tooltip>
                );
              })}
            </div>
            )}

            {acpMode && (
              <>
                {acpAvailableModels.length > 0 && (
                  <>
                    <div className="bitfun-model-selector__divider" />
                    <div className="bitfun-model-selector__dropdown-header" data-bf-component="model-selector" data-bf-part="dropdownHeader">
                      <span>{t('modelSelector.acpMode')}</span>
                    </div>
                  </>
                )}
                <div className="bitfun-model-selector__list" data-bf-component="model-selector" data-bf-part="list">
                  {acpMode.option.options.map(candidate => {
                    const isSelected = acpMode.currentValue === candidate.value;
                    // The row stays one line; what a mode does — or why it can no
                    // longer change — is hover-only.
                    const hint = acpMode.locked
                      ? (acpMode.option.description ?? t('modelSelector.acpModeLocked'))
                      : (candidate.description ?? candidate.name);

                    return (
                      <Tooltip
                        key={candidate.value}
                        content={hint}
                        placement="right"
                      >
                        <button
                          type="button"
                          role="menuitemradio"
                          aria-checked={isSelected}
                          data-testid="chat-acp-mode-option"
                          data-mode-value={candidate.value}
                          data-selected={isSelected ? 'true' : 'false'}
                          disabled={acpMode.locked || loading}
                          className={`bitfun-model-selector__option ${isSelected ? 'bitfun-model-selector__option--selected' : ''}`}
                          data-bf-component="model-selector"
                          data-bf-part="option"
                          data-bf-state={isSelected ? 'selected' : undefined}
                          onClick={() => { void handleSelectAcpMode(candidate.value); }}
                        >
                          <div className="bitfun-model-selector__option-main" data-bf-component="model-selector" data-bf-part="optionMain">
                            <span className="bitfun-model-selector__option-name">
                              {candidate.name}
                            </span>
                          </div>
                          {isSelected && (
                            <Check size={14} className="bitfun-model-selector__option-check" />
                          )}
                        </button>
                      </Tooltip>
                    );
                  })}
                </div>
              </>
            )}

            {acpFastMode && (
              <>
                <div className="bitfun-model-selector__divider" />
                <div className="bitfun-model-selector__config-row" data-bf-component="model-selector" data-bf-part="configRow">
                  <div className="bitfun-model-selector__config-copy">
                    <span className="bitfun-model-selector__config-name">
                      {t('modelSelector.fastMode')}
                    </span>
                    <span className="bitfun-model-selector__config-description">
                      {t('modelSelector.fastModeDescription')}
                    </span>
                  </div>
                  <Switch
                    size="small"
                    checked={acpFastMode.enabled}
                    loading={loading}
                    aria-label={t('modelSelector.fastMode')}
                    onChange={event => { void handleSetAcpFastMode(event.target.checked); }}
                  />
                </div>
              </>
            )}
            </div>,
            getAppearanceOverlayHost()
          )}
        </PresenceBoundary>
      </div>
    );
  }

  if (availableModels.length === 0) {
    return null;
  }

  const currentModelId = currentNativeModelId;

  const fallbackTooltip = t('modelSelector.autoModelDesc');
  const baseTooltip = getModelTooltipText(currentModel, fallbackTooltip);
  const tooltipContent = buildContextUsageTooltip({
    baseTooltip,
    usage: {
      current: currentTokens,
      max: maxTokens,
      source: resolvedContextUsageSource,
    },
    t,
  });

  return (
    <div data-bf-component="model-selector" data-bf-part="root" data-bf-state={dropdownOpen ? 'open' : undefined}
      ref={dropdownRef}
      className={`bitfun-model-selector ${className}`}
    >
      <Tooltip content={tooltipContent} disabled={dropdownOpen}>
        <button
          ref={triggerRef}
          data-testid="chat-model-selector-btn"
          className={`bitfun-model-selector__trigger ${dropdownOpen ? 'bitfun-model-selector__trigger--open' : ''}`}
          type="button"
          aria-haspopup="menu"
          aria-expanded={dropdownOpen}
          aria-controls={dropdownOpen ? menuId : undefined}
          onKeyDown={handleTriggerKeyDown}
          onClick={(event) => {
            const nextOpen = !dropdownOpen;
            if (nextOpen) {
              setKeyboardNavigationOpen(event.detail === 0);
            } else if (event.detail !== 0) {
              setKeyboardNavigationOpen(false);
            }
            setDropdownOpen(nextOpen);
          }}
          disabled={disabled || loading || reasoningLoading}
         data-bf-component="model-selector" data-bf-part="trigger" data-bf-state={dropdownOpen ? 'open' : undefined}>
          <span className="bitfun-model-selector__name" data-bf-component="model-selector" data-bf-part="name">
            {getModelDisplayLabel(currentModel, t('modelSelector.autoModel'))}
          </span>
          <ChevronDown size={10} className="bitfun-model-selector__chevron" />
        </button>
      </Tooltip>

      {sessionId && (
        <ReasoningPresetSelector
          projection={currentReasoningProjection}
          selectedPreset={selectedReasoningPreset}
          disabled={disabled || loading}
          loading={reasoningLoading}
          dropdownPlacement={dropdownPlacement}
          onSelect={handleSelectReasoningPreset}
        />
      )}

      {tokenPercentage > 0 && (
        <Tooltip content={tooltipContent}>
          <span className={`bitfun-model-selector__ctx-usage${tokenStatusClass ? ` bitfun-model-selector__ctx-usage--${tokenStatusClass}` : ''}`} data-bf-component="model-selector" data-bf-part="contextUsage">
            · {tokenPercentage}%
          </span>
        </Tooltip>
      )}

      <PresenceBoundary active={dropdownOpen}>
        {createPortal(
          <div
          id={menuId}
          className="bitfun-model-selector__dropdown"
          data-bf-component="model-selector"
          data-bf-part="dropdown"
          ref={portalDropdownRef}
          style={dropdownStyle}
          data-testid="chat-model-selector-menu"
          data-keyboard-open={keyboardNavigationOpen ? 'true' : 'false'}
          data-placement={resolvedDropdownPlacement}
          data-open={dropdownOpen ? 'true' : 'false'}
          aria-hidden={!dropdownOpen}
          {...(!dropdownOpen ? { inert: '' } : {})}
          role="menu"
          aria-label={t('modelSelector.modelSelection')}
          onKeyDown={handleDropdownKeyDown}
        >
          <div className="bitfun-model-selector__dropdown-header" data-bf-component="model-selector" data-bf-part="dropdownHeader">
            <span>{t('modelSelector.modelSelection')}</span>
          </div>

          <Tooltip content={t('modelSelector.autoModelDesc')} placement="right">
            <button
              type="button"
              role="menuitemradio"
              aria-checked={currentModelId === 'auto'}
              data-testid="chat-model-selector-option"
              data-model-id="auto"
              data-model-name="auto"
              data-selected={currentModelId === 'auto' ? 'true' : 'false'}
              className={`bitfun-model-selector__option bitfun-model-selector__option--special ${currentModelId === 'auto' ? 'bitfun-model-selector__option--selected' : ''}`}
              data-bf-component="model-selector"
              data-bf-part="option"
              data-bf-state={currentModelId === 'auto' ? 'selected' : undefined}
              onClick={() => handleSelectModel('auto')}
            >
              <div className="bitfun-model-selector__option-main" data-bf-component="model-selector" data-bf-part="optionMain">
                <span className="bitfun-model-selector__option-name">{t('modelSelector.autoModel')}</span>
              </div>
              {currentModelId === 'auto' && (
                <Check size={14} className="bitfun-model-selector__option-check" />
              )}
            </button>
          </Tooltip>

          {(() => {
            const primaryModel = allModels.find(m => m.id === defaultModels.primary);
            const primaryTooltip = primaryModel
              ? buildResolvedModelTooltipText(primaryModel.model_name, {
                providerName: getProviderDisplayName(primaryModel),
                contextWindow: primaryModel.context_window
              }, t('modelSelector.autoModelDesc'))
              : t('modelSelector.autoModelDesc');
            return (
              <Tooltip content={primaryTooltip} placement="right">
                <button
                  type="button"
                  role="menuitemradio"
                  aria-checked={currentModelId === 'primary'}
                  data-testid="chat-model-selector-option"
                  data-model-id="primary"
                  data-model-name={primaryModel?.model_name || 'primary'}
                  data-selected={currentModelId === 'primary' ? 'true' : 'false'}
                  className={`bitfun-model-selector__option bitfun-model-selector__option--special ${currentModelId === 'primary' ? 'bitfun-model-selector__option--selected' : ''}`}
                  data-bf-component="model-selector"
                  data-bf-part="option"
                  data-bf-state={currentModelId === 'primary' ? 'selected' : undefined}
                  onClick={() => handleSelectModel('primary')}
                >
                  <div className="bitfun-model-selector__option-main" data-bf-component="model-selector" data-bf-part="optionMain">
                    <span className="bitfun-model-selector__option-name">{t('modelSelector.primaryModel')}</span>
                  </div>
                  {currentModelId === 'primary' && (
                    <Check size={14} className="bitfun-model-selector__option-check" />
                  )}
                </button>
              </Tooltip>
            );
          })()}

          {(() => {
            const fastModel = allModels.find(m => m.id === defaultModels.fast);
            const fastTooltip = fastModel
              ? buildResolvedModelTooltipText(fastModel.model_name, {
                providerName: getProviderDisplayName(fastModel),
                contextWindow: fastModel.context_window
              }, t('modelSelector.autoModelDesc'))
              : t('modelSelector.autoModelDesc');
            return (
              <Tooltip content={fastTooltip} placement="right">
                <button
                  type="button"
                  role="menuitemradio"
                  aria-checked={currentModelId === 'fast'}
                  data-testid="chat-model-selector-option"
                  data-model-id="fast"
                  data-model-name={fastModel?.model_name || 'fast'}
                  data-selected={currentModelId === 'fast' ? 'true' : 'false'}
                  className={`bitfun-model-selector__option bitfun-model-selector__option--special ${currentModelId === 'fast' ? 'bitfun-model-selector__option--selected' : ''}`}
                  data-bf-component="model-selector"
                  data-bf-part="option"
                  data-bf-state={currentModelId === 'fast' ? 'selected' : undefined}
                  onClick={() => handleSelectModel('fast')}
                >
                  <div className="bitfun-model-selector__option-main" data-bf-component="model-selector" data-bf-part="optionMain">
                    <span className="bitfun-model-selector__option-name">{t('modelSelector.fastModel')}</span>
                  </div>
                  {currentModelId === 'fast' && (
                    <Check size={14} className="bitfun-model-selector__option-check" />
                  )}
                </button>
              </Tooltip>
            );
          })()}

          <div className="bitfun-model-selector__divider" />

          <div className="bitfun-model-selector__list" data-bf-component="model-selector" data-bf-part="list">
            {availableModels.map(model => {
              const isSelected = currentModelId === model.id;

              return (
                <Tooltip key={model.id} content={buildModelMetaText(model)} placement="right">
                  <button
                    type="button"
                    role="menuitemradio"
                    aria-checked={isSelected}
                    data-testid="chat-model-selector-option"
                    data-model-id={model.id}
                    data-model-name={model.modelName}
                    data-selected={isSelected ? 'true' : 'false'}
                    className={`bitfun-model-selector__option ${isSelected ? 'bitfun-model-selector__option--selected' : ''}`}
                    data-bf-component="model-selector"
                    data-bf-part="option"
                    data-bf-state={isSelected ? 'selected' : undefined}
                    onClick={() => handleSelectModel(model.id)}
                  >
                    <div className="bitfun-model-selector__option-main" data-bf-component="model-selector" data-bf-part="optionMain">
                      <span className="bitfun-model-selector__option-name">
                        {model.modelName}
                      </span>
                    </div>
                    {isSelected && (
                      <Check size={14} className="bitfun-model-selector__option-check" />
                    )}
                  </button>
                </Tooltip>
              );
            })}
          </div>
          </div>,
          getAppearanceOverlayHost()
        )}
      </PresenceBoundary>
    </div>
  );
};
export default ModelSelector;
