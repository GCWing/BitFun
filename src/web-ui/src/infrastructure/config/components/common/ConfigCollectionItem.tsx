import React, { useId, useState } from 'react';
import { Icon } from '@bitfun/ui';
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

  return (
    <div
      className={`bitfun-collection-item ${isExpanded ? 'is-expanded' : ''} ${disabled ? 'is-disabled' : ''} ${className}`}
      data-bf-component="config"
      data-bf-part="collectionItem"
      {...rootProps}
    >
      <div className="bitfun-config-page-row bitfun-config-page-row--center bitfun-collection-item__row" data-bf-component="config" data-bf-part="collectionRow">
        <div className="bitfun-config-page-row__meta" data-bf-component="config" data-bf-part="collectionMeta">
          <div
            className={`bitfun-config-page-row__label bitfun-collection-item__label ${
              badgePlacement === 'below' ? 'bitfun-collection-item__label--stacked' : ''
            }`}
          >
            <span id={labelId} className="bitfun-collection-item__name" data-bf-component="config" data-bf-part="collectionName">{label}</span>
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
        <div className="bitfun-config-page-row__control" data-bf-component="config" data-bf-part="collectionControl">
          <div className="bitfun-collection-item__control">
            {control}
            {hasDetails ? (
              <button
                type="button"
                className="bitfun-collection-btn bitfun-collection-item__details-toggle"
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
            className="bitfun-collection-item__details-collapse"
            data-open={isExpanded ? 'true' : 'false'}
            aria-hidden={!isExpanded}
            {...(!isExpanded ? { inert: '' } : {})}
          >
            <div className="bitfun-collection-item__details" data-bf-component="config" data-bf-part="collectionDetails">{details}</div>
          </div>
        </RetainedMountBoundary>
      ) : null}
    </div>
  );
};

export default ConfigCollectionItem;
