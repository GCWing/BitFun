/**
 * Shared left icon-slot component for tool card headers.
 * Provides consistent icon size (16px), container width (34px), alignment,
 * hover chevron swap, and optional right border divider.
 */
import React, { ReactNode } from 'react';
import { ChevronDown, ChevronRight } from 'lucide-react';
import type { ToolCardHeaderAffordanceKind } from './ToolCardHeaderLayoutContext';

export interface ToolCardIconSlotProps {
  /** Main tool icon (should be 16px lucide icon) */
  icon: ReactNode;
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
  /** Show right border divider (default true) */
  showDivider?: boolean;
  /** Additional class name for the root element */
  className?: string;
}

export const ToolCardIconSlot: React.FC<ToolCardIconSlotProps> = ({
  icon,
  iconClassName,
  expandable = false,
  affordanceKind = 'expand',
  isExpanded = false,
  onAffordanceClick,
  showDivider = true,
  className,
}) => {
  const isPanelAffordance = affordanceKind === 'open-panel-right';

  return (
    <div
      className={`tool-card-icon-slot${expandable ? ' tool-card-icon-slot--expandable' : ''}${expandable && isPanelAffordance ? ' tool-card-icon-slot--affordance-panel' : ''}${!showDivider ? ' tool-card-icon-slot--no-divider' : ''}${className ? ` ${className}` : ''}`}
    >
      <div className="tool-card-icon-marks">
        <div className={`tool-card-icon tool-identifier-icon tool-card-icon-main ${iconClassName || ''}`}>
          {icon}
        </div>
        {expandable && (
          <span
            className={`tool-card-icon-expand-hint${!isPanelAffordance && isExpanded ? ' tool-card-icon-expand-hint--open' : ''}`}
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
      {expandable && onAffordanceClick && (
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
  );
};
