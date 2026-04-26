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
import './CompactToolCard.scss';

export interface CompactToolCardProps {
  /** Tool status */
  status:
    | 'pending'
    | 'preparing'
    | 'streaming'
    | 'receiving'
    | 'running'
    | 'completed'
    | 'error'
    | 'cancelled'
    | 'analyzing'
    | 'pending_confirmation'
    | 'confirmed';
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
  /** Left status icon */
  statusIcon?: ReactNode;
  /** Action text */
  action?: string;
  /** Main content */
  content?: ReactNode;
  /** Right extra content (e.g., statistics) */
  extra?: ReactNode;
  /** Right status icon */
  rightIcon?: ReactNode;
}
export const CompactToolCardHeader: React.FC<CompactToolCardHeaderProps> = ({
  statusIcon,
  action,
  content,
  extra,
  rightIcon,
}) => {
  return (
    <>
      {statusIcon && (
        <span className="compact-card-status-icon">
          {statusIcon}
        </span>
      )}
      {action && <span className="compact-card-action">{action}</span>}
      {content && <span className="compact-card-content">{content}</span>}
      {extra && <span className="compact-card-extra">{extra}</span>}
      {rightIcon && (
        <span className="compact-card-right-icon">
          {rightIcon}
        </span>
      )}
    </>
  );
};
