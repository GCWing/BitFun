import React, { useState } from 'react';
import { Search, Bot, Users, Plus, ArrowRight } from 'lucide-react';
import {
  useTeamStore,
  MOCK_AGENTS,
  CAPABILITY_CATEGORIES,
  computeTeamCapabilities,
  type AgentWithCapabilities,
  type Team,
} from '../teamStore';
import { AGENT_ICON_MAP, TEAM_ICON_MAP } from '../teamIcons';
import { useI18n } from '@/infrastructure/i18n/hooks/useI18n';
import './TeamHomePage.scss';

// ─── Team card ────────────────────────────────────────────────────────────────

const TeamCard: React.FC<{ team: Team }> = ({ team }) => {
  const { t } = useI18n('scenes/team');
  const { openTeamEditor } = useTeamStore();
  const iconKey = team.icon as keyof typeof TEAM_ICON_MAP;
  const IconComp = TEAM_ICON_MAP[iconKey] ?? Users;

  const caps = computeTeamCapabilities(team, MOCK_AGENTS);
  const topCaps = CAPABILITY_CATEGORIES
    .filter((c) => caps[c] > 0)
    .sort((a, b) => caps[b] - caps[a])
    .slice(0, 3);

  const memberAgents = team.members
    .map((m) => MOCK_AGENTS.find((a) => a.id === m.agentId))
    .filter(Boolean) as AgentWithCapabilities[];

  return (
    <div
      className="th-card th-card--team"
      onClick={() => openTeamEditor(team.id)}
    >
      <div className="th-card__head">
        <div className="th-card__icon">
          <IconComp size={16} />
        </div>
        <span className="th-card__type th-card__type--team">
          <Users size={9} />
          团队
        </span>
      </div>
      <div className="th-card__name">{team.name}</div>
      <div className="th-card__desc">{team.description || '暂无描述'}</div>
      <div className="th-card__members">
        <div className="th-card__avatars">
          {memberAgents.slice(0, 4).map((a) => {
            const ik = (a.iconKey ?? 'bot') as keyof typeof AGENT_ICON_MAP;
            const IC = AGENT_ICON_MAP[ik] ?? Bot;
            return (
              <span key={a.id} className="th-card__avatar" title={a.name}>
                <IC size={10} />
              </span>
            );
          })}
          {team.members.length > 4 && (
            <span className="th-card__avatar th-card__avatar--more">
              +{team.members.length - 4}
            </span>
          )}
        </div>
        <span className="th-card__member-count">{team.members.length} 名成员</span>
      </div>
      {topCaps.length > 0 && (
        <div className="th-card__tags">
          {topCaps.map((c) => (
            <span key={c} className="th-card__tag">
              {c}
            </span>
          ))}
        </div>
      )}
      <div className="th-card__foot">
        <span className="th-card__strategy">
          {team.strategy === 'collaborative' ? '协作' : team.strategy === 'sequential' ? '顺序' : '自由'}
        </span>
        <span className="th-card__enter">
          {t('home.edit')} <ArrowRight size={10} />
        </span>
      </div>
    </div>
  );
};

// ─── Page ─────────────────────────────────────────────────────────────────────

const ExpertTeamsPage: React.FC = () => {
  const { t } = useI18n('scenes/team');
  const { teams, addTeam, openTeamEditor } = useTeamStore();
  const [query, setQuery] = useState('');

  const filteredTeams = teams.filter((t) => {
    if (!query) return true;
    const q = query.toLowerCase();
    return t.name.toLowerCase().includes(q) || (t.description?.toLowerCase().includes(q));
  });

  const handleCreateTeam = () => {
    const id = `team-${Date.now()}`;
    addTeam({ id, name: '新团队', icon: 'users', description: '', strategy: 'collaborative', shareContext: true });
    openTeamEditor(id);
  };

  return (
    <div className="th">
      <div className="th__bar">
        <div className="th__title-wrap">
          <h3 className="th__title">{t('expertTeams.title')}</h3>
          <span className="th__title-sub">{t('expertTeams.subtitle')}</span>
        </div>
        <div className="th__search-wrap">
          <Search size={12} className="th__search-ico" />
          <input
            className="th__search"
            placeholder={t('home.search')}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
          />
        </div>
      </div>
      <div className="th__body">
        <section className="th__section th__section--teams">
          <div className="th__section-head">
            <span className="th__section-title">{t('expertTeams.sectionTitle')}</span>
            <span className="th__section-count">{filteredTeams.length}</span>
          </div>
          <div className="th__section-grid">
            {filteredTeams.map((t) => (
              <TeamCard key={t.id} team={t} />
            ))}
            <button className="th-card th-card--add" onClick={handleCreateTeam}>
              <Plus size={20} strokeWidth={1.5} />
              <span>{t('home.createTeam')}</span>
            </button>
          </div>
        </section>
      </div>
    </div>
  );
};

export default ExpertTeamsPage;
