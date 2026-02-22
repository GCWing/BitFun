/**
 * NavItem — a single navigation row inside the NavPanel.
 *
 * Renders icon + label + optional expand chevron + optional badge.
 * Scene-type items display a compact badge (e.g. git branch name).
 * Optional action icon (e.g. Plus for new session) for quick actions.
 */

import React, { useRef } from 'react';
import type { LucideIcon } from 'lucide-react';
import { Tooltip } from '@/component-library';
import type { NavItem as NavItemConfig } from '../types';

interface NavItemProps {
  item: NavItemConfig;
  /** Translated label (from i18n) for display and tooltip */
  displayLabel: string;
  /** Custom tooltip content (overrides displayLabel as tooltip when provided) */
  tooltipContent?: string;
  isActive: boolean;
  isOpen?: boolean;
  /** Optional badge text shown at the right (e.g. branch name) */
  badge?: string;
  /** Called when badge area is clicked (e.g. open BranchQuickSwitch) */
  onBadgeClick?: (ref: React.RefObject<HTMLElement>) => void;
  /** Optional icon for quick action (e.g. Plus for new session), shown at right */
  actionIcon?: LucideIcon;
  /** Accessible label for the action icon (e.g. "New session") */
  actionTitle?: string;
  /** Called when action icon is clicked (event is stopped from propagating to item click) */
  onActionClick?: () => void;
  /** Custom render for the action area — replaces default actionIcon when provided */
  renderActions?: () => React.ReactNode;
  onClick: () => void;
}

const NavItem: React.FC<NavItemProps> = ({
  item,
  displayLabel,
  tooltipContent,
  isActive,
  isOpen = false,
  badge,
  onBadgeClick,
  actionIcon: ActionIcon,
  actionTitle,
  onActionClick,
  renderActions,
  onClick,
}) => {
  const { Icon, inlineExpandable } = item;
  const badgeRef = useRef<HTMLSpanElement>(null);

  const handleBadgeClick = (e: React.MouseEvent) => {
    if (onBadgeClick) {
      e.stopPropagation();
      onBadgeClick(badgeRef as React.RefObject<HTMLElement>);
    }
  };

  const handleActionClick = (e: React.MouseEvent) => {
    e.stopPropagation();
    onActionClick?.();
  };

  const button = (
    <button
      type="button"
      className={[
        'bitfun-nav-panel__item',
        isActive && 'is-active',
        isOpen   && 'is-open',
      ]
        .filter(Boolean)
        .join(' ')}
      onClick={onClick}
      aria-expanded={inlineExpandable ? isOpen : undefined}
    >
      <span className="bitfun-nav-panel__item-icon" aria-hidden="true">
        <Icon size={15} />
      </span>
      <span className="bitfun-nav-panel__item-label">{displayLabel}</span>

      {badge && (
        <span
          ref={badgeRef}
          className={`bitfun-nav-panel__item-badge ${onBadgeClick ? 'bitfun-nav-panel__item-badge--clickable' : ''}`}
          onClick={handleBadgeClick}
          title={badge}
        >
          {badge}
        </span>
      )}

      {renderActions ? (
        <span className="bitfun-nav-panel__item-actions-custom" onClick={e => e.stopPropagation()} onMouseDown={e => e.stopPropagation()}>
          {renderActions()}
        </span>
      ) : ActionIcon && onActionClick && (
        actionTitle ? (
          <Tooltip content={actionTitle} placement="right" followCursor>
            <span
              className="bitfun-nav-panel__item-action"
              onClick={handleActionClick}
              onMouseDown={e => e.stopPropagation()}
              role="button"
              tabIndex={-1}
              aria-label={actionTitle}
            >
              <ActionIcon size={14} />
            </span>
          </Tooltip>
        ) : (
          <span
            className="bitfun-nav-panel__item-action"
            onClick={handleActionClick}
            onMouseDown={e => e.stopPropagation()}
            role="button"
            tabIndex={-1}
          >
            <ActionIcon size={14} />
          </span>
        )
      )}
    </button>
  );

  const tooltipText = tooltipContent || displayLabel;

  return (
    <Tooltip content={tooltipText} placement="right" followCursor>
      {button}
    </Tooltip>
  );
};

export default NavItem;
