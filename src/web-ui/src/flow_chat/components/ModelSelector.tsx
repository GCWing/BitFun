/**
 * Model selector component.
 * Shows the active model and allows quick switching.
 *
 * Config linkage:
 * - Model selection is shared across all future mode sessions through
 *   ai.agent_model_defaults.mode. Delegated subagents keep separate defaults.
 * - Supports 'primary' | 'fast' | specific model IDs
 */

import { Menu, MenuItem, MenuSection, MenuSeparator, OverflowText } from '@bitfun/ui';
import React, { useState, useEffect, useId, useRef, useCallback, useLayoutEffect, useMemo, useSyncExternalStore } from 'react';
import { createPortal } from 'react-dom';
import { getAppearanceOverlayHost } from '@/infrastructure/appearance/runtime/AppearanceOverlayHost';
import { ChevronDown, ChevronLeft, ChevronRight, Check, RotateCcw, Zap } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { configManager } from '@/infrastructure/config/services/ConfigManager';
import { agentAPI } from '@/infrastructure/api/service-api/AgentAPI';
import {
  aiApi,
  type AIModelCatalog,
  type ReasoningCatalogProjection,
} from '@/infrastructure/api/service-api/AIApi';
import { ACPClientAPI, type AcpSessionOptions } from '@/infrastructure/api/service-api/ACPClientAPI';
import { getProviderDisplayName, getProviderGroupKey } from '@/infrastructure/config/services/modelConfigs';
import { globalEventBus } from '@/infrastructure/event-bus';
import type { AIModelConfig, AgentModelDefaultsConfig, DefaultModelsConfig } from '@/infrastructure/config/types';
import { Tooltip } from '@bitfun/ui';
import { RetainedMountBoundary } from '@/shared/presence';
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
  buildModelSelectorTooltipDetails,
  type ContextUsageSource,
  type ModelSelectorTooltipDetails,
} from '../utils/tokenUsageDisplay';
import { createLogger } from '@/shared/utils/logger';
import { getModelSelectorDropdownLayout } from './modelSelectorDropdownPosition';
import { AcpModeSelector } from './AcpModeSelector';
import {
  ReasoningIntensityMark,
  ReasoningPresetSelector,
  presetDisplayLabel,
  reasoningIntensityLevel,
} from './ReasoningPresetSelector';
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
  /** Compact trigger treatment supplied by the owning composer. */
  reasoningTriggerPresentation?: 'meter' | 'label';
}

interface ModelInfo {
  id: string;
  /** User-defined configuration name (AIModelConfig.name). */
  configName: string;
  /** Optional label used by symbolic selectors such as Primary and Fast. */
  displayName?: string;
  /** Actual model identifier (AIModelConfig.model_name). */
  modelName: string;
  providerName: string;
  provider: string;
  /** Provider instance this model is configured under, used to group the menu. */
  providerKey?: string;
  contextWindow?: number;
  maxOutputTokens?: number;
}

/** One provider instance and the chat models configured under it. */
interface ProviderGroupInfo {
  key: string;
  providerName: string;
  models: ModelInfo[];
}

type NativeSubmenuKind = 'models' | 'reasoning';
type ModelSelectorLevelDirection = 'none' | 'forward' | 'back';

const NATIVE_SUBMENU_GAP = 5;
const NATIVE_SUBMENU_FALLBACK_WIDTH = 228;
const NATIVE_SUBMENU_FALLBACK_HEIGHT = 320;
const NATIVE_SUBMENU_VIEWPORT_PADDING = 8;

const clampToRange = (value: number, min: number, max: number): number => (
  Math.min(Math.max(value, min), Math.max(min, max))
);

const ModelSelectorTooltipContent: React.FC<{ details: ModelSelectorTooltipDetails }> = ({ details }) => (
  <div className="bitfun-model-selector__tooltip">
    {details.rows.map(row => (
      <div key={row.key} className="bitfun-model-selector__tooltip-row">
        <span className="bitfun-model-selector__tooltip-label">{row.label}</span>
        <span className="bitfun-model-selector__tooltip-value">{row.value}</span>
      </div>
    ))}
    {details.warning ? (
      <div className="bitfun-model-selector__tooltip-warning">{details.warning}</div>
    ) : null}
  </div>
);

const ModelSelectorMenuLevel: React.FC<{
  children: React.ReactNode;
  direction: ModelSelectorLevelDirection;
}> = ({ children, direction }) => (
  <div
    className="bitfun-model-selector__level"
    data-bf-component="model-selector"
    data-bf-part="level"
    data-direction={direction}
  >
    <div
      className="bitfun-model-selector__list"
      data-bf-component="model-selector"
      data-bf-part="list"
    >
      <MenuSection>{children}</MenuSection>
    </div>
  </div>
);

// Helper: identify special model IDs.
const isSpecialModel = (value: string): value is 'primary' | 'fast' => {
  return value === 'primary' || value === 'fast';
};

function resolveConcreteModelId(
  modelId: string,
  defaultModels: DefaultModelsConfig,
): string | undefined {
  if (modelId === 'primary') return defaultModels.primary ?? undefined;
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
  return model.displayName || model.modelName || model.configName || fallback;
};

const getModelTooltipText = (model: ModelInfo | null, fallback: string): string => {
  if (!model) return fallback;
  if (isSpecialModel(model.id)) {
    return buildResolvedModelTooltipText(model.modelName, model, fallback);
  }
  return buildModelMetaText(model);
};

const buildSymbolicModelInfo = (
  modelId: 'primary' | 'fast',
  t: (key: string) => string,
): ModelInfo => ({
  id: modelId,
  configName: t(modelId === 'primary' ? 'modelSelector.primaryModel' : 'modelSelector.fastModel'),
  displayName: t(modelId === 'primary' ? 'modelSelector.primaryModel' : 'modelSelector.fastModel'),
  modelName: t(modelId === 'primary' ? 'modelSelector.primaryModel' : 'modelSelector.fastModel'),
  providerName: t(modelId === 'primary' ? 'modelSelector.primaryModelDesc' : 'modelSelector.fastModelDesc'),
  provider: 'symbolic',
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
  reasoningTriggerPresentation = 'meter',
}) => {
  const { t } = useTranslation('flow-chat');
  const [allModels, setAllModels] = useState<AIModelConfig[]>([]);
  const [modelCatalog, setModelCatalog] = useState<AIModelCatalog | null>(null);
  const [defaultModels, setDefaultModels] = useState<DefaultModelsConfig>({});
  const [modeModel, setModeModel] = useState('primary');
  const [acpOptions, setAcpOptions] = useState<AcpSessionOptions | null>(null);
  const [dropdownOpen, setDropdownOpen] = useState(false);
  const [keyboardNavigationOpen, setKeyboardNavigationOpen] = useState(false);
  /** Provider whose models the menu is currently showing; null is the provider level. */
  const [activeProviderKey, setActiveProviderKey] = useState<string | null>(null);
  /** Click-open detail menu beside the stable native settings summary. */
  const [nativeSubmenu, setNativeSubmenu] = useState<NativeSubmenuKind | null>(null);
  /** Which way the provider level stepped inside the model submenu. */
  const [levelDirection, setLevelDirection] = useState<ModelSelectorLevelDirection>('none');
  const [loading, setLoading] = useState(false);
  const [reasoningLoading, setReasoningLoading] = useState(false);
  const acpRestoreToastShownRef = useRef<string | null>(null);
  const acpOptionsRef = useRef<AcpSessionOptions | null>(null);

  const dropdownRef = useRef<HTMLDivElement>(null);
  const portalDropdownRef = useRef<HTMLDivElement>(null);
  const nativeSubmenuRef = useRef<HTMLDivElement>(null);
  const nativeModelMenuItemRef = useRef<HTMLButtonElement>(null);
  const nativeReasoningMenuItemRef = useRef<HTMLButtonElement>(null);
  const focusNativeSubmenuOnOpenRef = useRef(false);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const menuId = useId();
  const nativeSubmenuId = useId();

  useEffect(() => {
    onLoadingChange?.(loading || reasoningLoading);
  }, [loading, onLoadingChange, reasoningLoading]);

  const [dropdownStyle, setDropdownStyle] = useState<React.CSSProperties>({
    position: 'fixed',
    visibility: 'hidden',
  });
  const [nativeSubmenuStyle, setNativeSubmenuStyle] = useState<React.CSSProperties>({
    position: 'fixed',
    visibility: 'hidden',
  });
  const [nativeSubmenuPlacement, setNativeSubmenuPlacement] = useState<'left' | 'right'>('right');
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
      setModeModel(agentModelDefaults?.mode?.trim() || 'primary');
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
          && portalDropdownRef.current && !portalDropdownRef.current.contains(target)
          && !nativeSubmenuRef.current?.contains(target)) {
        setDropdownOpen(false);
        setKeyboardNavigationOpen(false);
        setNativeSubmenu(null);
        setActiveProviderKey(null);
      }
    };

    if (dropdownOpen) {
      document.addEventListener('mousedown', handleClickOutside);
    }

    return () => {
      document.removeEventListener('mousedown', handleClickOutside);
    };
  }, [dropdownOpen]);

  // Calculate the portalled dropdown position relative to the trigger button.
  useEffect(() => {
    if (!dropdownOpen || !dropdownRef.current) return;

    const updatePosition = () => {
      // Anchor on the trigger button, not the container: when a reasoning
      // preset selector sits beside it the container's right edge is not the
      // button's, and the menu is asked to right-align with the button.
      const anchor = triggerRef.current ?? dropdownRef.current;
      if (!anchor || !portalDropdownRef.current) return;
      const anchorRect = anchor.getBoundingClientRect();
      const dropdown = portalDropdownRef.current;
      const dropdownRect = dropdown.getBoundingClientRect();
      // max-height can make the rendered box shorter than its contents. Keep
      // measuring the intrinsic height so a later resize can still choose the
      // correct side and then size the scrollable surface to that side.
      const intrinsicDropdownWidth = Math.max(dropdownRect.width, dropdown.offsetWidth);
      const intrinsicDropdownHeight = Math.max(
        dropdownRect.height,
        dropdown.scrollHeight + Math.max(0, dropdown.offsetHeight - dropdown.clientHeight),
      );
      const layout = getModelSelectorDropdownLayout(
        anchorRect,
        { width: intrinsicDropdownWidth, height: intrinsicDropdownHeight },
        dropdownPlacement,
        { width: window.innerWidth, height: window.innerHeight },
        // The trigger lives near the composer's right side, so a start-aligned
        // wide menu overflows the window; right edges align instead.
        'end',
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

  // Native model and reasoning choices are separate click-open flyouts. Keep
  // them anchored to their summary row even though both menus are portalled.
  useLayoutEffect(() => {
    if (!dropdownOpen || !nativeSubmenu) return;

    const updatePosition = () => {
      const anchor = nativeSubmenu === 'models'
        ? nativeModelMenuItemRef.current
        : nativeReasoningMenuItemRef.current;
      const submenu = nativeSubmenuRef.current;
      if (!anchor || !submenu) return;

      const anchorRect = anchor.getBoundingClientRect();
      const submenuRect = submenu.getBoundingClientRect();
      const submenuWidth = submenuRect.width
        || submenu.offsetWidth
        || submenu.scrollWidth
        || NATIVE_SUBMENU_FALLBACK_WIDTH;
      const submenuHeight = submenuRect.height
        || submenu.offsetHeight
        || submenu.scrollHeight
        || NATIVE_SUBMENU_FALLBACK_HEIGHT;
      const preferredLeft = anchorRect.right + NATIVE_SUBMENU_GAP;
      const opensRight = preferredLeft + submenuWidth
        <= window.innerWidth - NATIVE_SUBMENU_VIEWPORT_PADDING;

      setNativeSubmenuPlacement(opensRight ? 'right' : 'left');
      setNativeSubmenuStyle({
        position: 'fixed',
        left: clampToRange(
          opensRight
            ? preferredLeft
            : anchorRect.left - NATIVE_SUBMENU_GAP - submenuWidth,
          NATIVE_SUBMENU_VIEWPORT_PADDING,
          window.innerWidth - submenuWidth - NATIVE_SUBMENU_VIEWPORT_PADDING,
        ),
        top: clampToRange(
          anchorRect.top - 4,
          NATIVE_SUBMENU_VIEWPORT_PADDING,
          window.innerHeight - submenuHeight - NATIVE_SUBMENU_VIEWPORT_PADDING,
        ),
        maxHeight: Math.max(
          80,
          window.innerHeight - NATIVE_SUBMENU_VIEWPORT_PADDING * 2,
        ),
        visibility: 'visible',
      });
    };

    updatePosition();
    window.addEventListener('scroll', updatePosition, true);
    window.addEventListener('resize', updatePosition);
    const resizeObserver = typeof ResizeObserver === 'undefined'
      ? null
      : new ResizeObserver(updatePosition);
    const anchor = nativeSubmenu === 'models'
      ? nativeModelMenuItemRef.current
      : nativeReasoningMenuItemRef.current;
    if (anchor) resizeObserver?.observe(anchor);
    if (nativeSubmenuRef.current) resizeObserver?.observe(nativeSubmenuRef.current);

    return () => {
      window.removeEventListener('scroll', updatePosition, true);
      window.removeEventListener('resize', updatePosition);
      resizeObserver?.disconnect();
    };
  }, [activeProviderKey, dropdownOpen, nativeSubmenu]);

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
              maxOutputTokens: localModel.max_tokens,
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
        const actualModelId = resolveConcreteModelId(sessionModelName, defaultModels);
        return allModels.some(model => model.enabled !== false && model.id === actualModelId)
          ? sessionModelName
          : 'primary';
      }
      return allModels.some(model => model.enabled !== false && model.id === sessionModelName)
        ? sessionModelName
        : 'primary';
    }

    if (targetIsSubagent) {
      return 'primary';
    }

    // Legacy sessions created without a model selector fall back to the current
    // mode default until they are migrated by the send path.
    const configuredModelId = modeDefaultModelId?.trim() || modeModel;
    if (configuredModelId === 'primary' || configuredModelId === 'fast') {
      const actualModelId = resolveConcreteModelId(configuredModelId, defaultModels);
      const model = allModels.find(m => m.enabled !== false && m.id === actualModelId);
      return model ? configuredModelId : 'primary';
    }
    const model = allModels.find(m => m.enabled !== false && m.id === configuredModelId);
    return model ? configuredModelId : 'primary';
  }, [allModels, modeDefaultModelId, modeModel, defaultModels, activeSession?.config.modelName, targetIsSubagent]);

  const currentModel = useMemo((): ModelInfo | null => {
    const modelId = getCurrentModelId();

    if (modelId === 'primary' || modelId === 'fast') {
      const actualModelId = resolveConcreteModelId(modelId, defaultModels);
      if (!actualModelId) return buildSymbolicModelInfo(modelId, t);

      const model = allModels.find(m => m.id === actualModelId);
      if (!model) return buildSymbolicModelInfo(modelId, t);

      return {
        id: modelId,
        configName: model.name,
        displayName: modelId === 'primary' ? t('modelSelector.primaryModel') : t('modelSelector.fastModel'),
        modelName: model.model_name,
        providerName: getProviderDisplayName(model),
        provider: model.provider,
        contextWindow: model.context_window,
        maxOutputTokens: model.max_tokens,
      };
    }

    const model = allModels.find(m => m.id === modelId);
    if (!model) return buildSymbolicModelInfo('primary', t);

    return {
      id: model.id || '',
      configName: model.name,
      modelName: model.model_name,
      providerName: getProviderDisplayName(model),
      provider: model.provider,
      contextWindow: model.context_window,
      maxOutputTokens: model.max_tokens,
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
        providerKey: getProviderGroupKey(m),
        contextWindow: m.context_window,
        maxOutputTokens: m.max_tokens,
      }));
  }, [allModels]);

  /**
   * Configuration order is preserved so a provider does not move between
   * openings, and so the models inside one provider keep the order the user
   * arranged them in on the settings page.
   */
  const providerGroups = useMemo((): ProviderGroupInfo[] => {
    const groups: ProviderGroupInfo[] = [];
    const groupsByKey = new Map<string, ProviderGroupInfo>();

    for (const model of availableModels) {
      const key = model.providerKey || model.id;
      const existing = groupsByKey.get(key);
      if (existing) {
        existing.models.push(model);
        continue;
      }
      const group: ProviderGroupInfo = {
        key,
        providerName: model.providerName,
        models: [model],
      };
      groupsByKey.set(key, group);
      groups.push(group);
    }

    return groups;
  }, [availableModels]);

  const activeProviderGroup = activeProviderKey
    ? providerGroups.find(group => group.key === activeProviderKey) ?? null
    : null;

  const focusPreferredNativeSubmenuItem = useCallback(() => {
    const submenu = nativeSubmenuRef.current;
    const preferredItem = submenu?.querySelector<HTMLButtonElement>(
      'button[role="menuitemradio"][aria-checked="true"], button[data-provider-key][data-selected="true"]',
    );
    const firstItem = submenu?.querySelector<HTMLButtonElement>(
      'button[role="menuitemradio"]:not(:disabled), button[role="menuitem"]:not(:disabled)',
    );
    (preferredItem ?? firstItem)?.focus();
  }, []);

  const openNativeSubmenu = useCallback((kind: NativeSubmenuKind, moveFocus: boolean) => {
    focusNativeSubmenuOnOpenRef.current = moveFocus;
    setActiveProviderKey(null);
    setLevelDirection('none');
    if (nativeSubmenu === kind && moveFocus && !activeProviderKey) {
      focusNativeSubmenuOnOpenRef.current = false;
      focusPreferredNativeSubmenuItem();
    }
    setNativeSubmenu(kind);
  }, [activeProviderKey, focusPreferredNativeSubmenuItem, nativeSubmenu]);

  const closeNativeSubmenu = useCallback((restoreFocus: boolean) => {
    const anchor = nativeSubmenu === 'models'
      ? nativeModelMenuItemRef.current
      : nativeReasoningMenuItemRef.current;
    focusNativeSubmenuOnOpenRef.current = false;
    setActiveProviderKey(null);
    setNativeSubmenu(null);
    setLevelDirection('none');
    if (restoreFocus) anchor?.focus();
  }, [nativeSubmenu]);

  const toggleNativeSubmenu = useCallback((kind: NativeSubmenuKind) => {
    if (nativeSubmenu === kind) {
      closeNativeSubmenu(false);
      return;
    }
    openNativeSubmenu(kind, false);
  }, [closeNativeSubmenu, nativeSubmenu, openNativeSubmenu]);

  const openProviderLevel = useCallback((providerKey: string) => {
    setActiveProviderKey(providerKey);
    setLevelDirection('forward');
  }, []);

  const closeProviderLevel = useCallback(() => {
    setActiveProviderKey(null);
    setLevelDirection('back');
  }, []);

  // Reopening starts with only the stable summary. A provider removed while
  // its submenu is open must not leave the model flyout on a missing level.
  useEffect(() => {
    if (!dropdownOpen) {
      setActiveProviderKey(null);
      setNativeSubmenu(null);
      setLevelDirection('none');
      return;
    }
    if (nativeSubmenu !== 'models' && activeProviderKey) {
      setActiveProviderKey(null);
      return;
    }
    if (activeProviderKey && !activeProviderGroup) {
      closeProviderLevel();
    }
  }, [activeProviderGroup, activeProviderKey, closeProviderLevel, dropdownOpen, nativeSubmenu]);

  const currentNativeModelId = getCurrentModelId();
  const concreteModelId = resolveConcreteModelId(currentNativeModelId, defaultModels);
  /** Provider that owns the pinned model, so the provider level can mark it. */
  const selectedProviderKey = useMemo((): string | null => {
    if (isSpecialModel(currentNativeModelId)) return null;
    return providerGroups.find(
      group => group.models.some(model => model.id === currentNativeModelId),
    )?.key ?? null;
  }, [currentNativeModelId, providerGroups]);
  const currentReasoningProjection = useMemo((): ReasoningCatalogProjection | null => {
    if (!concreteModelId) return null;
    return modelCatalog?.models.find(model => model.id === concreteModelId)?.reasoning ?? null;
  }, [concreteModelId, modelCatalog]);
  const selectedReasoningPreset = currentReasoningProjection?.status === 'known'
    && currentReasoningProjection.presets?.some(preset => preset.id === sessionReasoningPreset)
    ? sessionReasoningPreset
    : undefined;
  const orderedReasoningPresets = useMemo(
    () => currentReasoningProjection?.status === 'known'
      ? [...(currentReasoningProjection.presets ?? [])].sort((left, right) => left.order - right.order)
      : [],
    [currentReasoningProjection],
  );
  const selectedReasoningDescriptor = orderedReasoningPresets.find(
    preset => preset.id === selectedReasoningPreset,
  );
  const defaultReasoningDescriptor = orderedReasoningPresets.find(
    preset => preset.id === currentReasoningProjection?.default_preset,
  );
  const effectiveReasoningDescriptor = selectedReasoningDescriptor ?? defaultReasoningDescriptor;
  const currentReasoningLabel = effectiveReasoningDescriptor
    ? presetDisplayLabel(effectiveReasoningDescriptor, orderedReasoningPresets, t)
    : t('reasoningSelector.auto');
  const reasoningPresetLabels = orderedReasoningPresets.map(preset => (
    presetDisplayLabel(preset, orderedReasoningPresets, t)
  ));
  const hasNativeReasoningSettings = Boolean(sessionId && orderedReasoningPresets.length > 0);
  const nativeSettingsAreDefault = currentNativeModelId === 'primary' && !selectedReasoningPreset;

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
  
  const handleSelectModel = useCallback(async (
    modelId: string,
    options?: { resetReasoning?: boolean },
  ) => {
    if (disabled || loading || reasoningLoading) return;

    if (
      portalDropdownRef.current?.contains(document.activeElement)
      || nativeSubmenuRef.current?.contains(document.activeElement)
    ) {
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
    const nextReasoningPreset = options?.resetReasoning
      ? undefined
      : recentPresetForModel(modelId);
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
      // way to express "never pinned" without claiming a session binding.
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

  const handleResetNativeSettings = useCallback(() => {
    if (nativeSettingsAreDefault || disabled || loading || reasoningLoading) return;
    void handleSelectModel('primary', { resetReasoning: true });
  }, [
    disabled,
    handleSelectModel,
    loading,
    nativeSettingsAreDefault,
    reasoningLoading,
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

  const handleSelectReasoningPresetFromMenu = useCallback((presetId: string | null) => {
    if (
      portalDropdownRef.current?.contains(document.activeElement)
      || nativeSubmenuRef.current?.contains(document.activeElement)
    ) {
      triggerRef.current?.focus();
    }
    setDropdownOpen(false);
    void handleSelectReasoningPreset(presetId);
  }, [handleSelectReasoningPreset]);

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

  const handleNativeSubmenuTriggerKeyDown = useCallback((
    kind: NativeSubmenuKind,
    event: React.KeyboardEvent<HTMLButtonElement>,
  ) => {
    if (event.key !== 'ArrowRight') return;
    event.preventDefault();
    event.stopPropagation();
    openNativeSubmenu(kind, true);
  }, [openNativeSubmenu]);

  const handleDropdownKeyDown = useCallback((event: React.KeyboardEvent<HTMLDivElement>) => {
    if (event.defaultPrevented) return;

    if (event.key === 'Escape') {
      event.preventDefault();
      if (!externalSelection && !isAcpSession && nativeSubmenu) {
        closeNativeSubmenu(true);
        return;
      }
      triggerRef.current?.focus();
      setDropdownOpen(false);
      return;
    }

    if (event.key === 'ArrowRight') {
      const focusedElement = document.activeElement as HTMLElement | null;
      const focusedTarget = focusedElement?.dataset?.modelMenuTarget;
      if (!externalSelection && !isAcpSession) {
        if (focusedTarget === 'models') {
          event.preventDefault();
          openNativeSubmenu('models', true);
          return;
        }
        if (focusedTarget === 'reasoning') {
          event.preventDefault();
          openNativeSubmenu('reasoning', true);
          return;
        }
      }
    }

  }, [
    closeNativeSubmenu,
    externalSelection,
    isAcpSession,
    nativeSubmenu,
    openNativeSubmenu,
  ]);

  const handleNativeSubmenuKeyDown = useCallback((event: React.KeyboardEvent<HTMLDivElement>) => {
    if (event.defaultPrevented) return;

    if (event.key === 'Escape' || event.key === 'ArrowLeft') {
      event.preventDefault();
      event.stopPropagation();
      if (activeProviderKey) {
        closeProviderLevel();
      } else {
        closeNativeSubmenu(true);
      }
      return;
    }

    if (event.key === 'ArrowRight' && nativeSubmenu === 'models' && !activeProviderKey) {
      const focusedElement = document.activeElement as HTMLElement | null;
      const focusedProviderKey = focusedElement?.dataset?.providerKey;
      if (focusedProviderKey) {
        event.preventDefault();
        event.stopPropagation();
        openProviderLevel(focusedProviderKey);
      }
    }
  }, [
    activeProviderKey,
    closeNativeSubmenu,
    closeProviderLevel,
    nativeSubmenu,
    openProviderLevel,
  ]);

  useEffect(() => {
    if (!dropdownOpen || !keyboardNavigationOpen) return;

    const frameId = window.requestAnimationFrame(() => {
      const menu = portalDropdownRef.current;
      const selectedItem = menu?.querySelector<HTMLButtonElement>(
        'button[role="menuitemradio"][aria-checked="true"], button[role="menuitem"][data-selected="true"]',
      );
      const firstItem = menu?.querySelector<HTMLButtonElement>(
        'button[role="menuitemradio"]:not(:disabled), button[role="menuitem"]:not(:disabled)',
      );
      (selectedItem ?? firstItem)?.focus();
    });

    return () => window.cancelAnimationFrame(frameId);
  }, [dropdownOpen, keyboardNavigationOpen]);

  // Keyboard-opened flyouts receive focus. Provider steps also move focus
  // because the clicked row is replaced inside the model flyout.
  const previousNativeProviderKeyRef = useRef(activeProviderKey);
  useLayoutEffect(() => {
    const previousProviderKey = previousNativeProviderKeyRef.current;
    previousNativeProviderKeyRef.current = activeProviderKey;
    if (!dropdownOpen || !nativeSubmenu) return;
    const providerLevelChanged = nativeSubmenu === 'models'
      && previousProviderKey !== activeProviderKey;
    if (!focusNativeSubmenuOnOpenRef.current && !providerLevelChanged) return;
    focusNativeSubmenuOnOpenRef.current = false;

    const menu = nativeSubmenuRef.current;
    if (!menu) return;

    if (activeProviderKey) {
      const selectedModel = menu.querySelector<HTMLButtonElement>(
        'button[role="menuitemradio"][aria-checked="true"]',
      );
      const firstModel = menu.querySelector<HTMLButtonElement>(
        'button[role="menuitemradio"]:not(:disabled)',
      );
      (selectedModel ?? firstModel)?.focus();
      return;
    }

    if (nativeSubmenu === 'models' && previousProviderKey) {
      const providerRows = Array.from(
        menu.querySelectorAll<HTMLButtonElement>('button[data-provider-key]'),
      );
      const targetRow = providerRows.find(
        row => row.dataset.providerKey === previousProviderKey,
      );
      (targetRow ?? providerRows[0])?.focus();
      return;
    }

    focusPreferredNativeSubmenuItem();
  }, [
    activeProviderKey,
    dropdownOpen,
    focusPreferredNativeSubmenuItem,
    nativeSubmenu,
  ]);

  useEffect(() => {
    if (!dropdownOpen && keyboardNavigationOpen) {
      triggerRef.current?.focus();
    }
  }, [dropdownOpen, keyboardNavigationOpen]);
  
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
            <OverflowText className="bitfun-model-selector__name">
              {getModelDisplayLabel(externalCurrentModel, externalCurrentModelId)}
            </OverflowText>
            <ChevronDown size={10} className="bitfun-model-selector__chevron" />
          </button>
        </Tooltip>

        {externalSelection.onSelectReasoningPreset ? (
          <ReasoningPresetSelector
            projection={externalReasoningProjection}
            selectedPreset={externalSelection.selectedReasoningPreset === 'auto'
              ? undefined
              : externalSelection.selectedReasoningPreset}
            triggerPresentation={reasoningTriggerPresentation}
            disabled={disabled || externalSelection.disabled}
            loading={false}
            dropdownPlacement={dropdownPlacement}
            onSelect={externalSelection.onSelectReasoningPreset}
          />
        ) : null}

        <RetainedMountBoundary present={dropdownOpen}>
          {createPortal(
            <Menu
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
            aria-label={t('modelSelector.modelSelection')}
            onKeyDown={handleDropdownKeyDown}
          >
            <MenuSection title={`${t('modelSelector.modelSelection')} · ${externalSelection.providerLabel}`}>
              {externalAvailableModels.map(model => {
                const isSelected = externalCurrentModelId === model.id;
                return (
                  <Tooltip key={model.id} content={model.providerName} placement="right">
                    <MenuItem
                      role="menuitemradio"
                      checked={isSelected}
                      data-testid="chat-model-selector-option"
                      data-model-id={model.id}
                      data-model-name={model.modelName}
                      data-selected={isSelected ? 'true' : 'false'}
                      metadata={isSelected ? <Check size={14} aria-hidden /> : null}
                      onClick={() => handleSelectModel(model.id)}
                    >
                      {model.modelName}
                    </MenuItem>
                  </Tooltip>
                );
              })}
            </MenuSection>
            </Menu>,
            document.body,
          )}
        </RetainedMountBoundary>
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
    // The mode has a trigger of its own now. What is left here is the model
    // list and the fast-mode switch, so this picker only appears when the agent
    // published one of them — a mode-only agent shows the mode picker alone.
    const showModelTrigger = acpAvailableModels.length > 0 || acpFastMode !== null;
    let acpBaseTooltip: string;
    if (acpAvailableModels.length > 0) {
      acpBaseTooltip = getModelTooltipText(acpCurrentModel, acpClientId ? `${acpClientId} ACP` : 'ACP');
    } else if (showModelTrigger) {
      acpBaseTooltip = t('modelSelector.fastMode');
    } else {
      acpBaseTooltip = acpMode?.option.description ?? `${acpMode?.option.name ?? ''}: ${acpModeLabel}`;
    }
    const acpDropdownTitle = acpAvailableModels.length > 0
      ? 'ACP model'
      : t('modelSelector.fastMode');
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
        {showModelTrigger && (
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
            <OverflowText className="bitfun-model-selector__name" data-bf-component="model-selector" data-bf-part="name">
              {acpAvailableModels.length > 0
                ? getModelDisplayLabel(acpCurrentModel, currentAcpModelId)
                : t('modelSelector.fastMode')}
            </OverflowText>
            {acpFastMode?.enabled && (
              <Zap size={9} className="bitfun-model-selector__fast-icon" />
            )}
            <ChevronDown size={10} className="bitfun-model-selector__chevron" />
          </button>
        </Tooltip>
        )}

        {acpMode && (
          <AcpModeSelector
            mode={acpMode}
            clientId={acpClientId ?? undefined}
            disabled={disabled}
            loading={loading}
            dropdownPlacement={dropdownPlacement}
            onSelect={handleSelectAcpMode}
            {...(showModelTrigger ? {} : { tooltip: acpTooltip })}
          />
        )}

        {acpReasoning ? (
          <ReasoningPresetSelector
            projection={acpReasoning.projection}
            selectedPreset={acpReasoning.selectedPreset}
            triggerPresentation={reasoningTriggerPresentation}
            disabled={disabled || loading}
            loading={reasoningLoading}
            dropdownPlacement={dropdownPlacement}
            onSelect={handleSelectAcpReasoning}
          />
        ) : null}

        {showModelTrigger && (
        <RetainedMountBoundary present={dropdownOpen}>
          {createPortal(
            <Menu
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
            aria-label={acpDropdownTitle}
            onKeyDown={handleDropdownKeyDown}
          >
            {acpAvailableModels.length > 0 && (
            <MenuSection title={`${acpDropdownTitle}${acpClientId ? ` · ${acpClientId}` : ''}`} data-bf-component="model-selector" data-bf-part="list">
              {acpAvailableModels.map(model => {
                const isSelected = currentAcpModelId === model.id;

                return (
                  <Tooltip key={model.id} content={buildModelMetaText(model)} placement="right">
                    <MenuItem
                      role="menuitemradio"
                      checked={isSelected}
                      data-testid="chat-model-selector-option"
                      data-model-id={model.id}
                      data-model-name={model.modelName}
                      data-selected={isSelected ? 'true' : 'false'}
                      className="bitfun-model-selector__option"
                      data-bf-component="model-selector"
                      data-bf-part="option"
                      data-bf-state={isSelected ? 'selected' : undefined}
                      metadata={isSelected ? <Check size={14} aria-hidden /> : null}
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
                    </MenuItem>
                  </Tooltip>
                );
              })}
            </MenuSection>
            )}

            {acpFastMode && (
              <>
                {acpAvailableModels.length > 0 && (
                  <MenuSeparator />
                )}
                <Tooltip content={t('modelSelector.fastModeDescription')} placement="right">
                  <MenuItem
                    role="menuitemcheckbox"
                    checked={acpFastMode.enabled}
                    disabled={loading}
                    aria-busy={loading}
                    leading={<Zap size={13} aria-hidden />}
                    metadata={acpFastMode.enabled ? <Check size={14} aria-hidden /> : null}
                    onClick={() => { void handleSetAcpFastMode(!acpFastMode.enabled); }}
                  >
                    {t('modelSelector.fastMode')}
                  </MenuItem>
                </Tooltip>
              </>
            )}
            </Menu>,
            getAppearanceOverlayHost()
          )}
        </RetainedMountBoundary>
        )}
      </div>
    );
  }

  if (availableModels.length === 0) {
    return null;
  }

  const currentModelId = currentNativeModelId;

  const fallbackTooltip = t('modelSelector.primaryModelDesc');
  const tooltipDetails = buildModelSelectorTooltipDetails({
    configName: currentModel?.configName ?? fallbackTooltip,
    modelName: currentModel?.modelName,
    contextWindow: currentModel?.contextWindow,
    configuredMaxOutputTokens: currentModel?.maxOutputTokens,
    usage: {
      current: currentTokens,
      max: maxTokens,
      source: resolvedContextUsageSource,
    },
    t,
  });
  const tooltipContent = <ModelSelectorTooltipContent details={tooltipDetails} />;

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
          <OverflowText className="bitfun-model-selector__name" data-bf-component="model-selector" data-bf-part="name">
            {getModelDisplayLabel(currentModel, t('modelSelector.primaryModel'))}
          </OverflowText>
          {hasNativeReasoningSettings && (
            <span
              className="bitfun-model-selector__trigger-reasoning"
              data-testid="chat-model-selector-trigger-reasoning"
              data-bf-component="model-selector"
              data-bf-part="reasoningSummary"
            >
              {reasoningTriggerPresentation === 'label' ? (
                currentReasoningLabel
              ) : (
                <ReasoningIntensityMark
                  level={reasoningIntensityLevel(
                    effectiveReasoningDescriptor,
                    orderedReasoningPresets,
                  )}
                  compact
                />
              )}
            </span>
          )}
          <ChevronDown
            size={10}
            className="bitfun-model-selector__chevron"
            data-testid="chat-model-selector-dropdown-indicator"
          />
        </button>
      </Tooltip>

      <RetainedMountBoundary present={dropdownOpen}>
        {createPortal(
          <Menu
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
            data-menu-level="settings"
            aria-hidden={!dropdownOpen}
            {...(!dropdownOpen ? { inert: '' } : {})}
            aria-label={t('modelSelector.modelSettings')}
            onKeyDown={handleDropdownKeyDown}
          >
            <MenuSection
              data-testid="chat-model-selector-settings"
              aria-label={t('modelSelector.modelSettings')}
            >
              <MenuItem
                ref={nativeModelMenuItemRef}
                className={`bitfun-model-selector__settings-item${nativeSubmenu === 'models' ? ' is-open' : ''}`}
                data-testid="chat-model-selector-settings-model"
                data-model-menu-target="models"
                aria-haspopup="menu"
                aria-expanded={nativeSubmenu === 'models'}
                aria-controls={nativeSubmenu === 'models' ? nativeSubmenuId : undefined}
                metadata={(
                  <span className="bitfun-model-selector__settings-value">
                    {getModelDisplayLabel(currentModel, t('modelSelector.primaryModel'))}
                  </span>
                )}
                onClick={() => toggleNativeSubmenu('models')}
                onKeyDown={(event) => handleNativeSubmenuTriggerKeyDown('models', event)}
                shortcut={<ChevronRight size={14} aria-hidden />}
              >
                {t('modelSelector.model')}
              </MenuItem>

              {hasNativeReasoningSettings && (
                <MenuItem
                  ref={nativeReasoningMenuItemRef}
                  className={`bitfun-model-selector__settings-item${nativeSubmenu === 'reasoning' ? ' is-open' : ''}`}
                  data-testid="chat-model-selector-settings-reasoning"
                  data-model-menu-target="reasoning"
                  aria-haspopup="menu"
                  aria-expanded={nativeSubmenu === 'reasoning'}
                  aria-controls={nativeSubmenu === 'reasoning' ? nativeSubmenuId : undefined}
                  metadata={(
                    <span className="bitfun-model-selector__settings-value">
                      {currentReasoningLabel}
                    </span>
                  )}
                  onClick={() => toggleNativeSubmenu('reasoning')}
                  onKeyDown={(event) => handleNativeSubmenuTriggerKeyDown('reasoning', event)}
                  shortcut={<ChevronRight size={14} aria-hidden />}
                >
                  {t('reasoningSelector.title')}
                </MenuItem>
              )}

              <MenuSeparator />

              <MenuItem
                className="bitfun-model-selector__settings-item bitfun-model-selector__settings-reset"
                data-testid="chat-model-selector-settings-reset"
                disabled={nativeSettingsAreDefault || disabled || loading || reasoningLoading}
                onClick={handleResetNativeSettings}
                shortcut={<RotateCcw size={14} />}
              >
                {t('modelSelector.resetToDefaults')}
              </MenuItem>
            </MenuSection>
          </Menu>,
          getAppearanceOverlayHost()
        )}
      </RetainedMountBoundary>

      {dropdownOpen && nativeSubmenu && createPortal(
        <Menu
          id={nativeSubmenuId}
          ref={nativeSubmenuRef}
          className="bitfun-model-selector__submenu"
          style={nativeSubmenuStyle}
          data-testid="chat-model-selector-submenu"
          data-submenu-kind={nativeSubmenu}
          data-menu-level={activeProviderGroup ? 'provider' : nativeSubmenu}
          data-placement={nativeSubmenuPlacement}
          data-bf-component="model-selector"
          data-bf-part="dropdown"
          aria-label={activeProviderGroup
            ? activeProviderGroup.providerName
            : nativeSubmenu === 'reasoning'
              ? t('reasoningSelector.title')
              : t('modelSelector.modelSelection')}
          onKeyDown={handleNativeSubmenuKeyDown}
        >
          <ModelSelectorMenuLevel
            key={activeProviderGroup ? `provider:${activeProviderGroup.key}` : nativeSubmenu}
            direction={levelDirection}
          >
            {nativeSubmenu === 'reasoning' ? (
              <>
                <MenuItem
                  role="menuitemradio"
                  checked={!selectedReasoningDescriptor}
                  data-testid="chat-model-selector-reasoning-option"
                  data-preset-id="auto"
                  data-bf-component="model-selector"
                  data-bf-part="option"
                  data-bf-state={!selectedReasoningDescriptor ? 'selected' : undefined}
                  onClick={() => handleSelectReasoningPresetFromMenu(null)}
                >
                  {t('reasoningSelector.auto')}
                </MenuItem>

                {orderedReasoningPresets.map((preset, index) => {
                  const isSelected = selectedReasoningDescriptor?.id === preset.id;
                  const label = reasoningPresetLabels[index]
                    ?? presetDisplayLabel(preset, orderedReasoningPresets, t);

                  return (
                    <MenuItem
                      key={preset.id}
                      role="menuitemradio"
                      checked={isSelected}
                      data-testid="chat-model-selector-reasoning-option"
                      data-preset-id={preset.id}
                      data-bf-component="model-selector"
                      data-bf-part="option"
                      data-bf-state={isSelected ? 'selected' : undefined}
                      onClick={() => handleSelectReasoningPresetFromMenu(preset.id)}
                    >
                      {label}
                    </MenuItem>
                  );
                })}
              </>
            ) : activeProviderGroup ? (
              <>
                <MenuItem
                  data-testid="chat-model-selector-back"
                  data-bf-component="model-selector"
                  data-bf-part="back"
                  aria-label={t('modelSelector.backToProviders')}
                  leading={<ChevronLeft size={12} aria-hidden />}
                  onClick={closeProviderLevel}
                >
                  {activeProviderGroup.providerName}
                </MenuItem>

                {activeProviderGroup.models.map(model => {
                  const isSelected = currentModelId === model.id;

                  return (
                    <Tooltip key={model.id} content={buildModelMetaText(model)} placement="right">
                      <MenuItem
                        role="menuitemradio"
                        checked={isSelected}
                        data-testid="chat-model-selector-option"
                        data-model-id={model.id}
                        data-model-name={model.modelName}
                        data-selected={isSelected ? 'true' : 'false'}
                        data-bf-component="model-selector"
                        data-bf-part="option"
                        data-bf-state={isSelected ? 'selected' : undefined}
                        metadata={isSelected ? <Check size={14} aria-hidden /> : null}
                        onClick={() => handleSelectModel(model.id)}
                      >
                        {model.modelName}
                      </MenuItem>
                    </Tooltip>
                  );
                })}
              </>
            ) : (
              <>
                {providerGroups.map(group => {
                  const isSelected = selectedProviderKey === group.key;
                  const selectedModel = isSelected
                    ? group.models.find(model => model.id === currentModelId) ?? null
                    : null;

                  return (
                    <Tooltip
                      key={group.key}
                      content={`${group.providerName} · ${t('modelSelector.providerModelCount', { total: group.models.length })}`}
                      placement="right"
                    >
                      <MenuItem
                        aria-haspopup="menu"
                        aria-expanded={false}
                        data-testid="chat-model-selector-provider"
                        data-provider-key={group.key}
                        data-selected={isSelected ? 'true' : 'false'}
                        data-bf-component="model-selector"
                        data-bf-part="providerOption"
                        data-bf-state={isSelected ? 'selected' : undefined}
                        metadata={group.models.length}
                        shortcut={<ChevronRight size={14} aria-hidden />}
                        onClick={() => openProviderLevel(group.key)}
                      >
                        <div className="bitfun-model-selector__option-main" data-bf-component="model-selector" data-bf-part="optionMain">
                          <span className="bitfun-model-selector__option-name">
                            {group.providerName}
                          </span>
                          {selectedModel && (
                            <span
                              className="bitfun-model-selector__option-desc bitfun-model-selector__option-desc--selected-model"
                              data-testid="chat-model-selector-provider-selected-model"
                              data-model-id={selectedModel.id}
                            >
                              <span className="bitfun-model-selector__option-desc-label">
                                {selectedModel.modelName}
                              </span>
                              <Check
                                size={11}
                                aria-hidden="true"
                                className="bitfun-model-selector__option-selected-check"
                                data-testid="chat-model-selector-provider-selected-check"
                              />
                            </span>
                          )}
                        </div>
                      </MenuItem>
                    </Tooltip>
                  );
                })}

                <MenuSeparator />

                {(() => {
                  const primaryModel = allModels.find(m => m.id === defaultModels.primary);
                  const primaryTooltip = primaryModel
                    ? buildResolvedModelTooltipText(primaryModel.model_name, {
                      providerName: getProviderDisplayName(primaryModel),
                      contextWindow: primaryModel.context_window
                    }, t('modelSelector.primaryModelDesc'))
                    : t('modelSelector.primaryModelDesc');
                  return (
                    <Tooltip content={primaryTooltip} placement="right">
                      <MenuItem
                        role="menuitemradio"
                        checked={currentModelId === 'primary'}
                        data-testid="chat-model-selector-option"
                        data-model-id="primary"
                        data-model-name={primaryModel?.model_name || 'primary'}
                        data-selected={currentModelId === 'primary' ? 'true' : 'false'}
                        data-bf-component="model-selector"
                        data-bf-part="option"
                        data-bf-state={currentModelId === 'primary' ? 'selected' : undefined}
                        metadata={currentModelId === 'primary' ? <Check size={14} aria-hidden /> : null}
                        onClick={() => handleSelectModel('primary')}
                      >
                        {t('modelSelector.primaryModel')}
                      </MenuItem>
                    </Tooltip>
                  );
                })()}

                {(() => {
                  const fastModel = allModels.find(m => m.id === defaultModels.fast);
                  const fastTooltip = fastModel
                    ? buildResolvedModelTooltipText(fastModel.model_name, {
                      providerName: getProviderDisplayName(fastModel),
                      contextWindow: fastModel.context_window
                    }, t('modelSelector.fastModelDesc'))
                    : t('modelSelector.fastModelDesc');
                  return (
                    <Tooltip content={fastTooltip} placement="right">
                      <MenuItem
                        role="menuitemradio"
                        checked={currentModelId === 'fast'}
                        data-testid="chat-model-selector-option"
                        data-model-id="fast"
                        data-model-name={fastModel?.model_name || 'fast'}
                        data-selected={currentModelId === 'fast' ? 'true' : 'false'}
                        data-bf-component="model-selector"
                        data-bf-part="option"
                        data-bf-state={currentModelId === 'fast' ? 'selected' : undefined}
                        metadata={currentModelId === 'fast' ? <Check size={14} aria-hidden /> : null}
                        onClick={() => handleSelectModel('fast')}
                      >
                        {t('modelSelector.fastModel')}
                      </MenuItem>
                    </Tooltip>
                  );
                })()}

              </>
            )}
          </ModelSelectorMenuLevel>
        </Menu>,
        getAppearanceOverlayHost()
      )}
    </div>
  );
};
export default ModelSelector;
