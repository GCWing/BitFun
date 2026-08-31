import React from 'react';
import { Bot, Wrench, Cpu, UsersRound } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Icon, StatusPill } from '@bitfun/ui';
import type { AgentWithCapabilities } from '../agentsStore';
import { AGENT_ICON_MAP } from '../agentsIcons';
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
  /** Replaces the connected status when the agent's capability is toggled off in Settings. */
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
  const AgentGlyph = AGENT_ICON_MAP[(agent.iconKey ?? 'bot') as keyof typeof AGENT_ICON_MAP] ?? Bot;
  const totalTools = toolCount ?? agent.toolCount ?? agent.defaultTools?.length ?? 0;
  const openDetails = () => onOpenDetails(agent);
  const statusLabel = disabledReason ?? t('agentCard.status.connected');

  return (
    <div data-bf-component="core-agent-card" data-bf-part="root"
      className="core-agent-card"
      style={{
        '--surface-stagger-index': index,
      } as React.CSSProperties}
      onClick={openDetails}
      role="button"
      tabIndex={0}
      onKeyDown={(event) => {
        if (event.key === 'Enter' || event.key === ' ') {
          event.preventDefault();
          openDetails();
        }
      }}
      aria-label={agent.name}
      data-testid="agent-list-item"
      data-agent-id={agent.id}
      data-agent-name={agent.name}
      data-agent-kind={agent.agentKind}
    >
      <div className="core-agent-card__icon-rail">
        <div className="core-agent-card__icon-wrap" data-bf-component="core-agent-card" data-bf-part="icon">
          <AgentGlyph size={21} strokeWidth={1.6} />
        </div>
        <span className="core-agent-card__dot-field" aria-hidden="true" />
      </div>

      <div className="core-agent-card__content">
        <div className="core-agent-card__top" data-bf-component="core-agent-card" data-bf-part="header">
          <div className="core-agent-card__top-info" data-bf-component="core-agent-card" data-bf-part="headerInfo">
            <span className="core-agent-card__name" data-bf-component="core-agent-card" data-bf-part="name" data-testid="agent-list-item-title">{agent.name}</span>
            <span data-bf-component="core-agent-card" data-bf-part="role">
              <StatusPill tone="neutral" className="core-agent-card__role">
                {meta.role}
              </StatusPill>
            </span>
          </div>
          <span
            className={`core-agent-card__status${disabledReason ? ' is-disabled' : ''}`}
            data-bf-component="core-agent-card"
            data-bf-part="status"
            data-bf-state={disabledReason ? 'disabled' : 'connected'}
            title={statusLabel}
          >
            <Icon className="core-agent-card__status-icon" name="circle" size="2xs" />
            <span>{statusLabel}</span>
          </span>
        </div>

        <div className="core-agent-card__body" data-bf-component="core-agent-card" data-bf-part="body">
          <p className="core-agent-card__desc" data-bf-component="core-agent-card" data-bf-part="description" data-testid="agent-list-item-description">
            {getAgentDescription(t, agent)}
          </p>
        </div>

        <div className="core-agent-card__footer" data-bf-component="core-agent-card" data-bf-part="footer">
          <div className="core-agent-card__meta" data-bf-component="core-agent-card" data-bf-part="meta">
            <span className="core-agent-card__meta-item">
              <span className="core-agent-card__meta-icon"><Wrench size={11} /></span>
              <span className="core-agent-card__meta-label">{t('agentCard.metrics.tools')}</span>
              <strong>{totalTools}</strong>
            </span>
            {agent.agentKind === 'mode' ? (
              <>
                <span className="core-agent-card__meta-item">
                  <span className="core-agent-card__meta-icon"><Icon name="extension" size="2xs" /></span>
                  <span className="core-agent-card__meta-label">{t('agentCard.metrics.skills')}</span>
                  <strong>{skillCount}</strong>
                </span>
                <span className="core-agent-card__meta-item">
                  <span className="core-agent-card__meta-icon"><UsersRound size={11} /></span>
                  <span className="core-agent-card__meta-label">{t('agentCard.metrics.collaboration')}</span>
                  <strong>{subagentCount}</strong>
                </span>
              </>
            ) : null}
            {agent.agentKind === 'subagent' && agent.subagentModelDisplayName ? (
              <span className="core-agent-card__meta-item">
                <span className="core-agent-card__meta-icon"><Cpu size={11} /></span>
                <span className="core-agent-card__meta-label">{t('agentCard.metrics.model')}</span>
                <strong className="core-agent-card__meta-value--text" title={agent.subagentModelDisplayName}>
                  {agent.subagentModelDisplayName}
                </strong>
              </span>
            ) : null}
          </div>
        </div>
      </div>
    </div>
  );
};

export default CoreAgentCard;
