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
  onOpenDetails: (agent: AgentWithCapabilities) => void;
}

const CoreAgentCard: React.FC<CoreAgentCardProps> = ({
  agent,
  index = 0,
  meta,
  toolCount,
  skillCount = 0,
  onOpenDetails,
}) => {
  const { t } = useTranslation('scenes/agents');
  const Icon = AGENT_ICON_MAP[(agent.iconKey ?? 'bot') as keyof typeof AGENT_ICON_MAP] ?? Bot;
  const totalTools = toolCount ?? agent.toolCount ?? agent.defaultTools?.length ?? 0;
  const openDetails = () => onOpenDetails(agent);

  return (
    <div
      className={[
        'core-agent-card',
        !agent.enabled && 'core-agent-card--disabled',
      ].filter(Boolean).join(' ')}
      style={{
        '--card-index': index,
        '--core-accent': meta.accentColor,
        '--core-accent-bg': meta.accentBg,
        '--core-card-gradient': `linear-gradient(135deg, ${meta.accentColor}40 0%, ${meta.accentColor}15 100%)`,
      } as React.CSSProperties}
      onClick={openDetails}
      role="button"
      tabIndex={0}
      onKeyDown={(e) => e.key === 'Enter' && openDetails()}
      aria-label={agent.name}
    >
      <div className="core-agent-card__top">
        <div className="core-agent-card__icon-wrap">
          <Icon size={28} strokeWidth={1.6} />
        </div>
        <div className="core-agent-card__top-info">
          <span className="core-agent-card__name">{agent.name}</span>
          <span className="core-agent-card__role">
            <Sparkles size={10} strokeWidth={2} />
            {meta.role}
          </span>
        </div>
      </div>

      <div className="core-agent-card__body">
        <p className="core-agent-card__desc">
          {getAgentDescription(t, agent)}
        </p>
      </div>

      <div className="core-agent-card__footer">
        <span className="core-agent-card__tag">
          {t('coreAgentsZone.roleLabel')}
          <strong>{meta.role}</strong>
        </span>
        <div className="core-agent-card__meta">
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
          <span className="core-agent-card__meta-item">
            <Cpu size={11} />
            {agent.model ?? 'primary'}
          </span>
        </div>
      </div>
    </div>
  );
};

export default CoreAgentCard;
