/**
 * NavItem — a single navigation row inside the NavPanel.
 *
 * Renders icon + label + optional badge.
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
  badge,
  onBadgeClick,
  actionIcon: ActionIcon,
  actionTitle,
  onActionClick,
  renderActions,
  onClick,
}) => {
  const { Icon } = item;
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

  return (
    <button
      type="button"
      className={[
        'bitfun-nav-panel__item',
        isActive && 'is-active',
      ]
        .filter(Boolean)
        .join(' ')}
      onClick={onClick}
      title={tooltipContent ?? displayLabel}
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
              <ActionIcon size="var(--bitfun-nav-row-action-icon-size)" />
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
            <ActionIcon size="var(--bitfun-nav-row-action-icon-size)" />
          </span>
        )
      )}
    </button>
  );
};

export default NavItem;
