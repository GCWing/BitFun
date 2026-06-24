import React, { useState } from 'react';
import './ConfigCollectionItem.scss';

export interface ConfigCollectionItemProps extends React.HTMLAttributes<HTMLDivElement> {
  label: React.ReactNode;
  badge?: React.ReactNode;
  badgePlacement?: 'inline' | 'below';
  control: React.ReactNode;
  details?: React.ReactNode;
  disabled?: boolean;
  expanded?: boolean;
  onToggle?: () => void;
  className?: string;
}

export const ConfigCollectionItem: React.FC<ConfigCollectionItemProps> = ({
  label,
  badge,
  badgePlacement = 'inline',
  control,
  details,
  disabled = false,
  expanded: expandedProp,
  onToggle,
  className = '',
  ...rootProps
}) => {
  const [internalExpanded, setInternalExpanded] = useState(false);
  const isControlled = expandedProp !== undefined;
  const isExpanded = isControlled ? expandedProp : internalExpanded;
  const hasDetails = Boolean(details);

  const handleRowClick = () => {
    if (!hasDetails) return;
    if (isControlled) {
      onToggle?.();
    } else {
      setInternalExpanded((prev) => !prev);
    }
  };

  return (
    <div
      {...rootProps}
      className={`bitfun-collection-item ${isExpanded ? 'is-expanded' : ''} ${disabled ? 'is-disabled' : ''} ${className}`}
    >
      <div
        className={`bitfun-config-page-row bitfun-config-page-row--center bitfun-collection-item__row ${hasDetails ? 'is-clickable' : ''}`}
        onClick={handleRowClick}
      >
        <div className="bitfun-config-page-row__meta">
          <div
            className={`bitfun-config-page-row__label bitfun-collection-item__label ${
              badgePlacement === 'below' ? 'bitfun-collection-item__label--stacked' : ''
            }`}
          >
            <span className="bitfun-collection-item__name">{label}</span>
            {badge && (
              <span
                className={`bitfun-collection-item__badges ${
                  badgePlacement === 'below'
                    ? 'bitfun-collection-item__badges--stacked'
                    : 'bitfun-collection-item__badges--inline'
                }`}
              >
                {badge}
              </span>
            )}
          </div>
        </div>
        <div
          className="bitfun-config-page-row__control"
          onClick={(e) => e.stopPropagation()}
        >
          <div className="bitfun-collection-item__control">{control}</div>
        </div>
      </div>

      {isExpanded && details && (
        <div className="bitfun-collection-item__details">{details}</div>
      )}
    </div>
  );
};

export default ConfigCollectionItem;
