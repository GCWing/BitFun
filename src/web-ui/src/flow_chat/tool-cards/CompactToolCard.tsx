/**
 * Compact tool card component
 * Used for ReadFile, GrepSearch, WebSearch, etc. with transparent gray background
 *
 * Features:
 * - Collapsed: transparent background, no border, single-line display
 * - Expanded: shows detailed content with dark background box
 * - Simple gray style, text brightens on hover
 */

import React, { ReactNode } from 'react';
import { BaseToolCard, type BaseToolCardProps } from './BaseToolCard';
import { ToolCardIconSlot } from './ToolCardIconSlot';
import { ToolCardStatusIcon } from './ToolCardStatusIcon';
import type { ToolCardHeaderAffordanceKind } from './ToolCardHeaderLayoutContext';
import './CompactToolCard.scss';

export interface CompactToolCardProps {
  /** Tool status */
  status: BaseToolCardProps['status'];
  /** Whether expanded */
  isExpanded?: boolean;
  /** Card click callback */
  onClick?: (e: React.MouseEvent) => void;
  /** Custom class name */
  className?: string;
  /** Whether clickable */
  clickable?: boolean;
  /** Header content */
  header: ReactNode;
  /** Expanded content (optional) */
  expandedContent?: ReactNode;
}

export const CompactToolCard: React.FC<CompactToolCardProps> = ({
  status,
  isExpanded = false,
  onClick,
  className = '',
  clickable = false,
  header,
  expandedContent,
}) => {
  const handleWrapperClick = (e: React.MouseEvent) => {
    if (onClick) {
      onClick(e);
    }
  };

  const loadingShimmer =
    status === 'preparing' ||
    status === 'streaming' ||
    status === 'receiving' ||
    status === 'running' ||
    status === 'analyzing';

  if (isExpanded && expandedContent) {
    return (
      <BaseToolCard
        status={status}
        isExpanded
        onClick={handleWrapperClick}
        className={`compact-tool-card-wrapper--expanded-card ${className}`.trim()}
        header={header}
        expandedContent={expandedContent}
        headerExpandAffordance={clickable || Boolean(onClick)}
      />
    );
  }

  return (
    <div
      className={`compact-tool-card-wrapper${loadingShimmer ? ' compact-tool-card-wrapper--loading-shimmer' : ''} ${className}`.trim()}
    >
      <div
        className={`compact-tool-card status-${status} ${clickable ? 'clickable' : ''} ${isExpanded ? 'expanded' : ''}`}
        onClick={handleWrapperClick}
        style={{ cursor: clickable ? 'pointer' : 'default' }}
      >
        {header}
      </div>

      {isExpanded && expandedContent && (
        <div className="compact-tool-card-expanded">
          {expandedContent}
        </div>
      )}
    </div>
  );
};

export interface CompactToolCardHeaderProps {
  /** Left tool icon (should be 16px lucide icon) */
  icon?: ReactNode;
  /** Custom class name for the icon element */
  iconClassName?: string;
  /** Show hover chevron when expandable */
  expandable?: boolean;
  /** Expand vs open-right-panel hint icon */
  affordanceKind?: ToolCardHeaderAffordanceKind;
  /** Expanded state for chevron rotation */
  isExpanded?: boolean;
  /** Click handler for the left icon rail affordance */
  onAffordanceClick?: (e: React.MouseEvent<HTMLButtonElement>) => void;
  /** Whether to show the left icon divider (default false for compact) */
  showDivider?: boolean;
  /** Action text */
  action?: string;
  /** Main content */
  content?: ReactNode;
  /** Right extra content (e.g., statistics) */
  extra?: ReactNode;
  /** Right status icon (should be 14px) */
  rightStatusIcon?: ReactNode;
  /** Whether right status icon has a divider */
  rightStatusIconWithDivider?: boolean;
}

export const CompactToolCardHeader: React.FC<CompactToolCardHeaderProps> = ({
  icon,
  iconClassName,
  expandable = false,
  affordanceKind = 'expand',
  isExpanded = false,
  onAffordanceClick,
  showDivider = false,
  action,
  content,
  extra,
  rightStatusIcon,
  rightStatusIconWithDivider = false,
}) => {
  return (
    <>
      {icon && (
        <ToolCardIconSlot
          icon={icon}
          iconClassName={iconClassName}
          expandable={expandable}
          affordanceKind={affordanceKind}
          isExpanded={isExpanded}
          onAffordanceClick={onAffordanceClick}
          showDivider={showDivider}
        />
      )}
      {action && <span className="compact-card-action">{action}</span>}
      {content && <span className="compact-card-content">{content}</span>}
      {extra && <span className="compact-card-extra">{extra}</span>}
      {rightStatusIcon && (
        <ToolCardStatusIcon
          icon={rightStatusIcon}
          withDivider={rightStatusIconWithDivider}
          className="compact-card-right-status-icon"
        />
      )}
    </>
  );
};
