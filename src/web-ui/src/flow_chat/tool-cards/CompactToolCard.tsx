/**
 * Compact tool card component
 * Used for ReadFile, GrepSearch, WebSearch, etc. with transparent gray background
 *
 * Features:
 * - Collapsed: transparent background, no border, single-line display
 * - Expanded: shows detailed content with dark background box
 * - Simple gray style, text brightens on hover
 */

import React, { ReactNode, useEffect, useRef, useState } from 'react';
import { shouldIgnoreCardToggleClick } from '@/shared/utils/textSelection';
import { BaseToolCard, type BaseToolCardProps } from './BaseToolCard';
import { SmoothHeightCollapse } from '../components/modern/SmoothHeightCollapse';
import { FLOWCHAT_COLLAPSE_DURATION_MS } from '../components/modern/flowChatCollapseMotion';
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
  /** data-testid for the real click target that expands/collapses the card. */
  toggleTestId?: string;
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
  toggleTestId,
  header,
  expandedContent,
}) => {
  const handleWrapperClick = (event: React.MouseEvent) => {
    if (!onClick || shouldIgnoreCardToggleClick(event)) {
      return;
    }

    onClick(event);
  };

  const loadingShimmer =
    status === 'preparing' ||
    status === 'streaming' ||
    status === 'receiving' ||
    status === 'running' ||
    status === 'analyzing';

  const hasExpandedContent = Boolean(expandedContent);
  const collapseTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const [useExpandedShell, setUseExpandedShell] = useState(
    Boolean(isExpanded && hasExpandedContent),
  );

  useEffect(() => {
    if (collapseTimerRef.current !== null) {
      clearTimeout(collapseTimerRef.current);
      collapseTimerRef.current = null;
    }

    const isCurrentlyExpanded = Boolean(isExpanded && hasExpandedContent);

    if (isCurrentlyExpanded) {
      setUseExpandedShell(true);
      return;
    }

    if (!hasExpandedContent) {
      setUseExpandedShell(false);
      return;
    }

    if (useExpandedShell && hasExpandedContent) {
      collapseTimerRef.current = setTimeout(() => {
        collapseTimerRef.current = null;
        setUseExpandedShell(false);
      }, FLOWCHAT_COLLAPSE_DURATION_MS);
    }
  }, [hasExpandedContent, isExpanded, useExpandedShell]);

  useEffect(() => () => {
    if (collapseTimerRef.current !== null) {
      clearTimeout(collapseTimerRef.current);
    }
  }, []);

  const shouldRenderExpandedShell = Boolean(
    useExpandedShell || (isExpanded && hasExpandedContent),
  );

  if (shouldRenderExpandedShell) {
    return (
      <BaseToolCard
        status={status}
        isExpanded={isExpanded}
        onClick={handleWrapperClick}
        className={`compact-tool-card-wrapper--expanded-card ${className}`.trim()}
        header={header}
        expandedContent={expandedContent}
        toggleTestId={toggleTestId}
        headerExpandAffordance={clickable || Boolean(onClick)}
      />
    );
  }

  return (
    <div data-bf-component="compact-tool-card" data-bf-part="root" data-bf-status={status}
      data-bf-state={[loadingShimmer && 'loading', clickable && 'clickable', isExpanded && 'expanded'].filter(Boolean).join(' ')}
      className={`compact-tool-card-wrapper compact-tool-card-wrapper--dense-command${loadingShimmer ? ' compact-tool-card-wrapper--loading-shimmer' : ''} ${className}`.trim()}
    >
      <div
        data-bf-component="compact-tool-card"
        data-bf-part="surface"
        className={`compact-tool-card status-${status} ${clickable ? 'clickable' : ''} ${isExpanded ? 'expanded' : ''}`}
        data-testid={clickable || Boolean(onClick) ? toggleTestId : undefined}
        onClick={handleWrapperClick}
        style={{ cursor: clickable ? 'pointer' : 'default' }}
      >
        {header}
      </div>

      <SmoothHeightCollapse isOpen={Boolean(isExpanded && expandedContent)} className="compact-tool-card-expanded-collapse">
        <div data-bf-component="compact-tool-card" data-bf-part="expanded" className="compact-tool-card-expanded">
          {expandedContent}
        </div>
      </SmoothHeightCollapse>
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
  /** Action label (text or inline markup) */
  action?: ReactNode;
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
      {action && <span data-bf-component="compact-tool-card" data-bf-part="action" className="compact-card-action">{action}</span>}
      {content && <span data-bf-component="compact-tool-card" data-bf-part="content" className="compact-card-content">{content}</span>}
      {extra && <span data-bf-component="compact-tool-card" data-bf-part="extra" className="compact-card-extra">{extra}</span>}
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
