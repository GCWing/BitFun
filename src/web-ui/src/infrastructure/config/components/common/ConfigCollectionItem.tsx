import React, { useState } from 'react';
import './ConfigCollectionItem.scss';

export interface ConfigCollectionItemProps {
  label: React.ReactNode;
  badge?: React.ReactNode;
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
  control,
  details,
  disabled = false,
  expanded: expandedProp,
  onToggle,
  className = '',
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
      className={`bitfun-collection-item ${isExpanded ? 'is-expanded' : ''} ${disabled ? 'is-disabled' : ''} ${className}`}
    >
      <div
        className={`bitfun-config-page-row bitfun-config-page-row--center bitfun-collection-item__row ${hasDetails ? 'is-clickable' : ''}`}
        onClick={handleRowClick}
      >
        <div className="bitfun-config-page-row__meta">
          <p className="bitfun-config-page-row__label bitfun-collection-item__label">
            <span className="bitfun-collection-item__name">{label}</span>
            {badge}
          </p>
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
