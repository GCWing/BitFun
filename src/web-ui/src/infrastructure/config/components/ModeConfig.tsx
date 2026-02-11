 

import React, { useState, useEffect, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { RotateCcw, AlertCircle, ChevronDown, ChevronRight, CheckSquare, XSquare } from 'lucide-react';
import { Button, CubeLoading, Switch, IconButton, Card } from '@/component-library';
import { configAPI } from '@/infrastructure/api';
import { useNotification } from '@/shared/notification-system';
import { ModeConfigItem, AIModelConfig } from '../types';
import { configManager } from '../services/ConfigManager';
import { ConfigPageHeader, ConfigPageLayout, ConfigPageContent } from './common';
import { ModelSelectionRadio } from './ModelSelectionRadio';
import { createLogger } from '@/shared/utils/logger';
import './ModeConfig.scss';

const log = createLogger('ModeConfig');

interface ModeInfo {
  id: string;
  name: string;
  description: string;
  is_readonly: boolean;
  tool_count: number;
  default_tools?: string[];
}

type ModeCategory = 'all' | 'design' | 'development';


const MODE_CATEGORIES: Record<string, ModeCategory> = {
  
  'debug': 'development',
  'code_review': 'development',
  'test': 'development',
};

interface ToolInfo {
  name: string;
  description: string;
  is_readonly: boolean;
}

const ModeConfig: React.FC = () => {
  const { t } = useTranslation('settings/modes');
  const [loading, setLoading] = useState(true);
  const [syncing, setSyncing] = useState(false);
  const [modes, setModes] = useState<ModeInfo[]>([]);
  const [modeConfigs, setModeConfigs] = useState<Record<string, ModeConfigItem>>({});
  const [availableTools, setAvailableTools] = useState<ToolInfo[]>([]);
  const [availableModels, setAvailableModels] = useState<AIModelConfig[]>([]);
  const [agentModels, setAgentModels] = useState<Record<string, string>>({}); // mode_id -> model_id
  const [collapsedTools, setCollapsedTools] = useState<Record<string, boolean>>({});
  const [activeCategory, setActiveCategory] = useState<ModeCategory>('all');
  const notification = useNotification();

  useEffect(() => {
    loadData();
  }, []);

  const loadData = async () => {
    try {
      setLoading(true);

      
      const [modesData, configsData, toolsData, modelsData, agentModelsData] = await Promise.all([
        fetchAvailableModes(),
        configAPI.getModeConfigs(),
        fetchAvailableTools(),
        configManager.getConfig<AIModelConfig[]>('ai.models'),
        configManager.getConfig<Record<string, string>>('ai.agent_models') || {}
      ]);

      setModes(modesData);
      setModeConfigs(configsData || {});
      setAvailableTools(toolsData);
      setAvailableModels(modelsData || []);
      setAgentModels(agentModelsData);
    } catch (error) {
      log.error('Failed to load data', error);
      notification.error(t('messages.loadFailed'));
    } finally {
      setLoading(false);
    }
  };

  const fetchAvailableModes = async (): Promise<ModeInfo[]> => {
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      const allModes = await invoke<ModeInfo[]>('get_available_modes');
      
      return allModes.filter(mode => mode.id !== 'agentic');
    } catch (error) {
      log.error('Failed to fetch modes list', error);
      return [];
    }
  };

  const fetchAvailableTools = async (): Promise<ToolInfo[]> => {
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      const tools = await invoke<ToolInfo[]>('get_all_tools_info');
      return tools;
    } catch (error) {
      log.error('Failed to fetch tools list', error);
      return [];
    }
  };

  
  const getModelName = useCallback((modelId: string | null | undefined): string | undefined => {
    if (!modelId) return undefined;
    return availableModels.find(m => m.id === modelId)?.name;
  }, [availableModels]);

  const getModeConfig = (modeId: string): ModeConfigItem => {
    const userConfig = modeConfigs[modeId];
    const mode = modes.find(m => m.id === modeId);

    
    if (!userConfig) {
      return {
        mode_id: modeId,
        available_tools: mode?.default_tools || [],
        enabled: true,
        default_tools: mode?.default_tools || []
      };
    }

    
    if (!userConfig.available_tools || userConfig.available_tools.length === 0) {
      return {
        ...userConfig,
        available_tools: mode?.default_tools || [],
        default_tools: mode?.default_tools || []
      };
    }

    
    return {
      ...userConfig,
      default_tools: mode?.default_tools || userConfig.default_tools || []
    };
  };

  const updateModeConfig = async (modeId: string, updates: Partial<ModeConfigItem>) => {
    try {
      const config = getModeConfig(modeId);
      const updatedConfig = { ...config, ...updates };

      await configAPI.setModeConfig(modeId, updatedConfig);

      setModeConfigs(prev => ({
        ...prev,
        [modeId]: updatedConfig
      }));

      
      const { globalEventBus } = await import('@/infrastructure/event-bus');
      globalEventBus.emit('mode:config:updated');
    } catch (error) {
      log.error('Failed to update mode config', { modeId, error });
      notification.error(t('messages.saveFailed'));
    }
  };
  const handleModeModelChange = async (
    modeId: string,
    modelId: string
  ) => {
    try {
      
      const currentAgentModels = await configManager.getConfig<Record<string, string>>('ai.agent_models') || {};

      
      const updatedAgentModels = {
        ...currentAgentModels,
        [modeId]: modelId,
      };
      await configManager.setConfig('ai.agent_models', updatedAgentModels);

      setAgentModels(updatedAgentModels);

      
      let modelDesc = '';
      if (modelId === 'primary') {
        modelDesc = t('model.primary');
      } else if (modelId === 'fast') {
        modelDesc = t('model.fast');
      } else {
        modelDesc = getModelName(modelId) || modelId || '';
      }

      notification.success(
        t('messages.modelUpdated', {
          modeName: modes.find(m => m.id === modeId)?.name || modeId,
          modelName: modelDesc
        })
      );

      
      const { globalEventBus } = await import('@/infrastructure/event-bus');
      globalEventBus.emit('mode:config:updated');
    } catch (error) {
      log.error('Failed to update model', { modeId, modelId, error });
      notification.error(t('messages.modelUpdateFailed'));
    }
  };

  const toggleTool = async (modeId: string, toolName: string) => {
    try {
      const config = getModeConfig(modeId);
      const tools = config.available_tools || [];

      
      const isEnabling = !tools.includes(toolName);

      const newTools = isEnabling
        ? [...tools, toolName]
        : tools.filter(t => t !== toolName);

      
      const updatedConfig = { ...config, available_tools: newTools };
      await configAPI.setModeConfig(modeId, updatedConfig);

      
      setModeConfigs(prev => ({
        ...prev,
        [modeId]: updatedConfig
      }));

      
      const modeName = modes.find(m => m.id === modeId)?.name || modeId;

      notification.success(
        isEnabling
          ? t('messages.toolEnabled', { mode: modeName, tool: toolName })
          : t('messages.toolDisabled', { mode: modeName, tool: toolName })
      );

      
      const { globalEventBus } = await import('@/infrastructure/event-bus');
      globalEventBus.emit('mode:config:updated');
    } catch (error) {
      log.error('Failed to toggle tool', { modeId, toolName, error });
      notification.error(t('messages.toolToggleFailed'));
    }
  };

  const selectAllTools = async (modeId: string) => {
    try {
      const config = getModeConfig(modeId);
      const newTools = availableTools.map(t => t.name);
      const updatedConfig = { ...config, available_tools: newTools };

      if (!await window.confirm(t('messages.confirmSelectAll'))) {
        return;
      }

      await configAPI.setModeConfig(modeId, updatedConfig);

      setModeConfigs(prev => ({
        ...prev,
        [modeId]: updatedConfig
      }));

      notification.success(t('messages.allToolsEnabled'));

      const { globalEventBus } = await import('@/infrastructure/event-bus');
      globalEventBus.emit('mode:config:updated');
    } catch (error) {
      log.error('Failed to select all tools', { modeId, error });
      notification.error(t('messages.toolToggleFailed'));
    }
  };

  const clearAllTools = async (modeId: string) => {
    try {
      const config = getModeConfig(modeId);
      const updatedConfig = { ...config, available_tools: [] };

      if (!await window.confirm(t('messages.confirmClearAll'))) {
        return;
      }

      await configAPI.setModeConfig(modeId, updatedConfig);

      setModeConfigs(prev => ({
        ...prev,
        [modeId]: updatedConfig
      }));

      notification.success(t('messages.allToolsDisabled'));

      const { globalEventBus } = await import('@/infrastructure/event-bus');
      globalEventBus.emit('mode:config:updated');
    } catch (error) {
      log.error('Failed to clear all tools', { modeId, error });
      notification.error(t('messages.toolToggleFailed'));
    }
  };

  const syncTools = async () => {
    try {
      setSyncing(true);

      const { invoke } = await import('@tauri-apps/api/core');
      const report = await invoke<{
        new_tools: string[];
        deleted_tools: string[];
        updated_modes: Array<{
          mode_id: string;
          added_tools: string[];
          removed_tools: string[];
        }>;
      }>('sync_tool_configs');

      
      if (report.new_tools.length > 0 || report.deleted_tools.length > 0) {
        notification.success(
          t('messages.syncComplete', { 
            newTools: report.new_tools.length, 
            deletedTools: report.deleted_tools.length, 
            updatedModes: report.updated_modes.length 
          })
        );
      } else {
        notification.info(t('messages.syncUpToDate'));
      }

      
      await loadData();
    } catch (error) {
      log.error('Failed to sync tool configs', error);
      notification.error(`${t('messages.syncFailed')}: ` + (error instanceof Error ? error.message : String(error)));
    } finally {
      setSyncing(false);
    }
  };

  const resetModeToolsConfig = async (modeId: string) => {
    if (!await window.confirm(t('messages.confirmReset'))) {
      return;
    }

    try {
      
      await configAPI.resetModeConfig(modeId);

      
      const updatedConfigs = await configAPI.getModeConfigs();

      
      setModeConfigs(updatedConfigs);

      notification.success(t('messages.resetSuccess'));

      
      const { globalEventBus } = await import('@/infrastructure/event-bus');
      globalEventBus.emit('mode:config:updated');
    } catch (error) {
      log.error('Failed to reset mode tools config', { modeId, error });
      notification.error(t('messages.resetFailed'));
    }
  };

  const toggleToolsCollapse = (modeId: string) => {
    setCollapsedTools(prev => {
      const currentState = prev[modeId] ?? true; 
      return {
        ...prev,
        [modeId]: !currentState
      };
    });
  };

  
  const getFilteredModes = () => {
    if (activeCategory === 'all') {
      return modes;
    }
    return modes.filter(mode => MODE_CATEGORIES[mode.id] === activeCategory);
  };

  const renderModeSection = (mode: ModeInfo) => {
    const config = getModeConfig(mode.id);
    const selectedTools = config.available_tools || [];
    
    const effectiveTools = selectedTools;
    
    const isCollapsed = collapsedTools[mode.id] ?? true; 

    return (
      <Card key={mode.id} variant="default" padding="none" className="mode-section">
        
        <div className="mode-section__header">
          <h3 className="mode-section__name">
            {t(`mode.names.${mode.id}`, { defaultValue: '' }) || mode.name}
            {mode.is_readonly && <span className="readonly-badge">{t('mode.readonly')}</span>}
          </h3>
          <div className="mode-section__actions">
            <div className="mode-enabled">
              <span className="enabled-label">{t('mode.enabled')}</span>
              <Switch
                checked={config.enabled}
                onChange={(e) => updateModeConfig(mode.id, { enabled: e.target.checked })}
              />
            </div>
          </div>
        </div>

        
        <p className="mode-section__description">{t(`mode.descriptions.${mode.id}`, { defaultValue: '' }) || mode.description}</p>

        
        <div className="mode-section__content">
          
          <div className="config-row config-row--model">
            <span className="config-label__text">{t('model.label')}</span>
            <ModelSelectionRadio
              value={agentModels[mode.id] || 'primary'}
              models={availableModels}
              onChange={(modelId) => {
                handleModeModelChange(mode.id, modelId);
              }}
              layout="horizontal"
              size="small"
            />
          </div>

          
          <div 
            className="config-row config-row--full config-row--clickable"
            onClick={() => toggleToolsCollapse(mode.id)}
            title={isCollapsed ? t('tools.expand') : t('tools.collapse')}
          >
            <div className="config-row__label">
              <div className="config-label">
                <span className="config-label__text">{t('tools.label')}</span>
                <span className="tools-count">
                  {effectiveTools.length}/{availableTools.length}
                </span>
                <div className="config-label__actions">
                  {!isCollapsed && (
                    <>
                      <IconButton
                        onClick={(e) => {
                          e.stopPropagation();
                          selectAllTools(mode.id);
                        }}
                        tooltip={t('tools.selectAll')}
                        size="small"
                      >
                        <CheckSquare size={14} />
                      </IconButton>
                      <IconButton
                        onClick={(e) => {
                          e.stopPropagation();
                          clearAllTools(mode.id);
                        }}
                        tooltip={t('tools.clear')}
                        size="small"
                      >
                        <XSquare size={14} />
                      </IconButton>
                    </>
                  )}
                  <IconButton
                    onClick={(e) => {
                      e.stopPropagation();
                      resetModeToolsConfig(mode.id);
                    }}
                    tooltip={t('tools.reset')}
                    size="small"
                  >
                    <RotateCcw size={14} />
                  </IconButton>
                  <button
                    className="collapse-toggle-icon"
                    onClick={(e) => {
                      e.stopPropagation();
                      toggleToolsCollapse(mode.id);
                    }}
                    title={isCollapsed ? t('tools.expand') : t('tools.collapse')}
                  >
                    {isCollapsed ? <ChevronRight size={16} /> : <ChevronDown size={16} />}
                  </button>
                </div>
              </div>
            </div>
          </div>

          {!isCollapsed && (
            <div className="tools-section">
              <div className="tools-grid">
                
                {[...availableTools]
                  .sort((a, b) => {
                    const aSelected = effectiveTools.includes(a.name);
                    const bSelected = effectiveTools.includes(b.name);
                    if (aSelected && !bSelected) return -1;
                    if (!aSelected && bSelected) return 1;
                    return 0; 
                  })
                  .map(tool => {
                    const isSelected = effectiveTools.includes(tool.name);
                    
                    return (
                      <Card
                        key={tool.name}
                        variant="default"
                        padding="none"
                        interactive
                        className={`tool-item ${isSelected ? 'tool-item--selected' : ''}`}
                        onClick={() => {
                          toggleTool(mode.id, tool.name).catch(err => {
                            log.error('Failed to toggle tool', { modeId: mode.id, toolName: tool.name, error: err });
                          });
                        }}
                        title={tool.description || tool.name}
                      >
                        <span className="tool-item__name">{tool.name}</span>
                        {isSelected && (
                          <span className="tool-item__badge">{t('tools.enabled')}</span>
                        )}
                      </Card>
                    );
                  })}
              </div>
            </div>
          )}
        </div>
        
        
        <div className="mode-section__divider" />
      </Card>
    );
  };

  if (loading) {
    return (
      <ConfigPageLayout>
        <ConfigPageHeader
          title={t('title')}
          subtitle={t('subtitle')}
        />
        <ConfigPageContent>
          <div className="mode-config-loading">
            <CubeLoading size="small" />
            <p>{t('loading.text')}</p>
          </div>
        </ConfigPageContent>
      </ConfigPageLayout>
    );
  }

  return (
    <ConfigPageLayout>
      <ConfigPageHeader
        title={t('title')}
        subtitle={t('subtitle')}
        extra={
          <div className="mode-config__actions">
            <Button
              variant="secondary"
              size="small"
              onClick={syncTools}
              isLoading={syncing}
            >
              {t('actions.syncTools')}
            </Button>
          </div>
        }
      />

      <ConfigPageContent className="mode-config__content">
        
        <div className="mode-config__tabs">
          <button
            className={`mode-config__tab ${activeCategory === 'all' ? 'mode-config__tab--active' : ''}`}
            onClick={() => setActiveCategory('all')}
          >
            {t('tabs.all')}
          </button>
          <button
            className={`mode-config__tab ${activeCategory === 'design' ? 'mode-config__tab--active' : ''}`}
            onClick={() => setActiveCategory('design')}
          >
            {t('tabs.design')}
          </button>
          <button
            className={`mode-config__tab ${activeCategory === 'development' ? 'mode-config__tab--active' : ''}`}
            onClick={() => setActiveCategory('development')}
          >
            {t('tabs.development')}
          </button>
        </div>

        
        <div className="mode-config__tab-content">
          
          <div className="mode-config__sections">
            {getFilteredModes().map(mode => renderModeSection(mode))}
            {getFilteredModes().length === 0 && (
              <div className="mode-config__empty">
                <AlertCircle size={32} />
                <p>{t('empty.noModes')}</p>
              </div>
            )}
          </div>
        </div>
      </ConfigPageContent>
    </ConfigPageLayout>
  );
};

export default ModeConfig;
