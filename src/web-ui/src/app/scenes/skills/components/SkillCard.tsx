import React from 'react';
import { Icon } from '@bitfun/ui';
import { Package } from 'lucide-react';
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
  const glyph = iconKind === 'market'
    ? <Package size={20} strokeWidth={1.6} />
    : <Icon name="extension" size="lg" />;
  const openDetails = () => onOpenDetails?.();

  return (
    <div data-bf-component="skill-card" data-bf-part="root"
      {...rootProps}
      className={['skill-card', className].filter(Boolean).join(' ')}
      style={{
        ...style,
        '--surface-stagger-index': index,
        '--skill-card-gradient': getCardGradient(accentSeed ?? name),
      } as React.CSSProperties}
      data-bf-variant={iconKind}
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
      <div className="skill-card__header" data-bf-component="skill-card" data-bf-part="header">
        <div className="skill-card__icon-area" data-bf-component="skill-card" data-bf-part="iconArea">
          <div className="skill-card__icon" data-bf-component="skill-card" data-bf-part="icon">
            {glyph}
          </div>
        </div>
        {badges && <div className="skill-card__badges" data-bf-component="skill-card" data-bf-part="badges">{badges}</div>}
      </div>

      {/* Body: name + trend (meta) on one row, then description */}
      <div className="skill-card__body" data-bf-component="skill-card" data-bf-part="body">
        <div className="skill-card__title-row" data-bf-component="skill-card" data-bf-part="titleRow">
          <span className="skill-card__name" data-bf-component="skill-card" data-bf-part="name">{name}</span>
          {meta ? (
            <div
              className="skill-card__meta"
              data-bf-component="skill-card"
              data-bf-part="meta"
              onClick={(e) => e.stopPropagation()}
              onKeyDown={(e) => e.stopPropagation()}
            >
              {meta}
            </div>
          ) : null}
        </div>
        {description?.trim() && (
          <p className="skill-card__desc" data-bf-component="skill-card" data-bf-part="description">{description.trim()}</p>
        )}
      </div>

      {/* Footer: action buttons */}
      {actions.length > 0 && (
      <div className="skill-card__footer" data-bf-component="skill-card" data-bf-part="footer">
        <div className="skill-card__actions" data-bf-component="skill-card" data-bf-part="actions" onClick={(e) => e.stopPropagation()}>
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
                data-bf-component="skill-card"
                data-bf-part="action"
                data-bf-tone={action.tone}
                data-bf-state={action.disabled ? 'disabled' : undefined}
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
