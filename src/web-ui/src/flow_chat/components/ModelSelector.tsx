/**
 * Model selector component.
 * Shows the active model and allows quick switching.
 *
 * Config linkage:
 * - Unified logic: all modes use ai.agent_models[mode_id]
 * - Supports 'primary' | 'fast' | specific model IDs
 */

import React, { useState, useEffect, useRef, useCallback, useMemo } from 'react';
import { Cpu, ChevronDown, Check, Sparkles, Zap } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { configManager } from '@/infrastructure/config/services/ConfigManager';
import { globalEventBus } from '@/infrastructure/event-bus';
import type { AIModelConfig } from '@/infrastructure/config/types';
import { Tooltip } from '@/component-library';
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
}

interface ModelInfo {
  id: string;
  name: string;
  modelName: string;
  provider: string;
  contextWindow?: number;
  enableThinking?: boolean;
}

// Helper: identify special model IDs.
const isSpecialModel = (value: string): value is 'primary' | 'fast' => {
  return value === 'primary' || value === 'fast';
};

export const ModelSelector: React.FC<ModelSelectorProps> = ({
  currentMode,
  className = '',
  sessionId,
  ...restProps
}) => {
  const { t } = useTranslation('flow-chat');
  const [allModels, setAllModels] = useState<AIModelConfig[]>([]);
  const [defaultModels, setDefaultModels] = useState<Record<string, string>>({});
  const [agentModels, setAgentModels] = useState<Record<string, string>>({}); // mode_id -> model_id
  const [dropdownOpen, setDropdownOpen] = useState(false);
  const [loading, setLoading] = useState(false);

  const dropdownRef = useRef<HTMLDivElement>(null);

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
  
  const getCurrentModelId = useCallback((): string => {
    return agentModels[currentMode] || 'primary';
  }, [currentMode, agentModels]);

  const currentModel = useMemo((): ModelInfo | null => {
    const modelId = getCurrentModelId();

    if (isSpecialModel(modelId)) {
      const actualModelId = defaultModels[modelId];
      if (!actualModelId) return null;

      const model = allModels.find(m => m.id === actualModelId);
      if (!model) return null;

      return {
        id: modelId, // Keep 'primary' or 'fast'
        name: model.name,
        modelName: model.model_name,
        provider: model.provider,
        contextWindow: model.context_window,
        enableThinking: model.enable_thinking_process
      };
    }

    const model = allModels.find(m => m.id === modelId);
    if (!model) return null;

    return {
      id: model.id || '',
      name: model.name,
      modelName: model.model_name,
      provider: model.provider,
      contextWindow: model.context_window,
      enableThinking: model.enable_thinking_process
    };
  }, [getCurrentModelId, allModels, defaultModels]);
  
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
        name: m.name,
        modelName: m.model_name,
        provider: m.provider,
        contextWindow: m.context_window,
        enableThinking: m.enable_thinking_process
      }));
  }, [allModels]);
  
  const handleSelectModel = useCallback(async (modelId: string) => {
    if (loading) return;

    setLoading(true);
    try {
      const currentAgentModels = await configManager.getConfig<Record<string, string>>('ai.agent_models') || {};

      const updatedAgentModels = {
        ...currentAgentModels,
        [currentMode]: modelId,
      };

      await configManager.setConfig('ai.agent_models', updatedAgentModels);
      setAgentModels(updatedAgentModels);
      log.info('Mode model updated', { mode: currentMode, modelId });

      globalEventBus.emit('mode:config:updated');

      setDropdownOpen(false);
    } catch (error) {
      log.error('Failed to switch model', error);
    } finally {
      setLoading(false);
    }
  }, [currentMode, loading]);
  
  if (availableModels.length === 0) {
    return null;
  }

  const currentModelId = getCurrentModelId();

  return (
    <div
      ref={dropdownRef}
      className={`bitfun-model-selector ${className}`}
    >
      <Tooltip content={currentModel ? `${t('modelSelector.model')}: ${currentModel.modelName}` : t('modelSelector.modelNotConfigured')}>
        <button
          className={`bitfun-model-selector__trigger ${dropdownOpen ? 'bitfun-model-selector__trigger--open' : ''}`}
          onClick={() => setDropdownOpen(!dropdownOpen)}
          disabled={loading}
        >
          <Cpu size={10} className="bitfun-model-selector__icon" />
          <span className="bitfun-model-selector__name">
            {currentModel ? currentModel.name : t('modelSelector.modelNotConfigured')}
          </span>
          {currentModel?.enableThinking && (
            <Sparkles size={9} className="bitfun-model-selector__thinking-icon" />
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

          <div
            className={`bitfun-model-selector__option bitfun-model-selector__option--special ${currentModelId === 'primary' ? 'bitfun-model-selector__option--selected' : ''}`}
            onClick={() => handleSelectModel('primary')}
          >
            <div className="bitfun-model-selector__option-main">
              <div className="bitfun-model-selector__option-content">
                <span className="bitfun-model-selector__option-name">{t('modelSelector.primaryModel')}</span>
                <span className="bitfun-model-selector__option-desc">
                  {allModels.find(m => m.id === defaultModels.primary)?.name || t('modelSelector.modelNotConfigured')}
                </span>
              </div>
            </div>
            {currentModelId === 'primary' && (
              <Check size={14} className="bitfun-model-selector__option-check" />
            )}
          </div>

          <div
            className={`bitfun-model-selector__option bitfun-model-selector__option--special ${currentModelId === 'fast' ? 'bitfun-model-selector__option--selected' : ''}`}
            onClick={() => handleSelectModel('fast')}
          >
            <div className="bitfun-model-selector__option-main">
              <div className="bitfun-model-selector__option-content">
                <span className="bitfun-model-selector__option-name">{t('modelSelector.fastModel')}</span>
                <span className="bitfun-model-selector__option-desc">
                  {allModels.find(m => m.id === defaultModels.fast)?.name || t('modelSelector.modelNotConfigured')}
                </span>
              </div>
            </div>
            {currentModelId === 'fast' && (
              <Check size={14} className="bitfun-model-selector__option-check" />
            )}
          </div>

          <div className="bitfun-model-selector__divider" />

          <div className="bitfun-model-selector__list">
            {availableModels.map(model => {
              const isSelected = currentModelId === model.id;

              return (
                <div
                  key={model.id}
                  className={`bitfun-model-selector__option ${isSelected ? 'bitfun-model-selector__option--selected' : ''}`}
                  onClick={() => handleSelectModel(model.id)}
                >
                  <div className="bitfun-model-selector__option-main">
                    <span className="bitfun-model-selector__option-name">
                      {model.name}
                      {model.enableThinking && (
                        <Sparkles size={10} className="bitfun-model-selector__option-thinking" />
                      )}
                    </span>
                    <span className="bitfun-model-selector__option-desc">
                      {model.modelName}
                      {model.contextWindow && ` · ${Math.round(model.contextWindow / 1000)}k`}
                    </span>
                  </div>
                  {isSelected && (
                    <Check size={14} className="bitfun-model-selector__option-check" />
                  )}
                </div>
              );
            })}
          </div>
        </div>
      )}
    </div>
  );
};

export default ModelSelector;
