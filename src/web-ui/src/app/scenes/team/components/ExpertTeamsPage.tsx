import React, { useState } from 'react';
import { Bot, Users, Plus, Pencil } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Search, IconButton, Badge } from '@/component-library';
import {
  useTeamStore,
  MOCK_AGENTS,
  CAPABILITY_CATEGORIES,
  computeTeamCapabilities,
  type AgentWithCapabilities,
  type Team,
} from '../teamStore';
import { AGENT_ICON_MAP } from '../teamIcons';
import './TeamHomePage.scss';

// ─── Team list item ───────────────────────────────────────────────────────────

const TeamListItem: React.FC<{ team: Team; index: number }> = ({ team, index }) => {
  const { t } = useTranslation('scenes/team');
  const { openTeamEditor } = useTeamStore();
  const [expanded, setExpanded] = useState(false);

  const caps = computeTeamCapabilities(team, MOCK_AGENTS);
  const topCaps = CAPABILITY_CATEGORIES
    .filter((c) => caps[c] > 0)
    .sort((a, b) => caps[b] - caps[a])
    .slice(0, 3);

  const memberAgents = team.members
    .map((m) => MOCK_AGENTS.find((a) => a.id === m.agentId))
    .filter(Boolean) as AgentWithCapabilities[];

  const strategyLabel =
    team.strategy === 'collaborative'
      ? t('home.strategyCollab')
      : team.strategy === 'sequential'
        ? t('home.strategySeq')
        : t('home.strategyFree');

  return (
    <div
      className={['th-list__item', expanded && 'is-expanded'].filter(Boolean).join(' ')}
      style={{ '--item-index': index } as React.CSSProperties}
    >
      <div
        className="th-list__item-row"
        onClick={() => setExpanded((v) => !v)}
        role="button"
        tabIndex={0}
        onKeyDown={(e) => e.key === 'Enter' && setExpanded((v) => !v)}
      >
        <div className="th-list__item-info">
          <div className="th-list__item-name-row">
            <span className="th-list__item-name">{team.name}</span>
            <Badge variant="neutral">{strategyLabel}</Badge>
          </div>
          <p className="th-list__item-desc">{team.description || '—'}</p>
        </div>

        <div className="th-list__item-meta">
          <div className="th-list__avatars">
            {memberAgents.slice(0, 4).map((a) => {
              const ik = (a.iconKey ?? 'bot') as keyof typeof AGENT_ICON_MAP;
              const IC = AGENT_ICON_MAP[ik] ?? Bot;
              return (
                <span key={a.id} className="th-list__avatar" title={a.name}>
                  <IC size={9} />
                </span>
              );
            })}
            {team.members.length > 4 && (
              <span className="th-list__avatar th-list__avatar--more">
                +{team.members.length - 4}
              </span>
            )}
          </div>
          <span className="th-list__member-count">
            {t('home.members', { count: team.members.length })}
          </span>
        </div>

        <div className="th-list__item-action" onClick={(e) => e.stopPropagation()}>
          <IconButton
            variant="ghost"
            size="small"
            tooltip={t('home.edit')}
            onClick={() => openTeamEditor(team.id)}
          >
            <Pencil size={14} />
          </IconButton>
        </div>
      </div>

      {expanded && (
        <div className="th-list__item-details">
          {team.description && (
            <p className="th-list__detail-desc">{team.description}</p>
          )}
          {memberAgents.length > 0 && (
            <div className="th-list__detail-row">
              <span className="th-list__detail-label">成员</span>
              <div className="th-list__member-list">
                {memberAgents.map((a) => {
                  const ik = (a.iconKey ?? 'bot') as keyof typeof AGENT_ICON_MAP;
                  const IC = AGENT_ICON_MAP[ik] ?? Bot;
                  const role = team.members.find((m) => m.agentId === a.id)?.role ?? 'member';
                  const roleLabel =
                    role === 'leader'
                      ? t('composer.role.leader')
                      : role === 'reviewer'
                        ? t('composer.role.reviewer')
                        : t('composer.role.member');
                  return (
                    <span key={a.id} className="th-list__member-chip">
                      <IC size={10} />
                      {a.name}
                      <span className="th-list__member-role">{roleLabel}</span>
                    </span>
                  );
                })}
              </div>
            </div>
          )}
          {topCaps.length > 0 && (
            <div className="th-list__detail-row">
              <span className="th-list__detail-label">能力</span>
              <div className="th-list__cap-chips">
                {topCaps.map((c) => (
                  <span key={c} className="th-list__cap-chip">
                    {c}
                  </span>
                ))}
              </div>
            </div>
          )}
        </div>
      )}
    </div>
  );
};

// ─── Page ─────────────────────────────────────────────────────────────────────

const ExpertTeamsPage: React.FC = () => {
  const { t } = useTranslation('scenes/team');
  const { teams, addTeam, openTeamEditor } = useTeamStore();
  const [query, setQuery] = useState('');

  const filteredTeams = teams.filter((team) => {
    if (!query) return true;
    const q = query.toLowerCase();
    return team.name.toLowerCase().includes(q) || (team.description?.toLowerCase().includes(q));
  });

  const handleCreateTeam = () => {
    const id = `team-${Date.now()}`;
    addTeam({ id, name: '新团队', icon: 'users', description: '', strategy: 'collaborative', shareContext: true });
    openTeamEditor(id);
  };

  return (
    <div className="th">
      <div className="th__header">
        <div className="th__header-inner">
          <div className="th__title-row">
            <div>
              <h2 className="th__title">{t('expertTeams.title')}</h2>
              <p className="th__title-sub">{t('expertTeams.subtitle')}</p>
            </div>
            <button type="button" className="th__create-btn" onClick={handleCreateTeam}>
              <Plus size={13} />
              {t('home.createTeam')}
            </button>
          </div>
          <div className="th__toolbar">
            <Search
              placeholder={t('home.search')}
              value={query}
              onChange={setQuery}
              clearable
              size="small"
            />
          </div>
        </div>
      </div>

      <div className="th__list-body">
        <div className="th__list-inner">
          <div className="th-list__section-head">
            <span className="th-list__section-title">{t('expertTeams.sectionTitle')}</span>
            <span className="th-list__section-count">{filteredTeams.length}</span>
          </div>
          {filteredTeams.length === 0 ? (
            <div className="th-list__empty">
              <Users size={28} strokeWidth={1.5} />
              <span>{t('empty')}</span>
            </div>
          ) : (
            <div className="th-list">
              {filteredTeams.map((team, i) => (
                <TeamListItem key={team.id} team={team} index={i} />
              ))}
            </div>
          )}
        </div>
      </div>
    </div>
  );
};

export default ExpertTeamsPage;
