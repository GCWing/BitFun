/**
 * Model selector component.
 * Shows the active model and allows quick switching.
 *
 * Config linkage:
 * - Unified logic: all modes use ai.agent_models[mode_id]
 * - Supports 'auto' | 'primary' | 'fast' | specific model IDs
 */

import React, { useState, useEffect, useRef, useCallback, useMemo } from 'react';
import { Cpu, ChevronDown, Check, Sparkles } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { configManager } from '@/infrastructure/config/services/ConfigManager';
import { agentAPI } from '@/infrastructure/api/service-api/AgentAPI';
import { ACPClientAPI, type AcpSessionOptions } from '@/infrastructure/api/service-api/ACPClientAPI';
import { getProviderDisplayName } from '@/infrastructure/config/services/modelConfigs';
import { getEffectiveReasoningMode, isReasoningVisiblyEnabled } from '@/infrastructure/config/utils/reasoning';
import { globalEventBus } from '@/infrastructure/event-bus';
import type { AIModelConfig } from '@/infrastructure/config/types';
import { Tooltip } from '@/component-library';
import { FlowChatStore } from '../store/FlowChatStore';
import { createLogger } from '@/shared/utils/logger';
import './ModelSelector.scss';

const log = createLogger('ModelSelector');

interface ModelSelectorProps {
  /** Current mode ID. */
  currentMode: string;
  /** Custom class name. */
  className?: string;
  /** Current session ID (used to update session mode config). */
  sessionId?: string;
  /** Current token count. */
  currentTokens?: number;
  /** Max token capacity. */
  maxTokens?: number;
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

export const ModelSelector: React.FC<ModelSelectorProps> = ({
  currentMode,
  className = '',
  sessionId,
  currentTokens = 0,
  maxTokens = 0,
}) => {
  const { t } = useTranslation('flow-chat');
  const [allModels, setAllModels] = useState<AIModelConfig[]>([]);
  const [defaultModels, setDefaultModels] = useState<Record<string, string>>({});
  const [agentModels, setAgentModels] = useState<Record<string, string>>({}); // mode_id -> model_id
  const [acpOptions, setAcpOptions] = useState<AcpSessionOptions | null>(null);
  const [dropdownOpen, setDropdownOpen] = useState(false);
  const [loading, setLoading] = useState(false);

  const dropdownRef = useRef<HTMLDivElement>(null);
  const activeSession = sessionId ? FlowChatStore.getInstance().getState().sessions.get(sessionId) : undefined;
  const acpClientId = activeSession?.config.agentType?.startsWith('acp:')
    ? activeSession.config.agentType.slice('acp:'.length)
    : null;
  const isAcpSession = Boolean(acpClientId && sessionId);

  // Load configuration data.
  const loadConfigData = useCallback(async () => {
    try {
      const [models, defaultModelsData, agentModelsData] = await Promise.all([
        configManager.getConfig<AIModelConfig[]>('ai.models') || [],
        configManager.getConfig<any>('ai.default_models') || {},
        configManager.getConfig<Record<string, string>>('ai.agent_models') || {}
      ]);

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

    try {
      const options = await ACPClientAPI.getSessionOptions({
        sessionId,
        clientId: acpClientId,
        workspacePath: activeSession?.workspacePath || activeSession?.config.workspacePath,
        remoteConnectionId: activeSession?.remoteConnectionId,
        remoteSshHost: activeSession?.remoteSshHost,
      });
      setAcpOptions(options);
    } catch (error) {
      log.warn('Failed to load ACP session model options', { sessionId, acpClientId, error });
      setAcpOptions(null);
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
    loadAcpOptions();
  }, [loadAcpOptions]);
  
  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (dropdownRef.current && !dropdownRef.current.contains(event.target as Node)) {
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
    const configuredModelId = agentModels[currentMode] || 'auto';
    if (configuredModelId === 'auto') return 'auto';
    if (configuredModelId === 'primary' || configuredModelId === 'fast') {
      const actualModelId = defaultModels[configuredModelId];
      const model = allModels.find(m => m.id === actualModelId);
      return model ? configuredModelId : 'auto';
    }
    const model = allModels.find(m => m.id === configuredModelId);
    return model ? configuredModelId : 'auto';
  }, [allModels, currentMode, agentModels, defaultModels]);

  const currentModel = useMemo((): ModelInfo | null => {
    const modelId = getCurrentModelId();

    if (modelId === 'auto') {
      return buildAutoModelInfo(t);
    }

    if (isSpecialModel(modelId)) {
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
        FlowChatStore.getInstance().updateSessionModelName(sessionId, modelId);
        log.info('ACP session model updated', { sessionId, acpClientId, modelId });
        setDropdownOpen(false);
        return;
      }

      const currentAgentModels = await configManager.getConfig<Record<string, string>>('ai.agent_models') || {};

      const updatedAgentModels = {
        ...currentAgentModels,
        [currentMode]: modelId,
      };

      await configManager.setConfig('ai.agent_models', updatedAgentModels);
      setAgentModels(updatedAgentModels);

      if (sessionId) {
        const store = FlowChatStore.getInstance();
        store.updateSessionModelName(sessionId, modelId);
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

      setDropdownOpen(false);
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

  const formatTokenCount = (n: number) =>
    n >= 1000 ? `${Math.round(n / 1000)}K` : `${n}`;

  if (isAcpSession) {
    if (acpAvailableModels.length === 0) {
      return null;
    }

    const currentAcpModelId = acpOptions?.currentModelId || acpAvailableModels[0]?.id || '';
    const acpTooltip = getModelTooltipText(acpCurrentModel, acpClientId ? `${acpClientId} ACP` : 'ACP');

    return (
      <div
        ref={dropdownRef}
        className={`bitfun-model-selector ${className}`}
      >
        <Tooltip content={acpTooltip}>
          <button
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
            <Cpu size={10} className="bitfun-model-selector__icon" />
            <span className="bitfun-model-selector__name">
              {getModelDisplayLabel(acpCurrentModel, currentAcpModelId)}
            </span>
            <ChevronDown size={10} className="bitfun-model-selector__chevron" />
          </button>
        </Tooltip>

        {dropdownOpen && (
          <div className="bitfun-model-selector__dropdown">
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
          </div>
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
  const tooltipContent =
    currentTokens > 0 && maxTokens > 0
      ? `${baseTooltip} · ${formatTokenCount(currentTokens)}/${formatTokenCount(maxTokens)} (${tokenPercentage}%)`
      : baseTooltip;

  return (
    <div
      ref={dropdownRef}
      className={`bitfun-model-selector ${className}`}
    >
      <Tooltip content={tooltipContent}>
        <button
          className={`bitfun-model-selector__trigger ${dropdownOpen ? 'bitfun-model-selector__trigger--open' : ''}`}
          onClick={() => setDropdownOpen(!dropdownOpen)}
          disabled={loading}
        >
          <Cpu size={10} className="bitfun-model-selector__icon" />
          <span className="bitfun-model-selector__name">
            {getModelDisplayLabel(currentModel, t('modelSelector.autoModel'))}
          </span>
          {currentModel?.enableThinking && (
            <Sparkles size={9} className="bitfun-model-selector__thinking-icon" />
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

      {dropdownOpen && (
        <div className="bitfun-model-selector__dropdown">
          <div className="bitfun-model-selector__dropdown-header">
            <span>{t('modelSelector.modelSelection')}</span>
            <span className="bitfun-model-selector__dropdown-hint">
              {t('modelSelector.currentMode')}: {currentMode}
            </span>
          </div>

          <Tooltip content={t('modelSelector.autoModelDesc')} placement="right">
            <div
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
                    className={`bitfun-model-selector__option ${isSelected ? 'bitfun-model-selector__option--selected' : ''}`}
                    onClick={() => handleSelectModel(model.id)}
                  >
                    <div className="bitfun-model-selector__option-main">
                      <span className="bitfun-model-selector__option-name">
                        {model.modelName}
                        {model.enableThinking && (
                          <Sparkles size={10} className="bitfun-model-selector__option-thinking" />
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
        </div>
      )}
    </div>
  );
};
export default ModelSelector;
