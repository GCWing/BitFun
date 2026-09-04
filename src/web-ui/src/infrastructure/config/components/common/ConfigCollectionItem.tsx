import React, { useId, useState } from 'react';
import { Icon } from '@openbitfun/ui';
;
import { RetainedMountBoundary } from '@/shared/presence';
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
  /** Lets non-interactive row space toggle details while preserving nested controls. */
  toggleOnRowClick?: boolean;
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
  toggleOnRowClick = false,
  className = '',
  ...rootProps
}) => {
  const [internalExpanded, setInternalExpanded] = useState(false);
  const labelId = useId();
  const detailsId = useId();
  const isControlled = expandedProp !== undefined;
  const isExpanded = isControlled ? expandedProp : internalExpanded;
  const hasDetails = Boolean(details);

  const toggleDetails = () => {
    if (!hasDetails || disabled) return;
    if (isControlled) {
      onToggle?.();
    } else {
      setInternalExpanded((prev) => !prev);
    }
  };

  const handleRowClick = (event: React.MouseEvent<HTMLDivElement>) => {
    if (!toggleOnRowClick || !hasDetails || disabled) return;
    const target = event.target;
    if (
      target instanceof Element
      && target.closest('a, button, input, select, textarea, [role="button"], [role="link"], [role="checkbox"], [contenteditable="true"]')
    ) {
      return;
    }
    toggleDetails();
  };

  return (
    <div
      className={`openbitfun-collection-item ${isExpanded ? 'is-expanded' : ''} ${disabled ? 'is-disabled' : ''} ${className}`}
      data-openbitfun-component="config"
      data-openbitfun-part="collectionItem"
      {...rootProps}
    >
      <div
        className={`openbitfun-config-page-row openbitfun-config-page-row--center openbitfun-collection-item__row ${
          toggleOnRowClick && hasDetails && !disabled ? 'openbitfun-collection-item__row--toggleable' : ''
        }`}
        data-openbitfun-component="config"
        data-openbitfun-part="collectionRow"
        onClick={handleRowClick}
      >
        <div className="openbitfun-config-page-row__meta" data-openbitfun-component="config" data-openbitfun-part="collectionMeta">
          <div
            className={`openbitfun-config-page-row__label openbitfun-collection-item__label ${
              badgePlacement === 'below' ? 'openbitfun-collection-item__label--stacked' : ''
            }`}
          >
            <span id={labelId} className="openbitfun-collection-item__name" data-openbitfun-component="config" data-openbitfun-part="collectionName">{label}</span>
            {badge && (
              <span
                className={`openbitfun-collection-item__badges ${
                  badgePlacement === 'below'
                    ? 'openbitfun-collection-item__badges--stacked'
                    : 'openbitfun-collection-item__badges--inline'
                }`}
              >
                {badge}
              </span>
            )}
          </div>
        </div>
        <div className="openbitfun-config-page-row__control" data-openbitfun-component="config" data-openbitfun-part="collectionControl">
          <div className="openbitfun-collection-item__control">
            {control}
            {hasDetails ? (
              <button
                type="button"
                className="openbitfun-collection-btn openbitfun-collection-item__details-toggle"
                onClick={toggleDetails}
                disabled={disabled}
                aria-labelledby={labelId}
                aria-expanded={isExpanded}
                aria-controls={detailsId}
              >
                <Icon name="chevron-down" size="sm" aria-hidden="true" />
              </button>
            ) : null}
          </div>
        </div>
      </div>

      {details ? (
        <RetainedMountBoundary
          present={isExpanded}
          retainForMs={180}
          minimumRetainMs={180}
        >
          <div
            id={detailsId}
            className="openbitfun-collection-item__details-collapse"
            data-open={isExpanded ? 'true' : 'false'}
            aria-hidden={!isExpanded}
            {...(!isExpanded ? { inert: '' } : {})}
          >
            <div className="openbitfun-collection-item__details" data-openbitfun-component="config" data-openbitfun-part="collectionDetails">{details}</div>
          </div>
        </RetainedMountBoundary>
      ) : null}
    </div>
  );
};

export default ConfigCollectionItem;
