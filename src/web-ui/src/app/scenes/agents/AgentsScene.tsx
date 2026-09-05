import { Button, Combobox, Icon, IconButton, SearchField, Select, StatusPill, Tooltip, ScrollArea } from '@openbitfun/ui';
import React, { useCallback, useEffect, useMemo, useState } from 'react';
import type { TFunction } from 'i18next';
import { Bot, Cpu, FileText, MessageSquareText, RotateCcw, Wrench, type LucideIcon } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { useI18n } from '@/infrastructure/i18n/hooks/useI18n';
import { confirmDanger } from '@/infrastructure/confirm-dialog';
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
import AgentHarnessOverview from './components/AgentHarnessOverview';
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
  type AgentFilterLevel,
  type AgentFilterType,
  type AgentWithCapabilities,
  useAgentsStore,
} from './agentsStore';
import { useAgentsList } from './hooks/useAgentsList';
import { AGENT_ICON_MAP } from './agentsIcons';
import { CAPABILITY_ACCENT, CORE_AGENT_ACCENTS, DEFAULT_CORE_AGENT_ACCENT } from './agentAppearance';
import { getCardGradient } from '@/shared/utils/cardGradients';
import { isAgentProfileConfigurableToolName } from './agentToolVisibility';
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
import { useComputerUseEnabled } from '@/infrastructure/config/hooks/useComputerUseEnabled';
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
import { openEcosystemCompatibility } from '@/app/scenes/ecosystem-compatibility/ecosystemCompatibilityStore';

const DEFAULT_SUBAGENT_MODEL_OVERRIDE_VALUE = '__default_subagent_model__';

type CapabilityTab = 'model' | 'tools' | 'skills' | 'subagents';
type AgentDetailSection = 'basic' | 'behavior' | CapabilityTab;

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
  const { t: tComponents } = useI18n('components');
  const notification = useNotification();
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
  const [activeDetailSection, setActiveDetailSection] = React.useState<AgentDetailSection>('basic');
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
  const { computerUseEnabled } = useComputerUseEnabled();
  const { buildModelOption } = useModelSelectPresentation();
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
    toolCatalogStatus,
    configuredModels = [],
    getModeProfile,
    getAgentSkills,
    getModeManageableSubagents,
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

  // Tool-catalog load state from the host (available / unsupported / failed /
  // empty). When the host doesn't expose a catalog or the read failed, the
  // tools tab must say so instead of rendering as "no tools". Writes are gated
  // off too — toggling against a failed catalog would save a config the host
  // can't act on. See PR #2428 round 5 #2.
  const toolCatalogWritable = toolCatalogStatus === 'available' || toolCatalogStatus === 'empty';
  const toolCatalogMessage = toolCatalogStatus === 'unsupported'
    ? t('agentsOverview.toolsUnsupported')
    : toolCatalogStatus === 'failed'
      ? t('agentsOverview.toolsFailed')
      : null;

  useGallerySceneAutoRefresh({
    sceneId: 'agents',
    refetch: () => {
      void loadAgents();
    },
  });

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

  const coreAgents = useMemo(
    () => filteredAgents.filter((agent) => CORE_AGENT_IDS.has(agent.id)),
    [filteredAgents],
  );

  const visibleAgents = useMemo(
    () => filteredAgents.filter((agent) => isAgentInOverviewZone(agent, hiddenAgentIds)),
    [filteredAgents, hiddenAgentIds],
  );

  const catalogAgents = useMemo(
    () => [...coreAgents, ...visibleAgents],
    [coreAgents, visibleAgents],
  );

  const sourceFilterOptions = useMemo(() => [
    { value: 'all', label: t('filters.anySource') },
    { value: 'builtin', label: t('filters.builtin') },
    { value: 'user', label: t('filters.user') },
    { value: 'project', label: t('filters.project') },
    { value: 'external', label: t('filters.external') },
  ], [t]);

  const typeFilterOptions = useMemo(() => [
    { value: 'all', label: t('filters.anyKind') },
    { value: 'mode', label: t('filters.mode') },
    { value: 'subagent', label: t('filters.subagent') },
  ], [t]);

  const renderSkeletons = (prefix: string) => (
    <GallerySkeleton
      count={6}
      cardHeight={148}
      minCardWidth={300}
      className={`${prefix}-skeleton`}
    />
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
    () => selectedAgentConfiguredTools.filter(isAgentProfileConfigurableToolName),
    [selectedAgentConfiguredTools],
  );
  const agentProfileAvailableTools = useMemo(
    () => availableTools.filter((tool) => isAgentProfileConfigurableToolName(tool.name)),
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
      return configuredTools.filter(isAgentProfileConfigurableToolName).length;
    }
    return agent.toolCount ?? 0;
  }, [getModeConfig]);
  const getDisplayedSkillCount = useCallback((agent: AgentWithCapabilities): number => {
    const configuredTools = agent.agentKind === 'mode'
      ? (getModeConfig(agent.id)?.enabled_tools ?? agent.defaultTools ?? [])
      : (agent.defaultTools ?? []);
    return hasSkillTool(configuredTools)
      ? getConfiguredEnabledSkillKeys(getAgentSkills(agent.id)).length
      : 0;
  }, [getAgentSkills, getModeConfig]);
  const getDisplayedSubagentCount = useCallback((agent: AgentWithCapabilities): number => {
    if (agent.agentKind !== 'mode') {
      return 0;
    }
    const configuredTools = getModeConfig(agent.id)?.enabled_tools ?? agent.defaultTools ?? [];
    return hasTaskTool(configuredTools) ? (agent.visibleSubagentCount ?? 0) : 0;
  }, [getModeConfig]);
  const selectedAgentSourceLabel = selectedAgent
    ? subagentSourceLabel(selectedAgent.source ?? selectedAgent.subagentSource, t)
    : '';
  const selectedAgentBadge = selectedAgent
    ? getAgentBadge(
      t,
      selectedAgent.agentKind,
      selectedAgent.source ?? selectedAgent.subagentSource,
    )
    : null;
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
    value: string | number,
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
      icon: LucideIcon;
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
          ? (pendingTools ?? selectedAgentTools).length
          : selectedAgentTools.length)
        : selectedAgentTools.length;
      const totalToolCount = selectedAgent?.agentKind === 'mode'
        ? agentProfileAvailableTools.length
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
        icon: (({ size = 14 }: { size?: number | string }) => {
          const n = typeof size === 'number' ? size : 14;
          const mapped = n <= 11 ? '2xs' : n <= 13 ? 'xs' : n <= 15 ? 'sm' : n <= 17 ? 'md' : 'lg';
          return <Icon name="extension" size={mapped} />;
        }) as LucideIcon,
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
    agentProfileAvailableTools.length,
    pendingSkills,
    pendingSubagentIds,
    pendingTools,
    selectedAgent,
    selectedAgentIsExternal,
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
    if (selectedAgentCapabilityTabs.some((tab) => tab.key === activeDetailSection)) {
      return activeDetailSection as CapabilityTab;
    }
    return selectedAgentCapabilityTabs[0]?.key ?? 'tools';
  }, [activeDetailSection, selectedAgentCapabilityTabs]);
  const currentCapabilityMeta = selectedAgentCapabilityTabs.find(
    (tab) => tab.key === currentCapabilityTab,
  );
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
    setActiveDetailSection('basic');
    resetEditState();
  }, [resetEditState]);

  const closeAgentDetails = useCallback(() => {
    setSelectedAgentId(null);
    setActiveDetailSection('basic');
    resetEditState();
  }, [resetEditState]);

  useEffect(() => {
    if (
      activeDetailSection !== 'basic'
      && activeDetailSection !== 'behavior'
      && !selectedAgentCapabilityTabs.some((tab) => tab.key === activeDetailSection)
    ) {
      setActiveDetailSection('basic');
    }
  }, [activeDetailSection, selectedAgentCapabilityTabs]);

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
      // CustomAgentAPI emits `custom-agent:updated` after the delete; the
      // useAgentsList subscriber owns the single refresh so two overlapping
      // catalog loads cannot race their status snapshots.
    } catch (e) {
      notification.error(
        `${t('agentsOverview.deleteFailed')}${e instanceof Error ? e.message : String(e)}`,
      );
    } finally {
      setDeletingAgent(false);
    }
  }, [selectedAgent, closeAgentDetails, notification, t, workspacePath]);

  const canManageCustomAgent = Boolean(
    selectedAgent
    && !['builtin', 'external'].includes(
      selectedAgent.source ?? selectedAgent.subagentSource ?? 'builtin',
    ),
  );

  return (
    <GalleryLayout
      className="openbitfun-agents-scene"
      data-testid="agent-skill-panel"
      data-openbitfun-scene="agents"
      data-openbitfun-part="root"
    >
      <GalleryPageHeader
        title={t('page.title')}
        subtitle={t('page.subtitle')}
        actions={(
          <>
            <SearchField
              className="openbitfun-agents-scene__search"
              value={searchQuery}
              onValueChange={setSearchQuery}
              leadingIcon={<Icon name="search" size="sm" aria-hidden />}
              placeholder={t('page.searchPlaceholder')}
              aria-label={t('page.searchPlaceholder')}
              size="sm"
              clearLabel={searchQuery ? tComponents('search.clear') : undefined}
              onClear={searchQuery ? () => setSearchQuery('') : undefined}
              data-testid="agents-search"
            />
            <Button
              variant="fill"
              size="sm"
              leadingIcon={<Icon name="plus" size="sm" />}
              onClick={openCreateAgent}
              data-testid="agents-create-agent-btn"
            >
              {t('page.newAgent')}
            </Button>
          </>
        )}
      />

      <div className="gallery-zones" data-openbitfun-scene="agents" data-openbitfun-part="zones" data-testid="agent-list">
        <AgentHarnessOverview />

        <GalleryZone
          id="agents-zone"
          data-testid="agents-catalog-zone"
          title={t('agentsZone.title')}
          subtitle={t('agentsZone.subtitle')}
          tools={(
            <>
              <div className="openbitfun-agents-scene__agent-filters" data-openbitfun-scene="agents" data-openbitfun-part="filters">
                <div
                  className="openbitfun-agents-scene__agent-filter-group"
                  data-testid="agents-source-filter"
                >
                  <span className="openbitfun-agents-scene__agent-filter-label">
                    {t('filters.source')}
                  </span>
                  <Select
                    className="openbitfun-agents-scene__agent-filter-select"
                    size="sm"
                    value={agentFilterLevel}
                    options={sourceFilterOptions}
                    aria-label={t('filters.source')}
                    onValueChange={(value) => setAgentFilterLevel(
                      normalizeSelectValue(value) as AgentFilterLevel,
                    )}
                  />
                </div>
                <div
                  className="openbitfun-agents-scene__agent-filter-group"
                  data-testid="agents-kind-filter"
                >
                  <span className="openbitfun-agents-scene__agent-filter-label">
                    {t('filters.kind')}
                  </span>
                  <Select
                    className="openbitfun-agents-scene__agent-filter-select"
                    size="sm"
                    value={agentFilterType}
                    options={typeFilterOptions}
                    aria-label={t('filters.kind')}
                    onValueChange={(value) => setAgentFilterType(
                      normalizeSelectValue(value) as AgentFilterType,
                    )}
                  />
                </div>
              </div>
              <span className="gallery-zone-count">{catalogAgents.length}</span>
            </>
          )}
        >
          {loading ? renderSkeletons('agent') : null}

          {!loading && catalogAgents.length === 0 ? (
            <GalleryEmpty
              icon={<Bot size={32} strokeWidth={1.5} />}
              message={allAgents.length === 0 ? t('agentsZone.empty.noAgents') : t('agentsZone.empty.noMatch')}
              testId="agent-list-empty"
            />
          ) : null}

          {!loading && catalogAgents.length > 0 ? (
            <GalleryGrid
              minCardWidth={300}
              data-openbitfun-scene="agents"
              data-openbitfun-part="catalogGrid"
            >
              {catalogAgents.map((agent, index) => {
                const commonCardProps = {
                  agent,
                  index,
                  toolCount: getDisplayedToolCount(agent),
                  skillCount: getDisplayedSkillCount(agent),
                  subagentCount: getDisplayedSubagentCount(agent),
                  onOpenDetails: openAgentDetails,
                };

                if (CORE_AGENT_IDS.has(agent.id)) {
                  return (
                    <CoreAgentCard
                      key={agent.id}
                      {...commonCardProps}
                      meta={coreAgentMeta[agent.id] ?? {
                        role: agent.name,
                        ...DEFAULT_CORE_AGENT_ACCENT,
                      }}
                      disabledReason={
                        agent.id === 'ComputerUse' && !computerUseEnabled
                          ? t('coreAgentsZone.computerUseDisabledBadge')
                          : undefined
                      }
                    />
                  );
                }

                return <AgentCard key={agent.id} {...commonCardProps} />;
              })}
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
        titlePlacement="hero"
        size="2xl"
        stableHeight
        badges={selectedAgent ? (
          <>
            <StatusPill
              tone={selectedAgentBadge?.variant ?? 'neutral'}
              leading={selectedAgent.agentKind === 'mode' ? <Cpu size={10} /> : <Bot size={10} />}
            >
              {selectedAgentBadge?.label}
            </StatusPill>
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
            <span>{selectedAgentSourceLabel}</span>
            {selectedAgent.externalProviderLabel ? (
              <span>{t('agentCard.meta.externalProvider', { provider: selectedAgent.externalProviderLabel })}</span>
            ) : null}
            {selectedAgent.supportsFollowUp === false ? (
              <span>{t('agentCard.meta.singleRun')}</span>
            ) : null}
          </>
        ) : null}
        heroActions={selectedAgent && canManageCustomAgent ? (
          <>
            <Button
              variant="outline"
              size="sm"
              leadingIcon={<Icon name="edit" size="lg" />}
              onClick={() => {
                const id = selectedAgent.id;
                closeAgentDetails();
                openEditAgent(id);
              }}
            >
              {t('agentsOverview.editAgent')}
            </Button>
            <Tooltip content={t('agentsOverview.deleteAgent')}>
              <IconButton
                aria-label={t('agentsOverview.deleteAgent')}
                size="sm"
                loading={deletingAgent}
                onClick={() => void handleDeleteCustomAgent()}
                icon={<Icon name="delete" size="sm" />}
              />
            </Tooltip>
          </>
        ) : selectedAgent && selectedAgentIsExternal ? (
          <Button
            variant="outline"
            size="sm"
            leadingIcon={<Icon name="extension" size="lg" />}
            onClick={() => {
              closeAgentDetails();
              openEcosystemCompatibility({ ownerSurface: 'external-sources' });
            }}
          >
            {t('agentsOverview.manageExternalAgent')}
          </Button>
        ) : null}
      >
        {selectedAgent ? (
          <div className="agent-card__configuration" data-testid="agent-detail-configuration">
                <nav className="agent-card__config-nav" aria-label={t('agentsOverview.detail.configuration')}>
                  <button
                    type="button"
                    className={`agent-card__config-nav-item${activeDetailSection === 'basic' ? ' is-active' : ''}`}
                    aria-current={activeDetailSection === 'basic' ? 'page' : undefined}
                    onClick={() => setActiveDetailSection('basic')}
                  >
                    <FileText size={14} />
                    <span>{t('agentsOverview.detail.basicInfo')}</span>
                  </button>
                  <button
                    type="button"
                    className={`agent-card__config-nav-item${activeDetailSection === 'behavior' ? ' is-active' : ''}`}
                    aria-current={activeDetailSection === 'behavior' ? 'page' : undefined}
                    onClick={() => setActiveDetailSection('behavior')}
                  >
                    <MessageSquareText size={14} />
                    <span>{t('agentsOverview.detail.behaviorContext')}</span>
                    <span className="agent-card__config-nav-count">{selectedAgent.capabilities.length}</span>
                  </button>
                  <div className="agent-card__config-nav-divider" />
                  {selectedAgentCapabilityTabs.map((tab) => {
                    const TabIcon = tab.icon;
                    const isActive = activeDetailSection === tab.key;
                    return (
                      <button
                        key={tab.key}
                        type="button"
                        className={`agent-card__config-nav-item${isActive ? ' is-active' : ''}`}
                        aria-current={isActive ? 'page' : undefined}
                        data-detail-section={tab.key}
                        onClick={() => setActiveDetailSection(tab.key)}
                      >
                        <TabIcon size={14} />
                        <span>{tab.label}</span>
                        {tab.count ? <span className="agent-card__config-nav-count">{tab.count}</span> : null}
                      </button>
                    );
                  })}
                </nav>

                <ScrollArea className="agent-card__config-main">
                  {activeDetailSection === 'basic' ? (
                    <section className="agent-card__config-panel" data-testid="agent-detail-basic-section">
                      <div className="agent-card__config-panel-head">
                        <div>
                          <h3>{t('agentsOverview.detail.basicInfo')}</h3>
                          <p>{t('agentsOverview.detail.basicInfoHint')}</p>
                        </div>
                      </div>
                      <div className="agent-card__field-grid">
                        <div className="agent-card__field">
                          <span>{t('agentsOverview.detail.name')}</span>
                          <strong>{selectedAgent.name}</strong>
                        </div>
                        <div className="agent-card__field">
                          <span>{t('agentsOverview.detail.source')}</span>
                          <strong>{selectedAgentSourceLabel}</strong>
                        </div>
                        <div className="agent-card__field">
                          <span>{t('agentsOverview.detail.type')}</span>
                          <strong>{selectedAgentBadge?.label}</strong>
                        </div>
                        <div className="agent-card__field">
                          <span>{t('agentsOverview.detail.followUp')}</span>
                          <strong>
                            {selectedAgent.supportsFollowUp === false
                              ? t('agentsOverview.detail.unsupported')
                              : t('agentsOverview.detail.supported')}
                          </strong>
                        </div>
                        <div className="agent-card__field agent-card__field--wide">
                          <span>{t('agentsOverview.detail.description')}</span>
                          <p>{getAgentDescription(t, selectedAgent)}</p>
                        </div>
                      </div>
                    </section>
                  ) : null}

                  {activeDetailSection === 'behavior' ? (
                    <div className="agent-card__config-panel" data-testid="agent-detail-behavior-section">
                      <div className="agent-card__config-panel-head">
                        <div>
                          <h3>{t('agentsOverview.detail.behaviorContext')}</h3>
                          <p>{t('agentsOverview.detail.behaviorContextHint')}</p>
                        </div>
                      </div>
                      <div
              className="agent-card__section agent-card__section--capabilities"
              data-testid="agent-detail-capabilities-section"
            >
              <div className="agent-card__section-head">
                <div className="agent-card__section-title">
                  <span>{t('agentsOverview.capabilities')}</span>
                  <span className="agent-card__section-count">
                    {selectedAgent.capabilities.length}
                  </span>
                </div>
              </div>
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
            </div>

            {selectedAgent.agentKind === 'mode' && selectedAgentUsesSharedProfile ? (
              <div className="agent-card__section" data-openbitfun-scene="agents" data-openbitfun-part="detailSection">
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
                    </div>
                  ) : null}

                  {selectedAgentCapabilityTabs.some((tab) => tab.key === activeDetailSection) ? (
              <div className="agent-card__section" data-testid="agent-detail-tools-section">
                <div className="agent-card__section-head agent-card__section-head--tabs">
                  <div className="agent-card__section-title">
                    {currentCapabilityMeta ? React.createElement(currentCapabilityMeta.icon, { size: 14 }) : null}
                    <span>{currentCapabilityMeta?.label}</span>
                    {currentCapabilityMeta?.count ? (
                      <span className="agent-card__section-count">{currentCapabilityMeta.count}</span>
                    ) : null}
                  </div>
                  {canManageCurrentCapability ? (
                    <div className="agent-card__section-actions">
                      {isCurrentTabEditing ? (
                        <>
                          <Tooltip content={
                              currentCapabilityTab === 'tools'
                                ? t('agentsOverview.toolsReset')
                                : currentCapabilityTab === 'skills'
                                  ? t('agentsOverview.reset')
                                  : t('agentsOverview.reset')
                            }>
                            <IconButton
                              aria-label={
                                currentCapabilityTab === 'tools'
                                  ? t('agentsOverview.toolsReset')
                                  : currentCapabilityTab === 'skills'
                                    ? t('agentsOverview.reset')
                                    : t('agentsOverview.reset')
                              }
                              size="sm"
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
                              icon={<RotateCcw size={12} />}
                            />
                          </Tooltip>
                          <Button
                            variant="outline"
                            size="sm"
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
                            variant="fill"
                            size="sm"
                            loading={
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
                          variant="outline"
                          size="sm"
                          disabled={currentCapabilityTab === 'tools' && !toolCatalogWritable}
                          onClick={() => {
                            if (currentCapabilityTab === 'tools') {
                              if (!toolCatalogWritable) return;
                              setPendingTools([...selectedAgentTools]);
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
                  <Combobox
                    size="sm"
                    className="openbitfun-agents-scene__subagent-model-select"
                    options={subagentModelOptions}
                    value={selectedSubagentModelValue}
                    onValueChange={(value) => void handleSubagentModelChange(value)}
                    disabled={savingSubagentModel}
                    data-testid="agent-detail-subagent-model-select"
                  />
                ) : null}

                {currentCapabilityTab === 'tools' ? (
                  toolCatalogMessage ? (
                    <span className="agent-card__empty-inline" data-testid="agent-detail-tools-catalog-status">
                      {toolCatalogMessage}
                    </span>
                  ) : selectedAgent.agentKind === 'mode' && toolsEditing ? (
                    <ToolGroupPicker
                      tools={agentProfileAvailableTools}
                      selectedToolNames={pendingTools ?? selectedAgentTools}
                      userGroups={userToolGroups}
                      onSelectionChange={setPendingTools}
                      onSaveUserGroups={saveUserToolGroups}
                      disabled={savingTools || !toolCatalogWritable}
                      testId="agent-detail-tool-groups"
                    />
                  ) : (
                    <ToolGroupSummary
                      tools={agentProfileAvailableTools}
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
              </ScrollArea>
            </div>
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
      <div className="openbitfun-agents-scene openbitfun-agents-scene--page" data-openbitfun-scene="agents" data-openbitfun-part="root">
        <CreateAgentPage />
      </div>
    );
  }

  return <AgentsHomeView />;
};

export default AgentsScene;
