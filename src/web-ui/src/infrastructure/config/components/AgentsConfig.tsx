import React, { useState, useEffect, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { Switch, Input, Textarea, Checkbox, Button, IconButton, Tabs, TabPane } from '@/component-library';
import { Trash2, Plus, RefreshCw, X, RotateCcw, ChevronDown, ChevronRight, CheckSquare, XSquare } from 'lucide-react';
import { ConfigPageHeader, ConfigPageLayout, ConfigPageContent, ConfigPageSection, ConfigCollectionItem } from './common';
import { ModelSelectionRadio } from './ModelSelectionRadio';
import { useCurrentWorkspace } from '../../hooks/useWorkspace';
import { useNotification } from '@/shared/notification-system';
import { SubagentAPI, type SubagentInfo, type SubagentLevel } from '../../api/service-api/SubagentAPI';
import { configAPI } from '../../api/service-api/ConfigAPI';
import { configManager } from '../services/ConfigManager';
import type { AIModelConfig, ModeConfigItem } from '../types';
import { createLogger } from '@/shared/utils/logger';
import { isBuiltinSubAgent } from '@/infrastructure/agents/constants';
import './AgentsConfig.scss';

const log = createLogger('AgentsConfig');

interface ModeInfo {
  id: string;
  name: string;
  description: string;
  is_readonly: boolean;
  tool_count: number;
  default_tools?: string[];
}

interface ToolInfo {
  name: string;
  description: string;
  is_readonly: boolean;
}

const NAME_REGEX = /^[a-zA-Z][a-zA-Z0-9_-]*$/;

function getBadgeLabel(agent: SubagentInfo, t: (key: string) => string): string {
  const source = agent.subagentSource;
  if (source === 'builtin' && isBuiltinSubAgent(agent.id)) return t('list.item.subAgent');
  switch (source) {
    case 'builtin': return t('list.item.builtin');
    case 'user': return t('filters.user');
    case 'project': return t('filters.project');
    default: return '';
  }
}

const AgentsConfig: React.FC = () => {
  const { t } = useTranslation('settings/agents');
  const [expandedAgentIds, setExpandedAgentIds] = useState<Set<string>>(new Set());
  const [allSubagents, setAllSubagents] = useState<SubagentInfo[]>([]);
  const [modes, setModes] = useState<ModeInfo[]>([]);
  const [modeConfigs, setModeConfigs] = useState<Record<string, ModeConfigItem>>({});
  const [availableTools, setAvailableTools] = useState<ToolInfo[]>([]);
  const [collapsedModeTools, setCollapsedModeTools] = useState<Record<string, boolean>>({});
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [availableModels, setAvailableModels] = useState<AIModelConfig[]>([]);
  const [agentModels, setAgentModels] = useState<Record<string, string>>({});

  const [showAddForm, setShowAddForm] = useState(false);
  const [customLevelTab, setCustomLevelTab] = useState<SubagentLevel>('user');
  const [formLevel, setFormLevel] = useState<SubagentLevel>('user');
  const [toolNames, setToolNames] = useState<string[]>([]);
  const [formName, setFormName] = useState('');
  const [formDescription, setFormDescription] = useState('');
  const [formPrompt, setFormPrompt] = useState('');
  const [formReadonly, setFormReadonly] = useState(true);
  const [formSelectedTools, setFormSelectedTools] = useState<Set<string>>(new Set());
  const [formNameError, setFormNameError] = useState<string | null>(null);
  const [formSubmitting, setFormSubmitting] = useState(false);

  const { workspacePath, hasWorkspace } = useCurrentWorkspace();
  const notification = useNotification();

  const refreshAllSubagents = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const list = await SubagentAPI.listSubagents();
      setAllSubagents(list);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  }, []);

  const fetchAvailableModes = useCallback(async (): Promise<ModeInfo[]> => {
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      const allModes = await invoke<ModeInfo[]>('get_available_modes');
      return allModes.filter((m) => m.id !== 'agentic');
    } catch (err) {
      log.error('Failed to fetch modes', err);
      return [];
    }
  }, []);

  const fetchAvailableTools = useCallback(async (): Promise<ToolInfo[]> => {
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      return await invoke<ToolInfo[]>('get_all_tools_info');
    } catch (err) {
      log.error('Failed to fetch tools', err);
      return [];
    }
  }, []);

  useEffect(() => {
    const load = async () => {
      setLoading(true);
      setError(null);
      try {
        const [agents, allModels, agentModelsData, modesData, configsData, toolsData] = await Promise.all([
          SubagentAPI.listSubagents(),
          configManager.getConfig<AIModelConfig[]>('ai.models') || [],
          configManager.getConfig<Record<string, string>>('ai.agent_models') || {},
          fetchAvailableModes(),
          configAPI.getModeConfigs(),
          fetchAvailableTools(),
        ]);
        setAllSubagents(agents);
        setAvailableModels(allModels);
        setAgentModels(agentModelsData);
        setModes(modesData);
        setModeConfigs(configsData || {});
        setAvailableTools(toolsData);
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err));
      } finally {
        setLoading(false);
      }
    };
    load();
  }, [fetchAvailableModes, fetchAvailableTools]);

  useEffect(() => {
    if (hasWorkspace && workspacePath) refreshAllSubagents();
  }, [hasWorkspace, workspacePath, refreshAllSubagents]);

  useEffect(() => {
    if (showAddForm && toolNames.length === 0) {
      SubagentAPI.listAgentToolNames().then(setToolNames).catch(() => setToolNames([]));
    }
  }, [showAddForm, toolNames.length]);

  const validateName = useCallback((name: string): string | null => {
    const s = name.trim();
    if (!s) return t('messages.nameRequired');
    if (!NAME_REGEX.test(s)) return t('messages.nameFormatError');
    return null;
  }, [t]);

  const handleFormNameChange = useCallback((val: string) => {
    setFormName(val);
    setFormNameError(validateName(val) || null);
  }, [validateName]);

  const handleFormNameBlur = useCallback(() => {
    if (formName.trim()) setFormNameError(validateName(formName) || null);
  }, [formName, validateName]);

  const toggleFormTool = useCallback((tool: string) => {
    setFormSelectedTools(prev => {
      const next = new Set(prev);
      if (next.has(tool)) next.delete(tool);
      else next.add(tool);
      return next;
    });
  }, []);

  const resetAddForm = useCallback(() => {
    setFormLevel('user');
    setFormName('');
    setFormDescription('');
    setFormPrompt('');
    setFormReadonly(true);
    setFormSelectedTools(new Set());
    setFormNameError(null);
    setShowAddForm(false);
  }, []);

  const handleCreateSubmit = useCallback(async () => {
    const name = formName.trim();
    const desc = formDescription.trim();
    const prompt = formPrompt.trim();
    const nameErr = validateName(name);
    if (nameErr) { setFormNameError(nameErr); return; }
    if (!desc) { notification.error(t('messages.descriptionRequired')); return; }
    if (!prompt) { notification.error(t('messages.contentRequired')); return; }
    setFormSubmitting(true);
    try {
      await SubagentAPI.createSubagent({
        level: formLevel,
        name,
        description: desc,
        prompt,
        readonly: formReadonly,
        tools: formSelectedTools.size > 0 ? Array.from(formSelectedTools) : undefined,
      });
      notification.success(t('messages.createSuccess', { name }));
      resetAddForm();
      await refreshAllSubagents();
    } catch (err) {
      notification.error(t('messages.operationFailed', { operation: t('messages.create'), error: err instanceof Error ? err.message : String(err) }));
    } finally {
      setFormSubmitting(false);
    }
  }, [formLevel, formName, formDescription, formPrompt, formReadonly, formSelectedTools, validateName, notification, t, resetAddForm, refreshAllSubagents]);

  const getAgentModel = useCallback((agent: SubagentInfo): string => {
    const isCustom = agent.subagentSource === 'user' || agent.subagentSource === 'project';
    return isCustom ? (agent.model || 'primary') : (agentModels[agent.id] || 'primary');
  }, [agentModels]);

  const enabledModels = availableModels.filter(m => m.enabled);

  const handleAgentModelChange = async (agent: SubagentInfo, modelId: string) => {
    try {
      const isCustom = agent.subagentSource === 'user' || agent.subagentSource === 'project';
      if (isCustom) {
        await SubagentAPI.updateSubagentConfig({ subagentId: agent.id, model: modelId });
        await refreshAllSubagents();
      } else {
        const current = await configManager.getConfig<Record<string, string>>('ai.agent_models') || {};
        const updated = { ...current, [agent.id]: modelId };
        await configManager.setConfig('ai.agent_models', updated);
        setAgentModels(updated);
      }
      let modelName = modelId;
      if (modelId === 'primary') modelName = t('model.primary');
      else if (modelId === 'fast') modelName = t('model.fast');
      notification.success(t('messages.modelUpdated', { name: agent.name, model: modelName }), { duration: 2000 });
    } catch {
      notification.error(t('messages.modelUpdateFailed'));
    }
  };

  const handleToggle = useCallback(async (agent: SubagentInfo) => {
    const newEnabled = !agent.enabled;
    const isCustom = agent.subagentSource === 'user' || agent.subagentSource === 'project';
    try {
      if (isCustom) {
        await SubagentAPI.updateSubagentConfig({ subagentId: agent.id, enabled: newEnabled });
      } else {
        await configAPI.setSubagentConfig(agent.id, newEnabled);
      }
      await refreshAllSubagents();
      notification.success(t('messages.toggleSuccess', { name: agent.name, status: newEnabled ? t('messages.enabled') : t('messages.disabled') }));
    } catch (err) {
      notification.error(t('messages.toggleFailed', { error: err instanceof Error ? err.message : String(err) }));
    }
  }, [refreshAllSubagents, notification, t]);

  const handleDelete = useCallback(async (agent: SubagentInfo, e: React.MouseEvent) => {
    e.stopPropagation();
    const confirmed = await window.confirm(t('messages.confirmDeleteSubagent', { name: agent.name }));
    if (!confirmed) return;
    try {
      await configAPI.deleteSubagent(agent.id);
      await refreshAllSubagents();
      notification.success(t('messages.deleteSuccess', { name: agent.name }));
    } catch (err) {
      notification.error(t('messages.deleteFailed', { error: err instanceof Error ? err.message : String(err) }));
    }
  }, [refreshAllSubagents, notification, t]);

  const getModeConfig = useCallback((modeId: string): ModeConfigItem => {
    const userConfig = modeConfigs[modeId];
    const mode = modes.find((m) => m.id === modeId);
    if (!userConfig) {
      return {
        mode_id: modeId,
        available_tools: mode?.default_tools || [],
        enabled: true,
        default_tools: mode?.default_tools || [],
      };
    }
    if (!userConfig.available_tools || userConfig.available_tools.length === 0) {
      return {
        ...userConfig,
        available_tools: mode?.default_tools || [],
        default_tools: mode?.default_tools || [],
      };
    }
    return {
      ...userConfig,
      default_tools: mode?.default_tools || userConfig.default_tools || [],
    };
  }, [modeConfigs, modes]);

  const updateModeConfig = useCallback(async (modeId: string, updates: Partial<ModeConfigItem>) => {
    try {
      const config = getModeConfig(modeId);
      const updatedConfig = { ...config, ...updates };
      await configAPI.setModeConfig(modeId, updatedConfig);
      setModeConfigs((prev) => ({ ...prev, [modeId]: updatedConfig }));
      const { globalEventBus } = await import('@/infrastructure/event-bus');
      globalEventBus.emit('mode:config:updated');
    } catch (err) {
      log.error('Failed to update mode config', { modeId, err });
      notification.error(t('messages.saveFailed'));
    }
  }, [getModeConfig, notification, t]);

  const handleModeModelChange = useCallback(async (modeId: string, modelId: string) => {
    try {
      const current = await configManager.getConfig<Record<string, string>>('ai.agent_models') || {};
      const updated = { ...current, [modeId]: modelId };
      await configManager.setConfig('ai.agent_models', updated);
      setAgentModels(updated);
      let modelName = modelId;
      if (modelId === 'primary') modelName = t('model.primary');
      else if (modelId === 'fast') modelName = t('model.fast');
      const modeName = modes.find((m) => m.id === modeId)?.name || modeId;
      notification.success(t('mode.messages.modelUpdated', { name: modeName, model: modelName }), { duration: 2000 });
      const { globalEventBus } = await import('@/infrastructure/event-bus');
      globalEventBus.emit('mode:config:updated');
    } catch {
      notification.error(t('mode.messages.modelUpdateFailed'));
    }
  }, [modes, notification, t]);

  const toggleModeTool = useCallback(async (modeId: string, toolName: string) => {
    try {
      const config = getModeConfig(modeId);
      const tools = config.available_tools || [];
      const isEnabling = !tools.includes(toolName);
      const newTools = isEnabling ? [...tools, toolName] : tools.filter((t) => t !== toolName);
      await updateModeConfig(modeId, { ...config, available_tools: newTools });
      const modeName = modes.find((m) => m.id === modeId)?.name || modeId;
      notification.success(
        isEnabling ? t('mode.messages.toolEnabled', { name: modeName, tool: toolName }) : t('mode.messages.toolDisabled', { name: modeName, tool: toolName })
      );
    } catch {
      notification.error(t('mode.messages.toolToggleFailed'));
    }
  }, [getModeConfig, updateModeConfig, modes, notification, t]);

  const selectAllModeTools = useCallback(async (modeId: string) => {
    if (!(await window.confirm(t('mode.messages.confirmSelectAll')))) return;
    try {
      const config = getModeConfig(modeId);
      await updateModeConfig(modeId, { ...config, available_tools: availableTools.map((t) => t.name) });
      notification.success(t('mode.messages.allToolsEnabled'));
    } catch {
      notification.error(t('mode.messages.toolToggleFailed'));
    }
  }, [getModeConfig, updateModeConfig, availableTools, notification, t]);

  const clearAllModeTools = useCallback(async (modeId: string) => {
    if (!(await window.confirm(t('mode.messages.confirmClearAll')))) return;
    try {
      const config = getModeConfig(modeId);
      await updateModeConfig(modeId, { ...config, available_tools: [] });
      notification.success(t('mode.messages.allToolsDisabled'));
    } catch {
      notification.error(t('mode.messages.toolToggleFailed'));
    }
  }, [getModeConfig, updateModeConfig, notification, t]);

  const resetModeToolsConfig = useCallback(async (modeId: string) => {
    if (!(await window.confirm(t('mode.messages.confirmReset')))) return;
    try {
      await configAPI.resetModeConfig(modeId);
      const updated = await configAPI.getModeConfigs();
      setModeConfigs(updated);
      notification.success(t('mode.messages.resetSuccess'));
      const { globalEventBus } = await import('@/infrastructure/event-bus');
      globalEventBus.emit('mode:config:updated');
    } catch {
      notification.error(t('mode.messages.resetFailed'));
    }
  }, [notification, t]);

  const toggleModeToolsCollapse = useCallback((modeId: string) => {
    setCollapsedModeTools((prev) => ({ ...prev, [modeId]: !(prev[modeId] ?? true) }));
  }, []);

  const toggleAgentExpanded = (agentId: string) => {
    setExpandedAgentIds(prev => {
      const next = new Set(prev);
      if (next.has(agentId)) next.delete(agentId);
      else next.add(agentId);
      return next;
    });
  };

  const renderAddForm = (level: SubagentLevel) => {
    if (!showAddForm || formLevel !== level) return null;
    return (
      <div className="bitfun-collection-form">
        <div className="bitfun-collection-form__header">
          <h3>{t('form.titleCreate')}</h3>
          <IconButton variant="ghost" size="small" onClick={resetAddForm} tooltip={t('form.actions.cancel')}>
            <X size={14} />
          </IconButton>
        </div>
        <div className="bitfun-collection-form__body">
          <div className="bitfun-agents-config__form-group">
            <label>{t('form.fields.name')}</label>
            <Input
              value={formName}
              onChange={(e) => handleFormNameChange(e.target.value)}
              onBlur={handleFormNameBlur}
              placeholder={t('form.fields.namePlaceholder')}
              inputSize="small"
              error={!!formNameError}
            />
            {formNameError && <span className="bitfun-agents-config__form-error">{formNameError}</span>}
          </div>
          <div className="bitfun-agents-config__form-group">
            <label>{t('form.fields.description')}</label>
            <Input
              value={formDescription}
              onChange={(e) => setFormDescription(e.target.value)}
              placeholder={t('form.fields.descriptionPlaceholder')}
              inputSize="small"
            />
          </div>
          <div className="bitfun-agents-config__form-group">
            <label>{t('form.fields.systemPrompt')}</label>
            <Textarea
              value={formPrompt}
              onChange={(e) => setFormPrompt(e.target.value)}
              placeholder={t('form.fields.systemPromptPlaceholder')}
              rows={5}
            />
          </div>
          <div className="bitfun-agents-config__form-group bitfun-agents-config__form-group--row">
            <label>{t('form.fields.readonly')}</label>
            <Switch checked={formReadonly} onChange={(e) => setFormReadonly(e.target.checked)} size="small" />
          </div>
          {formReadonly && (
            <div className="bitfun-agents-config__form-hint">{t('form.fields.readonlyDescription')}</div>
          )}
          <div className="bitfun-agents-config__form-group">
            <label>{t('form.fields.tools')} ({t('form.fields.toolsOptional')})</label>
            <div className="bitfun-agents-config__form-tools">
              {toolNames.map((tool) => (
                <label key={tool} className="bitfun-agents-config__form-tool-check">
                  <Checkbox checked={formSelectedTools.has(tool)} onChange={() => toggleFormTool(tool)} />
                  <span>{tool}</span>
                </label>
              ))}
            </div>
          </div>
        </div>
        <div className="bitfun-collection-form__footer">
          <Button variant="secondary" size="small" onClick={resetAddForm} disabled={formSubmitting}>
            {t('form.actions.cancel')}
          </Button>
          <Button variant="primary" size="small" onClick={handleCreateSubmit} disabled={formSubmitting}>
            {formSubmitting ? '...' : t('form.actions.create')}
          </Button>
        </div>
      </div>
    );
  };

  const renderAgentDetails = (agent: SubagentInfo) => (
    <>
      <div className="bitfun-collection-details__field">{agent.description}</div>
      <div className="bitfun-agents-config__model-row">
        <span className="bitfun-collection-details__label">{t('list.item.modelLabel')}</span>
        <ModelSelectionRadio
          value={getAgentModel(agent)}
          models={enabledModels}
          onChange={(modelId) => handleAgentModelChange(agent, modelId)}
          layout="horizontal"
          size="small"
        />
      </div>
      {agent.defaultTools.length > 0 && (
        <div>
          <div className="bitfun-collection-details__label">{t('list.item.toolsLabel')}</div>
          <div className="bitfun-agents-config__tools-list">
            {agent.defaultTools.map((tool, idx) => (
              <span key={idx} className="bitfun-agents-config__tool-tag">{tool}</span>
            ))}
          </div>
        </div>
      )}
    </>
  );

  const renderModeRow = (mode: ModeInfo) => {
    const config = getModeConfig(mode.id);
    const effectiveTools = config.available_tools || [];
    const isToolsCollapsed = collapsedModeTools[mode.id] ?? true;
    const badge = (
      <>
        <span className="bitfun-agents-config__meta-tag">{t('list.item.mode')}</span>
        <span className="bitfun-agents-config__meta-tag">{t('list.item.toolsCount', { count: effectiveTools.length })}</span>
      </>
    );
    const control = (
      <Switch
        checked={config.enabled}
        onChange={(e) => updateModeConfig(mode.id, { enabled: e.target.checked })}
        size="small"
      />
    );
    const details = (
      <>
        <div className="bitfun-agents-config__model-row">
          <span className="bitfun-collection-details__label">{t('list.item.modelLabel')}</span>
          <ModelSelectionRadio
            value={agentModels[mode.id] || 'primary'}
            models={enabledModels}
            onChange={(modelId) => handleModeModelChange(mode.id, modelId)}
            layout="horizontal"
            size="small"
          />
        </div>
        <div className="bitfun-agents-config__mode-tools-row">
          <span className="bitfun-collection-details__label">{t('mode.tools.label')}</span>
          <div className="bitfun-agents-config__mode-tools-actions">
            {!isToolsCollapsed && (
              <>
                <IconButton size="small" onClick={() => selectAllModeTools(mode.id)} tooltip={t('mode.tools.selectAll')}>
                  <CheckSquare size={14} />
                </IconButton>
                <IconButton size="small" onClick={() => clearAllModeTools(mode.id)} tooltip={t('mode.tools.clear')}>
                  <XSquare size={14} />
                </IconButton>
              </>
            )}
            <IconButton size="small" onClick={() => resetModeToolsConfig(mode.id)} tooltip={t('mode.tools.reset')}>
              <RotateCcw size={14} />
            </IconButton>
            <IconButton
              size="small"
              onClick={() => toggleModeToolsCollapse(mode.id)}
              tooltip={isToolsCollapsed ? t('mode.tools.expand') : t('mode.tools.collapse')}
            >
              {isToolsCollapsed ? <ChevronRight size={14} /> : <ChevronDown size={14} />}
            </IconButton>
          </div>
        </div>
        {!isToolsCollapsed && (
          <div className="bitfun-agents-config__tools-panel">
            {[...availableTools]
              .sort((a, b) => {
                const aSel = effectiveTools.includes(a.name);
                const bSel = effectiveTools.includes(b.name);
                if (aSel && !bSel) return -1;
                if (!aSel && bSel) return 1;
                return 0;
              })
              .map((tool) => {
                const isSelected = effectiveTools.includes(tool.name);
                return (
                  <button
                    key={tool.name}
                    type="button"
                    className={`bitfun-agents-config__tool-item ${isSelected ? 'bitfun-agents-config__tool-item--selected' : ''}`}
                    onClick={() => toggleModeTool(mode.id, tool.name)}
                    title={tool.description || tool.name}
                  >
                    <span className="bitfun-agents-config__tool-name">{tool.name}</span>
                    {isSelected && <span className="bitfun-agents-config__tool-badge">{t('mode.tools.enabled')}</span>}
                  </button>
                );
              })}
          </div>
        )}
      </>
    );
    return (
      <ConfigCollectionItem
        key={`mode-${mode.id}`}
        label={mode.name}
        badge={badge}
        control={control}
        details={details}
        disabled={!config.enabled}
        expanded={expandedAgentIds.has(mode.id)}
        onToggle={() => toggleAgentExpanded(mode.id)}
      />
    );
  };

  const renderAgentRow = (agent: SubagentInfo) => {
    const isBuiltin = agent.subagentSource === 'builtin';
    const toolCount = agent.toolCount ?? agent.defaultTools.length;
    const badge = (
      <>
        <span className="bitfun-agents-config__meta-tag">{getBadgeLabel(agent, t)}</span>
        <span className="bitfun-agents-config__meta-tag">{t('list.item.toolsCount', { count: toolCount })}</span>
      </>
    );
    const control = (
      <>
        <Switch
          checked={agent.enabled}
          onChange={(e) => { e.stopPropagation(); handleToggle(agent); }}
          size="small"
        />
        {!isBuiltin && (
          <button
            type="button"
            className="bitfun-collection-btn bitfun-collection-btn--danger"
            onClick={(e) => handleDelete(agent, e)}
            title={t('list.item.deleteTooltip')}
          >
            <Trash2 size={14} />
          </button>
        )}
      </>
    );
    return (
      <ConfigCollectionItem
        key={agent.id}
        label={agent.name}
        badge={badge}
        control={control}
        details={renderAgentDetails(agent)}
        disabled={!agent.enabled}
        expanded={expandedAgentIds.has(agent.id)}
        onToggle={() => toggleAgentExpanded(agent.id)}
      />
    );
  };

  const refreshExtra = (
    <IconButton
      variant="ghost"
      size="small"
      onClick={async () => {
        try {
          await SubagentAPI.reloadSubagents();
          await refreshAllSubagents();
          notification.success(t('toolbar.refreshSuccess'));
        } catch (err) {
          notification.error(err instanceof Error ? err.message : String(err));
        }
      }}
      tooltip={t('toolbar.refreshTooltip')}
    >
      <RefreshCw size={16} />
    </IconButton>
  );

  const customAgentsExtra = (
    <>
      {refreshExtra}
      <IconButton
        variant="primary"
        size="small"
        onClick={() => { setFormLevel(customLevelTab); setShowAddForm(true); }}
        tooltip={t('toolbar.addTooltip')}
        disabled={customLevelTab === 'project' && !hasWorkspace}
      >
        <Plus size={16} />
      </IconButton>
    </>
  );

  if (loading) {
    return (
      <ConfigPageLayout className="bitfun-agents-config">
        <ConfigPageHeader title={t('title')} subtitle={t('subtitle')} />
        <ConfigPageContent>
          <div className="bitfun-collection-empty"><p>{t('list.loading')}</p></div>
        </ConfigPageContent>
      </ConfigPageLayout>
    );
  }

  if (error) {
    return (
      <ConfigPageLayout className="bitfun-agents-config">
        <ConfigPageHeader title={t('title')} subtitle={t('subtitle')} />
        <ConfigPageContent>
          <div className="bitfun-collection-empty"><p>{t('list.errorPrefix')}{error}</p></div>
        </ConfigPageContent>
      </ConfigPageLayout>
    );
  }

  const builtinAgents = allSubagents.filter(a => a.subagentSource === 'builtin');
  const userAgents = allSubagents.filter(a => a.subagentSource === 'user');
  const projectAgents = allSubagents.filter(a => a.subagentSource === 'project');

  return (
    <ConfigPageLayout className="bitfun-agents-config">
      <ConfigPageHeader title={t('title')} subtitle={t('subtitle')} />

      <ConfigPageContent>
        <ConfigPageSection
          title={t('filters.builtin', { defaultValue: '内置 Agent' })}
          description={t('section.builtin.description', { defaultValue: '系统内置的 AI 子 Agent 与工作模式。' })}
          extra={refreshExtra}
        >
          {modes.map(renderModeRow)}
          {builtinAgents.length === 0 && modes.length === 0 ? (
            <div className="bitfun-collection-empty" />
          ) : (
            builtinAgents.map(renderAgentRow)
          )}
        </ConfigPageSection>

        <ConfigPageSection
          title={t('section.customAgents.title', { defaultValue: '自定义 Agent' })}
          description={t('section.customAgents.description', { defaultValue: '用户级与项目级 Agent，通过下方标签切换。' })}
          extra={customAgentsExtra}
        >
          <Tabs
            type="line"
            size="small"
            activeKey={customLevelTab}
            onChange={(key) => setCustomLevelTab(key as SubagentLevel)}
            className="bitfun-agents-config__custom-tabs"
          >
            <TabPane tabKey="user" label={t('filters.user', { defaultValue: '用户级' })}>
              {renderAddForm('user')}
              {userAgents.length === 0 && !(showAddForm && formLevel === 'user') ? (
                <div className="bitfun-collection-empty">
                  <Button variant="dashed" size="small" onClick={() => { setFormLevel('user'); setShowAddForm(true); }}>
                    <Plus size={14} />
                    {t('toolbar.addTooltip')}
                  </Button>
                </div>
              ) : (
                userAgents.map(renderAgentRow)
              )}
            </TabPane>
            <TabPane tabKey="project" label={t('filters.project', { defaultValue: '项目级' })}>
              {renderAddForm('project')}
              {projectAgents.length === 0 && !(showAddForm && formLevel === 'project') ? (
                <div className="bitfun-collection-empty">
                  {!hasWorkspace && <p>{t('messages.noWorkspace')}</p>}
                  {hasWorkspace && (
                    <Button variant="dashed" size="small" onClick={() => { setFormLevel('project'); setShowAddForm(true); }}>
                      <Plus size={14} />
                      {t('toolbar.addTooltip')}
                    </Button>
                  )}
                </div>
              ) : (
                projectAgents.map(renderAgentRow)
              )}
            </TabPane>
          </Tabs>
        </ConfigPageSection>
      </ConfigPageContent>
    </ConfigPageLayout>
  );
};

export default AgentsConfig;
