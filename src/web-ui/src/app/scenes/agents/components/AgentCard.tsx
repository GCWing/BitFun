import React from 'react';
import {
  Bot,
  Wrench,
  Puzzle,
  Cpu,
} from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Badge } from '@/component-library';
import type { AgentWithCapabilities } from '../agentsStore';
import { AGENT_ICON_MAP, CAPABILITY_ACCENT } from '../agentsIcons';
import { getCardGradient } from '@/shared/utils/cardGradients';
import { getAgentBadge, getAgentDescription, getCapabilityLabel } from '../utils';
import './AgentCard.scss';

interface AgentCardProps {
  agent: AgentWithCapabilities;
  index?: number;
  toolCount?: number;
  skillCount?: number;
  subagentCount?: number;
  onOpenDetails: (agent: AgentWithCapabilities) => void;
}

const AgentCard: React.FC<AgentCardProps> = ({
  agent,
  index = 0,
  toolCount,
  skillCount = 0,
  subagentCount = 0,
  onOpenDetails,
}) => {
  const { t } = useTranslation('scenes/agents');
  const badge = getAgentBadge(t, agent.agentKind, agent.subagentSource);
  const Icon = AGENT_ICON_MAP[(agent.iconKey ?? 'bot') as keyof typeof AGENT_ICON_MAP] ?? Bot;
  const totalTools = toolCount ?? agent.toolCount ?? agent.defaultTools?.length ?? 0;
  const openDetails = () => onOpenDetails(agent);

  return (
    <div
      className="agent-card"
      style={{
        '--card-index': index,
        '--agent-card-gradient': getCardGradient(agent.id || agent.name),
      } as React.CSSProperties}
      onClick={openDetails}
      role="button"
      tabIndex={0}
      onKeyDown={(e) => e.key === 'Enter' && openDetails()}
      aria-label={agent.name}
    >
      {/* Header: icon + name */}
      <div className="agent-card__header">
        <div className="agent-card__icon-area">
          <div className="agent-card__icon">
            <Icon size={20} strokeWidth={1.6} />
          </div>
        </div>
        <div className="agent-card__header-info">
          <div className="agent-card__title-row">
            <span className="agent-card__name">{agent.name}</span>
            <div className="agent-card__badges">
              <Badge variant={badge.variant}>
                {agent.agentKind === 'mode' ? <Cpu size={10} /> : <Bot size={10} />}
                {badge.label}
              </Badge>
              {agent.model ? (
                <Badge variant="neutral">{agent.model}</Badge>
              ) : null}
            </div>
          </div>
        </div>
      </div>

      {/* Body: description + meta */}
      <div className="agent-card__body">
        <p className="agent-card__desc">
          {getAgentDescription(t, agent)}
        </p>
      </div>

      <div className="agent-card__footer">
        <div className="agent-card__cap-chips">
          {agent.capabilities.slice(0, 3).map((cap) => (
            <span
              key={cap.category}
              className="agent-card__cap-chip"
              style={{
                color: CAPABILITY_ACCENT[cap.category],
                borderColor: `${CAPABILITY_ACCENT[cap.category]}44`,
              }}
            >
              {getCapabilityLabel(t, cap.category)}
            </span>
          ))}
        </div>
        <div className="agent-card__meta">
          <span className="agent-card__meta-item">
            <Wrench size={12} />
            {totalTools}
          </span>
          {agent.agentKind === 'mode' && skillCount > 0 ? (
            <span className="agent-card__meta-item">
              <Puzzle size={12} />
              {skillCount}
            </span>
          ) : null}
          {agent.agentKind === 'mode' && subagentCount > 0 ? (
            <span className="agent-card__meta-item">
              <Bot size={12} />
              {subagentCount}
            </span>
          ) : null}
        </div>
      </div>
    </div>
  );
};

export default AgentCard;
