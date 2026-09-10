import React from 'react';
import { Bot } from 'lucide-react';
import { CardBody, CardFooter, CardHeader, Icon, OverflowText } from '@openbitfun/ui';
import { useTranslation } from 'react-i18next';
import type { AgentWithCapabilities } from '../agentsStore';
import { AGENT_ICON_MAP } from '../agentsIcons';
import { getAgentBadge, getAgentDescription, getCapabilityLabel } from '../utils';
import AgentCatalogCard, { AgentCatalogMetrics } from './AgentCatalogCard';
import './AgentCatalogCard.scss';

interface AgentCardProps {
  agent: AgentWithCapabilities;
  toolCount?: number;
  skillCount?: number;
  subagentCount?: number;
  disabledReason?: string;
  onOpenDetails: (agent: AgentWithCapabilities) => void;
}

const AgentCard: React.FC<AgentCardProps> = ({ agent, disabledReason, onOpenDetails, ...metrics }) => {
  const { t } = useTranslation('scenes/agents');
  const badge = getAgentBadge(t, agent.agentKind, agent.source ?? agent.subagentSource);
  const agentIcon = AGENT_ICON_MAP[(agent.iconKey ?? 'bot') as keyof typeof AGENT_ICON_MAP] ?? { glyph: Bot };
  const capabilities = agent.capabilities.slice(0, 2).map(cap => getCapabilityLabel(t, cap.category)).join(' · ');

  return (
    <AgentCatalogCard
      agent={agent}
      onOpenDetails={onOpenDetails}
      data-openbitfun-product-component="agent-card"
      data-openbitfun-product-part="root"
    >
      <CardHeader
        className="agent-catalog-card__header"
        data-openbitfun-product-component="agent-card"
        data-openbitfun-product-part="header"
        leading={(
          <span className="agent-catalog-card__icon" data-openbitfun-product-component="agent-card" data-openbitfun-product-part="iconArea" aria-hidden="true">
            <span data-openbitfun-product-component="agent-card" data-openbitfun-product-part="icon">
              <Icon {...agentIcon} size="md" />
            </span>
          </span>
        )}
        title={(
          <div className="agent-catalog-card__title" data-openbitfun-product-component="agent-card" data-openbitfun-product-part="headerInfo">
            <div data-openbitfun-product-component="agent-card" data-openbitfun-product-part="titleRow">
              <OverflowText data-openbitfun-product-component="agent-card" data-openbitfun-product-part="name" data-testid="agent-list-item-title">
                {agent.name}
              </OverflowText>
            </div>
          </div>
        )}
        description={(
          <OverflowText className="agent-catalog-card__identity" behavior="marquee">
            <span data-openbitfun-product-component="agent-card" data-openbitfun-product-part="badges">{badge.label}</span>
            <span data-openbitfun-product-component="agent-card" data-openbitfun-product-part="capabilities">{capabilities ? ` · ${capabilities}` : null}</span>
          </OverflowText>
        )}
      />
      <CardBody data-openbitfun-product-component="agent-card" data-openbitfun-product-part="body">
        <OverflowText as="p" lines={2} className="agent-catalog-card__description" data-openbitfun-product-component="agent-card" data-openbitfun-product-part="description" data-testid="agent-list-item-description">
          {getAgentDescription(t, agent)}
        </OverflowText>
      </CardBody>
      <CardFooter align={disabledReason ? 'between' : 'start'} className="agent-catalog-card__footer" data-openbitfun-product-component="agent-card" data-openbitfun-product-part="footer">
        <AgentCatalogMetrics {...metrics} agent={agent} data-openbitfun-product-component="agent-card" data-openbitfun-product-part="meta" />
        {disabledReason && (
          <span className="agent-catalog-card__status" data-openbitfun-state="disabled" title={disabledReason}>
            <Icon className="agent-catalog-card__status-icon" name="unselected" size="2xs" />
            <OverflowText>{disabledReason}</OverflowText>
          </span>
        )}
      </CardFooter>
    </AgentCatalogCard>
  );
};

export default AgentCard;
