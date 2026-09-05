import React from 'react';
import { Icon } from '@openbitfun/ui';
import { Package } from 'lucide-react';
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
  accentSeed: _accentSeed,
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
    <div data-openbitfun-component="skill-card" data-openbitfun-part="root"
      {...rootProps}
      className={['skill-card', className].filter(Boolean).join(' ')}
      style={{
        ...style,
        '--surface-stagger-index': index,
      } as React.CSSProperties}
      data-openbitfun-variant={iconKind}
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
      <div className="skill-card__header" data-openbitfun-component="skill-card" data-openbitfun-part="header">
        <div className="skill-card__icon-area" data-openbitfun-component="skill-card" data-openbitfun-part="iconArea">
          <div className="skill-card__icon" data-openbitfun-component="skill-card" data-openbitfun-part="icon">
            {glyph}
          </div>
        </div>
        {badges && <div className="skill-card__badges" data-openbitfun-component="skill-card" data-openbitfun-part="badges">{badges}</div>}
      </div>

      {/* Body: name + trend (meta) on one row, then description */}
      <div className="skill-card__body" data-openbitfun-component="skill-card" data-openbitfun-part="body">
        <div className="skill-card__title-row" data-openbitfun-component="skill-card" data-openbitfun-part="titleRow">
          <span className="skill-card__name" data-openbitfun-component="skill-card" data-openbitfun-part="name">{name}</span>
          {meta ? (
            <div
              className="skill-card__meta"
              data-openbitfun-component="skill-card"
              data-openbitfun-part="meta"
              onClick={(e) => e.stopPropagation()}
              onKeyDown={(e) => e.stopPropagation()}
            >
              {meta}
            </div>
          ) : null}
        </div>
        {description?.trim() && (
          <p className="skill-card__desc" data-openbitfun-component="skill-card" data-openbitfun-part="description">{description.trim()}</p>
        )}
      </div>

      {/* Footer: action buttons */}
      {actions.length > 0 && (
      <div className="skill-card__footer" data-openbitfun-component="skill-card" data-openbitfun-part="footer">
        <div className="skill-card__actions" data-openbitfun-component="skill-card" data-openbitfun-part="actions" onClick={(e) => e.stopPropagation()}>
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
                data-openbitfun-component="skill-card"
                data-openbitfun-part="action"
                data-openbitfun-tone={action.tone}
                data-openbitfun-state={action.disabled ? 'disabled' : undefined}
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
