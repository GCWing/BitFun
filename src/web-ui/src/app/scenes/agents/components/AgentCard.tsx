import React from 'react';
import {
  Bot,
  Wrench,
  Puzzle,
  Cpu,
} from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Badge, Switch } from '@/component-library';
import type { AgentWithCapabilities } from '../agentsStore';
import { AGENT_ICON_MAP, CAPABILITY_ACCENT } from '../agentsIcons';
import { getCardGradient } from '@/shared/utils/cardGradients';
import { getAgentBadge } from '../utils';
import './AgentCard.scss';

interface AgentCardProps {
  agent: AgentWithCapabilities;
  index?: number;
  soloEnabled: boolean;
  toolCount?: number;
  skillCount?: number;
  onToggleSolo: (agentId: string, enabled: boolean) => void;
  onOpenDetails: (agent: AgentWithCapabilities) => void;
}

const AgentCard: React.FC<AgentCardProps> = ({
  agent,
  index = 0,
  soloEnabled,
  toolCount,
  skillCount = 0,
  onToggleSolo,
  onOpenDetails,
}) => {
  const { t } = useTranslation('scenes/agents');
  const badge = getAgentBadge(t, agent.agentKind, agent.subagentSource);
  const Icon = AGENT_ICON_MAP[(agent.iconKey ?? 'bot') as keyof typeof AGENT_ICON_MAP] ?? Bot;
  const totalTools = toolCount ?? agent.toolCount ?? agent.defaultTools?.length ?? 0;
  const openDetails = () => onOpenDetails(agent);

  return (
    <div
      className={[
        'agent-card',
        !agent.enabled && 'agent-card--disabled',
      ].filter(Boolean).join(' ')}
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
              {!agent.enabled ? (
                <Badge variant="neutral">{t('agentCard.badges.disabled', '已禁用')}</Badge>
              ) : null}
              {agent.model ? (
                <Badge variant="neutral">{agent.model}</Badge>
              ) : null}
            </div>
          </div>
        </div>
      </div>

      {/* Body: description + meta */}
      <div className="agent-card__body">
        <p className="agent-card__desc">{agent.description?.trim() || '—'}</p>

        <div className="agent-card__meta">
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
                {cap.category}
              </span>
            ))}
          </div>
          <span className="agent-card__meta-item">
            <Wrench size={12} />
            {t('agentCard.meta.tools', '{{count}} 个工具', { count: totalTools })}
          </span>
          {agent.agentKind === 'mode' && skillCount > 0 ? (
            <span className="agent-card__meta-item">
              <Puzzle size={12} />
              {t('agentCard.meta.skills', '{{count}} 个 Skills', { count: skillCount })}
            </span>
          ) : null}
        </div>
      </div>

      {/* Footer: switch */}
      <div className="agent-card__footer">
        <div className="agent-card__footer-actions" onClick={(e) => e.stopPropagation()}>
          <Switch
            checked={soloEnabled}
            onChange={() => onToggleSolo(agent.id, !soloEnabled)}
            size="small"
          />
        </div>
      </div>
    </div>
  );
};

export default AgentCard;
