/**
 * Common tool card component
 * Provides unified card styles and interaction logic
 */
import React, { ReactNode } from 'react';
import { ChevronDown, ChevronRight } from 'lucide-react';
import {
  ToolCardHeaderLayoutContext,
  useToolCardHeaderLayout,
  type ToolCardHeaderAffordanceKind,
  type ToolCardHeaderLayoutContextValue,
} from './ToolCardHeaderLayoutContext';
import './BaseToolCard.scss';

const LOADING_SHIMMER_STATUSES = new Set([
  'preparing',
  'streaming',
  'receiving',
  'running',
  'analyzing',
]);

function statusUsesLoadingShimmer(status: string): boolean {
  return LOADING_SHIMMER_STATUSES.has(status);
}

export interface BaseToolCardProps {
  /** Tool status */
  status: 'pending' | 'preparing' | 'streaming' | 'receiving' | 'running' | 'completed' | 'error' | 'cancelled' | 'analyzing' | 'pending_confirmation' | 'confirmed';
  /** Whether expanded */
  isExpanded?: boolean;
  /** Card click callback */
  onClick?: (e: React.MouseEvent) => void;
  /** Custom class name */
  className?: string;
  /** Header content */
  header: ReactNode;
  /** Expanded content (optional) */
  expandedContent?: ReactNode;
  /** Error content (optional) */
  errorContent?: ReactNode;
  /** Whether to show error */
  isFailed?: boolean;
  /** Whether user confirmation is required (for highlighting border) */
  requiresConfirmation?: boolean;
  /**
   * When set, controls hover chevron on the left tool icon.
   * When omitted: true if the card is clickable, not failed, and expandedContent is passed and truthy.
   * (Some cards pass expandedContent only while expanded; set this explicitly for those.)
   */
  headerExpandAffordance?: boolean;
  /** Hover icon: chevron-down (inline expand) vs chevron-right (open right). Default `expand`. */
  headerAffordanceKind?: ToolCardHeaderAffordanceKind;
}

/**
 * Base tool card component
 */
export const BaseToolCard: React.FC<BaseToolCardProps> = ({
  status,
  isExpanded = false,
  onClick,
  className = '',
  header,
  expandedContent,
  errorContent,
  isFailed = false,
  requiresConfirmation = false,
  headerExpandAffordance: headerExpandAffordanceProp,
  headerAffordanceKind: headerAffordanceKindProp = 'expand',
}) => {
  const hasExpandedContent = isExpanded && expandedContent && !isFailed;
  const showConfirmationHighlight = requiresConfirmation && 
    status !== 'completed' && 
    status !== 'confirmed' &&
    status !== 'cancelled' && 
    status !== 'error';

  const resolvedHeaderExpandAffordance =
    headerExpandAffordanceProp !== undefined
      ? headerExpandAffordanceProp
      : Boolean(onClick) && !isFailed && Boolean(expandedContent);

  const headerLayoutValue: ToolCardHeaderLayoutContextValue = {
    headerExpandAffordance: resolvedHeaderExpandAffordance,
    headerAffordanceKind: headerAffordanceKindProp,
    isExpanded,
  };

  const loadingShimmer = statusUsesLoadingShimmer(status);
  
  return (
    <div
      className={`base-tool-card-wrapper ${showConfirmationHighlight ? 'requires-confirmation' : ''} ${loadingShimmer ? 'base-tool-card-wrapper--loading-shimmer' : ''} ${className}`.trim()}
    >
      <div 
        className={`base-tool-card status-${status} ${isExpanded ? 'expanded' : ''} ${resolvedHeaderExpandAffordance ? 'base-tool-card--header-expandable' : ''}`.trim()}
        onClick={onClick}
      >
        <ToolCardHeaderLayoutContext.Provider value={headerLayoutValue}>
          <div className="base-tool-card-header">
            {header}
          </div>
        </ToolCardHeaderLayoutContext.Provider>
      </div>
      
      {hasExpandedContent && (
        <div className="base-tool-card-expanded">
          {expandedContent}
        </div>
      )}
      
      {isFailed && errorContent && (
        <div className="base-tool-card-error">
          {errorContent}
        </div>
      )}
    </div>
  );
};

/**
 * Tool card header subcomponent Props
 */
export interface ToolCardHeaderProps {
  /** Left tool identifier icon (colored) */
  icon?: ReactNode;
  /** Custom class name for tool icon */
  iconClassName?: string;
  /** Override context: show hover chevron when expandable */
  expandAffordance?: boolean;
  /** Override context: expand vs open-right-panel hint icon */
  affordanceKind?: ToolCardHeaderAffordanceKind;
  /** Override context: expanded state for chevron rotation */
  headerExpanded?: boolean;
  /** Optional dedicated affordance click handler for the left icon rail. */
  onAffordanceClick?: (e: React.MouseEvent<HTMLButtonElement>) => void;
  /** Action text */
  action?: string;
  /** Main content */
  content?: ReactNode;
  /** Right extra content (e.g., statistics, buttons, etc.) */
  extra?: ReactNode;
  /** Status icon at right border */
  statusIcon?: ReactNode;
}

/**
 * Tool card header component
 */
export const ToolCardHeader: React.FC<ToolCardHeaderProps> = ({
  icon,
  iconClassName,
  expandAffordance,
  affordanceKind,
  headerExpanded,
  onAffordanceClick,
  action,
  content,
  extra,
  statusIcon,
}) => {
  const layout = useToolCardHeaderLayout();
  const showExpandHint =
    expandAffordance !== undefined ? expandAffordance : layout.headerExpandAffordance;
  const resolvedAffordanceKind =
    affordanceKind !== undefined ? affordanceKind : layout.headerAffordanceKind;
  const expandedForChevron =
    headerExpanded !== undefined ? headerExpanded : layout.isExpanded;
  const isPanelAffordance = resolvedAffordanceKind === 'open-panel-right';

  return (
    <>
      {icon != null && icon !== false && icon !== '' && (
        <div
          className={`tool-card-icon-slot${showExpandHint ? ' tool-card-icon-slot--expandable' : ''}${showExpandHint && isPanelAffordance ? ' tool-card-icon-slot--affordance-panel' : ''}`}
        >
          <div className="tool-card-icon-marks">
            <div className={`tool-card-icon tool-identifier-icon tool-card-icon-main ${iconClassName || ''}`}>
              {icon}
            </div>
            {showExpandHint && (
              <span
                className={`tool-card-icon-expand-hint${!isPanelAffordance && expandedForChevron ? ' tool-card-icon-expand-hint--open' : ''}`}
                aria-hidden
              >
                {isPanelAffordance ? (
                  <ChevronRight size={16} strokeWidth={2} absoluteStrokeWidth />
                ) : (
                  <ChevronDown size={16} strokeWidth={2} absoluteStrokeWidth />
                )}
              </span>
            )}
          </div>
          {showExpandHint && onAffordanceClick && (
            <button
              type="button"
              className="tool-card-icon-affordance-hit"
              onClick={(e) => {
                e.stopPropagation();
                onAffordanceClick(e);
              }}
              aria-label={isPanelAffordance ? 'Open details' : 'Expand details'}
            />
          )}
        </div>
      )}
      {action && <span className="tool-card-action">{action}</span>}
      {content && <div className="tool-card-content">{content}</div>}
      {extra && <div className="tool-card-extra">{extra}</div>}
      {statusIcon && (
        <div className={`tool-card-status-icon ${extra ? 'tool-card-status-icon--with-divider' : ''}`}>
          {statusIcon}
        </div>
      )}
    </>
  );
};
