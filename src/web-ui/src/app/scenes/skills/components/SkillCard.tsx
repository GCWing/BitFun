import React from 'react';
import { Package, Puzzle } from 'lucide-react';
import { getCardGradient } from '@/shared/utils/cardGradients';
import './SkillCard.scss';

type SkillCardActionTone = 'primary' | 'danger' | 'success' | 'muted';

export interface SkillCardAction {
  id: string;
  icon: React.ReactNode;
  ariaLabel: string;
  title?: string;
  disabled?: boolean;
  tone?: SkillCardActionTone;
  onClick: () => void;
}

interface SkillCardProps extends Omit<React.HTMLAttributes<HTMLDivElement>, 'children'> {
  name: string;
  description?: string;
  index?: number;
  accentSeed?: string;
  iconKind?: 'skill' | 'market';
  badges?: React.ReactNode;
  meta?: React.ReactNode;
  actions?: SkillCardAction[];
  onOpenDetails?: () => void;
}

const SkillCard: React.FC<SkillCardProps> = ({
  name,
  description,
  index = 0,
  accentSeed,
  iconKind = 'skill',
  badges,
  meta,
  actions = [],
  onOpenDetails,
  className,
  style,
  ...rootProps
}) => {
  const Icon = iconKind === 'market' ? Package : Puzzle;
  const openDetails = () => onOpenDetails?.();

  return (
    <div
      {...rootProps}
      className={['skill-card', className].filter(Boolean).join(' ')}
      style={{
        ...style,
        '--surface-stagger-index': index,
        '--skill-card-gradient': getCardGradient(accentSeed ?? name),
      } as React.CSSProperties}
      onClick={openDetails}
      tabIndex={0}
      onKeyDown={(e) => {
        if (e.key === 'Enter' || e.key === ' ') {
          e.preventDefault();
          openDetails();
        }
      }}
      aria-label={name}
    >
      {/* Header: icon + badges */}
      <div className="skill-card__header">
        <div className="skill-card__icon-area">
          <div className="skill-card__icon">
            <Icon size={20} strokeWidth={1.6} />
          </div>
        </div>
        {badges && <div className="skill-card__badges">{badges}</div>}
      </div>

      {/* Body: name + trend (meta) on one row, then description */}
      <div className="skill-card__body">
        <div className="skill-card__title-row">
          <span className="skill-card__name">{name}</span>
          {meta ? (
            <div
              className="skill-card__meta"
              onClick={(e) => e.stopPropagation()}
              onKeyDown={(e) => e.stopPropagation()}
            >
              {meta}
            </div>
          ) : null}
        </div>
        {description?.trim() && (
          <p className="skill-card__desc">{description.trim()}</p>
        )}
      </div>

      {/* Footer: action buttons */}
      {actions.length > 0 && (
        <div className="skill-card__footer">
          <div className="skill-card__actions" onClick={(e) => e.stopPropagation()}>
            {actions.map((action) => (
              <button
                key={action.id}
                type="button"
                className={[
                  'skill-card__action-btn',
                  action.tone && `skill-card__action-btn--${action.tone}`,
                ].filter(Boolean).join(' ')}
                onClick={action.onClick}
                disabled={action.disabled}
                aria-label={action.ariaLabel}
                title={action.title ?? action.ariaLabel}
                data-testid="skills-card-action"
                data-skill-action={action.id}
              >
                {action.icon}
              </button>
            ))}
          </div>
        </div>
      )}
    </div>
  );
};

export default SkillCard;
