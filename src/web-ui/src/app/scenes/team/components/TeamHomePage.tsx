import React, { useState } from 'react';
import { Search, Bot, Users, Plus, ArrowRight, User } from 'lucide-react';
import {
  useTeamStore,
  MOCK_AGENTS,
  CAPABILITY_CATEGORIES,
  computeTeamCapabilities,
  type AgentWithCapabilities,
  type Team,
} from '../teamStore';
import { AGENT_ICON_MAP, TEAM_ICON_MAP } from '../teamIcons';
import './TeamHomePage.scss';

// ─── Agent card ───────────────────────────────────────────────────────────────

const AgentCard: React.FC<{
  agent: AgentWithCapabilities;
  soloEnabled: boolean;
  onToggleSolo: (agentId: string, enabled: boolean) => void;
}> = ({ agent, soloEnabled, onToggleSolo }) => {
  const iconKey = (agent.iconKey ?? 'bot') as keyof typeof AGENT_ICON_MAP;
  const IconComp = AGENT_ICON_MAP[iconKey] ?? Bot;

  return (
    <div className="th-card">
      {/* Icon + type badge */}
      <div className="th-card__head">
        <div className="th-card__icon">
          <IconComp size={16} />
        </div>
        <span className="th-card__type">
          <User size={9} />
          Agent
        </span>
      </div>

      {/* Name */}
      <div className="th-card__name">{agent.name}</div>

      {/* Description */}
      <div className="th-card__desc">{agent.description}</div>

      {/* Capabilities */}
      <div className="th-card__tags">
        {agent.capabilities.slice(0, 3).map((c) => (
          <span key={c.category} className="th-card__tag">
            {c.category}
          </span>
        ))}
      </div>

      {/* Footer */}
      <div className="th-card__foot">
        <span className="th-card__model">{agent.model ?? 'primary'}</span>
        <div className="th-card__foot-right">
          <button
            className={`th-card__solo-toggle ${soloEnabled ? 'is-on' : ''}`}
            type="button"
            onClick={() => onToggleSolo(agent.id, !soloEnabled)}
          >
            {soloEnabled ? '可独立使用' : '仅团队协作'}
          </button>
        </div>
      </div>
    </div>
  );
};

// ─── Team card ────────────────────────────────────────────────────────────────

const TeamCard: React.FC<{ team: Team }> = ({ team }) => {
  const { openTeamEditor } = useTeamStore();
  const iconKey = team.icon as keyof typeof TEAM_ICON_MAP;
  const IconComp = TEAM_ICON_MAP[iconKey] ?? Users;

  const caps = computeTeamCapabilities(team, MOCK_AGENTS);
  const topCaps = CAPABILITY_CATEGORIES
    .filter((c) => caps[c] > 0)
    .sort((a, b) => caps[b] - caps[a])
    .slice(0, 3);

  // Member avatar stack
  const memberAgents = team.members
    .map((m) => MOCK_AGENTS.find((a) => a.id === m.agentId))
    .filter(Boolean) as AgentWithCapabilities[];

  return (
    <div
      className="th-card th-card--team"
      onClick={() => openTeamEditor(team.id)}
    >
      {/* Icon + type badge */}
      <div className="th-card__head">
        <div className="th-card__icon">
          <IconComp size={16} />
        </div>
        <span className="th-card__type th-card__type--team">
          <Users size={9} />
          团队
        </span>
      </div>

      {/* Name */}
      <div className="th-card__name">{team.name}</div>

      {/* Description */}
      <div className="th-card__desc">{team.description || '暂无描述'}</div>

      {/* Member stack */}
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

      {/* Capability coverage */}
      {topCaps.length > 0 && (
        <div className="th-card__tags">
          {topCaps.map((c) => (
            <span key={c} className="th-card__tag">
              {c}
            </span>
          ))}
        </div>
      )}

      {/* Footer */}
      <div className="th-card__foot">
        <span className="th-card__strategy">
          {team.strategy === 'collaborative' ? '协作' : team.strategy === 'sequential' ? '顺序' : '自由'}
        </span>
        <span className="th-card__enter">
          编辑 <ArrowRight size={10} />
        </span>
      </div>
    </div>
  );
};

// ─── Page ─────────────────────────────────────────────────────────────────────

const TeamHomePage: React.FC = () => {
  const {
    teams,
    addTeam,
    openTeamEditor,
    agentSoloEnabled,
    setAgentSoloEnabled,
  } = useTeamStore();
  const [query, setQuery] = useState('');

  const agents = MOCK_AGENTS;
  const filteredAgents = agents.filter((a) => {
    if (!query) return true;
    const q = query.toLowerCase();
    return a.name.toLowerCase().includes(q) || a.description.toLowerCase().includes(q);
  });

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
      {/* ── Top bar ── */}
      <div className="th__bar">
        <div className="th__title-wrap">
          <h3 className="th__title">资源总览</h3>
          <span className="th__title-sub">左侧 Agent，右侧团队</span>
        </div>

        <div className="th__search-wrap">
          <Search size={12} className="th__search-ico" />
          <input
            className="th__search"
            placeholder="搜索名称、描述..."
            value={query}
            onChange={(e) => setQuery(e.target.value)}
          />
        </div>
      </div>

      {/* ── Split layout ── */}
      <div className="th__body">
        <div className="th__split">
          <section className="th__section th__section--agents">
            <div className="th__section-head">
              <span className="th__section-title">Agents</span>
              <span className="th__section-count">{filteredAgents.length}</span>
            </div>
            <div className="th__section-grid">
              {filteredAgents.map((a) => (
                <AgentCard
                  key={a.id}
                  agent={a}
                  soloEnabled={agentSoloEnabled[a.id] ?? false}
                  onToggleSolo={setAgentSoloEnabled}
                />
              ))}
            </div>
          </section>

          <section className="th__section th__section--teams">
            <div className="th__section-head">
              <span className="th__section-title">工作团队</span>
              <span className="th__section-count">{filteredTeams.length}</span>
            </div>
            <div className="th__section-grid">
              {filteredTeams.map((t) => (
                <TeamCard key={t.id} team={t} />
              ))}
              <button className="th-card th-card--add" onClick={handleCreateTeam}>
                <Plus size={20} strokeWidth={1.5} />
                <span>创建团队</span>
              </button>
            </div>
          </section>
        </div>
      </div>

    </div>
  );
};

export default TeamHomePage;
