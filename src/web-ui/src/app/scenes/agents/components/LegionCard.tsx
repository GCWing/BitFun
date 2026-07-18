import React from 'react';
import { GitBranch, Users, Network } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Badge } from '@/component-library';
import type { LegionPattern } from '../data/orchestration-patterns';
import './LegionCard.scss';

interface LegionCardProps {
  pattern: LegionPattern;
  index?: number;
  onOpenDetails: (pattern: LegionPattern) => void;
}

const LegionCard: React.FC<LegionCardProps> = ({
  pattern,
  index = 0,
  onOpenDetails,
}) => {
  const { t } = useTranslation('scenes/agents');
  const gateNodes = pattern.nodes.filter((n) => n.gate).length;
  const openDetails = () => onOpenDetails(pattern);

  const complexityLabel =
    t(`legionPattern.complexityLabel.l${pattern.complexityLevel}`, {
      defaultValue: `L${pattern.complexityLevel}`,
    });

  return (
    <div
      className="legion-card"
      style={{ '--surface-stagger-index': index } as React.CSSProperties}
      onClick={openDetails}
      role="button"
      tabIndex={0}
      onKeyDown={(e) => e.key === 'Enter' && openDetails()}
      aria-label={pattern.name}
      data-testid="legion-list-item"
      data-legion-id={pattern.id}
    >
      <div className="legion-card__header">
        <div className="legion-card__icon-area">
          <div className="legion-card__icon">
            <Network size={20} strokeWidth={1.6} />
          </div>
        </div>
        <div className="legion-card__header-info">
          <div className="legion-card__title-row">
            <span className="legion-card__name">{pattern.name}</span>
            <div className="legion-card__badges">
              <Badge variant="neutral">
                {complexityLabel}
              </Badge>
            </div>
          </div>
        </div>
      </div>

      <div className="legion-card__body">
        <p className="legion-card__desc">{pattern.description}</p>
      </div>

      <div className="legion-card__footer">
        <div className="legion-card__meta">
          <span className="legion-card__meta-item">
            <Users size={12} />
            {t('legionPattern.nodesCount', { count: pattern.nodes.length })}
          </span>
          <span className="legion-card__meta-item">
            <GitBranch size={12} />
            {t('legionPattern.edgesCount', { count: pattern.edges.length })}
          </span>
          {gateNodes > 0 ? (
            <span className="legion-card__meta-item">
              {gateNodes} {t('legionPattern.meta.gate')}
            </span>
          ) : null}
        </div>
      </div>
    </div>
  );
};

export default LegionCard;
