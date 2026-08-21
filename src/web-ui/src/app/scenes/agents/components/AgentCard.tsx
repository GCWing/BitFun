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
import { AGENT_ICON_MAP } from '../agentsIcons';
import { CAPABILITY_ACCENT, getCapabilityAccentBorder } from '../agentAppearance';
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
  const badge = getAgentBadge(t, agent.agentKind, agent.source ?? agent.subagentSource, agent.id);
  const Icon = AGENT_ICON_MAP[(agent.iconKey ?? 'bot') as keyof typeof AGENT_ICON_MAP] ?? Bot;
  const totalTools = toolCount ?? agent.toolCount ?? agent.defaultTools?.length ?? 0;
  const openDetails = () => onOpenDetails(agent);

  return (
    <div data-bf-component="agent-card" data-bf-part="root"
      className="agent-card"
      style={{
        '--surface-stagger-index': index,
        '--agent-card-gradient': getCardGradient(agent.id || agent.name),
      } as React.CSSProperties}
      onClick={openDetails}
      role="button"
      tabIndex={0}
      onKeyDown={(e) => e.key === 'Enter' && openDetails()}
      aria-label={agent.name}
      data-testid="agent-list-item"
      data-agent-id={agent.id}
      data-agent-name={agent.name}
      data-agent-kind={agent.agentKind}
      data-subagent-source={agent.subagentSource ?? ''}
    >
      {/* Header: icon + name */}
      <div className="agent-card__header" data-bf-component="agent-card" data-bf-part="header">
        <div className="agent-card__icon-area" data-bf-component="agent-card" data-bf-part="iconArea">
          <div className="agent-card__icon" data-bf-component="agent-card" data-bf-part="icon">
            <Icon size={20} strokeWidth={1.6} />
          </div>
        </div>
        <div className="agent-card__header-info" data-bf-component="agent-card" data-bf-part="headerInfo">
          <div className="agent-card__title-row" data-bf-component="agent-card" data-bf-part="titleRow">
            <span className="agent-card__name" data-bf-component="agent-card" data-bf-part="name" data-testid="agent-list-item-title">{agent.name}</span>
            <div className="agent-card__badges" data-bf-component="agent-card" data-bf-part="badges">
              <Badge variant={badge.variant}>
                {agent.agentKind === 'mode' ? <Cpu size={10} /> : <Bot size={10} />}
                {badge.label}
              </Badge>
            </div>
          </div>
        </div>
      </div>

      {/* Body: description + meta */}
      <div className="agent-card__body" data-bf-component="agent-card" data-bf-part="body">
        <p className="agent-card__desc" data-bf-component="agent-card" data-bf-part="description" data-testid="agent-list-item-description">
          {getAgentDescription(t, agent)}
        </p>
      </div>

      <div className="agent-card__footer" data-bf-component="agent-card" data-bf-part="footer">
        <div className="agent-card__cap-chips" data-bf-component="agent-card" data-bf-part="capabilities">
          {agent.capabilities.slice(0, 3).map((cap) => (
            <span
              key={cap.category}
              className="agent-card__cap-chip"
              style={{
                color: CAPABILITY_ACCENT[cap.category],
                borderColor: getCapabilityAccentBorder(cap.category),
              }}
            >
              {getCapabilityLabel(t, cap.category)}
            </span>
          ))}
        </div>
        <div className="agent-card__meta" data-bf-component="agent-card" data-bf-part="meta">
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
          {agent.agentKind === 'subagent' && agent.subagentModelDisplayName ? (
            <span className="agent-card__meta-item">
              <Cpu size={11} />
              {agent.subagentModelDisplayName}
            </span>
          ) : null}
        </div>
      </div>
    </div>
  );
};

export default AgentCard;
