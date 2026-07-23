import React, { useCallback, useEffect, useMemo, useState } from 'react';
import type { TFunction } from 'i18next';
import {
  Bot,
  Cpu,
  RotateCcw,
  Pencil,
  Plus,
  Puzzle,
  Search as SearchIcon,
  Trash2,
  Wrench,
} from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Badge, Button, IconButton, Search, Select, confirmDanger } from '@/component-library';
import {
  GalleryDetailModal,
  GalleryEmpty,
  GalleryGrid,
  GalleryLayout,
  GalleryPageHeader,
  GallerySkeleton,
  GalleryZone,
} from '@/app/components';
import AgentCard from './components/AgentCard';
import CoreAgentCard, { type CoreAgentMeta } from './components/CoreAgentCard';
import CreateAgentPage from './components/CreateAgentPage';
import {
  AgentCapabilityTooltip,
  type AgentCapabilityTooltipField,
} from './components/AgentCapabilityTooltip';
import { capabilityTooltipAriaLabel } from './components/agentCapabilityTooltipUtils';
import { SkillGroupPicker, SkillGroupSummary } from './components/SkillGroupPicker';
import { ToolGroupPicker, ToolGroupSummary } from './components/ToolGroupPicker';
import { useUserSkillGroups } from './components/useUserSkillGroups';
import { useUserToolGroups } from './components/useUserToolGroups';
import {
  type AgentWithCapabilities,
  useAgentsStore,
} from './agentsStore';
import { useAgentsList } from './hooks/useAgentsList';
import { AGENT_ICON_MAP } from './agentsIcons';
import { CAPABILITY_ACCENT, CORE_AGENT_ACCENTS, DEFAULT_CORE_AGENT_ACCENT } from './agentTheme';
import { getCardGradient } from '@/shared/utils/cardGradients';
import { isUserSelectableToolName } from '@/shared/utils/toolVisibility';
import { getAgentBadge, getAgentDescription, getCapabilityLabel } from './utils';
import './AgentsView.scss';
import './AgentsScene.scss';
import { useGallerySceneAutoRefresh } from '@/app/hooks/useGallerySceneAutoRefresh';
import {
  CORE_AGENT_IDS,
  isAgentInOverviewZone,
  isLocallyManageableSubagent,
} from './agentVisibility';
import { CustomAgentAPI } from '@/infrastructure/api/service-api/CustomAgentAPI';
import { configManager } from '@/infrastructure/config/services/ConfigManager';
import type { ModeSkillInfo, SubagentModelSelection } from '@/infrastructure/config/types';
import {
  buildSkillCoverageSourceMap,
  getModeSkillRuntimeStatus,
} from '@/infrastructure/config/skillSourcePresentation';
import type { SubagentInfo } from '@/infrastructure/api/service-api/SubagentAPI';
import { useNotification } from '@/shared/notification-system';
import {
  type ModelSelectOption,
  useModelSelectPresentation,
} from '@/infrastructure/config/components/ModelSelectPresentation';
import { useSceneManager } from '@/app/hooks/useSceneManager';
import { useSettingsStore } from '@/app/scenes/settings/settingsStore';

const DEFAULT_SUBAGENT_MODEL_OVERRIDE_VALUE = '__default_subagent_model__';

type CapabilityTab = 'model' | 'tools' | 'skills' | 'subagents';

function normalizeSelectValue(value: string | number | (string | number)[]): string {
  return String(Array.isArray(value) ? (value[0] ?? '') : value);
}

function subagentModelOverrideValue(selection: SubagentModelSelection | undefined): string {
  if (!selection) {
    return DEFAULT_SUBAGENT_MODEL_OVERRIDE_VALUE;
  }
  return selection.kind === 'inherit' ? 'inherit' : selection.model_id;
}

function subagentModelSelectionFromValue(value: string): SubagentModelSelection | undefined {
  if (value === DEFAULT_SUBAGENT_MODEL_OVERRIDE_VALUE) {
    return undefined;
  }
  return value === 'inherit'
    ? { kind: 'inherit' }
    : { kind: 'fixed', model_id: value };
}

function getConfiguredEnabledSkillKeys(skills: ModeSkillInfo[]): string[] {
  return skills.filter((skill) => skill.effectiveEnabled).map((skill) => skill.key);
}

function hasSkillTool(enabledTools: string[]): boolean {
  return enabledTools.includes('Skill');
}

function hasTaskTool(enabledTools: string[]): boolean {
  return enabledTools.includes('Task');
}

function skillRuntimeStatusLabel(
  skill: ModeSkillInfo,
  coverageSourceBySkillKey: ReadonlyMap<string, string>,
  t: TFunction<'scenes/agents'>,
): string | undefined {
  const status = getModeSkillRuntimeStatus(
    skill,
    coverageSourceBySkillKey,
    t('agentsOverview.unknownSkillSource'),
  );
  switch (status.kind) {
    case 'selected':
      return t('agentsOverview.skillRuntimeSelected');
    case 'covered':
      return t('agentsOverview.skillRuntimeCovered', { source: status.sourceLabel });
    case 'enabled':
      return t('agentsOverview.skillRuntimeEnabled');
    case 'disabled':
      return undefined;
  }
}

function subagentSourceLabel(
  source: SubagentInfo['source'] | undefined,
  t: TFunction<'scenes/agents'>,
): string {
  switch (source) {
    case 'project':
      return t('filters.project');
    case 'user':
      return t('filters.user');
    case 'external':
      return t('filters.external');
    default:
      return t('filters.builtin');
  }
}

function subagentTooltipFields(
  subagent: SubagentInfo,
  t: TFunction<'scenes/agents'>,
  isExternal: boolean,
): AgentCapabilityTooltipField[] {
  const source = subagent.subagentSource ?? subagent.source;
  return [
    {
      label: t('agentsOverview.capabilityTooltip.subagentId'),
      value: subagent.id,
      monospace: true,
    },
    {
      label: t('agentsOverview.capabilityTooltip.source'),
      value: subagentSourceLabel(source, t),
    },
    {
      label: t('agentsOverview.capabilityTooltip.toolCount'),
      value: String(subagent.toolCount),
    },
    ...(isExternal ? [{
      label: t('agentsOverview.capabilityTooltip.status'),
      value: t('agentsOverview.capabilityTooltip.externalManaged'),
    }] : []),
  ];
}

const AgentsHomeView: React.FC = () => {
  const { t } = useTranslation('scenes/agents');
  const notification = useNotification();
  const { openScene } = useSceneManager();
  const setSettingsTab = useSettingsStore((state) => state.setActiveTab);
  const [deletingAgent, setDeletingAgent] = useState(false);
  const {
    searchQuery,
    agentFilterLevel,
    agentFilterType,
    setSearchQuery,
    setAgentFilterLevel,
    setAgentFilterType,
    openCreateAgent,
    openEditAgent,
  } = useAgentsStore();
  const [selectedAgentId, setSelectedAgentId] = React.useState<string | null>(null);
  const [activeCapabilityTab, setActiveCapabilityTab] = React.useState<CapabilityTab | null>(null);
  const [toolsEditing, setToolsEditing] = React.useState(false);
  const [skillsEditing, setSkillsEditing] = React.useState(false);
  const [subagentsEditing, setSubagentsEditing] = React.useState(false);
  const [pendingTools, setPendingTools] = React.useState<string[] | null>(null);
  const [pendingSkills, setPendingSkills] = React.useState<string[] | null>(null);
  const [pendingSubagentIds, setPendingSubagentIds] = React.useState<string[] | null>(null);
  const [savingTools, setSavingTools] = React.useState(false);
  const [savingSkills, setSavingSkills] = React.useState(false);
  const [savingSubagents, setSavingSubagents] = React.useState(false);
  const [savingSubagentModel, setSavingSubagentModel] = React.useState(false);
  const [computerUseEnabled, setComputerUseEnabled] = useState(true);
  const { buildModelOption, renderModelOption, renderModelValue } = useModelSelectPresentation();
  const {
    groups: userToolGroups,
    saveGroups: saveUserToolGroups,
  } = useUserToolGroups();
  const {
    groups: userSkillGroups,
    saveGroups: saveUserSkillGroups,
  } = useUserSkillGroups();

  const {
    workspacePath,
    allAgents,
    filteredAgents,
    loading,
    availableTools,
    configuredModels = [],
    getModeProfile,
    getAgentSkills,
    getModeManageableSubagents,
    counts,
    hiddenAgentIds,
    loadAgents,
    getModeConfig,
    handleSetTools,
    handleResetTools,
    handleSetSkills,
    handleResetSkills,
    handleSetSubagentEnabled,
    handleSetSubagentModel,
  } = useAgentsList({
    searchQuery,
    filterLevel: agentFilterLevel,
    filterType: agentFilterType,
    t,
  });

  useGallerySceneAutoRefresh({
    sceneId: 'agents',
    refetch: () => {
      void loadAgents();
    },
  });

  useEffect(() => {
    let cancelled = false;
    const loadComputerUseEnabled = () => {
      void configManager.getConfig<boolean>('ai.computer_use_enabled').then((enabled) => {
        if (!cancelled) setComputerUseEnabled(enabled ?? false);
      });
    };
    loadComputerUseEnabled();
    const unsubscribe = configManager.onConfigChange((path) => {
      if (path === 'ai.computer_use_enabled' || path === 'ai') loadComputerUseEnabled();
    });
    return () => {
      cancelled = true;
      unsubscribe();
    };
  }, []);

  const coreAgentMeta = useMemo((): Record<string, CoreAgentMeta> => ({
    agentic: {
      role: t('coreAgentsZone.modes.agentic.role'),
      ...CORE_AGENT_ACCENTS.agentic,
    },
    Cowork: {
      role: t('coreAgentsZone.modes.cowork.role'),
      ...CORE_AGENT_ACCENTS.Cowork,
    },
    ComputerUse: {
      role: t('coreAgentsZone.modes.computerUse.role'),
      ...CORE_AGENT_ACCENTS.ComputerUse,
    },
  }), [t]);

  const coreAgents = useMemo(() => allAgents.filter((agent) => CORE_AGENT_IDS.has(agent.id)), [allAgents]);

  const visibleAgents = useMemo(
    () => filteredAgents.filter((agent) => isAgentInOverviewZone(agent, hiddenAgentIds)),
    [filteredAgents, hiddenAgentIds],
  );

  const scrollToZone = useCallback((targetId: string) => {
    document.getElementById(targetId)?.scrollIntoView({ behavior: 'smooth', block: 'start' });
  }, []);

  const levelFilters = [
    { key: 'builtin', label: t('filters.builtin'), count: counts.builtin },
    { key: 'user', label: t('filters.user'), count: counts.user },
    { key: 'project', label: t('filters.project'), count: counts.project },
    { key: 'external', label: t('filters.external'), count: counts.external },
  ] as const;

  const typeFilters = [
    { key: 'mode', label: t('filters.mode'), count: counts.mode },
    { key: 'subagent', label: t('filters.subagent'), count: counts.subagent },
  ] as const;

  const renderSkeletons = (prefix: string) => (
    <GallerySkeleton count={6} cardHeight={138} className={`${prefix}-skeleton`} />
  );

  const selectedAgent = useMemo(
    () => allAgents.find((agent) => agent.id === selectedAgentId) ?? null,
    [allAgents, selectedAgentId],
  );
  const selectedAgentIsExternal = (
    selectedAgent?.source ?? selectedAgent?.subagentSource
  ) === 'external';
  const selectedAgentModeConfig = useMemo(
    () => (selectedAgent?.agentKind === 'mode' ? getModeConfig(selectedAgent.id) : null),
    [getModeConfig, selectedAgent],
  );
  const selectedAgentModeProfile = useMemo(
    () => (selectedAgent?.agentKind === 'mode' ? getModeProfile(selectedAgent.id) : null),
    [getModeProfile, selectedAgent],
  );
  const selectedAgentSkillConfigs = useMemo(
    () => (selectedAgent ? getAgentSkills(selectedAgent.id) : []),
    [getAgentSkills, selectedAgent],
  );
  const selectedAgentManageableSubagents = useMemo(
    () => (selectedAgent?.agentKind === 'mode' ? getModeManageableSubagents(selectedAgent.id) : []),
    [getModeManageableSubagents, selectedAgent],
  );
  const selectedAgentEditableSubagents = useMemo(
    () => selectedAgentManageableSubagents.filter(isLocallyManageableSubagent),
    [selectedAgentManageableSubagents],
  );
  const selectedAgentConfiguredTools = useMemo(() => (
    selectedAgent?.agentKind === 'mode'
      ? (selectedAgentModeConfig?.enabled_tools ?? selectedAgent.defaultTools ?? [])
      : (selectedAgent?.defaultTools ?? [])
  ), [selectedAgent, selectedAgentModeConfig]);
  const selectedAgentTools = useMemo(
    () => selectedAgentConfiguredTools.filter(isUserSelectableToolName),
    [selectedAgentConfiguredTools],
  );
  const userSelectableAvailableTools = useMemo(
    () => availableTools.filter((tool) => isUserSelectableToolName(tool.name)),
    [availableTools],
  );
  const selectedAgentHasSkillTool = hasSkillTool(selectedAgentConfiguredTools);
  const selectedAgentHasTaskTool = selectedAgent?.agentKind === 'mode'
    ? hasTaskTool(selectedAgentConfiguredTools)
    : false;
  const selectedAgentEnabledSubagents = useMemo(
    () => selectedAgentManageableSubagents.filter((subagent) => subagent.effectiveEnabled),
    [selectedAgentManageableSubagents],
  );
  const selectedAgentDefaultEnabledSubagentIds = useMemo(
    () => selectedAgentManageableSubagents
      .filter((subagent) => subagent.defaultEnabled)
      .map((subagent) => subagent.id),
    [selectedAgentManageableSubagents],
  );
  const selectedAgentEnabledSubagentIds = useMemo(
    () => selectedAgentEnabledSubagents.map((subagent) => subagent.id),
    [selectedAgentEnabledSubagents],
  );
  const selectedAgentSkills = useMemo(
    () => getConfiguredEnabledSkillKeys(selectedAgentSkillConfigs),
    [selectedAgentSkillConfigs],
  );
  const selectedAgentCoverageSourceBySkillKey = useMemo(
    () => buildSkillCoverageSourceMap(
      selectedAgentSkillConfigs,
      t('agentsOverview.unknownSkillSource'),
    ),
    [selectedAgentSkillConfigs, t],
  );
  const selectedAgentSkillItems = useMemo(
    () => selectedAgentSkillConfigs.map((skill) => ({
      ...skill,
      runtimeStatus: skillRuntimeStatusLabel(skill, selectedAgentCoverageSourceBySkillKey, t),
    })),
    [selectedAgentCoverageSourceBySkillKey, selectedAgentSkillConfigs, t],
  );
  const selectedAgentRuntimeSkillCount = useMemo(
    () => selectedAgentSkillConfigs.filter((skill) => skill.selectedForRuntime).length,
    [selectedAgentSkillConfigs],
  );
  const selectedAgentProfileMemberNames = useMemo(() => {
    if (!selectedAgentModeProfile) {
      return [];
    }

    return selectedAgentModeProfile.memberModeIds.map((memberId) => (
      allAgents.find((agent) => agent.agentKind === 'mode' && agent.id === memberId)?.name ?? memberId
    ));
  }, [allAgents, selectedAgentModeProfile]);
  const selectedAgentUsesSharedProfile = (selectedAgentModeProfile?.memberModeIds.length ?? 0) > 1;
  const getDisplayedToolCount = useCallback((agent: AgentWithCapabilities): number => {
    const configuredTools = agent.agentKind === 'mode'
      ? (getModeConfig(agent.id)?.enabled_tools ?? agent.defaultTools)
      : agent.defaultTools;
    if (configuredTools) {
      return configuredTools.filter(isUserSelectableToolName).length;
    }
    return agent.toolCount ?? 0;
  }, [getModeConfig]);
  const selectedAgentToolCount = selectedAgent ? getDisplayedToolCount(selectedAgent) : 0;
  const selectedSubagentModelValue = selectedAgent?.agentKind === 'subagent'
    ? subagentModelOverrideValue(selectedAgent.subagentModelOverride)
    : DEFAULT_SUBAGENT_MODEL_OVERRIDE_VALUE;
  const subagentModelOptions = useMemo<ModelSelectOption[]>(() => [
    {
      label: t('agentCard.modelSelector.default'),
      value: DEFAULT_SUBAGENT_MODEL_OVERRIDE_VALUE,
    },
    { label: t('agentCard.modelSelector.inherit'), value: 'inherit' },
    { label: t('agentCard.modelSelector.fast'), value: 'fast' },
    { label: t('agentCard.modelSelector.primary'), value: 'primary' },
    { label: t('agentCard.modelSelector.auto'), value: 'auto' },
    ...configuredModels
      .filter((model): model is typeof model & { id: string } => (
        typeof model.id === 'string'
        && model.id.trim().length > 0
        && model.enabled !== false
        && (model.capabilities ?? []).includes('text_chat')
      ))
      .map(buildModelOption),
  ], [buildModelOption, configuredModels, t]);
  const handleSubagentModelChange = useCallback(async (
    value: string | number | (string | number)[],
  ) => {
    if (
      !selectedAgent
      || selectedAgent.agentKind !== 'subagent'
      || selectedAgentIsExternal
      || savingSubagentModel
    ) {
      return;
    }

    setSavingSubagentModel(true);
    try {
      await handleSetSubagentModel(
        selectedAgent.id,
        subagentModelSelectionFromValue(normalizeSelectValue(value)),
      );
    } finally {
      setSavingSubagentModel(false);
    }
  }, [handleSetSubagentModel, savingSubagentModel, selectedAgent, selectedAgentIsExternal]);
  const selectedAgentCapabilityTabs = useMemo(() => {
    const tabs: Array<{
      key: CapabilityTab;
      icon: typeof Wrench;
      label: string;
      count?: string;
    }> = [];

    if (selectedAgent?.agentKind === 'subagent' && !selectedAgentIsExternal) {
      tabs.push({
        key: 'model',
        icon: Cpu,
        label: t('agentCard.modelSelector.label'),
      });
    }

    if (selectedAgentTools.length > 0) {
      const currentToolCount = selectedAgent?.agentKind === 'mode'
        ? (toolsEditing
          ? (pendingTools ?? selectedAgentConfiguredTools).filter(isUserSelectableToolName).length
          : selectedAgentTools.length)
        : selectedAgentTools.length;
      const totalToolCount = selectedAgent?.agentKind === 'mode'
        ? userSelectableAvailableTools.length
        : selectedAgentTools.length;

      tabs.push({
        key: 'tools',
        icon: Wrench,
        label: t('agentsOverview.tools'),
        count: selectedAgent?.agentKind === 'mode'
          ? `${currentToolCount}/${totalToolCount}`
          : `${currentToolCount}`,
      });
    }

    if (selectedAgentHasSkillTool && selectedAgentSkillConfigs.length > 0) {
      const currentSkillCount = skillsEditing
        ? (pendingSkills ?? selectedAgentSkills).length
        : selectedAgent?.agentKind === 'mode'
          ? selectedAgentRuntimeSkillCount
          : selectedAgentSkills.length;
      tabs.push({
        key: 'skills',
        icon: Puzzle,
        label: t('agentsOverview.skills'),
        count: `${currentSkillCount}/${selectedAgentSkillConfigs.length}`,
      });
    }

    if (selectedAgent?.agentKind === 'mode' && selectedAgentHasTaskTool) {
      const currentSubagentIds = subagentsEditing
        ? (pendingSubagentIds ?? selectedAgentEnabledSubagentIds)
        : selectedAgentEnabledSubagentIds;
      tabs.push({
        key: 'subagents',
        icon: Bot,
        label: t('agentsOverview.subagents'),
        count: `${currentSubagentIds.length}/${selectedAgentManageableSubagents.length}`,
      });
    }

    return tabs;
  }, [
    userSelectableAvailableTools.length,
    pendingSkills,
    pendingSubagentIds,
    pendingTools,
    selectedAgent,
    selectedAgentIsExternal,
    selectedAgentConfiguredTools,
    selectedAgentEnabledSubagentIds,
    selectedAgentHasSkillTool,
    selectedAgentHasTaskTool,
    selectedAgentManageableSubagents.length,
    selectedAgentSkillConfigs.length,
    selectedAgentSkills,
    selectedAgentRuntimeSkillCount,
    selectedAgentTools,
    skillsEditing,
    subagentsEditing,
    t,
    toolsEditing,
  ]);
  const currentCapabilityTab = useMemo(() => {
    if (selectedAgentCapabilityTabs.some((tab) => tab.key === activeCapabilityTab)) {
      return activeCapabilityTab;
    }
    return selectedAgentCapabilityTabs[0]?.key ?? 'tools';
  }, [activeCapabilityTab, selectedAgentCapabilityTabs]);
  const canManageCurrentCapability = selectedAgent?.agentKind === 'mode'
    || (
      currentCapabilityTab === 'skills'
      && selectedAgent?.agentKind === 'subagent'
      && !selectedAgentIsExternal
    );
  const isCurrentTabEditing = currentCapabilityTab === 'tools'
    ? toolsEditing
    : currentCapabilityTab === 'skills'
      ? skillsEditing
      : currentCapabilityTab === 'subagents'
        ? subagentsEditing
        : false;
  const resetEditState = useCallback(() => {
    setToolsEditing(false);
    setSkillsEditing(false);
    setSubagentsEditing(false);
    setPendingTools(null);
    setPendingSkills(null);
    setPendingSubagentIds(null);
    setSavingTools(false);
    setSavingSkills(false);
    setSavingSubagents(false);
  }, []);

  const openAgentDetails = useCallback((agent: AgentWithCapabilities) => {
    setSelectedAgentId(agent.id);
    setActiveCapabilityTab(null);
    resetEditState();
  }, [resetEditState]);

  const closeAgentDetails = useCallback(() => {
    setSelectedAgentId(null);
    setActiveCapabilityTab(null);
    resetEditState();
  }, [resetEditState]);

  useEffect(() => {
    if (!selectedAgentCapabilityTabs.some((tab) => tab.key === activeCapabilityTab)) {
      setActiveCapabilityTab(selectedAgentCapabilityTabs[0]?.key ?? null);
    }
  }, [activeCapabilityTab, selectedAgentCapabilityTabs]);

  const handleDeleteCustomAgent = useCallback(async () => {
    if (!selectedAgent) return;
    if (['builtin', 'external'].includes(
      selectedAgent.source ?? selectedAgent.subagentSource ?? 'builtin',
    )) {
      return;
    }
    const id = selectedAgent.id;
    const name = selectedAgent.name;
    const ok = await confirmDanger(
      t('agentsOverview.deleteAgent'),
      t('agentsOverview.deleteConfirm', { name }),
    );
    if (!ok) return;
    setDeletingAgent(true);
    try {
      await CustomAgentAPI.deleteCustomAgent(id, workspacePath || undefined);
      notification.success(t('agentsOverview.deleteSuccess', { name }));
      closeAgentDetails();
      await loadAgents();
    } catch (e) {
      notification.error(
        `${t('agentsOverview.deleteFailed')}${e instanceof Error ? e.message : String(e)}`,
      );
    } finally {
      setDeletingAgent(false);
    }
  }, [selectedAgent, closeAgentDetails, loadAgents, notification, t, workspacePath]);

  const canManageCustomAgent = Boolean(
    selectedAgent
    && !['builtin', 'external'].includes(
      selectedAgent.source ?? selectedAgent.subagentSource ?? 'builtin',
    ),
  );

  return (
    <GalleryLayout className="bitfun-agents-scene" data-testid="agent-skill-panel">
      <GalleryPageHeader
        title={t('page.title')}
        subtitle={t('page.subtitle')}
        extraContent={(
          <div className="gallery-anchor-bar">
            <button
              type="button"
              className="gallery-anchor-btn"
              onClick={() => scrollToZone('core-agents-zone')}
              data-testid="agents-anchor-core"
            >
              {t('nav.coreAgents')}
            </button>
            <button
              type="button"
              className="gallery-anchor-btn"
              onClick={() => scrollToZone('agents-zone')}
              data-testid="agents-anchor-custom"
            >
              {t('nav.agents')}
            </button>
          </div>
        )}
        actions={(
          <>
            <Search
              value={searchQuery}
              onChange={setSearchQuery}
              placeholder={t('page.searchPlaceholder')}
              size="small"
              clearable
              prefixIcon={<></>}
              suffixContent={(
                <button
                  type="button"
                  className="gallery-search-btn"
                  aria-label={t('page.searchPlaceholder')}
                  data-testid="agents-search-btn"
                >
                  <SearchIcon size={14} />
                </button>
              )}
            />
          </>
        )}
      />

      <div className="gallery-zones" data-testid="agent-list">
        <GalleryZone
          id="core-agents-zone"
          data-testid="agents-core-zone"
          title={t('coreAgentsZone.title')}
          subtitle={t('coreAgentsZone.subtitle')}
          tools={(
            <span className="gallery-zone-count">{coreAgents.length}</span>
          )}
        >
          {loading ? (
            <GallerySkeleton count={3} cardHeight={160} className="core-agent-skeleton" />
          ) : coreAgents.length === 0 ? (
            <GalleryEmpty
              icon={<Cpu size={32} strokeWidth={1.5} />}
              message={t('coreAgentsZone.empty')}
              testId="agent-list-empty"
            />
          ) : (
            <div className="core-agents-grid">
              {coreAgents.map((agent, index) => (
                <CoreAgentCard
                  key={agent.id}
                  agent={agent}
                  index={index}
                  meta={coreAgentMeta[agent.id] ?? { role: agent.name, ...DEFAULT_CORE_AGENT_ACCENT }}
                  toolCount={getDisplayedToolCount(agent)}
                  skillCount={hasSkillTool(
                    agent.agentKind === 'mode'
                      ? (getModeConfig(agent.id)?.enabled_tools ?? agent.defaultTools ?? [])
                      : (agent.defaultTools ?? []),
                  )
                    ? getConfiguredEnabledSkillKeys(getAgentSkills(agent.id)).length
                    : 0}
                  subagentCount={agent.agentKind === 'mode' && hasTaskTool(getModeConfig(agent.id)?.enabled_tools ?? agent.defaultTools ?? [])
                    ? (agent.visibleSubagentCount ?? 0)
                    : 0}
                  onOpenDetails={openAgentDetails}
                  disabledReason={
                    agent.id === 'ComputerUse' && !computerUseEnabled
                      ? t('coreAgentsZone.computerUseDisabledBadge')
                      : undefined
                  }
                />
              ))}
            </div>
          )}
        </GalleryZone>

        <GalleryZone
          id="agents-zone"
          data-testid="agents-custom-zone"
          title={t('agentsZone.title')}
          subtitle={t('agentsZone.subtitle')}
          tools={(
            <>
              <div className="bitfun-agents-scene__agent-filters">
                <div className="bitfun-agents-scene__agent-filter-group">
                  <span className="bitfun-agents-scene__agent-filter-label">
                    {t('filters.source')}
                  </span>
                  {levelFilters.map(({ key, label, count }) => (
                    <button
                      key={key}
                      type="button"
                      className={[
                        'gallery-cat-chip',
                        agentFilterLevel === key && 'gallery-cat-chip--active',
                      ].filter(Boolean).join(' ')}
                      onClick={() => setAgentFilterLevel(agentFilterLevel === key ? 'all' : key)}
                      data-testid="agents-source-filter"
                      data-agent-source={key}
                    >
                      <span>{label}</span>
                      <span className="gallery-filter-count">{count}</span>
                    </button>
                  ))}
                </div>
                <div className="bitfun-agents-scene__agent-filter-group">
                  <span className="bitfun-agents-scene__agent-filter-label">
                    {t('filters.kind')}
                  </span>
                  {typeFilters.map(({ key, label, count }) => (
                    <button
                      key={key}
                      type="button"
                      className={[
                        'gallery-cat-chip',
                        agentFilterType === key && 'gallery-cat-chip--active',
                      ].filter(Boolean).join(' ')}
                      onClick={() => setAgentFilterType(agentFilterType === key ? 'all' : key)}
                      data-testid="agents-kind-filter"
                      data-agent-kind={key}
                    >
                      <span>{label}</span>
                      <span className="gallery-filter-count">{count}</span>
                    </button>
                  ))}
                </div>
              </div>
              <button
                type="button"
                className="gallery-action-btn gallery-action-btn--primary"
                onClick={openCreateAgent}
                data-testid="agents-create-agent-btn"
              >
                <Plus size={15} />
                <span>{t('page.newAgent')}</span>
              </button>
              <span className="gallery-zone-count">{visibleAgents.length}</span>
            </>
          )}
        >
          {loading ? renderSkeletons('agent') : null}

          {!loading && visibleAgents.length === 0 ? (
            <GalleryEmpty
              icon={<Bot size={32} strokeWidth={1.5} />}
              message={allAgents.length === 0 ? t('agentsZone.empty.noAgents') : t('agentsZone.empty.noMatch')}
              testId="agent-list-empty"
            />
          ) : null}

          {!loading && visibleAgents.length > 0 ? (
            <GalleryGrid minCardWidth={360}>
              {visibleAgents.map((agent, index) => (
                <AgentCard
                  key={agent.id}
                  agent={agent}
                  index={index}
                  toolCount={getDisplayedToolCount(agent)}
                  skillCount={hasSkillTool(
                    agent.agentKind === 'mode'
                      ? (getModeConfig(agent.id)?.enabled_tools ?? agent.defaultTools ?? [])
                      : (agent.defaultTools ?? []),
                  )
                    ? getConfiguredEnabledSkillKeys(getAgentSkills(agent.id)).length
                    : 0}
                  subagentCount={agent.agentKind === 'mode' && hasTaskTool(getModeConfig(agent.id)?.enabled_tools ?? agent.defaultTools ?? [])
                    ? (agent.visibleSubagentCount ?? 0)
                    : 0}
                  onOpenDetails={openAgentDetails}
                />
              ))}
            </GalleryGrid>
          ) : null}
        </GalleryZone>
      </div>

      <GalleryDetailModal
        isOpen={Boolean(selectedAgent)}
        onClose={closeAgentDetails}
        icon={selectedAgent ? React.createElement(
          AGENT_ICON_MAP[(selectedAgent.iconKey ?? 'bot') as keyof typeof AGENT_ICON_MAP] ?? Bot,
          { size: 24, strokeWidth: 1.7 },
        ) : <Bot size={24} />}
        iconGradient={selectedAgent ? getCardGradient(selectedAgent.id || selectedAgent.name) : undefined}
        title={selectedAgent?.name ?? ''}
        badges={selectedAgent ? (
          <>
            <Badge
              variant={
                getAgentBadge(
                  t,
                  selectedAgent.agentKind,
                  selectedAgent.source ?? selectedAgent.subagentSource,
                ).variant
              }
            >
              {selectedAgent.agentKind === 'mode' ? <Cpu size={10} /> : <Bot size={10} />}
              {
                getAgentBadge(
                  t,
                  selectedAgent.agentKind,
                  selectedAgent.source ?? selectedAgent.subagentSource,
                ).label
              }
            </Badge>
          </>
        ) : null}
        description={selectedAgent
          ? getAgentDescription(t, selectedAgent)
          : undefined}
        testId="agent-detail-panel"
        titleTestId="agent-detail-title"
        descriptionTestId="agent-detail-description"
        closeButtonTestId="agent-detail-close"
        meta={selectedAgent ? (
          <>
            <span>{t('agentCard.meta.tools', { count: selectedAgentToolCount })}</span>
            {selectedAgent.externalProviderLabel ? (
              <span>{t('agentCard.meta.externalProvider', { provider: selectedAgent.externalProviderLabel })}</span>
            ) : null}
            {selectedAgent.supportsFollowUp === false ? (
              <span>{t('agentCard.meta.singleRun')}</span>
            ) : null}
            {selectedAgentHasSkillTool ? (
              <span>{t('agentCard.meta.skills', {
                count: selectedAgent.agentKind === 'mode'
                  ? selectedAgentRuntimeSkillCount
                  : selectedAgentSkills.length,
              })}</span>
            ) : null}
            {selectedAgent.agentKind === 'mode' && selectedAgentHasTaskTool ? (
              <span>{t('agentCard.meta.subagents', { count: selectedAgentManageableSubagents.filter((subagent) => subagent.effectiveEnabled).length })}</span>
            ) : null}
          </>
        ) : null}
      >
        {selectedAgent ? (
          <>
            <div className="agent-card__cap-grid">
              {selectedAgent.capabilities.map((cap) => (
                <div key={cap.category} className="agent-card__cap-row">
                  <span
                    className="agent-card__cap-label"
                    style={{ color: CAPABILITY_ACCENT[cap.category] }}
                  >
                    {getCapabilityLabel(t, cap.category)}
                  </span>
                  <div className="agent-card__cap-bar">
                    {Array.from({ length: 5 }).map((_, i) => (
                      <span
                        key={i}
                        className="agent-card__cap-pip"
                        style={i < cap.level ? { backgroundColor: CAPABILITY_ACCENT[cap.category] } : undefined}
                      />
                    ))}
                  </div>
                  <span className="agent-card__cap-level">{cap.level}/5</span>
                </div>
              ))}
            </div>

            {selectedAgent.agentKind === 'mode' && selectedAgentUsesSharedProfile ? (
              <div className="agent-card__section">
                <div className="agent-card__section-head">
                  <div className="agent-card__section-title">
                    <span>{t('agentsOverview.sharedProfileLabel')}</span>
                  </div>
                </div>
                <div className="agent-card__chip-grid">
                  <span className="agent-card__chip">
                    {selectedAgentModeProfile?.profileLabel ?? t('agentsOverview.sharedProfileDefaultLabel')}
                  </span>
                </div>
                <p className="agent-card__section-note">
                  {t('agentsOverview.sharedProfileDescription', {
                    modes: selectedAgentProfileMemberNames.join(', '),
                  })}
                </p>
              </div>
            ) : null}

            {selectedAgentCapabilityTabs.length > 0 ? (
              <div className="agent-card__section" data-testid="agent-detail-tools-section">
                <div className="agent-card__section-head">
                  <div className="agent-card__tab-list" role="tablist" aria-label={t('agentsOverview.capabilities')}>
                    {selectedAgentCapabilityTabs.map((tab) => {
                      const TabIcon = tab.icon;
                      const isActive = tab.key === currentCapabilityTab;
                      return (
                        <button
                          key={tab.key}
                          type="button"
                          role="tab"
                          aria-selected={isActive}
                          className={`agent-card__tab${isActive ? ' is-active' : ''}`}
                          onClick={() => setActiveCapabilityTab(tab.key)}
                        >
                          <TabIcon size={12} />
                          <span>{tab.label}</span>
                          {isActive && tab.count ? (
                            <span className="agent-card__tab-count">{tab.count}</span>
                          ) : null}
                        </button>
                      );
                    })}
                  </div>
                  {canManageCurrentCapability ? (
                    <div className="agent-card__section-actions">
                      {isCurrentTabEditing ? (
                        <>
                          <IconButton
                            size="small"
                            variant="ghost"
                            tooltip={
                              currentCapabilityTab === 'tools'
                                ? t('agentsOverview.toolsReset')
                                : currentCapabilityTab === 'skills'
                                  ? t('agentsOverview.reset')
                                  : t('agentsOverview.reset')
                            }
                            onClick={async () => {
                              if (currentCapabilityTab === 'tools') {
                                await handleResetTools(selectedAgent.id);
                                setToolsEditing(false);
                                setPendingTools(null);
                                return;
                              }
                              if (currentCapabilityTab === 'skills') {
                                await handleResetSkills(selectedAgent.id);
                                setSkillsEditing(false);
                                setPendingSkills(null);
                                return;
                              }
                              setSavingSubagents(true);
                              try {
                                const currentEnabledIds = new Set(selectedAgentEnabledSubagentIds);
                                const defaultEnabledIds = new Set(selectedAgentDefaultEnabledSubagentIds);
                                const changedSubagents = selectedAgentEditableSubagents.filter((subagent) =>
                                  currentEnabledIds.has(subagent.id) !== defaultEnabledIds.has(subagent.id));

                                if (changedSubagents.length === 0) {
                                  setSubagentsEditing(false);
                                  setPendingSubagentIds(null);
                                  return;
                                }

                                for (const subagent of changedSubagents) {
                                  await handleSetSubagentEnabled(
                                    selectedAgent.id,
                                    subagent.id,
                                    defaultEnabledIds.has(subagent.id),
                                  );
                                }
                              } finally {
                                setSavingSubagents(false);
                                setSubagentsEditing(false);
                                setPendingSubagentIds(null);
                              }
                            }}
                          >
                            <RotateCcw size={12} />
                          </IconButton>
                          <Button
                            variant="ghost"
                            size="small"
                            onClick={() => {
                              if (currentCapabilityTab === 'tools') {
                                setToolsEditing(false);
                                setPendingTools(null);
                                return;
                              }
                              if (currentCapabilityTab === 'skills') {
                                setSkillsEditing(false);
                                setPendingSkills(null);
                                return;
                              }
                              setSubagentsEditing(false);
                              setPendingSubagentIds(null);
                            }}
                          >
                            {t('agentsOverview.cancel')}
                          </Button>
                          <Button
                            variant="primary"
                            size="small"
                            isLoading={
                              currentCapabilityTab === 'tools'
                                ? savingTools
                                : currentCapabilityTab === 'skills'
                                  ? savingSkills
                                  : savingSubagents
                            }
                            onClick={async () => {
                              if (currentCapabilityTab === 'tools') {
                                if (!pendingTools) {
                                  setToolsEditing(false);
                                  return;
                                }
                                setSavingTools(true);
                                try {
                                  await handleSetTools(selectedAgent.id, pendingTools);
                                } finally {
                                  setSavingTools(false);
                                  setToolsEditing(false);
                                  setPendingTools(null);
                                }
                                return;
                              }

                              if (currentCapabilityTab === 'skills') {
                                if (!pendingSkills) {
                                  setSkillsEditing(false);
                                  return;
                                }
                                setSavingSkills(true);
                                try {
                                  await handleSetSkills(selectedAgent.id, pendingSkills);
                                } finally {
                                  setSavingSkills(false);
                                  setSkillsEditing(false);
                                  setPendingSkills(null);
                                }
                                return;
                              }

                              const nextEnabledIds = new Set(pendingSubagentIds ?? selectedAgentEnabledSubagentIds);
                              const currentEnabledIds = new Set(selectedAgentEnabledSubagentIds);
                              const changedSubagents = selectedAgentEditableSubagents.filter((subagent) =>
                                currentEnabledIds.has(subagent.id) !== nextEnabledIds.has(subagent.id));

                              if (changedSubagents.length === 0) {
                                setSubagentsEditing(false);
                                setPendingSubagentIds(null);
                                return;
                              }

                              setSavingSubagents(true);
                              try {
                                for (const subagent of changedSubagents) {
                                  await handleSetSubagentEnabled(
                                    selectedAgent.id,
                                    subagent.id,
                                    nextEnabledIds.has(subagent.id),
                                  );
                                }
                              } finally {
                                setSavingSubagents(false);
                                setSubagentsEditing(false);
                                setPendingSubagentIds(null);
                              }
                            }}
                          >
                            {t('agentsOverview.save')}
                          </Button>
                        </>
                      ) : (
                        <Button
                          variant="secondary"
                          size="small"
                          onClick={() => {
                            if (currentCapabilityTab === 'tools') {
                              setPendingTools([...selectedAgentConfiguredTools]);
                              setToolsEditing(true);
                              return;
                            }
                            if (currentCapabilityTab === 'skills') {
                              setPendingSkills([...selectedAgentSkills]);
                              setSkillsEditing(true);
                              return;
                            }
                            setPendingSubagentIds([...selectedAgentEnabledSubagentIds]);
                            setSubagentsEditing(true);
                          }}
                        >
                          {t('manage')}
                        </Button>
                      )}
                    </div>
                  ) : null}
                </div>

                {currentCapabilityTab === 'model'
                && selectedAgent.agentKind === 'subagent'
                && !selectedAgentIsExternal ? (
                  <Select
                    size="small"
                    searchable
                    className="bitfun-agents-scene__subagent-model-select model-select-presentation__select"
                    options={subagentModelOptions}
                    value={selectedSubagentModelValue}
                    onChange={(value) => void handleSubagentModelChange(value)}
                    renderOption={renderModelOption}
                    renderValue={renderModelValue}
                    disabled={savingSubagentModel}
                    triggerTestId="agent-detail-subagent-model-select"
                  />
                ) : null}

                {currentCapabilityTab === 'tools' ? (
                  selectedAgent.agentKind === 'mode' && toolsEditing ? (
                    <ToolGroupPicker
                      tools={userSelectableAvailableTools}
                      selectedToolNames={pendingTools ?? selectedAgentConfiguredTools}
                      userGroups={userToolGroups}
                      onSelectionChange={setPendingTools}
                      onSaveUserGroups={saveUserToolGroups}
                      disabled={savingTools}
                      testId="agent-detail-tool-groups"
                    />
                  ) : (
                    <ToolGroupSummary
                      tools={userSelectableAvailableTools}
                      selectedToolNames={selectedAgentTools}
                      userGroups={userToolGroups}
                    />
                  )
                ) : null}

                {currentCapabilityTab === 'skills'
                && selectedAgentHasSkillTool
                && selectedAgentSkillConfigs.length > 0 ? (
                  skillsEditing ? (
                    <SkillGroupPicker
                      skills={selectedAgentSkillItems}
                      selectedSkillKeys={pendingSkills ?? selectedAgentSkills}
                      userGroups={userSkillGroups}
                      onSelectionChange={setPendingSkills}
                      onSaveUserGroups={saveUserSkillGroups}
                      disabled={savingSkills}
                      testId="agent-detail-skill-groups"
                    />
                  ) : (
                    <SkillGroupSummary
                      skills={selectedAgentSkillItems}
                      selectedSkillKeys={selectedAgentSkills}
                      userGroups={userSkillGroups}
                    />
                  )
                ) : null}

                {currentCapabilityTab === 'subagents'
                && selectedAgent.agentKind === 'mode'
                && selectedAgentHasTaskTool ? (
                  selectedAgentManageableSubagents.length === 0 ? (
                    <span className="agent-card__empty-inline">
                      {t('agentsOverview.noSubagents')}
                    </span>
                  ) : subagentsEditing ? (
                    <div className="agent-card__token-grid">
                      {selectedAgentManageableSubagents.map((subagent: SubagentInfo) => {
                        const isOn = (pendingSubagentIds ?? selectedAgentEnabledSubagentIds).includes(subagent.id);
                        const isExternal = !isLocallyManageableSubagent(subagent);
                        const tooltipFields = subagentTooltipFields(subagent, t, isExternal);
                        return (
                          <AgentCapabilityTooltip
                            key={subagent.key}
                            title={subagent.name}
                            description={subagent.description}
                            fields={tooltipFields}
                          >
                            <span className="agent-card__tooltip-trigger">
                              <button
                                type="button"
                                className={`agent-card__token${isOn ? ' is-on' : ''}${isExternal ? ' is-readonly' : ''}`}
                                disabled={isExternal}
                                aria-label={capabilityTooltipAriaLabel(
                                  subagent.name,
                                  subagent.description,
                                  tooltipFields,
                                )}
                                onClick={isExternal ? undefined : () => {
                                  setPendingSubagentIds((prev) => {
                                    const current = prev ?? selectedAgentEnabledSubagentIds;
                                    return isOn
                                      ? current.filter((id) => id !== subagent.id)
                                      : [...current, subagent.id];
                                  });
                                }}
                              >
                                <span className="agent-card__token-name">
                                  {subagent.name}{isExternal ? ` · ${t('filters.external')}` : ''}
                                </span>
                              </button>
                            </span>
                          </AgentCapabilityTooltip>
                        );
                      })}
                    </div>
                  ) : (
                    <div className="agent-card__chip-grid">
                      {selectedAgentEnabledSubagents.length === 0 ? (
                        <span className="agent-card__empty-inline">
                          {t('agentsOverview.noSubagents')}
                        </span>
                      ) : (
                        selectedAgentEnabledSubagents.map((subagent: SubagentInfo) => {
                          const tooltipFields = subagentTooltipFields(
                            subagent,
                            t,
                            !isLocallyManageableSubagent(subagent),
                          );
                          return (
                            <AgentCapabilityTooltip
                              key={subagent.key}
                              title={subagent.name}
                              description={subagent.description}
                              fields={tooltipFields}
                            >
                              <span className="agent-card__chip">{subagent.name}</span>
                            </AgentCapabilityTooltip>
                          );
                        })
                      )}
                    </div>
                  )
                ) : null}
              </div>
            ) : null}
            {canManageCustomAgent ? (
              <div className="agent-card__section">
                <div className="agent-card__section-head">
                  <div className="agent-card__section-title">
                    <span>{t('agentsOverview.customActions')}</span>
                  </div>
                </div>
                <div className="agent-card__section-actions" style={{ gap: 8 }}>
                  <Button
                    variant="secondary"
                    size="small"
                    onClick={() => {
                      const id = selectedAgent?.id;
                      closeAgentDetails();
                      if (id) openEditAgent(id);
                    }}
                  >
                    <Pencil size={12} style={{ marginRight: 6 }} />
                    {t('agentsOverview.editAgent')}
                  </Button>
                  <Button
                    variant="secondary"
                    size="small"
                    isLoading={deletingAgent}
                    onClick={() => void handleDeleteCustomAgent()}
                  >
                    <Trash2 size={12} style={{ marginRight: 6 }} />
                    {t('agentsOverview.deleteAgent')}
                  </Button>
                </div>
              </div>
            ) : null}
            {(selectedAgent.source ?? selectedAgent.subagentSource) === 'external' ? (
              <div className="agent-card__section">
                <div className="agent-card__section-head">
                  <div className="agent-card__section-title">
                    <span>{t('agentsOverview.externalActions')}</span>
                  </div>
                </div>
                <div className="agent-card__section-actions">
                  <Button
                    variant="secondary"
                    size="small"
                    onClick={() => {
                      setSettingsTab('external-sources');
                      closeAgentDetails();
                      openScene('settings');
                    }}
                  >
                    <Puzzle size={12} style={{ marginRight: 6 }} />
                    {t('agentsOverview.manageExternalAgent')}
                  </Button>
                </div>
              </div>
            ) : null}
          </>
        ) : null}
      </GalleryDetailModal>
    </GalleryLayout>
  );
};

const AgentsScene: React.FC = () => {
  const { page, openHome } = useAgentsStore();

  useEffect(() => {
    return () => {
      openHome();
    };
  }, [openHome]);

  if (page === 'createAgent') {
    return (
      <div className="bitfun-agents-scene bitfun-agents-scene--page">
        <CreateAgentPage />
      </div>
    );
  }

  return <AgentsHomeView />;
};

export default AgentsScene;
