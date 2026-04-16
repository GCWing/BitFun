import React, { useCallback, useMemo, useState } from 'react';
import type { TFunction } from 'i18next';
import {
  Bot,
  Cpu,
  Pencil,
  Plus,
  Puzzle,
  RefreshCw,
  Search as SearchIcon,
  Trash2,
  Wrench,
} from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Badge, Button, IconButton, Search, Switch, confirmDanger } from '@/component-library';
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
  type AgentWithCapabilities,
  useAgentsStore,
} from './agentsStore';
import { useAgentsList } from './hooks/useAgentsList';
import { AGENT_ICON_MAP, CAPABILITY_ACCENT } from './agentsIcons';
import { getCardGradient } from '@/shared/utils/cardGradients';
import { getAgentBadge } from './utils';
import './AgentsView.scss';
import './AgentsScene.scss';
import { useGallerySceneAutoRefresh } from '@/app/hooks/useGallerySceneAutoRefresh';
import { CORE_AGENT_IDS, isAgentInOverviewZone } from './agentVisibility';
import { SubagentAPI } from '@/infrastructure/api/service-api/SubagentAPI';
import type { ModeSkillInfo } from '@/infrastructure/config/types';
import { useNotification } from '@/shared/notification-system';

const UNGROUPED_SKILL_GROUP = '__ungrouped__';

const SKILL_GROUP_ORDER: Record<string, number> = {
  office: 0,
  meta: 1,
  team: 2,
  [UNGROUPED_SKILL_GROUP]: 99,
};

interface SkillGroup {
  key: string;
  label: string;
  skills: ModeSkillInfo[];
  enabledCount: number;
  totalCount: number;
}

function getConfiguredEnabledSkillKeys(skills: ModeSkillInfo[]): string[] {
  return skills.filter((skill) => !skill.disabledByMode).map((skill) => skill.key);
}

function modeHasSkillTool(enabledTools: string[]): boolean {
  return enabledTools.includes('Skill');
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

function getSkillGroupKey(skill: ModeSkillInfo): string {
  return skill.groupKey?.trim() || UNGROUPED_SKILL_GROUP;
}

function getSkillGroupLabel(groupKey: string, t: TFunction<'scenes/agents'>): string {
  switch (groupKey) {
    case 'office':
      return t('agentsOverview.skillGroups.office');
    case 'computer-use':
      return t('agentsOverview.skillGroups.computerUse');
    case 'meta':
      return t('agentsOverview.skillGroups.meta');
    case 'team':
      return t('agentsOverview.skillGroups.team');
    case 'superpowers':
      return t('agentsOverview.skillGroups.superpowers');
    default:
      return t('agentsOverview.skillGroups.other');
  }
}

function getSkillTitle(skill: ModeSkillInfo, t: TFunction<'scenes/agents'>): string {
  return [
    skill.description || skill.name,
    `key: ${skill.key}`,
    !skill.disabledByMode && !skill.selectedForRuntime
      ? t('agentsOverview.skillShadowed')
      : null,
  ].filter(Boolean).join('\n');
}

function buildSkillGroups(
  skills: ModeSkillInfo[],
  enabledSkillKeys: string[],
  t: TFunction<'scenes/agents'>,
): SkillGroup[] {
  const enabledSkillKeySet = new Set(enabledSkillKeys);
  const groups = new Map<string, ModeSkillInfo[]>();

  for (const skill of skills) {
    const groupKey = getSkillGroupKey(skill);
    const items = groups.get(groupKey);
    if (items) {
      items.push(skill);
    } else {
      groups.set(groupKey, [skill]);
    }
  }

  return [...groups.entries()]
    .map(([groupKey, groupSkills]) => ({
      key: groupKey,
      label: getSkillGroupLabel(groupKey, t),
      skills: [...groupSkills].sort((a, b) => {
        const aEnabled = enabledSkillKeySet.has(a.key);
        const bEnabled = enabledSkillKeySet.has(b.key);
        if (aEnabled && !bEnabled) return -1;
        if (!aEnabled && bEnabled) return 1;
        return a.name.localeCompare(b.name) || a.key.localeCompare(b.key);
      }),
      enabledCount: groupSkills.filter((skill) => enabledSkillKeySet.has(skill.key)).length,
      totalCount: groupSkills.length,
    }))
    .sort((a, b) => {
      const orderDiff = (SKILL_GROUP_ORDER[a.key] ?? 50) - (SKILL_GROUP_ORDER[b.key] ?? 50);
      if (orderDiff !== 0) {
        return orderDiff;
      }
      return a.label.localeCompare(b.label);
    });
}

const AgentsHomeView: React.FC = () => {
  const { t } = useTranslation('scenes/agents');
  const notification = useNotification();
  const [deletingAgent, setDeletingAgent] = useState(false);
  const {
    agentSoloEnabled,
    searchQuery,
    agentFilterLevel,
    agentFilterType,
    setSearchQuery,
    setAgentFilterLevel,
    setAgentFilterType,
    setAgentSoloEnabled,
    openCreateAgent,
    openEditAgent,
  } = useAgentsStore();
  const [selectedAgentId, setSelectedAgentId] = React.useState<string | null>(null);
  const [toolsEditing, setToolsEditing] = React.useState(false);
  const [skillsEditing, setSkillsEditing] = React.useState(false);
  const [pendingTools, setPendingTools] = React.useState<string[] | null>(null);
  const [pendingSkills, setPendingSkills] = React.useState<string[] | null>(null);
  const [savingTools, setSavingTools] = React.useState(false);
  const [savingSkills, setSavingSkills] = React.useState(false);

  const {
    allAgents,
    filteredAgents,
    loading,
    availableTools,
    getModeSkills,
    counts,
    loadAgents,
    getModeConfig,
    handleSetTools,
    handleResetTools,
    handleSetSkills,
  } = useAgentsList({
    searchQuery,
    filterLevel: agentFilterLevel,
    filterType: agentFilterType,
    t,
  });

  useGallerySceneAutoRefresh({
    sceneId: 'agents',
    refetch: () => void loadAgents(),
  });

  const coreAgentMeta = useMemo((): Record<string, CoreAgentMeta> => ({
    agentic: {
      role: t('coreAgentsZone.modes.agentic.role'),
      accentColor: '#6366f1',
      accentBg: 'rgba(99,102,241,0.10)',
    },
    Cowork: {
      role: t('coreAgentsZone.modes.cowork.role'),
      accentColor: '#14b8a6',
      accentBg: 'rgba(20,184,166,0.10)',
    },
  }), [t]);

  const coreAgents = useMemo(() => allAgents.filter((agent) => CORE_AGENT_IDS.has(agent.id)), [allAgents]);

  const visibleAgents = useMemo(
    () => filteredAgents.filter(isAgentInOverviewZone),
    [filteredAgents],
  );

  const scrollToZone = useCallback((targetId: string) => {
    document.getElementById(targetId)?.scrollIntoView({ behavior: 'smooth', block: 'start' });
  }, []);

  const levelFilters = [
    { key: 'builtin', label: t('filters.builtin'), count: counts.builtin },
    { key: 'user', label: t('filters.user'), count: counts.user },
    { key: 'project', label: t('filters.project'), count: counts.project },
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
  const selectedAgentModeConfig = useMemo(
    () => (selectedAgent?.agentKind === 'mode' ? getModeConfig(selectedAgent.id) : null),
    [getModeConfig, selectedAgent],
  );
  const selectedAgentModeSkills = useMemo(
    () => (selectedAgent?.agentKind === 'mode' ? getModeSkills(selectedAgent.id) : []),
    [getModeSkills, selectedAgent],
  );
  const selectedAgentTools = selectedAgent?.agentKind === 'mode'
    ? (selectedAgentModeConfig?.enabled_tools ?? selectedAgent.defaultTools ?? [])
    : (selectedAgent?.defaultTools ?? []);
  const selectedAgentHasSkillTool = selectedAgent?.agentKind === 'mode'
    ? modeHasSkillTool(selectedAgentTools)
    : false;
  const selectedAgentSkills = useMemo(
    () => getConfiguredEnabledSkillKeys(selectedAgentModeSkills),
    [selectedAgentModeSkills],
  );
  const selectedAgentSkillItems = useMemo(
    () => selectedAgentModeSkills.filter((skill) => !skill.disabledByMode),
    [selectedAgentModeSkills],
  );
  const selectedAgentSkillGroups = useMemo(
    () => buildSkillGroups(selectedAgentModeSkills, selectedAgentSkills, t),
    [selectedAgentModeSkills, selectedAgentSkills, t],
  );
  const editableSkillGroups = useMemo(
    () => buildSkillGroups(selectedAgentModeSkills, pendingSkills ?? selectedAgentSkills, t),
    [pendingSkills, selectedAgentModeSkills, selectedAgentSkills, t],
  );
  const selectedAgentDuplicateSkillNames = useMemo(
    () => buildDuplicateSkillNameSet(selectedAgentModeSkills),
    [selectedAgentModeSkills],
  );
  const getDisplayedToolCount = useCallback((agent: AgentWithCapabilities): number => {
    if (agent.agentKind === 'mode') {
      return getModeConfig(agent.id)?.enabled_tools?.length
        ?? agent.defaultTools?.length
        ?? agent.toolCount
        ?? 0;
    }
    return agent.toolCount ?? agent.defaultTools?.length ?? 0;
  }, [getModeConfig]);
  const selectedAgentToolCount = selectedAgent ? getDisplayedToolCount(selectedAgent) : 0;
  const resetEditState = useCallback(() => {
    setToolsEditing(false);
    setSkillsEditing(false);
    setPendingTools(null);
    setPendingSkills(null);
    setSavingTools(false);
    setSavingSkills(false);
  }, []);

  const togglePendingSkill = useCallback((skillKey: string) => {
    setPendingSkills((prev) => {
      const current = prev ?? selectedAgentSkills;
      return current.includes(skillKey)
        ? current.filter((key) => key !== skillKey)
        : [...current, skillKey];
    });
  }, [selectedAgentSkills]);

  const setPendingSkillGroupEnabled = useCallback((skills: ModeSkillInfo[], enabled: boolean) => {
    setPendingSkills((prev) => {
      const current = prev ?? selectedAgentSkills;
      const groupKeys = new Set(skills.map((skill) => skill.key));

      if (!enabled) {
        return current.filter((key) => !groupKeys.has(key));
      }

      const next = [...current];
      for (const skill of skills) {
        if (!next.includes(skill.key)) {
          next.push(skill.key);
        }
      }
      return next;
    });
  }, [selectedAgentSkills]);

  const openAgentDetails = useCallback((agent: AgentWithCapabilities) => {
    setSelectedAgentId(agent.id);
    resetEditState();
  }, [resetEditState]);

  const closeAgentDetails = useCallback(() => {
    setSelectedAgentId(null);
    resetEditState();
  }, [resetEditState]);

  const handleDeleteCustomAgent = useCallback(async () => {
    if (!selectedAgent) return;
    if (
      selectedAgent.agentKind !== 'subagent'
      || (selectedAgent.subagentSource !== 'user' && selectedAgent.subagentSource !== 'project')
    ) {
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
      await SubagentAPI.deleteSubagent(id);
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
  }, [selectedAgent, closeAgentDetails, loadAgents, notification, t]);

  const canManageCustomSubagent = Boolean(
    selectedAgent
    && selectedAgent.agentKind === 'subagent'
    && (selectedAgent.subagentSource === 'user' || selectedAgent.subagentSource === 'project'),
  );

  return (
    <GalleryLayout className="bitfun-agents-scene">
      <GalleryPageHeader
        title={t('page.title')}
        subtitle={t('page.subtitle')}
        extraContent={(
          <div className="gallery-anchor-bar">
            <button
              type="button"
              className="gallery-anchor-btn"
              onClick={() => scrollToZone('core-agents-zone')}
            >
              {t('nav.coreAgents')}
            </button>
            <button
              type="button"
              className="gallery-anchor-btn"
              onClick={() => scrollToZone('agents-zone')}
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
                >
                  <SearchIcon size={14} />
                </button>
              )}
            />
          </>
        )}
      />

      <div className="gallery-zones">
        <GalleryZone
          id="core-agents-zone"
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
            />
          ) : (
            <div className="core-agents-grid">
              {coreAgents.map((agent, index) => (
                <CoreAgentCard
                  key={agent.id}
                  agent={agent}
                  index={index}
                  meta={coreAgentMeta[agent.id] ?? { role: agent.name, accentColor: '#6366f1', accentBg: 'rgba(99,102,241,0.10)' }}
                  toolCount={getDisplayedToolCount(agent)}
                  skillCount={agent.agentKind === 'mode' && modeHasSkillTool(getModeConfig(agent.id)?.enabled_tools ?? agent.defaultTools ?? [])
                    ? getConfiguredEnabledSkillKeys(getModeSkills(agent.id)).length
                    : 0}
                  onOpenDetails={openAgentDetails}
                />
              ))}
            </div>
          )}
        </GalleryZone>

        <GalleryZone
          id="agents-zone"
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
            />
          ) : null}

          {!loading && visibleAgents.length > 0 ? (
            <GalleryGrid minCardWidth={360}>
              {visibleAgents.map((agent, index) => (
                <AgentCard
                  key={agent.id}
                  agent={agent}
                  index={index}
                  soloEnabled={agentSoloEnabled[agent.id] ?? agent.enabled}
                  toolCount={getDisplayedToolCount(agent)}
                  skillCount={agent.agentKind === 'mode' && modeHasSkillTool(getModeConfig(agent.id)?.enabled_tools ?? agent.defaultTools ?? [])
                    ? getConfiguredEnabledSkillKeys(getModeSkills(agent.id)).length
                    : 0}
                  onToggleSolo={setAgentSoloEnabled}
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
            <Badge variant={getAgentBadge(t, selectedAgent.agentKind, selectedAgent.subagentSource).variant}>
              {selectedAgent.agentKind === 'mode' ? <Cpu size={10} /> : <Bot size={10} />}
              {getAgentBadge(t, selectedAgent.agentKind, selectedAgent.subagentSource).label}
            </Badge>
            {!selectedAgent.enabled ? <Badge variant="neutral">{t('agentCard.badges.disabled')}</Badge> : null}
            {selectedAgent.model ? <Badge variant="neutral">{selectedAgent.model}</Badge> : null}
          </>
        ) : null}
        description={selectedAgent?.description}
        meta={selectedAgent ? (
          <>
            <span>{t('agentCard.meta.tools', { count: selectedAgentToolCount })}</span>
            {selectedAgent.agentKind === 'mode' && selectedAgentHasSkillTool ? (
              <span>{t('agentCard.meta.skills', { count: selectedAgentSkills.length })}</span>
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
                    {cap.category}
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

            {selectedAgentTools.length > 0 ? (
              <div className="agent-card__section">
                <div className="agent-card__section-head">
                  <div className="agent-card__section-title">
                    <Wrench size={12} />
                    <span>{t('agentsOverview.tools')}</span>
                    <span className="agent-card__section-count">
                      {selectedAgent.agentKind === 'mode'
                        ? `${(toolsEditing ? (pendingTools ?? selectedAgentTools) : selectedAgentTools).length}/${availableTools.length}`
                        : `${selectedAgentTools.length}`}
                    </span>
                  </div>
                  {selectedAgent.agentKind === 'mode' ? (
                    <div className="agent-card__section-actions">
                      {toolsEditing ? (
                        <>
                          <IconButton
                            size="small"
                            variant="ghost"
                            tooltip={t('agentsOverview.toolsReset')}
                            onClick={async () => {
                              await handleResetTools(selectedAgent.id);
                              setToolsEditing(false);
                              setPendingTools(null);
                            }}
                          >
                            <RefreshCw size={12} />
                          </IconButton>
                          <Button
                            variant="ghost"
                            size="small"
                            onClick={() => {
                              setToolsEditing(false);
                              setPendingTools(null);
                            }}
                          >
                            {t('agentsOverview.toolsCancel')}
                          </Button>
                          <Button
                            variant="primary"
                            size="small"
                            isLoading={savingTools}
                            onClick={async () => {
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
                            }}
                          >
                            {t('agentsOverview.toolsSave')}
                          </Button>
                        </>
                      ) : (
                        <Button
                          variant="secondary"
                          size="small"
                          onClick={() => {
                            setPendingTools([...selectedAgentTools]);
                            setToolsEditing(true);
                          }}
                        >
                          {t('agentsOverview.toolsEdit')}
                        </Button>
                      )}
                    </div>
                  ) : null}
                </div>

                {selectedAgent.agentKind === 'mode' && toolsEditing ? (
                  <div className="agent-card__token-grid">
                    {[...availableTools]
                      .sort((a, b) => {
                        const draft = pendingTools ?? selectedAgentTools;
                        const aOn = draft.includes(a.name);
                        const bOn = draft.includes(b.name);
                        if (aOn && !bOn) return -1;
                        if (!aOn && bOn) return 1;
                        return 0;
                      })
                      .map((tool) => {
                        const draft = pendingTools ?? selectedAgentTools;
                        const isOn = draft.includes(tool.name);
                        return (
                          <button
                            key={tool.name}
                            type="button"
                            className={`agent-card__token${isOn ? ' is-on' : ''}`}
                            title={tool.description || tool.name}
                            onClick={() => {
                              setPendingTools((prev) => {
                                const current = prev ?? selectedAgentTools;
                                return isOn
                                  ? current.filter((n) => n !== tool.name)
                                  : [...current, tool.name];
                              });
                            }}
                          >
                            <span className="agent-card__token-name">{tool.name}</span>
                          </button>
                        );
                      })}
                  </div>
                ) : (
                  <div className="agent-card__chip-grid">
                    {selectedAgentTools.map((tool) => (
                      <span key={tool} className="agent-card__chip" title={tool}>
                        {tool.replace(/_/g, ' ')}
                      </span>
                    ))}
                  </div>
                )}
              </div>
            ) : null}

            {selectedAgent.agentKind === 'mode' && selectedAgentHasSkillTool && selectedAgentModeSkills.length > 0 ? (
              <div className="agent-card__section">
                <div className="agent-card__section-head">
                  <div className="agent-card__section-title">
                    <Puzzle size={12} />
                    <span>{t('agentsOverview.skills')}</span>
                    <span className="agent-card__section-count">
                      {`${(skillsEditing ? (pendingSkills ?? selectedAgentSkills) : selectedAgentSkills).length}/${selectedAgentModeSkills.length}`}
                    </span>
                  </div>
                  <div className="agent-card__section-actions">
                    {skillsEditing ? (
                      <>
                        <Button
                          variant="ghost"
                          size="small"
                          onClick={() => {
                            setSkillsEditing(false);
                            setPendingSkills(null);
                          }}
                        >
                          {t('agentsOverview.skillsCancel')}
                        </Button>
                        <Button
                          variant="primary"
                          size="small"
                          isLoading={savingSkills}
                          onClick={async () => {
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
                          }}
                        >
                          {t('agentsOverview.skillsSave')}
                        </Button>
                      </>
                    ) : (
                      <Button
                        variant="secondary"
                        size="small"
                        onClick={() => {
                          setPendingSkills([...selectedAgentSkills]);
                          setSkillsEditing(true);
                        }}
                      >
                        {t('agentsOverview.skillsEdit')}
                      </Button>
                    )}
                  </div>
                </div>

                {skillsEditing ? (
                  <div className="agent-card__skill-groups">
                    {editableSkillGroups.map((group) => {
                      const allEnabled = group.enabledCount === group.totalCount;
                      const someEnabled = group.enabledCount > 0;

                      return (
                        <div key={group.key} className="agent-card__skill-group">
                          <div className="agent-card__skill-group-head">
                            <div className="agent-card__skill-group-title-wrap">
                              <span className="agent-card__skill-group-title">{group.label}</span>
                              <span className="agent-card__skill-group-count">
                                {`${group.enabledCount}/${group.totalCount}`}
                              </span>
                            </div>
                            <div
                              className="agent-card__skill-group-actions"
                              onClick={(e) => e.stopPropagation()}
                            >
                              <Switch
                                size="small"
                                checked={allEnabled}
                                onChange={(e) =>
                                  setPendingSkillGroupEnabled(group.skills, e.target.checked)
                                }
                                aria-label={
                                  allEnabled
                                    ? t('agentsOverview.disableGroup')
                                    : t('agentsOverview.enableGroup')
                                }
                              />
                              {someEnabled && !allEnabled ? (
                                <Button
                                  variant="ghost"
                                  size="small"
                                  onClick={() => setPendingSkillGroupEnabled(group.skills, false)}
                                >
                                  {t('agentsOverview.clearGroup')}
                                </Button>
                              ) : null}
                            </div>
                          </div>
                          <div className="agent-card__token-grid">
                            {group.skills.map((skill) => {
                              const isOn = (pendingSkills ?? selectedAgentSkills).includes(skill.key);
                              const displayName = formatSkillDisplayName(
                                skill,
                                selectedAgentDuplicateSkillNames,
                              );

                              return (
                                <button
                                  key={skill.key}
                                  type="button"
                                  className={`agent-card__token${isOn ? ' is-on' : ''}`}
                                  title={getSkillTitle(skill, t)}
                                  onClick={() => togglePendingSkill(skill.key)}
                                >
                                  <span className="agent-card__token-name">{displayName}</span>
                                </button>
                              );
                            })}
                          </div>
                        </div>
                      );
                    })}
                  </div>
                ) : (
                  <div className="agent-card__skill-groups">
                    {selectedAgentSkillItems.length === 0 ? (
                      <span className="agent-card__empty-inline">
                        {t('agentsOverview.noSkills')}
                      </span>
                    ) : (
                      selectedAgentSkillGroups
                        .filter((group) => group.enabledCount > 0)
                        .map((group) => (
                          <div key={group.key} className="agent-card__skill-group">
                            <div className="agent-card__skill-group-head">
                              <div className="agent-card__skill-group-title-wrap">
                                <span className="agent-card__skill-group-title">{group.label}</span>
                                <span className="agent-card__skill-group-count">
                                  {group.enabledCount}
                                </span>
                              </div>
                            </div>
                            <div className="agent-card__chip-grid">
                              {group.skills
                                .filter((skill) => !skill.disabledByMode)
                                .map((skill) => (
                                  <span
                                    key={skill.key}
                                    className="agent-card__chip"
                                    title={getSkillTitle(skill, t)}
                                  >
                                    {formatSkillDisplayName(skill, selectedAgentDuplicateSkillNames)}
                                  </span>
                                ))}
                            </div>
                          </div>
                        ))
                    )}
                  </div>
                )}
              </div>
            ) : null}

            {canManageCustomSubagent ? (
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
          </>
        ) : null}
      </GalleryDetailModal>
    </GalleryLayout>
  );
};

const AgentsScene: React.FC = () => {
  const { page } = useAgentsStore();

  if (page === 'createAgent') {
    return (
      <div className="bitfun-agents-scene">
        <CreateAgentPage />
      </div>
    );
  }

  return <AgentsHomeView />;
};

export default AgentsScene;
