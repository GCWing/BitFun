import React from 'react';
import { Bot, Pencil, Users } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Badge } from '@/component-library';
import type { AgentTeam, AgentWithCapabilities } from '../agentsStore';
import { AGENT_ICON_MAP, AGENT_TEAM_ICON_MAP, getAgentTeamAccent } from '../agentsIcons';
import { CAPABILITY_ACCENT } from '../agentAppearance';
import './AgentTeamCard.scss';

interface AgentTeamCardProps {
  team: AgentTeam;
  allAgents: AgentWithCapabilities[];
  index?: number;
  isExample?: boolean;
  onEdit: (teamId: string) => void;
  onOpenDetails: (team: AgentTeam) => void;
  topCapabilities: string[];
}

const AgentTeamCard: React.FC<AgentTeamCardProps> = ({
  team,
  allAgents,
  index = 0,
  isExample = false,
  onEdit,
  onOpenDetails,
  topCapabilities,
}) => {
  const { t } = useTranslation('scenes/agents');
  const Icon = AGENT_TEAM_ICON_MAP[team.icon as keyof typeof AGENT_TEAM_ICON_MAP] ?? Users;
  const accent = getAgentTeamAccent(team.id);
  const memberAgents = team.members
    .map((member) => allAgents.find((agent) => agent.id === member.agentId))
    .filter(Boolean) as AgentWithCapabilities[];

  const strategyLabel =
    team.strategy === 'collaborative'
      ? t('composer.strategy.collaborative')
      : team.strategy === 'sequential'
        ? t('composer.strategy.sequential')
        : t('composer.strategy.free');

  const openDetails = () => onOpenDetails(team);

  return (
    <div
      className="agent-team-card"
      data-bf-component="agent-team-card"
      data-bf-part="root"
      style={{
        '--card-index': index,
        '--agent-team-card-accent': accent,
        '--agent-team-card-gradient': `linear-gradient(135deg, ${accent}40 0%, ${accent}15 100%)`,
      } as React.CSSProperties}
      onClick={openDetails}
      role="button"
      tabIndex={0}
      onKeyDown={(e) => e.key === 'Enter' && openDetails()}
      aria-label={team.name}
    >
      <div className="agent-team-card__header" data-bf-component="agent-team-card" data-bf-part="header">
        <div className="agent-team-card__icon-area" data-bf-component="agent-team-card" data-bf-part="iconArea">
          <div className="agent-team-card__icon" data-bf-component="agent-team-card" data-bf-part="icon">
            <Icon size={20} strokeWidth={1.6} />
          </div>
        </div>
        <div className="agent-team-card__header-info" data-bf-component="agent-team-card" data-bf-part="headerInfo">
          <span className="agent-team-card__name" data-bf-component="agent-team-card" data-bf-part="name">{team.name}</span>
          <div className="agent-team-card__actions" data-bf-component="agent-team-card" data-bf-part="actions" onClick={(e) => e.stopPropagation()}>
            <button
              type="button"
              className="agent-team-card__icon-btn"
              onClick={() => onEdit(team.id)}
              aria-label={t('agentsOverview.editAgent')}
              title={t('agentsOverview.editAgent')}
            >
              <Pencil size={14} />
            </button>
          </div>
        </div>
      </div>

      <div className="agent-team-card__body" data-bf-component="agent-team-card" data-bf-part="body">
        <p className="agent-team-card__desc" data-bf-component="agent-team-card" data-bf-part="description">{team.description?.trim() || '—'}</p>

        <div className="agent-team-card__meta" data-bf-component="agent-team-card" data-bf-part="meta">
          <div className="agent-team-card__avatars" data-bf-component="agent-team-card" data-bf-part="avatars">
            {memberAgents.slice(0, 4).map((agent) => {
              const AgentIcon = AGENT_ICON_MAP[(agent.iconKey ?? 'bot') as keyof typeof AGENT_ICON_MAP] ?? Bot;
              return (
                <span key={agent.id} className="agent-team-card__avatar" title={agent.name}>
                  <AgentIcon size={10} />
                </span>
              );
            })}
            {team.members.length > 4 ? (
              <span className="agent-team-card__avatar agent-team-card__avatar--more">
                +{team.members.length - 4}
              </span>
            ) : null}
          </div>
          <span className="agent-team-card__meta-item">
            {t('home.members', { count: team.members.length })}
          </span>
          {topCapabilities.length > 0 ? (
            <div className="agent-team-card__cap-chips">
              {topCapabilities.map((cap) => (
                <span
                  key={cap}
                  className="agent-team-card__cap-chip"
                  style={{
                    color: CAPABILITY_ACCENT[cap as keyof typeof CAPABILITY_ACCENT],
                    borderColor: `${CAPABILITY_ACCENT[cap as keyof typeof CAPABILITY_ACCENT]}44`,
                  }}
                >
                  {cap}
                </span>
              ))}
            </div>
          ) : null}
        </div>
      </div>

      <div className="agent-team-card__footer" data-bf-component="agent-team-card" data-bf-part="footer">
        <div className="agent-team-card__state-badges">
          {isExample ? <Badge variant="neutral">{t('teamCard.badges.example')}</Badge> : null}
          <Badge variant="neutral">{strategyLabel}</Badge>
          {team.shareContext ? (
            <Badge variant="success">{t('teamCard.badges.sharedContext')}</Badge>
          ) : null}
        </div>
      </div>
    </div>
  );
};

export default AgentTeamCard;
