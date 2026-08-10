import React from 'react';
import {
  Bot,
  Wrench,
  Puzzle,
  Cpu,
  Sparkles,
} from 'lucide-react';
import { useTranslation } from 'react-i18next';
import type { AgentWithCapabilities } from '../agentsStore';
import { AGENT_ICON_MAP } from '../agentsIcons';
import { getAlphaColor } from '../agentAppearance';
import { getAgentDescription } from '../utils';
import './CoreAgentCard.scss';

export interface CoreAgentMeta {
  role: string;
  accentColor: string;
  accentBg: string;
}

interface CoreAgentCardProps {
  agent: AgentWithCapabilities;
  index?: number;
  meta: CoreAgentMeta;
  toolCount?: number;
  skillCount?: number;
  subagentCount?: number;
  onOpenDetails: (agent: AgentWithCapabilities) => void;
  /** Shown as a small footer tag when the agent's capability is toggled off in Settings (e.g. Computer Use). */
  disabledReason?: string;
}

const CoreAgentCard: React.FC<CoreAgentCardProps> = ({
  agent,
  index = 0,
  meta,
  toolCount,
  skillCount = 0,
  subagentCount = 0,
  onOpenDetails,
  disabledReason,
}) => {
  const { t } = useTranslation('scenes/agents');
  const Icon = AGENT_ICON_MAP[(agent.iconKey ?? 'bot') as keyof typeof AGENT_ICON_MAP] ?? Bot;
  const totalTools = toolCount ?? agent.toolCount ?? agent.defaultTools?.length ?? 0;
  const openDetails = () => onOpenDetails(agent);
  const cardGradient = [
    `linear-gradient(135deg, ${getAlphaColor(meta.accentColor, '40', 25)} 0%,`,
    `${getAlphaColor(meta.accentColor, '15', 8)} 100%)`,
  ].join(' ');

  return (
    <div data-bf-component="core-agent-card" data-bf-part="root"
      className="core-agent-card"
      style={{
        '--surface-stagger-index': index,
        '--core-accent': meta.accentColor,
        '--core-accent-bg': meta.accentBg,
        '--core-card-gradient': cardGradient,
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
    >
      <div className="core-agent-card__top" data-bf-component="core-agent-card" data-bf-part="header">
        <div className="core-agent-card__icon-wrap" data-bf-component="core-agent-card" data-bf-part="icon">
          <Icon size={28} strokeWidth={1.6} />
        </div>
        <div className="core-agent-card__top-info" data-bf-component="core-agent-card" data-bf-part="headerInfo">
          <span className="core-agent-card__name" data-bf-component="core-agent-card" data-bf-part="name" data-testid="agent-list-item-title">{agent.name}</span>
          <span className="core-agent-card__role" data-bf-component="core-agent-card" data-bf-part="role">
            <Sparkles size={10} strokeWidth={2} />
            {meta.role}
          </span>
        </div>
      </div>

      <div className="core-agent-card__body" data-bf-component="core-agent-card" data-bf-part="body">
        <p className="core-agent-card__desc" data-bf-component="core-agent-card" data-bf-part="description" data-testid="agent-list-item-description">
          {getAgentDescription(t, agent)}
        </p>
      </div>

      <div className="core-agent-card__footer" data-bf-component="core-agent-card" data-bf-part="footer">
        <div className="core-agent-card__tags" data-bf-component="core-agent-card" data-bf-part="tags">
          <span className="core-agent-card__tag">
            <strong>{meta.role}</strong>
          </span>
          {disabledReason ? (
            <span className="core-agent-card__tag core-agent-card__tag--disabled" title={disabledReason}>
              {disabledReason}
            </span>
          ) : null}
        </div>
        <div className="core-agent-card__meta" data-bf-component="core-agent-card" data-bf-part="meta">
          <span className="core-agent-card__meta-item">
            <Wrench size={11} />
            {totalTools}
          </span>
          {agent.agentKind === 'mode' && skillCount > 0 ? (
            <span className="core-agent-card__meta-item">
              <Puzzle size={11} />
              {skillCount}
            </span>
          ) : null}
          {agent.agentKind === 'mode' && subagentCount > 0 ? (
            <span className="core-agent-card__meta-item">
              <Bot size={11} />
              {subagentCount}
            </span>
          ) : null}
          {agent.agentKind === 'subagent' && agent.subagentModelDisplayName ? (
            <span className="core-agent-card__meta-item">
              <Cpu size={11} />
              {agent.subagentModelDisplayName}
            </span>
          ) : null}
        </div>
      </div>
    </div>
  );
};

export default CoreAgentCard;
