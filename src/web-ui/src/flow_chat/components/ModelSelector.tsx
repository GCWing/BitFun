/**
 * Model selector component.
 * Shows the active model and allows quick switching.
 *
 * Config linkage:
 * - Model selection is shared across all agent types via ai.agent_models
 *   — switching in one session refreshes every existing agent mapping plus
 *   the current session's agent type, so new sessions of any type inherit it.
 * - Supports 'auto' | 'primary' | 'fast' | specific model IDs
 */

import React, { useState, useEffect, useRef, useCallback, useMemo } from 'react';
import { createPortal } from 'react-dom';
import { Brain, ChevronDown, Check } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { configManager } from '@/infrastructure/config/services/ConfigManager';
import { agentAPI } from '@/infrastructure/api/service-api/AgentAPI';
import { ACPClientAPI, type AcpSessionOptions } from '@/infrastructure/api/service-api/ACPClientAPI';
import { getProviderDisplayName } from '@/infrastructure/config/services/modelConfigs';
import { getEffectiveReasoningMode, isReasoningVisiblyEnabled } from '@/infrastructure/config/utils/reasoning';
import { globalEventBus } from '@/infrastructure/event-bus';
import type { AIModelConfig, DefaultModelsConfig } from '@/infrastructure/config/types';
import { Tooltip } from '@/component-library';
import { FlowChatStore } from '../store/FlowChatStore';
import { getModelMaxTokens } from '../services/flow-chat-manager/SessionModule';
import { acpClientIdFromAgentType } from '../utils/acpSession';
import {
  buildContextUsageTooltip,
  type ContextUsageSource,
} from '../utils/tokenUsageDisplay';
import { createLogger } from '@/shared/utils/logger';
import './ModelSelector.scss';

const log = createLogger('ModelSelector');
const ACP_SESSION_OPTIONS_TIMEOUT_MS = 65_000;

interface ModelSelectorProps {
  /** Current target agent type. */
  currentMode: string;
  /** Custom class name. */
  className?: string;
  /** Preferred dropdown placement relative to the trigger. */
  dropdownPlacement?: 'top' | 'bottom';
  /** Current session ID (used to update session mode config). */
  sessionId?: string;
  /** Current token count. */
  currentTokens?: number;
  /** Max token capacity. */
  maxTokens?: number;
  /** Semantic source for the context usage number. */
  contextUsageSource?: ContextUsageSource;
  /** Called when model switching starts or completes, so the parent can gate sending. */
  onLoadingChange?: (loading: boolean) => void;
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
  enableThinking?: boolean;
  reasoningEffort?: string;
}

// Helper: identify special model IDs.
const isSpecialModel = (value: string): value is 'auto' | 'primary' | 'fast' => {
  return value === 'auto' || value === 'primary' || value === 'fast';
};

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
  currentTokens = 0,
  maxTokens = 0,
  contextUsageSource,
  onLoadingChange,
}) => {
  const { t } = useTranslation('flow-chat');
  const [allModels, setAllModels] = useState<AIModelConfig[]>([]);
  const [defaultModels, setDefaultModels] = useState<DefaultModelsConfig>({});
  const [agentModels, setAgentModels] = useState<Record<string, string>>({}); // mode_id -> model_id
  const [acpOptions, setAcpOptions] = useState<AcpSessionOptions | null>(null);
  const [dropdownOpen, setDropdownOpen] = useState(false);
  const [loading, setLoading] = useState(false);
  const acpRestoreToastShownRef = useRef<string | null>(null);
  const acpOptionsRef = useRef<AcpSessionOptions | null>(null);

  const dropdownRef = useRef<HTMLDivElement>(null);
  const portalDropdownRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    onLoadingChange?.(loading);
  }, [loading, onLoadingChange]);

  const [dropdownStyle, setDropdownStyle] = useState<React.CSSProperties>({
    position: 'fixed',
    visibility: 'hidden',
  });
  const activeSession = sessionId ? FlowChatStore.getInstance().getState().sessions.get(sessionId) : undefined;
  const acpClientId =
    acpClientIdFromAgentType(activeSession?.config.agentType) ??
    acpClientIdFromAgentType(activeSession?.mode);
  const isAcpSession = Boolean(acpClientId && sessionId);

  // Load configuration data.
  const loadConfigData = useCallback(async () => {
    try {
      const configData = await configManager.getConfigs([
        'ai.models',
        'ai.default_models',
        'ai.agent_models',
      ]);
      const models = (configData['ai.models'] as AIModelConfig[] | undefined) || [];
      const defaultModelsData = (configData['ai.default_models'] as DefaultModelsConfig | undefined) || {};
      const agentModelsData = (configData['ai.agent_models'] as Record<string, string> | undefined) || {};

      setAllModels(models);
      setDefaultModels(defaultModelsData);
      setAgentModels(agentModelsData);

      log.debug('Configuration loaded', {
        modelsCount: models.length
      });
    } catch (error) {
      log.error('Failed to load configuration', error);
    }
  }, []);
  
  useEffect(() => {
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
    };
  }, [loadConfigData]);

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
    } catch (error) {
      log.warn('Failed to load ACP session model options', { sessionId, acpClientId, error });
      setAcpOptions(null);
    } finally {
      if (shouldShowRestoreToast) {
        window.dispatchEvent(new CustomEvent('bitfun:acp-session-creation', {
          detail: { phase: 'finish', clientId: acpClientId, action: 'restore', requestId: restoreRequestId },
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
      if (!dropdownRef.current) return;
      const rect = dropdownRef.current.getBoundingClientRect();
      const placementStyle = dropdownPlacement === 'bottom'
        ? { top: `${rect.bottom + 6}px`, bottom: 'auto' }
        : { top: 'auto', bottom: `${window.innerHeight - rect.top + 6}px` };
      setDropdownStyle({
        position: 'fixed',
        visibility: 'visible',
        left: `${rect.left}px`,
        minWidth: '220px',
        maxWidth: '280px',
        ...placementStyle,
      });
    };

    updatePosition();

    window.addEventListener('scroll', updatePosition, true);
    window.addEventListener('resize', updatePosition);

    return () => {
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
      providerName: acpClientId ? `${acpClientId} ACP` : 'ACP',
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
  
  const getCurrentModelId = useCallback((): string => {
    // Session-owned model takes priority so that each session remembers
    // its own model selection independently.
    const sessionModelName = activeSession?.config.modelName?.trim();
    if (sessionModelName && sessionModelName !== 'auto') {
      if (sessionModelName === 'primary' || sessionModelName === 'fast') {
        const actualModelId = defaultModels[sessionModelName];
        const model = allModels.find(m => m.id === actualModelId);
        if (model) return sessionModelName;
      } else {
        const model = allModels.find(m => m.id === sessionModelName);
        if (model) return sessionModelName;
      }
    }

    // Fall back to global per-mode configuration.
    const configuredModelId = agentModels[currentMode] || 'auto';
    if (configuredModelId === 'auto') return 'auto';
    if (configuredModelId === 'primary' || configuredModelId === 'fast') {
      const actualModelId = defaultModels[configuredModelId];
      const model = allModels.find(m => m.id === actualModelId);
      return model ? configuredModelId : 'auto';
    }
    const model = allModels.find(m => m.id === configuredModelId);
    return model ? configuredModelId : 'auto';
  }, [allModels, currentMode, agentModels, defaultModels, activeSession?.config.modelName]);

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
        enableThinking: isReasoningVisiblyEnabled(getEffectiveReasoningMode(model)),
        reasoningEffort: model.reasoning_effort,
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
      enableThinking: isReasoningVisiblyEnabled(getEffectiveReasoningMode(model)),
      reasoningEffort: model.reasoning_effort,
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
        enableThinking: isReasoningVisiblyEnabled(getEffectiveReasoningMode(m)),
        reasoningEffort: m.reasoning_effort,
      }));
  }, [allModels]);
  
  const handleSelectModel = useCallback(async (modelId: string) => {
    if (loading) return;

    setLoading(true);
    setDropdownOpen(false);

    try {
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
        FlowChatStore.getInstance().updateSessionModelName(sessionId, modelId);
        log.info('ACP session model updated', { sessionId, acpClientId, modelId });
        return;
      }

      const currentAgentModels = await configManager.getConfig<Record<string, string>>('ai.agent_models') || {};

      // Refresh **every** agent mapping already present in the config so
      // all configured agents pick up the new model.  Also ensure the
      // current session's agent type is included.  No hardcoded list —
      // any agent type added to the config in the future is covered
      // automatically.
      const updatedAgentModels = { ...currentAgentModels };
      for (const agentType of Object.keys(updatedAgentModels)) {
        updatedAgentModels[agentType] = modelId;
      }
      updatedAgentModels[currentMode] = modelId;

      await configManager.setConfig('ai.agent_models', updatedAgentModels);
      setAgentModels(updatedAgentModels);

      if (sessionId) {
        const store = FlowChatStore.getInstance();
        // Update the frontend session model immediately so the UI reflects the
        // switch without waiting for the backend IPC round-trip.
        store.updateSessionModelName(sessionId, modelId);
        const maxContextTokens = await getModelMaxTokens(modelId, currentMode);
        store.updateSessionMaxContextTokens(sessionId, maxContextTokens);
        const session = store.getState().sessions.get(sessionId);
        if (!session?.isTransient) {
          await agentAPI.updateSessionModel({
            sessionId,
            modelName: modelId,
          });
        }
      }

      log.info('Mode model updated', { mode: currentMode, modelId });

      globalEventBus.emit('mode:config:updated');
    } catch (error) {
      log.error('Failed to switch model', error);
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
    isAcpSession,
    loading,
    sessionId,
  ]);
  
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

  if (isAcpSession) {
    if (acpAvailableModels.length === 0) {
      return null;
    }

    const currentAcpModelId = acpOptions?.currentModelId || acpAvailableModels[0]?.id || '';
    const acpBaseTooltip = getModelTooltipText(acpCurrentModel, acpClientId ? `${acpClientId} ACP` : 'ACP');
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
      <div
        ref={dropdownRef}
        className={`bitfun-model-selector ${className}`}
      >
        <Tooltip content={acpTooltip}>
          <button
            data-testid="chat-model-selector-btn"
            className={`bitfun-model-selector__trigger ${dropdownOpen ? 'bitfun-model-selector__trigger--open' : ''}`}
            onClick={() => {
              const nextOpen = !dropdownOpen;
              setDropdownOpen(nextOpen);
              if (nextOpen) {
                loadAcpOptions();
              }
            }}
            disabled={loading}
          >
            <span className="bitfun-model-selector__name">
              {getModelDisplayLabel(acpCurrentModel, currentAcpModelId)}
            </span>
            {tokenPercentage > 0 && (
              <span className={`bitfun-model-selector__ctx-usage${tokenStatusClass ? ` bitfun-model-selector__ctx-usage--${tokenStatusClass}` : ''}`}>
                · {tokenPercentage}%
              </span>
            )}
            <ChevronDown size={10} className="bitfun-model-selector__chevron" />
          </button>
        </Tooltip>

        {dropdownOpen && createPortal(
          <div
            className="bitfun-model-selector__dropdown"
            ref={portalDropdownRef}
            style={dropdownStyle}
            data-testid="chat-model-selector-menu"
          >
            <div className="bitfun-model-selector__dropdown-header">
              <span>ACP model</span>
              <span className="bitfun-model-selector__dropdown-hint">
                {acpClientId}
              </span>
            </div>

            <div className="bitfun-model-selector__list">
              {acpAvailableModels.map(model => {
                const isSelected = currentAcpModelId === model.id;

                return (
                  <Tooltip key={model.id} content={model.id} placement="right">
                    <div
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
                      {isSelected && (
                        <Check size={14} className="bitfun-model-selector__option-check" />
                      )}
                    </div>
                  </Tooltip>
                );
              })}
            </div>
          </div>,
          document.body
        )}
      </div>
    );
  }

  if (availableModels.length === 0) {
    return null;
  }

  const currentModelId = getCurrentModelId();

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
    <div
      ref={dropdownRef}
      className={`bitfun-model-selector ${className}`}
    >
      <Tooltip content={tooltipContent}>
        <button
          data-testid="chat-model-selector-btn"
          className={`bitfun-model-selector__trigger ${dropdownOpen ? 'bitfun-model-selector__trigger--open' : ''}`}
          onClick={() => setDropdownOpen(!dropdownOpen)}
          disabled={loading}
        >
          <span className="bitfun-model-selector__name">
            {getModelDisplayLabel(currentModel, t('modelSelector.autoModel'))}
          </span>
          {currentModel?.enableThinking && (
            <Brain size={9} className="bitfun-model-selector__thinking-icon" />
          )}
          {currentModel?.reasoningEffort && (
            <span className="bitfun-model-selector__effort-badge">
              {currentModel.reasoningEffort}
            </span>
          )}
          {tokenPercentage > 0 && (
            <span className={`bitfun-model-selector__ctx-usage${tokenStatusClass ? ` bitfun-model-selector__ctx-usage--${tokenStatusClass}` : ''}`}>
              · {tokenPercentage}%
            </span>
          )}
          <ChevronDown size={10} className="bitfun-model-selector__chevron" />
        </button>
      </Tooltip>

      {dropdownOpen && createPortal(
        <div
          className="bitfun-model-selector__dropdown"
          ref={portalDropdownRef}
          style={dropdownStyle}
          data-testid="chat-model-selector-menu"
        >
          <div className="bitfun-model-selector__dropdown-header">
            <span>{t('modelSelector.modelSelection')}</span>
            <span className="bitfun-model-selector__dropdown-hint">
              {t('modelSelector.currentMode')}: {currentMode}
            </span>
          </div>

          <Tooltip content={t('modelSelector.autoModelDesc')} placement="right">
            <div
              data-testid="chat-model-selector-option"
              data-model-id="auto"
              data-model-name="auto"
              data-selected={currentModelId === 'auto' ? 'true' : 'false'}
              className={`bitfun-model-selector__option bitfun-model-selector__option--special ${currentModelId === 'auto' ? 'bitfun-model-selector__option--selected' : ''}`}
              onClick={() => handleSelectModel('auto')}
            >
              <div className="bitfun-model-selector__option-main">
                <span className="bitfun-model-selector__option-name">{t('modelSelector.autoModel')}</span>
              </div>
              {currentModelId === 'auto' && (
                <Check size={14} className="bitfun-model-selector__option-check" />
              )}
            </div>
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
                <div
                  data-testid="chat-model-selector-option"
                  data-model-id="primary"
                  data-model-name={primaryModel?.model_name || 'primary'}
                  data-selected={currentModelId === 'primary' ? 'true' : 'false'}
                  className={`bitfun-model-selector__option bitfun-model-selector__option--special ${currentModelId === 'primary' ? 'bitfun-model-selector__option--selected' : ''}`}
                  onClick={() => handleSelectModel('primary')}
                >
                  <div className="bitfun-model-selector__option-main">
                    <span className="bitfun-model-selector__option-name">{t('modelSelector.primaryModel')}</span>
                  </div>
                  {currentModelId === 'primary' && (
                    <Check size={14} className="bitfun-model-selector__option-check" />
                  )}
                </div>
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
                <div
                  data-testid="chat-model-selector-option"
                  data-model-id="fast"
                  data-model-name={fastModel?.model_name || 'fast'}
                  data-selected={currentModelId === 'fast' ? 'true' : 'false'}
                  className={`bitfun-model-selector__option bitfun-model-selector__option--special ${currentModelId === 'fast' ? 'bitfun-model-selector__option--selected' : ''}`}
                  onClick={() => handleSelectModel('fast')}
                >
                  <div className="bitfun-model-selector__option-main">
                    <span className="bitfun-model-selector__option-name">{t('modelSelector.fastModel')}</span>
                  </div>
                  {currentModelId === 'fast' && (
                    <Check size={14} className="bitfun-model-selector__option-check" />
                  )}
                </div>
              </Tooltip>
            );
          })()}

          <div className="bitfun-model-selector__divider" />

          <div className="bitfun-model-selector__list">
            {availableModels.map(model => {
              const isSelected = currentModelId === model.id;

              return (
                <Tooltip key={model.id} content={buildModelMetaText(model)} placement="right">
                  <div
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
                        {model.enableThinking && (
                          <Brain size={10} className="bitfun-model-selector__option-thinking" />
                        )}
                      </span>
                    </div>
                    {isSelected && (
                      <Check size={14} className="bitfun-model-selector__option-check" />
                    )}
                  </div>
                </Tooltip>
              );
            })}
          </div>
        </div>,
        document.body
      )}
    </div>
  );
};
export default ModelSelector;
