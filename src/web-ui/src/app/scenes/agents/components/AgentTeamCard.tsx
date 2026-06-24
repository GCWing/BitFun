import React from 'react';
import { ShieldCheck, Sparkles } from 'lucide-react';
import { AGENT_TEAM_TAG_COLORS } from '../agentTheme';
import './AgentTeamCard.scss';

interface AgentTeamCardProps {
  teamId?: string;
  index?: number;
  title: string;
  subtitle: string;
  roleName: string;
  tagNames: string[];
  onOpen: () => void;
}

const AgentTeamCard: React.FC<AgentTeamCardProps> = ({
  teamId,
  index = 0,
  title,
  subtitle,
  roleName,
  tagNames,
  onOpen,
}) => {
  return (
    <div
      className="agent-team-card"
      style={{ '--surface-stagger-index': index } as React.CSSProperties}
      role="button"
      tabIndex={0}
      onClick={onOpen}
      onKeyDown={(event) => {
        if (event.key === 'Enter') {
          onOpen();
        }
      }}
      aria-label={title}
      data-testid="agents-team-card"
      data-team-id={teamId ?? ''}
    >
      <div className="agent-team-card__header">
        <div className="agent-team-card__icon">
          <ShieldCheck size={20} strokeWidth={1.8} />
        </div>
        <div className="agent-team-card__header-copy">
          <div className="agent-team-card__title-row">
            <span className="agent-team-card__title">{title}</span>
          </div>
          <span className="agent-team-card__role">
            <Sparkles size={10} strokeWidth={2} />
            {roleName}
          </span>
        </div>
      </div>

      <div className="agent-team-card__body">
        <p className="agent-team-card__desc">{subtitle}</p>
      </div>

      <div className="agent-team-card__footer">
        <div className="agent-team-card__tags">
          {tagNames.slice(0, 3).map((name, i) => (
            <span
              key={name}
              className="agent-team-card__tag-chip"
              style={{
                color: AGENT_TEAM_TAG_COLORS[i % AGENT_TEAM_TAG_COLORS.length].color,
                borderColor: AGENT_TEAM_TAG_COLORS[i % AGENT_TEAM_TAG_COLORS.length].border,
              }}
            >
              {name}
            </span>
          ))}
        </div>
      </div>
    </div>
  );
};

export default AgentTeamCard;
