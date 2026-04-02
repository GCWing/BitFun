import React, { useCallback, useMemo, useState } from 'react';
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
import { Badge, Button, IconButton, Search, confirmDanger } from '@/component-library';
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
import { useNotification } from '@/shared/notification-system';

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
    availableSkills,
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
  const selectedAgentTools = selectedAgent?.agentKind === 'mode'
    ? (selectedAgentModeConfig?.enabled_tools ?? selectedAgent.defaultTools ?? [])
    : (selectedAgent?.defaultTools ?? []);
  const selectedAgentSkills = selectedAgentModeConfig?.available_skills ?? [];
  const selectedAgentSkillItems = availableSkills.filter((skill) => selectedAgentSkills.includes(skill.name));
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
                  skillCount={agent.agentKind === 'mode' ? (getModeConfig(agent.id)?.available_skills?.length ?? 0) : 0}
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
                  skillCount={agent.agentKind === 'mode' ? (getModeConfig(agent.id)?.available_skills?.length ?? 0) : 0}
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
            {selectedAgent.agentKind === 'mode' ? (
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

            {selectedAgent.agentKind === 'mode' && availableSkills.length > 0 ? (
              <div className="agent-card__section">
                <div className="agent-card__section-head">
                  <div className="agent-card__section-title">
                    <Puzzle size={12} />
                    <span>{t('agentsOverview.skills')}</span>
                    <span className="agent-card__section-count">
                      {`${(skillsEditing ? (pendingSkills ?? selectedAgentSkills) : selectedAgentSkills).length}/${availableSkills.length}`}
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
                  <div className="agent-card__token-grid">
                    {[...availableSkills]
                      .sort((a, b) => {
                        const draft = pendingSkills ?? selectedAgentSkills;
                        const aOn = draft.includes(a.name);
                        const bOn = draft.includes(b.name);
                        if (aOn && !bOn) return -1;
                        if (!aOn && bOn) return 1;
                        return 0;
                      })
                      .map((skill) => {
                        const draft = pendingSkills ?? selectedAgentSkills;
                        const isOn = draft.includes(skill.name);
                        return (
                          <button
                            key={skill.name}
                            type="button"
                            className={`agent-card__token${isOn ? ' is-on' : ''}`}
                            title={skill.description || skill.name}
                            onClick={() => {
                              setPendingSkills((prev) => {
                                const current = prev ?? selectedAgentSkills;
                                return isOn
                                  ? current.filter((n) => n !== skill.name)
                                  : [...current, skill.name];
                              });
                            }}
                          >
                            <span className="agent-card__token-name">{skill.name}</span>
                          </button>
                        );
                      })}
                  </div>
                ) : (
                  <div className="agent-card__chip-grid">
                    {selectedAgentSkillItems.length === 0 ? (
                      <span className="agent-card__empty-inline">
                        {t('agentsOverview.noSkills')}
                      </span>
                    ) : (
                      selectedAgentSkillItems.map((skill) => (
                        <span key={skill.name} className="agent-card__chip" title={skill.description || skill.name}>
                          {skill.name}
                        </span>
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
