import React, { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { RotateCcw, ListChecks, X } from 'lucide-react';
import { IconButton, Card, Switch } from '@/component-library';
import { notificationService } from '@/shared/notification-system';
import { configAPI } from '@/infrastructure/api';
import type { ModeConfigItem, AIExperienceConfig } from '../types';
import { ConfigPageHeader, ConfigPageLayout, ConfigPageContent } from './common';
import { createLogger } from '@/shared/utils/logger';
import './AgenticModeConfig.scss';

const log = createLogger('AgenticModeConfig');

interface ToolInfo {
  name: string;
  description: string;
  is_readonly: boolean;
}

interface AgenticModeConfigProps {
  embedded?: boolean;
}

export const AgenticModeConfig: React.FC<AgenticModeConfigProps> = ({ embedded = false }) => {
  const { t } = useTranslation('settings/super-agent');
  const [loading, setLoading] = useState(false);
  const [availableTools, setAvailableTools] = useState<ToolInfo[]>([]);
  const [agenticConfig, setAgenticConfig] = useState<ModeConfigItem | null>(null);
  const [visualMode, setVisualMode] = useState(false);
  
  const [toolsCollapsed, setToolsCollapsed] = useState(true);

  useEffect(() => {
    loadData();
  }, []);

  const loadData = async () => {
    try {
      setLoading(true);
      
      const [toolsData, modeConfig, aiExperience] = await Promise.all([
        fetchAvailableTools(),
        configAPI.getModeConfig('agentic'),
        configAPI.getConfig('app.ai_experience') as Promise<AIExperienceConfig | undefined>
      ]);
      
      setAvailableTools(toolsData);
      setAgenticConfig(modeConfig);
      if (aiExperience) {
        setVisualMode(aiExperience.enable_visual_mode ?? false);
      }
      
      log.debug('Data loaded', { toolsCount: toolsData.length, agenticConfig: modeConfig, visualMode: aiExperience?.enable_visual_mode });
    } catch (error) {
      log.error('Failed to load data', { error });
      const errorMsg = error instanceof Error ? error.message : 'Unknown error';
      notificationService.error(`${t('messages.loadFailed')}: ${errorMsg}`, {
        duration: 3000
      });
    } finally {
      setLoading(false);
    }
  };

  const fetchAvailableTools = async (): Promise<ToolInfo[]> => {
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      const tools = await invoke<ToolInfo[]>('get_all_tools_info');
      return tools;
    } catch (error) {
      log.error('Failed to fetch tools list', { error });
      return [];
    }
  };

  const saveToolsConfig = async (newConfig: ModeConfigItem) => {
    try {
      setAgenticConfig(newConfig);
      await configAPI.setModeConfig('agentic', newConfig);
      
      const { globalEventBus } = await import('@/infrastructure/event-bus');
      globalEventBus.emit('mode:config:updated');
      log.debug('Mode config saved and event emitted');
    } catch (error) {
      log.error('Failed to save tools config', { error });
      setAgenticConfig(agenticConfig);
      notificationService.error(`${t('messages.saveFailed')}: ` + (error instanceof Error ? error.message : String(error)));
    }
  };

  const toggleTool = async (toolName: string) => {
    if (!agenticConfig) return;
    
    const tools = agenticConfig.available_tools || [];
    const isEnabling = !tools.includes(toolName);
    
    const newTools = isEnabling
      ? [...tools, toolName]
      : tools.filter(t => t !== toolName);
    
    await saveToolsConfig({ ...agenticConfig, available_tools: newTools });
  };

  const selectAllTools = async () => {
    if (!agenticConfig) return;
    await saveToolsConfig({ ...agenticConfig, available_tools: availableTools.map(t => t.name) });
  };

  const clearAllTools = async () => {
    if (!agenticConfig) return;
    await saveToolsConfig({ ...agenticConfig, available_tools: [] });
  };

  const resetToolsConfig = async () => {
    if (!await window.confirm(t('messages.confirmReset'))) {
      return;
    }

    try {
      setLoading(true);
      await configAPI.resetModeConfig('agentic');
      await loadData();
      notificationService.success(t('messages.resetSuccess'));
    } catch (error) {
      log.error('Failed to reset tools config', { error });
      notificationService.error(t('messages.resetFailed'));
    } finally {
      setLoading(false);
    }
  };

  const handleVisualModeChange = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const checked = e.target.checked;
    setVisualMode(checked);
    try {
      await configAPI.setConfig('app.ai_experience.enable_visual_mode', checked);
      log.debug('Visual mode updated', { enabled: checked });
    } catch (error) {
      log.error('Failed to save visual mode config', { error });
      setVisualMode(!checked);
      notificationService.error(t('messages.saveFailed', 'Failed to save'));
    }
  };

  return (
    <ConfigPageLayout className="agentic-mode-config">
      <ConfigPageHeader
        title={t('tools.title', 'Agentic Tools Configuration')}
        subtitle={t('tools.subtitle', 'Configure available tools for Agentic mode')}
      />

      <ConfigPageContent className="agentic-mode-config__content">
        <div className="agentic-mode-config__preferences-section">
          <div className="agentic-mode-config__preferences-content">
            <div className="agentic-mode-config__preference-item">
              <div className="agentic-mode-config__preference-info">
                <span className="agentic-mode-config__preference-label">
                  {t('preferences.visualMode.label', 'Visual Mode')}
                </span>
                <span className="agentic-mode-config__preference-desc">
                  {t('preferences.visualMode.description', 'Use Mermaid diagrams to visualize complex logic and flows')}
                </span>
              </div>
              <Switch
                checked={visualMode}
                onChange={handleVisualModeChange}
                size="small"
                disabled={loading}
              />
            </div>
          </div>
        </div>

        {agenticConfig && (
          <div className={`agentic-mode-config__tools-section ${!toolsCollapsed ? 'agentic-mode-config__tools-section--expanded' : ''}`}>
            <div 
              className="agentic-mode-config__expand-trigger"
              onClick={() => setToolsCollapsed(!toolsCollapsed)}
              role="button"
              tabIndex={0}
            >
              <span className="agentic-mode-config__expand-line" />
              <span className="agentic-mode-config__expand-text">
                {t('tools.available', 'Available Tools')} ({agenticConfig.available_tools?.length || 0}/{availableTools.length})
              </span>
              <span className="agentic-mode-config__expand-line" />
            </div>
            
            {!toolsCollapsed && (
              <div className="agentic-mode-config__expand-content">
                <div className="agentic-mode-config__tools-actions">
                  <IconButton
                    variant="ghost"
                    size="medium"
                    onClick={selectAllTools}
                    disabled={loading}
                    tooltip={t('tools.selectAll')}
                  >
                    <ListChecks size={18} />
                  </IconButton>
                  <IconButton
                    variant="ghost"
                    size="medium"
                    onClick={clearAllTools}
                    disabled={loading}
                    tooltip={t('tools.clear')}
                  >
                    <X size={18} />
                  </IconButton>
                  <IconButton
                    variant="ghost"
                    size="medium"
                    onClick={resetToolsConfig}
                    disabled={loading}
                    tooltip={t('tools.reset')}
                  >
                    <RotateCcw size={18} />
                  </IconButton>
                </div>

                <div className="agentic-mode-config__tools-grid">
                  {[...availableTools]
                    .sort((a, b) => {
                      const aSelected = agenticConfig.available_tools?.includes(a.name);
                      const bSelected = agenticConfig.available_tools?.includes(b.name);
                      if (aSelected && !bSelected) return -1;
                      if (!aSelected && bSelected) return 1;
                      return 0;
                    })
                    .map(tool => {
                      const isSelected = agenticConfig.available_tools?.includes(tool.name);
                      return (
                        <Card
                          key={tool.name}
                          variant="default"
                          padding="none"
                          interactive
                          className={`agentic-mode-config__tool-item ${isSelected ? 'agentic-mode-config__tool-item--selected' : ''}`}
                          onClick={() => {
                            if (!loading) {
                              toggleTool(tool.name).catch(err => {
                                log.error('Failed to toggle tool', { toolName: tool.name, error: err });
                              });
                            }
                          }}
                          style={{ cursor: loading ? 'default' : 'pointer' }}
                        >
                          <span className="agentic-mode-config__tool-content">
                            <span className="agentic-mode-config__tool-name">{tool.name}</span>
                            {isSelected && (
                              <span className="agentic-mode-config__tool-badge">{t('tools.enabled')}</span>
                            )}
                          </span>
                        </Card>
                      );
                    })}
                </div>
              </div>
            )}
          </div>
        )}
      </ConfigPageContent>
    </ConfigPageLayout>
  );
};

export default AgenticModeConfig;
