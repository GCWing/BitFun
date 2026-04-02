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
import { Select, Switch, type SelectOption } from '@/component-library';
import { configAPI } from '@/infrastructure/api/service-api/ConfigAPI';
import { configManager } from '@/infrastructure/config/services/ConfigManager';
import type { AIModelConfig, ModeConfigItem, SkillInfo } from '@/infrastructure/config/types';
import { MCPAPI, type MCPServerInfo } from '@/infrastructure/api/service-api/MCPAPI';
import { notificationService } from '@/shared/notification-system';
import { createLogger } from '@/shared/utils/logger';
import { useNurseryStore } from '../nurseryStore';
import { formatTokenCount } from './useTokenEstimate';

const log = createLogger('TemplateConfigPage');

interface ToolInfo { name: string; description: string; is_readonly: boolean; }

type TemplateDetail =
  | { type: 'tool'; tool: ToolInfo; isMcp: boolean }
  | { type: 'mcpServer'; serverId: string }
  | { type: 'skill'; skill: SkillInfo };

type ModelSlot = 'primary' | 'fast';

// MCP tools are registered as "mcp_{server_id}_{tool_name}" (single underscores)
function isMcpTool(name: string): boolean {
  return name.startsWith('mcp_');
}

// Extract server id: "mcp_github_create_issue" → "github"
function getMcpServerName(toolName: string): string {
  return toolName.split('_')[1] ?? toolName;
}

// Short display name: "mcp_github_create_issue" → "create_issue"
function getMcpShortName(toolName: string): string {
  const parts = toolName.split('_');
  return parts.slice(2).join('_') || toolName;
}

type CtxSegKey = 'systemPrompt' | 'toolInjection' | 'rules' | 'memories';

const CTX_SEGMENT_ORDER: readonly CtxSegKey[] = ['systemPrompt', 'toolInjection', 'rules', 'memories'];

const CTX_SEGMENT_COLORS: Record<CtxSegKey, string> = {
  systemPrompt: '#34d399',
  toolInjection: '#60a5fa',
  rules: '#a78bfa',
  memories: '#f472b6',
};

const CTX_LABEL_I18N_KEY: Record<CtxSegKey, string> = {
  systemPrompt: 'nursery.template.tokenSystemPrompt',
  toolInjection: 'nursery.template.tokenToolInjection',
  rules: 'nursery.template.tokenRules',
  memories: 'nursery.template.tokenMemories',
};

function fmtPct(val: number, total: number): string {
  if (total === 0) return '0%';
  return `${Math.round((val / total) * 100)}%`;
}

// ── Claw agent token estimates (based on actual prompt files) ─────────────
// claw_mode.md ≈ 838 tok + persona files (BOOTSTRAP/SOUL/USER/IDENTITY) ≈ 600 tok
const CLAW_SYS_TOKENS = 1438;
const TOKENS_PER_TOOL = 45;   // matches backend estimation
const TOKENS_PER_RULE = 80;
const TOKENS_PER_MEMORY = 60;
const CTX_WINDOW = 128_000;

interface MockBreakdown {
  systemPrompt: number;
  toolInjection: number;
  rules: number;
  memories: number;
  total: number;
}

function buildMockBreakdown(
  toolCount: number,
  rulesCount: number,
  memoriesCount: number,
): MockBreakdown {
  const systemPrompt = CLAW_SYS_TOKENS;
  const toolInjection = toolCount * TOKENS_PER_TOOL;
  const rules = rulesCount * TOKENS_PER_RULE;
  const memories = memoriesCount * TOKENS_PER_MEMORY;
  return { systemPrompt, toolInjection, rules, memories, total: systemPrompt + toolInjection + rules + memories };
}

const TemplateConfigPage: React.FC = () => {
  const { t } = useTranslation('scenes/profile');
  const { openGallery } = useNurseryStore();

  const [models, setModels] = useState<AIModelConfig[]>([]);
  const [funcAgentModels, setFuncAgentModels] = useState<Record<string, string>>({});
  const [agenticConfig, setAgenticConfig] = useState<ModeConfigItem | null>(null);
  const [availableTools, setAvailableTools] = useState<ToolInfo[]>([]);
  const [mcpServers, setMcpServers] = useState<MCPServerInfo[]>([]);
  const [skills, setSkills] = useState<SkillInfo[]>([]);
  const [toolsLoading, setToolsLoading] = useState<Record<string, boolean>>({});
  const [skillsLoading, setSkillsLoading] = useState<Record<string, boolean>>({});
  const [loading, setLoading] = useState(true);
  const [collapsedGroups, setCollapsedGroups] = useState<Set<string>>(new Set());
  const [detail, setDetail] = useState<TemplateDetail | null>(null);

  const enabledToolCount = useMemo(
    () => agenticConfig?.enabled_tools?.length ?? 0,
    [agenticConfig],
  );

  // Whether a skill is enabled in this template:
  // - If available_skills is undefined, fall back to the skill's global enabled state
  // - Otherwise check if the skill name appears in the list
  const isSkillEnabled = useCallback(
    (skillName: string): boolean => {
      if (agenticConfig?.available_skills == null) {
        return skills.find((s) => s.name === skillName)?.enabled ?? true;
      }
      return agenticConfig.available_skills.includes(skillName);
    },
    [agenticConfig, skills],
  );

  const skillsEnabled = useMemo(
    () => skills.filter((s) => isSkillEnabled(s.name)),
    [skills, isSkillEnabled],
  );

  const skillsDisabled = useMemo(
    () => skills.filter((s) => !isSkillEnabled(s.name)),
    [skills, isSkillEnabled],
  );

  const tokenBreakdown = useMemo(
    () => buildMockBreakdown(enabledToolCount, 0, 0),
    [enabledToolCount],
  );

  const ctxSegments = useMemo(
    () => CTX_SEGMENT_ORDER.map((key) => ({
      key,
      color: CTX_SEGMENT_COLORS[key],
      label: t(CTX_LABEL_I18N_KEY[key]),
    })),
    [t],
  );

  // Split tools into built-in vs MCP
  const builtinTools = useMemo(
    () => availableTools.filter((t) => !isMcpTool(t.name)),
    [availableTools],
  );

  const builtinToolsEnabled = useMemo(
    () => builtinTools.filter((tool) => agenticConfig?.enabled_tools?.includes(tool.name)),
    [builtinTools, agenticConfig],
  );

  const builtinToolsDisabled = useMemo(
    () => builtinTools.filter((tool) => !agenticConfig?.enabled_tools?.includes(tool.name)),
    [builtinTools, agenticConfig],
  );

  // MCP tools grouped by server id
  const mcpToolsByServer = useMemo(() => {
    const map = new Map<string, ToolInfo[]>();
    for (const tool of availableTools) {
      if (!isMcpTool(tool.name)) continue;
      const server = getMcpServerName(tool.name);
      if (!map.has(server)) map.set(server, []);
      map.get(server)!.push(tool);
    }
    return map;
  }, [availableTools]);

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
        const [allModels, funcModels, modeConf, tools, skillList, servers] = await Promise.all([
          configManager.getConfig<AIModelConfig[]>('ai.models').catch(() => [] as AIModelConfig[]),
          configManager.getConfig<Record<string, string>>('ai.func_agent_models').catch(() => ({} as Record<string, string>)),
          configAPI.getModeConfig('agentic').catch(() => null as ModeConfigItem | null),
          invoke<ToolInfo[]>('get_all_tools_info').catch(() => [] as ToolInfo[]),
          configAPI.getSkillConfigs({}).catch(() => [] as SkillInfo[]),
          MCPAPI.getServers().catch(() => [] as MCPServerInfo[]),
        ]);
        setModels(allModels ?? []);
        setFuncAgentModels(funcModels ?? {});
        setAgenticConfig(modeConf);
        setAvailableTools(tools);
        setSkills(skillList ?? []);
        setMcpServers(servers ?? []);
      } catch (e) {
        log.error('Failed to load template config', e);
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

  const buildModelOptions = useCallback((slot: ModelSlot): SelectOption[] => {
    const presets: SelectOption[] = [
      { value: 'preset:primary', label: t('slotDefault.primary'), group: t('modelGroups.presets') },
      { value: 'preset:fast',    label: t('slotDefault.fast'),    group: t('modelGroups.presets') },
    ];
    const modelOptions: SelectOption[] = models
      .filter((m) => m.enabled && !!m.id)
      .map((m) => ({ value: `model:${m.id}`, label: m.name, group: t('modelGroups.models') }));
    if (slot === 'fast') return [...presets, ...modelOptions];
    return [presets[0], ...modelOptions];
  }, [models, t]);

  const getSelectedValue = useCallback((slot: ModelSlot): string => {
    const id = funcAgentModels[slot] ?? '';
    if (!id) return '';
    return ['primary', 'fast'].includes(id) ? `preset:${id}` : `model:${id}`;
  }, [funcAgentModels]);

  const handleModelChange = useCallback(async (
    slot: ModelSlot,
    raw: string | number | (string | number)[],
  ) => {
    if (Array.isArray(raw)) return;
    const rawStr = String(raw);
    const newId = rawStr.startsWith('preset:') ? rawStr.replace('preset:', '') : rawStr.replace('model:', '');
    const updated = { ...funcAgentModels, [slot]: newId };
    setFuncAgentModels(updated);
    try {
      await configManager.setConfig('ai.func_agent_models', updated);
      notificationService.success(t('notifications.modelUpdated'));
    } catch (e) {
      log.error('Failed to update model', e);
      notificationService.error(t('notifications.updateFailed'));
    }
  }, [funcAgentModels, t]);

  const handleToolToggle = useCallback(async (toolName: string) => {
    if (!agenticConfig) return;
    setToolsLoading((prev) => ({ ...prev, [toolName]: true }));
    const current = agenticConfig.enabled_tools ?? [];
    const isEnabled = current.includes(toolName);
    const newTools = isEnabled ? current.filter((n) => n !== toolName) : [...current, toolName];
    const newConfig = { ...agenticConfig, enabled_tools: newTools };
    setAgenticConfig(newConfig);
    try {
      await configAPI.setModeConfig('agentic', newConfig);
      const { globalEventBus } = await import('@/infrastructure/event-bus');
      globalEventBus.emit('mode:config:updated');
    } catch (e) {
      log.error('Failed to toggle tool', e);
      notificationService.error(t('notifications.toggleFailed'));
      setAgenticConfig(agenticConfig);
    } finally {
      setToolsLoading((prev) => ({ ...prev, [toolName]: false }));
    }
  }, [agenticConfig, t]);

  const handleResetTools = useCallback(async () => {
    try {
      await configAPI.resetModeConfig('agentic');
      const modeConf = await configAPI.getModeConfig('agentic');
      setAgenticConfig(modeConf);
      const { globalEventBus } = await import('@/infrastructure/event-bus');
      globalEventBus.emit('mode:config:updated');
      notificationService.success(t('notifications.resetSuccess'));
    } catch (e) {
      log.error('Failed to reset tools', e);
      notificationService.error(t('notifications.resetFailed'));
    }
  }, [t]);

  const handleGroupToggleAll = useCallback(async (toolNames: string[]) => {
    if (!agenticConfig) return;
    const current = agenticConfig.enabled_tools ?? [];
    const allEnabled = toolNames.every((n) => current.includes(n));
    const newTools = allEnabled
      ? current.filter((n) => !toolNames.includes(n))
      : [...new Set([...current, ...toolNames])];
    const newConfig = { ...agenticConfig, enabled_tools: newTools };
    setAgenticConfig(newConfig);
    try {
      await configAPI.setModeConfig('agentic', newConfig);
      const { globalEventBus } = await import('@/infrastructure/event-bus');
      globalEventBus.emit('mode:config:updated');
    } catch (e) {
      log.error('Failed to toggle group', e);
      notificationService.error(t('notifications.toggleFailed'));
      setAgenticConfig(agenticConfig);
    }
  }, [agenticConfig, t]);

  const handleSkillToggle = useCallback(async (skillName: string) => {
    if (!agenticConfig) return;
    setSkillsLoading((prev) => ({ ...prev, [skillName]: true }));
    // Initialise from global state when available_skills is not yet set
    const current =
      agenticConfig.available_skills ??
      skills.filter((s) => s.enabled).map((s) => s.name);
    const isEnabled = current.includes(skillName);
    const next = isEnabled
      ? current.filter((n) => n !== skillName)
      : [...current, skillName];
    const newConfig = { ...agenticConfig, available_skills: next };
    setAgenticConfig(newConfig);
    try {
      await configAPI.setModeConfig('agentic', newConfig);
      const { globalEventBus } = await import('@/infrastructure/event-bus');
      globalEventBus.emit('mode:config:updated');
    } catch (e) {
      log.error('Failed to toggle skill', e);
      notificationService.error(t('notifications.toggleFailed'));
      setAgenticConfig(agenticConfig);
    } finally {
      setSkillsLoading((prev) => ({ ...prev, [skillName]: false }));
    }
  }, [agenticConfig, skills, t]);

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

  // Context breakdown: each segment = part / total  (composition of consumed tokens)
  const ctxTotal = tokenBreakdown.total;

  const segmentWidths = useMemo(() => {
    if (ctxTotal === 0) return CTX_SEGMENT_ORDER.map(() => 0);
    return CTX_SEGMENT_ORDER.map((key) => {
      const val = tokenBreakdown[key];
      return typeof val === 'number' ? (val / ctxTotal) * 100 : 0;
    });
  }, [tokenBreakdown, ctxTotal]);

  const contextZoneSubtitle = useMemo(
    () => (
      <>
        <strong>{formatTokenCount(ctxTotal)}</strong>
        {' tok · '}
        <span>
          {fmtPct(ctxTotal, CTX_WINDOW)} of {formatTokenCount(CTX_WINDOW)}
        </span>
      </>
    ),
    [ctxTotal],
  );

  const openToolDetail = useCallback((tool: ToolInfo, isMcp: boolean) => {
    setDetail((prev) => (
      prev?.type === 'tool' && prev.tool.name === tool.name
        ? null
        : { type: 'tool', tool, isMcp }
    ));
  }, []);

  const openSkillDetail = useCallback((skill: SkillInfo) => {
    setDetail((prev) => (
      prev?.type === 'skill' && prev.skill.name === skill.name
        ? null
        : { type: 'skill', skill }
    ));
  }, []);

  // ── Render helpers ───────────────────────────────────────────────────────

  const renderToolList = (tools: ToolInfo[], isMcp: boolean) => (
    <div className="tc-tool-list">
      {tools.map((tool) => {
        const enabled = agenticConfig?.enabled_tools?.includes(tool.name) ?? false;
        const displayName = isMcp ? getMcpShortName(tool.name) : tool.name;
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

  const renderSkillList = (list: SkillInfo[]) => (
    <div className="tc-skill-list">
      {list.map((skill) => {
        const on = isSkillEnabled(skill.name);
        const selected = detail?.type === 'skill' && detail.skill.name === skill.name;
        return (
          <div
            key={skill.name}
            className={`tc-skill-row${!on ? ' tc-skill-row--off' : ''}${selected ? ' tc-skill-row--selected' : ''}`}
          >
            <button
              type="button"
              className="tc-skill-row__hit"
              onClick={() => openSkillDetail(skill)}
            >
              <span className="tc-skill-row__name">{skill.name}</span>
              <span className="tc-skill-row__level">{skill.level}</span>
            </button>
            <Switch
              checked={on}
              onChange={() => handleSkillToggle(skill.name)}
              disabled={skillsLoading[skill.name]}
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
      (n) => agenticConfig?.enabled_tools?.includes(n),
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
      const displayName = isMcp ? getMcpShortName(tool.name) : tool.name;
      const enabled = agenticConfig?.enabled_tools?.includes(tool.name) ?? false;
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
                  <li key={tool.name}>{getMcpShortName(tool.name)}</li>
                ))}
              </ul>
            )}
          </div>
        </aside>
      );
    }

    const { skill } = detail;
    const on = isSkillEnabled(skill.name);
    return (
      <aside className="tc-template-detail" aria-label={t('nursery.template.detailPanel')}>
        <div className="tc-template-detail__head tc-template-detail__head--center-line">
          <span className="tc-template-detail__head-spacer" aria-hidden />
          <div className="tc-template-detail__head-text">
            <div className="tc-template-detail__head-line">
              <span className="tc-template-detail__kind">{t('cards.skills')}</span>
              <h3 className="tc-template-detail__title">{skill.name}</h3>
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
          <p className="tc-template-detail__meta">{t('nursery.template.skillLevel', { level: skill.level })}</p>
          <p className="tc-template-detail__desc">
            {skill.description?.trim() ? skill.description : '—'}
          </p>
          <div className="tc-template-detail__actions">
            <Switch
              checked={on}
              onChange={() => handleSkillToggle(skill.name)}
              disabled={skillsLoading[skill.name]}
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
            <div className="tc-template-model-context-row">
            <GalleryZone
              title={t('cards.model')}
              subtitle={t('nursery.template.sectionModelsSubtitle')}
            >
              <div className="tc-template-model-panel">
                <div className="tc-hero__models">
                  <div className="tc-model-slot">
                    <span className="tc-model-slot__label">{t('modelSlots.primary.label')}</span>
                    <div className="tc-model-slot__select">
                      <Select
                        size="small"
                        options={buildModelOptions('primary')}
                        value={getSelectedValue('primary')}
                        onChange={(v) => handleModelChange('primary', v)}
                        placeholder={t('slotDefault.primary')}
                      />
                    </div>
                  </div>
                  <div className="tc-model-slot">
                    <span className="tc-model-slot__label">{t('modelSlots.fast.label')}</span>
                    <div className="tc-model-slot__select">
                      <Select
                        size="small"
                        options={buildModelOptions('fast')}
                        value={getSelectedValue('fast')}
                        onChange={(v) => handleModelChange('fast', v)}
                        placeholder={t('slotDefault.fast')}
                      />
                    </div>
                  </div>
                </div>
              </div>
            </GalleryZone>

            <GalleryZone
              title={t('nursery.template.tokenTitle')}
              subtitle={contextZoneSubtitle}
            >
              <div className="tc-template-context-panel">
                <div className="tc-ctx__bar">
                  {ctxTotal === 0 ? (
                    <div className="tc-ctx__segment tc-ctx__segment--empty" />
                  ) : ctxSegments.map(({ key, color, label }, i) => (
                    segmentWidths[i] > 0 && (
                      <div
                        key={key}
                        className="tc-ctx__segment"
                        style={{ width: `${segmentWidths[i]}%`, background: color }}
                        title={`${label}: ${formatTokenCount(tokenBreakdown[key as keyof typeof tokenBreakdown] as number)} (${fmtPct(tokenBreakdown[key as keyof typeof tokenBreakdown] as number, ctxTotal)})`}
                      />
                    )
                  ))}
                </div>

                <div className="tc-ctx__legend tc-ctx__legend--template-split">
                  {ctxSegments.map(({ key, color, label }) => {
                    const val = tokenBreakdown[key as keyof typeof tokenBreakdown];
                    const num = typeof val === 'number' ? val : 0;
                    return (
                      <div key={key} className="tc-ctx__legend-item">
                        <span className="tc-ctx__legend-dot" style={{ background: color }} />
                        <span className="tc-ctx__legend-name">{label}</span>
                        <span className="tc-ctx__legend-val">{formatTokenCount(num)}</span>
                        <span className="tc-ctx__legend-pct">{fmtPct(num, ctxTotal)}</span>
                      </div>
                    );
                  })}
                </div>
              </div>
            </GalleryZone>
            </div>

            <GalleryZone
              title={t('cards.skills')}
            >
              {skills.length === 0 ? (
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
                    const mcpEnabled = serverTools.filter((tool) => agenticConfig?.enabled_tools?.includes(tool.name));
                    const mcpDisabled = serverTools.filter((tool) => !agenticConfig?.enabled_tools?.includes(tool.name));

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

export default TemplateConfigPage;
