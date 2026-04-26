import React from 'react';
import { BadgeCheck, GitBranch, ShieldCheck, Users } from 'lucide-react';
import { Badge } from '@/component-library';
import './AgentTeamCard.scss';

interface AgentTeamCardProps {
  index?: number;
  title: string;
  subtitle: string;
  localOnlyLabel: string;
  qualityGateLabel: string;
  membersLabel: string;
  openLabel: string;
  memberNames: string[];
  onOpen: () => void;
}

const AgentTeamCard: React.FC<AgentTeamCardProps> = ({
  index = 0,
  title,
  subtitle,
  localOnlyLabel,
  qualityGateLabel,
  membersLabel,
  openLabel,
  memberNames,
  onOpen,
}) => {
  return (
    <div
      className="agent-team-card"
      style={{ '--card-index': index } as React.CSSProperties}
      role="button"
      tabIndex={0}
      onClick={onOpen}
      onKeyDown={(event) => {
        if (event.key === 'Enter') {
          onOpen();
        }
      }}
      aria-label={title}
    >
      <div className="agent-team-card__header">
        <div className="agent-team-card__icon">
          <ShieldCheck size={20} strokeWidth={1.8} />
        </div>
        <div className="agent-team-card__header-copy">
          <div className="agent-team-card__title-row">
            <span className="agent-team-card__title">{title}</span>
            <div className="agent-team-card__badges">
              <Badge variant="accent">{localOnlyLabel}</Badge>
              <Badge variant="purple">{qualityGateLabel}</Badge>
            </div>
          </div>
          <p className="agent-team-card__subtitle">{subtitle}</p>
        </div>
      </div>

      <div className="agent-team-card__body">
        <div className="agent-team-card__metrics" aria-label={memberNames.join(', ')}>
          <span className="agent-team-card__metric agent-team-card__metric--primary">
            <Users size={13} />
            <strong>{membersLabel}</strong>
          </span>
          <span className="agent-team-card__metric">
            <GitBranch size={13} />
            <span>{localOnlyLabel}</span>
          </span>
          <span className="agent-team-card__metric">
            <BadgeCheck size={13} />
            <span>{qualityGateLabel}</span>
          </span>
        </div>
      </div>

      <div className="agent-team-card__footer">
        <span className="agent-team-card__open">{openLabel}</span>
      </div>
    </div>
  );
};

export default AgentTeamCard;
