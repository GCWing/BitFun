import React, { useCallback, useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  ArrowLeft,
  ChevronDown,
  Cpu,
  Info,
  Plug2,
  RefreshCw,
  Wrench,
  X,
} from 'lucide-react';
import { GalleryZone } from '@/app/components';
import '@/app/components/GalleryLayout/GalleryLayout.scss';
import { Switch } from '@/component-library';
import { configAPI } from '@/infrastructure/api/service-api/ConfigAPI';
import type { AgentProfileConfigItem, ModeSkillInfo } from '@/infrastructure/config/types';
import { MCPAPI, type MCPServerInfo } from '@/infrastructure/api/service-api/MCPAPI';
import { notificationService } from '@/shared/notification-system';
import type { DynamicToolInfo } from '@/shared/types/agent-api';
import { createLogger } from '@/shared/utils/logger';
import { isUserSelectableToolName } from '@/shared/utils/toolVisibility';
import { ModelSelector } from '@/flow_chat/components/ModelSelector';
import { useNurseryStore } from '../nurseryStore';

const log = createLogger('AssistantDefaultsPage');
const ASSISTANT_MODE_ID = 'Claw';

interface ToolInfo {
  name: string;
  description: string;
  is_readonly: boolean;
  dynamic_info?: DynamicToolInfo;
}

type TemplateDetail =
  | { type: 'tool'; tool: ToolInfo; isMcp: boolean }
  | { type: 'mcpServer'; serverId: string }
  | { type: 'skill'; skill: ModeSkillInfo };

function isMcpTool(tool: ToolInfo): boolean {
  return tool.dynamic_info?.providerKind === 'mcp' && Boolean(tool.dynamic_info.mcp);
}

function getMcpServerName(tool: ToolInfo): string {
  return tool.dynamic_info?.mcp?.serverId ?? tool.name;
}

function getMcpShortName(tool: ToolInfo): string {
  return tool.dynamic_info?.mcp?.toolName ?? tool.name;
}

function buildDuplicateSkillNameSet(skills: ModeSkillInfo[]): Set<string> {
  const counts = new Map<string, number>();
  for (const skill of skills) {
    counts.set(skill.name, (counts.get(skill.name) ?? 0) + 1);
  }
  return new Set(
    [...counts.entries()]
      .filter(([, count]) => count > 1)
      .map(([name]) => name),
  );
}

function formatSkillOrigin(skill: ModeSkillInfo): string {
  return `${skill.level}/${skill.sourceSlot}`;
}

function formatSkillDisplayName(skill: ModeSkillInfo, duplicateNames: Set<string>): string {
  if (!duplicateNames.has(skill.name)) {
    return skill.name;
  }
  return `${skill.name} [${formatSkillOrigin(skill)}]`;
}

const AssistantDefaultsPage: React.FC = () => {
  const { t } = useTranslation('scenes/profile');
  const { openGallery } = useNurseryStore();

  const [assistantModeConfig, setAssistantModeConfig] = useState<AgentProfileConfigItem | null>(null);
  const [availableTools, setAvailableTools] = useState<ToolInfo[]>([]);
  const [mcpServers, setMcpServers] = useState<MCPServerInfo[]>([]);
  const [modeSkills, setModeSkills] = useState<ModeSkillInfo[]>([]);
  const [toolsLoading, setToolsLoading] = useState<Record<string, boolean>>({});
  const [skillsLoading, setSkillsLoading] = useState<Record<string, boolean>>({});
  const [loading, setLoading] = useState(true);
  const [collapsedGroups, setCollapsedGroups] = useState<Set<string>>(new Set());
  const [detail, setDetail] = useState<TemplateDetail | null>(null);

  const skillsEnabled = useMemo(
    () => modeSkills.filter((skill) => skill.effectiveEnabled),
    [modeSkills],
  );

  const skillsDisabled = useMemo(
    () => modeSkills.filter((skill) => !skill.effectiveEnabled),
    [modeSkills],
  );
  const duplicateSkillNames = useMemo(
    () => buildDuplicateSkillNameSet(modeSkills),
    [modeSkills],
  );

  const userSelectableTools = useMemo(
    () => availableTools.filter((tool) => isUserSelectableToolName(tool.name)),
    [availableTools],
  );

  // Split tools into built-in vs MCP
  const builtinTools = useMemo(
    () => userSelectableTools.filter((tool) => !isMcpTool(tool)),
    [userSelectableTools],
  );

  const builtinToolsEnabled = useMemo(
    () => builtinTools.filter((tool) => assistantModeConfig?.enabled_tools?.includes(tool.name)),
    [builtinTools, assistantModeConfig],
  );

  const builtinToolsDisabled = useMemo(
    () => builtinTools.filter((tool) => !assistantModeConfig?.enabled_tools?.includes(tool.name)),
    [builtinTools, assistantModeConfig],
  );

  // MCP tools grouped by server id
  const mcpToolsByServer = useMemo(() => {
    const map = new Map<string, ToolInfo[]>();
    for (const tool of userSelectableTools) {
      if (!isMcpTool(tool)) continue;
      const server = getMcpServerName(tool);
      if (!map.has(server)) map.set(server, []);
      map.get(server)!.push(tool);
    }
    return map;
  }, [userSelectableTools]);

  // All known MCP server ids — union of detected tool servers + registered servers
  const mcpServerIds = useMemo(() => {
    const fromTools = new Set(mcpToolsByServer.keys());
    const fromRegistry = new Set(mcpServers.map((s) => s.id));
    return new Set([...fromTools, ...fromRegistry]);
  }, [mcpToolsByServer, mcpServers]);

  useEffect(() => {
    (async () => {
      setLoading(true);
      try {
        const { invoke } = await import('@tauri-apps/api/core');
        const [modeConf, tools, skillList, servers] = await Promise.all([
          configAPI.getAgentProfileConfig(ASSISTANT_MODE_ID).catch(() => null as AgentProfileConfigItem | null),
          invoke<ToolInfo[]>('get_all_tools_info').catch(() => [] as ToolInfo[]),
          configAPI.getModeSkillConfigs({ modeId: ASSISTANT_MODE_ID }).catch(() => [] as ModeSkillInfo[]),
          MCPAPI.getServers().catch(() => [] as MCPServerInfo[]),
        ]);
        setAssistantModeConfig(modeConf);
        setAvailableTools(tools);
        setModeSkills(skillList ?? []);
        setMcpServers(servers ?? []);
      } catch (e) {
        log.error('Failed to load assistant defaults config', e);
      } finally {
        setLoading(false);
      }
    })();
  }, []);

  useEffect(() => {
    if (!detail) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') setDetail(null);
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [detail]);

  const handleToolToggle = useCallback(async (toolName: string) => {
    if (!assistantModeConfig || !isUserSelectableToolName(toolName)) return;
    setToolsLoading((prev) => ({ ...prev, [toolName]: true }));
    const current = assistantModeConfig.enabled_tools ?? [];
    const isEnabled = current.includes(toolName);
    const newTools = isEnabled ? current.filter((n) => n !== toolName) : [...current, toolName];
    const newConfig = { ...assistantModeConfig, enabled_tools: newTools };
    setAssistantModeConfig(newConfig);
    try {
      await configAPI.setAgentProfileConfig(ASSISTANT_MODE_ID, newConfig);
      const { globalEventBus } = await import('@/infrastructure/event-bus');
      globalEventBus.emit('mode:config:updated');
    } catch (e) {
      log.error('Failed to toggle tool', e);
      notificationService.error(t('notifications.toggleFailed'));
      setAssistantModeConfig(assistantModeConfig);
    } finally {
      setToolsLoading((prev) => ({ ...prev, [toolName]: false }));
    }
  }, [assistantModeConfig, t]);

  const handleResetTools = useCallback(async () => {
    try {
      await configAPI.resetAgentProfileConfig(ASSISTANT_MODE_ID);
      const [modeConf, skills] = await Promise.all([
        configAPI.getAgentProfileConfig(ASSISTANT_MODE_ID),
        configAPI.getModeSkillConfigs({ modeId: ASSISTANT_MODE_ID }),
      ]);
      setAssistantModeConfig(modeConf);
      setModeSkills(skills);
      const { globalEventBus } = await import('@/infrastructure/event-bus');
      globalEventBus.emit('mode:config:updated');
      notificationService.success(t('notifications.resetSuccess'));
    } catch (e) {
      log.error('Failed to reset tools', e);
      notificationService.error(t('notifications.resetFailed'));
    }
  }, [t]);

  const handleGroupToggleAll = useCallback(async (toolNames: string[]) => {
    if (!assistantModeConfig) return;
    const selectableToolNames = toolNames.filter(isUserSelectableToolName);
    if (selectableToolNames.length === 0) return;
    const current = assistantModeConfig.enabled_tools ?? [];
    const allEnabled = selectableToolNames.every((n) => current.includes(n));
    const newTools = allEnabled
      ? current.filter((n) => !selectableToolNames.includes(n))
      : [...new Set([...current, ...selectableToolNames])];
    const newConfig = { ...assistantModeConfig, enabled_tools: newTools };
    setAssistantModeConfig(newConfig);
    try {
      await configAPI.setAgentProfileConfig(ASSISTANT_MODE_ID, newConfig);
      const { globalEventBus } = await import('@/infrastructure/event-bus');
      globalEventBus.emit('mode:config:updated');
    } catch (e) {
      log.error('Failed to toggle group', e);
      notificationService.error(t('notifications.toggleFailed'));
      setAssistantModeConfig(assistantModeConfig);
    }
  }, [assistantModeConfig, t]);

  const handleSkillToggle = useCallback(async (skill: ModeSkillInfo) => {
    const loadingKey = skill.key;
    setSkillsLoading((prev) => ({ ...prev, [loadingKey]: true }));
    const nextDisabled = skill.effectiveEnabled;
    try {
      await configAPI.setModeSkillDisabled({
        modeId: ASSISTANT_MODE_ID,
        skillKey: skill.key,
        disabled: nextDisabled,
      });
      const updatedSkills = await configAPI.getModeSkillConfigs({ modeId: ASSISTANT_MODE_ID });
      setModeSkills(updatedSkills);
      const { globalEventBus } = await import('@/infrastructure/event-bus');
      globalEventBus.emit('mode:config:updated');
    } catch (e) {
      log.error('Failed to toggle skill', e);
      notificationService.error(t('notifications.toggleFailed'));
    } finally {
      setSkillsLoading((prev) => ({ ...prev, [loadingKey]: false }));
    }
  }, [t]);

  const toggleCollapse = useCallback((id: string) => {
    setCollapsedGroups((prev) => {
      const next = new Set(prev);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      return next;
    });
  }, []);

  const openToolDetail = useCallback((tool: ToolInfo, isMcp: boolean) => {
    setDetail((prev) => (
      prev?.type === 'tool' && prev.tool.name === tool.name
        ? null
        : { type: 'tool', tool, isMcp }
    ));
  }, []);

  const openSkillDetail = useCallback((skill: ModeSkillInfo) => {
    setDetail((prev) => (
      prev?.type === 'skill' && prev.skill.key === skill.key
        ? null
        : { type: 'skill', skill }
    ));
  }, []);

  // ── Render helpers ───────────────────────────────────────────────────────

  const renderToolList = (tools: ToolInfo[], isMcp: boolean) => (
    <div className="tc-tool-list">
      {tools.map((tool) => {
        const enabled = assistantModeConfig?.enabled_tools?.includes(tool.name) ?? false;
        const displayName = isMcp ? getMcpShortName(tool) : tool.name;
        const selected = detail?.type === 'tool' && detail.tool.name === tool.name;
        return (
          <div
            key={tool.name}
            className={`tc-tool-row${!enabled ? ' tc-tool-row--off' : ''}${selected ? ' tc-tool-row--selected' : ''}`}
          >
            <button
              type="button"
              className="tc-tool-row__hit"
              onClick={() => openToolDetail(tool, isMcp)}
            >
              <span className={`tc-tool-row__icon${isMcp ? ' tc-tool-row__icon--mcp' : ''}`}>
                {isMcp ? <Plug2 size={12} /> : <Wrench size={12} />}
              </span>
              <span className="tc-tool-row__name" title={tool.name}>{displayName}</span>
            </button>
            <Switch
              size="small"
              checked={enabled}
              loading={toolsLoading[tool.name]}
              onChange={() => handleToolToggle(tool.name)}
              aria-label={tool.name}
            />
          </div>
        );
      })}
    </div>
  );

  const renderToolEnabledDisabledSplit = (
    enabledList: ToolInfo[],
    disabledList: ToolInfo[],
    isMcp: boolean,
  ) => (
    <div className="tc-enabled-disabled-split">
      <div className="tc-enabled-disabled-split__col">
        <p className="tc-enabled-disabled-split__title">{t('nursery.template.colEnabled')}</p>
        {enabledList.length > 0 ? (
          renderToolList(enabledList, isMcp)
        ) : (
          <p className="tc-enabled-disabled-split__empty">{t('nursery.template.colEmpty')}</p>
        )}
      </div>
      <div className="tc-enabled-disabled-split__col">
        <p className="tc-enabled-disabled-split__title">{t('nursery.template.colDisabled')}</p>
        {disabledList.length > 0 ? (
          renderToolList(disabledList, isMcp)
        ) : (
          <p className="tc-enabled-disabled-split__empty">{t('nursery.template.colEmpty')}</p>
        )}
      </div>
    </div>
  );

  const renderSkillList = (list: ModeSkillInfo[]) => (
    <div className="tc-skill-list">
      {list.map((skill) => {
        const on = skill.effectiveEnabled;
        const selected = detail?.type === 'skill' && detail.skill.key === skill.key;
        const displayName = formatSkillDisplayName(skill, duplicateSkillNames);
        return (
          <div
            key={skill.key}
            className={`tc-skill-row${!on ? ' tc-skill-row--off' : ''}${selected ? ' tc-skill-row--selected' : ''}`}
          >
            <button
              type="button"
              className="tc-skill-row__hit"
              onClick={() => openSkillDetail(skill)}
            >
              <span className="tc-skill-row__name">{displayName}</span>
              <span className="tc-skill-row__level">{formatSkillOrigin(skill)}</span>
            </button>
            <Switch
              checked={on}
              onChange={() => handleSkillToggle(skill)}
              disabled={skillsLoading[skill.key]}
              size="small"
            />
          </div>
        );
      })}
    </div>
  );

  const renderSkillEnabledDisabledSplit = () => (
    <div className="tc-enabled-disabled-split">
      <div className="tc-enabled-disabled-split__col">
        <p className="tc-enabled-disabled-split__title">{t('nursery.template.colEnabled')}</p>
        {skillsEnabled.length > 0 ? (
          renderSkillList(skillsEnabled)
        ) : (
          <p className="tc-enabled-disabled-split__empty">{t('nursery.template.colEmpty')}</p>
        )}
      </div>
      <div className="tc-enabled-disabled-split__col">
        <p className="tc-enabled-disabled-split__title">{t('nursery.template.colDisabled')}</p>
        {skillsDisabled.length > 0 ? (
          renderSkillList(skillsDisabled)
        ) : (
          <p className="tc-enabled-disabled-split__empty">{t('nursery.template.colEmpty')}</p>
        )}
      </div>
    </div>
  );

  const renderGroupHeader = (
    id: string,
    label: string,
    toolNames: string[],
    isMcp: boolean,
    serverStatus?: string,
    mcpServerId?: string,
  ) => {
      const groupEnabled = toolNames.filter(
      (n) => assistantModeConfig?.enabled_tools?.includes(n),
    ).length;
    const isCollapsed = collapsedGroups.has(id);
    const allOn = toolNames.length > 0 && groupEnabled === toolNames.length;

    return (
      <div className="tc-group-header">
        {toolNames.length > 0 && (
          <button
            type="button"
            className="tc-group-header__toggle"
            onClick={() => toggleCollapse(id)}
          >
            <ChevronDown
              size={13}
              className={`tc-group-header__chevron ${isCollapsed ? 'tc-group-header__chevron--collapsed' : ''}`}
            />
          </button>
        )}
        {isMcp
          ? <Plug2 size={13} className="tc-group-header__icon tc-group-header__icon--mcp" />
          : <Cpu size={13} className="tc-group-header__icon" />
        }
        <span className="tc-group-header__name">{label}</span>
        {serverStatus && (
          <span className={`tc-group-header__status tc-group-header__status--${serverStatus.toLowerCase()}`}>
            {serverStatus}
          </span>
        )}
        <span className="tc-group-header__count">
          {toolNames.length > 0 ? `${groupEnabled}/${toolNames.length}` : t('nursery.template.groupCountEmpty')}
        </span>
        {isMcp && mcpServerId && (
          <button
            type="button"
            className="tc-group-header__detail-btn"
            title={t('nursery.template.openServerDetail')}
            aria-label={t('nursery.template.openServerDetail')}
            onClick={(e) => {
              e.stopPropagation();
              setDetail((prev) => (
                prev?.type === 'mcpServer' && prev.serverId === mcpServerId
                  ? null
                  : { type: 'mcpServer', serverId: mcpServerId }
              ));
            }}
          >
            <Info size={14} />
          </button>
        )}
        {toolNames.length > 0 && (
          <Switch
            size="small"
            checked={allOn}
            onChange={() => handleGroupToggleAll(toolNames)}
            aria-label={`Toggle all in ${label}`}
          />
        )}
      </div>
    );
  };

  const renderDetailPanel = () => {
    if (!detail) return null;

    if (detail.type === 'tool') {
      const { tool, isMcp } = detail;
      const displayName = isMcp ? getMcpShortName(tool) : tool.name;
      const enabled = assistantModeConfig?.enabled_tools?.includes(tool.name) ?? false;
      return (
        <aside className="tc-template-detail" aria-label={t('nursery.template.detailPanel')}>
          <div className="tc-template-detail__head tc-template-detail__head--center-line">
            <span className="tc-template-detail__head-spacer" aria-hidden />
            <div className="tc-template-detail__head-text">
              <div className="tc-template-detail__head-line">
                <span className="tc-template-detail__kind">
                  {isMcp ? t('nursery.template.toolTypeMcp') : t('nursery.template.toolTypeBuiltin')}
                </span>
                <h3 className="tc-template-detail__title">{displayName}</h3>
              </div>
            </div>
            <button
              type="button"
              className="tc-template-detail__close"
              onClick={() => setDetail(null)}
              aria-label={t('nursery.template.closeDetail')}
            >
              <X size={14} strokeWidth={2} />
            </button>
          </div>
          <div className="tc-template-detail__body">
            {tool.is_readonly && (
              <span className="tc-template-detail__badge">{t('nursery.template.readonlyTool')}</span>
            )}
            <p className="tc-template-detail__desc">
              {tool.description?.trim() ? tool.description : '—'}
            </p>
            <div className="tc-template-detail__actions">
              <Switch
                size="small"
                checked={enabled}
                loading={toolsLoading[tool.name]}
                onChange={() => handleToolToggle(tool.name)}
                aria-label={tool.name}
              />
            </div>
          </div>
        </aside>
      );
    }

    if (detail.type === 'mcpServer') {
      const { serverId } = detail;
      const serverInfo = mcpServers.find((s) => s.id === serverId);
      const serverTools = mcpToolsByServer.get(serverId) ?? [];
      const status = serverInfo?.status ?? (serverTools.length > 0 ? 'Connected' : 'Unknown');
      return (
        <aside className="tc-template-detail" aria-label={t('nursery.template.detailPanel')}>
          <div className="tc-template-detail__head tc-template-detail__head--center-line">
            <span className="tc-template-detail__head-spacer" aria-hidden />
            <div className="tc-template-detail__head-text">
              <div className="tc-template-detail__head-line">
                <span className="tc-template-detail__kind">MCP</span>
                <h3 className="tc-template-detail__title">{serverInfo?.name ?? serverId}</h3>
              </div>
            </div>
            <button
              type="button"
              className="tc-template-detail__close"
              onClick={() => setDetail(null)}
              aria-label={t('nursery.template.closeDetail')}
            >
              <X size={14} strokeWidth={2} />
            </button>
          </div>
          <div className="tc-template-detail__body">
            <span className={`tc-template-detail__status tc-group-header__status tc-group-header__status--${status.toLowerCase()}`}>
              {status}
            </span>
            <p className="tc-template-detail__subhead">{t('nursery.template.serverToolsHeading')}</p>
            {serverTools.length === 0 ? (
              <p className="nursery-empty">{t('nursery.template.mcpServerNoTools')}</p>
            ) : (
              <ul className="tc-template-detail__tool-names">
                {serverTools.map((tool) => (
                  <li key={tool.name}>{getMcpShortName(tool)}</li>
                ))}
              </ul>
            )}
          </div>
        </aside>
      );
    }

    const { skill } = detail;
    const on = skill.effectiveEnabled;
    return (
      <aside className="tc-template-detail" aria-label={t('nursery.template.detailPanel')}>
        <div className="tc-template-detail__head tc-template-detail__head--center-line">
          <span className="tc-template-detail__head-spacer" aria-hidden />
          <div className="tc-template-detail__head-text">
            <div className="tc-template-detail__head-line">
              <span className="tc-template-detail__kind">{t('cards.skills')}</span>
              <h3 className="tc-template-detail__title">{formatSkillDisplayName(skill, duplicateSkillNames)}</h3>
            </div>
          </div>
          <button
            type="button"
            className="tc-template-detail__close"
            onClick={() => setDetail(null)}
            aria-label={t('nursery.template.closeDetail')}
          >
            <X size={14} strokeWidth={2} />
          </button>
        </div>
        <div className="tc-template-detail__body">
          <p className="tc-template-detail__meta">{t('nursery.template.skillLevel', { level: formatSkillOrigin(skill) })}</p>
          <p className="tc-template-detail__desc">
            {skill.description?.trim() ? skill.description : '—'}
          </p>
          <div className="tc-template-detail__actions">
            <Switch
              checked={on}
              onChange={() => handleSkillToggle(skill)}
              disabled={skillsLoading[skill.key]}
              size="small"
            />
          </div>
        </div>
      </aside>
    );
  };

  return (
    <div className="nursery-page">
      <div className="nursery-page__bar">
        <button
          type="button"
          className="nursery-page__back"
          onClick={openGallery}
          aria-label={t('nursery.backToGallery')}
        >
          <ArrowLeft size={13} />
        </button>
      </div>

      <div className="nursery-page__content">
        {loading ? (
          <div className="nursery-page__loading">
            <RefreshCw size={16} className="nursery-spinning" />
          </div>
        ) : (
          <div className={`tc-template-shell${detail ? ' tc-template-shell--has-detail' : ''}`}>
            <div className="tc-template-shell__main">
            <div className="tc-template-main-column">
            <div className="gallery-page-header tc-template-page-header">
              <div className="gallery-page-header__identity">
                <h2 className="gallery-page-header__title">{t('nursery.template.title')}</h2>
                <div className="gallery-page-header__subtitle">{t('nursery.template.subtitle')}</div>
              </div>
            </div>

            <div className="gallery-zones tc-template-shell__zones">
            <GalleryZone
              className="tc-template-model-zone"
              title={t('cards.model')}
              subtitle={t('nursery.template.sectionModelsSubtitle')}
              tools={(
                <div className="tc-model-slot tc-model-slot--header">
                  <div className="tc-model-slot__select tc-model-slot__select--model-selector">
                    <ModelSelector
                      currentMode={ASSISTANT_MODE_ID}
                      className="tc-model-slot__selector"
                      dropdownPlacement="bottom"
                    />
                  </div>
                </div>
              )}
            >
              {null}
            </GalleryZone>

            <GalleryZone
              title={t('cards.skills')}
            >
              {modeSkills.length === 0 ? (
                <p className="nursery-empty">{t('empty.skills')}</p>
              ) : (
                renderSkillEnabledDisabledSplit()
              )}
            </GalleryZone>

            <GalleryZone
              title={t('nursery.template.builtinToolsSection')}
              tools={(
                <button
                  type="button"
                  className="gallery-plain-icon-btn"
                  onClick={handleResetTools}
                  title={t('actions.reset')}
                  aria-label={t('actions.reset')}
                >
                  <RefreshCw size={14} />
                </button>
              )}
            >
              {builtinTools.length === 0 ? (
                <p className="nursery-empty">{t('empty.tools')}</p>
              ) : (
                renderToolEnabledDisabledSplit(builtinToolsEnabled, builtinToolsDisabled, false)
              )}
            </GalleryZone>

            <GalleryZone
              title={t('nursery.template.mcpToolsSection')}
            >
              {mcpServerIds.size === 0 ? (
                <div className="tc-mcp-empty">
                  <Plug2 size={20} className="tc-mcp-empty__icon" />
                  <span className="tc-mcp-empty__text">{t('nursery.template.mcpEmptyTitle')}</span>
                  <span className="tc-mcp-empty__hint">{t('nursery.template.mcpEmptyHint')}</span>
                </div>
              ) : (
                <div className="tc-tool-groups">
                  {[...mcpServerIds].map((serverId) => {
                    const serverTools = mcpToolsByServer.get(serverId) ?? [];
                    const serverInfo = mcpServers.find((s) => s.id === serverId);
                    const status = serverInfo?.status ?? (serverTools.length > 0 ? 'Connected' : 'Unknown');
                    const groupId = `mcp_${serverId}`;
                    const mcpEnabled = serverTools.filter((tool) => assistantModeConfig?.enabled_tools?.includes(tool.name));
                    const mcpDisabled = serverTools.filter((tool) => !assistantModeConfig?.enabled_tools?.includes(tool.name));

                    return (
                      <div key={serverId} className="tc-tool-block">
                        {renderGroupHeader(
                          groupId,
                          serverInfo?.name ?? serverId,
                          serverTools.map((tool) => tool.name),
                          true,
                          status,
                          serverId,
                        )}
                        {!collapsedGroups.has(groupId) && serverTools.length > 0
                          && renderToolEnabledDisabledSplit(mcpEnabled, mcpDisabled, true)}
                        {!collapsedGroups.has(groupId) && serverTools.length === 0 && (
                          <p className="tc-tool-block__empty">{t('nursery.template.mcpServerNoTools')}</p>
                        )}
                      </div>
                    );
                  })}
                </div>
              )}
            </GalleryZone>
            </div>
            </div>
            </div>
            {renderDetailPanel()}
          </div>
        )}
      </div>
    </div>
  );
};

export default AssistantDefaultsPage;
