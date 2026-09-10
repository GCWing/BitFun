import React from 'react';
import { Bot } from 'lucide-react';
import { CardBody, CardFooter, CardHeader, Icon, OverflowText } from '@openbitfun/ui';
import { useTranslation } from 'react-i18next';
import type { AgentWithCapabilities } from '../agentsStore';
import { AGENT_ICON_MAP } from '../agentsIcons';
import { getAgentDescription } from '../utils';
import AgentCatalogCard, { AgentCatalogMetrics } from './AgentCatalogCard';
import './AgentCatalogCard.scss';

export interface CoreAgentMeta {
  role: string;
  accentColor: string;
  accentBg: string;
}

interface CoreAgentCardProps {
  agent: AgentWithCapabilities;
  meta: CoreAgentMeta;
  toolCount?: number;
  skillCount?: number;
  subagentCount?: number;
  onOpenDetails: (agent: AgentWithCapabilities) => void;
  /** Replaces the connected status when the capability is off in Settings. */
  disabledReason?: string;
}

const CoreAgentCard: React.FC<CoreAgentCardProps> = ({
  agent, meta, disabledReason, onOpenDetails, ...metrics
}) => {
  const { t } = useTranslation('scenes/agents');
  const statusLabel = disabledReason ?? t('agentCard.status.connected');
  const agentIcon = AGENT_ICON_MAP[(agent.iconKey ?? 'bot') as keyof typeof AGENT_ICON_MAP] ?? { glyph: Bot };

  return (
    <AgentCatalogCard
      agent={agent}
      onOpenDetails={onOpenDetails}
      data-openbitfun-product-component="core-agent-card"
      data-openbitfun-product-part="root"
    >
      <CardHeader
        className="agent-catalog-card__header"
        data-openbitfun-product-component="core-agent-card"
        data-openbitfun-product-part="header"
        leading={(
          <span className="agent-catalog-card__icon" data-openbitfun-product-component="core-agent-card" data-openbitfun-product-part="icon" aria-hidden="true">
            <Icon {...agentIcon} size="md" />
          </span>
        )}
        title={(
          <div className="agent-catalog-card__title" data-openbitfun-product-component="core-agent-card" data-openbitfun-product-part="headerInfo">
            <OverflowText data-openbitfun-product-component="core-agent-card" data-openbitfun-product-part="name" data-testid="agent-list-item-title">
              {agent.name}
            </OverflowText>
          </div>
        )}
        description={(
          <OverflowText className="agent-catalog-card__identity" data-openbitfun-product-component="core-agent-card" data-openbitfun-product-part="role">
            {meta.role}
          </OverflowText>
        )}
      />
      <CardBody data-openbitfun-product-component="core-agent-card" data-openbitfun-product-part="body">
        <OverflowText as="p" lines={2} className="agent-catalog-card__description" data-openbitfun-product-component="core-agent-card" data-openbitfun-product-part="description" data-testid="agent-list-item-description">
          {getAgentDescription(t, agent)}
        </OverflowText>
      </CardBody>
      <CardFooter align="between" className="agent-catalog-card__footer" data-openbitfun-product-component="core-agent-card" data-openbitfun-product-part="footer">
        <AgentCatalogMetrics {...metrics} agent={agent} data-openbitfun-product-component="core-agent-card" data-openbitfun-product-part="meta" />
        <span
          className="agent-catalog-card__status"
          data-openbitfun-product-component="core-agent-card"
          data-openbitfun-product-part="status"
          data-openbitfun-state={disabledReason ? 'disabled' : 'connected'}
          title={statusLabel}
        >
          <Icon className="agent-catalog-card__status-icon" name="unselected" size="2xs" />
          <OverflowText>{statusLabel}</OverflowText>
        </span>
      </CardFooter>
    </AgentCatalogCard>
  );
};

export default CoreAgentCard;
