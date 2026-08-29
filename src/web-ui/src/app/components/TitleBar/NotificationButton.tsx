/**
 * NotificationButton — global notification indicator for TitleBar.
 *
 * Extracted from StatusBar. Shows bell icon (with dot on unread),
 * or active task progress indicator. Clicking opens NotificationCenter.
 */

import React, { useRef, useEffect, useState } from 'react';
import { BellDot, BellRing } from 'lucide-react';
import { Icon, MenuItem, Tooltip } from '@bitfun/ui';

import { useI18n } from '@/infrastructure/i18n/hooks/useI18n';
import {
  useUnreadCount,
  useLatestTaskNotification,
} from '../../../shared/notification-system/hooks/useNotificationState';
import { notificationService } from '../../../shared/notification-system/services/NotificationService';
import './NotificationButton.scss';

interface NotificationButtonProps {
  className?: string;
  navFooterHoverIconSwap?: boolean;
  menuItem?: boolean;
  onActivate?: () => void;
}

const NotificationButton: React.FC<NotificationButtonProps> = ({
  className = '',
  navFooterHoverIconSwap = false,
  menuItem = false,
  onActivate,
}) => {
  const { t } = useI18n('common');
  const buttonRef = useRef<HTMLButtonElement | null>(null);
  const [tooltipOffset, setTooltipOffset] = useState(0);

  const unreadCount = useUnreadCount();
  const activeNotification = useLatestTaskNotification();

  useEffect(() => {
    if (activeNotification?.title && buttonRef.current) {
      const rect = buttonRef.current.getBoundingClientRect();
      const vw = window.innerWidth;
      const center = rect.left + rect.width / 2;
      const len = activeNotification.title.length || 0;
      const estW = Math.min(Math.max(len * 8, 120), 300);
      const right = center + estW / 2;
      let offset = 0;
      if (right > vw - 16) offset = vw - 16 - right;
      if (center - estW / 2 + offset < 16) offset = 16 - (center - estW / 2);
      setTooltipOffset(offset);
    }
  }, [activeNotification]);

  const handleActivate = () => {
    notificationService.toggleCenter();
    onActivate?.();
  };

  if (menuItem) {
    const progressLabel = activeNotification?.variant === 'loading'
      ? null
      : activeNotification
        ? `${Math.round(activeNotification.progress || 0)}%`
        : null;

    return (
      <MenuItem
        className={className || undefined}
        leading={activeNotification?.variant === 'loading' ? (
          <svg
            width="14"
            height="14"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2.5"
            className="bitfun-notification-btn__spinner"
            aria-hidden="true"
          >
            <path d="M12 2 A 10 10 0 0 1 22 12" strokeLinecap="round" />
          </svg>
        ) : unreadCount > 0 ? (
          <BellDot size={14} className="bitfun-notification-btn__icon--has-message" aria-hidden="true" />
        ) : (
          <Icon name="bell" size="sm" aria-hidden="true" />
        )}
        metadata={progressLabel ? (
          <span className="bitfun-notification-btn__menu-status">{progressLabel}</span>
        ) : unreadCount > 0 ? (
          <span className="bitfun-notification-btn__menu-count">{unreadCount}</span>
        ) : undefined}
        onClick={handleActivate}
        aria-label={t('nav.notifications')}
        data-testid="notification-button"
      >
        {t('nav.notifications')}
      </MenuItem>
    );
  }

  return (
    <Tooltip content={t('nav.notifications')} placement="right" disabled={!!activeNotification}>
    <button data-bf-component="notification-button" data-bf-part="root"
      ref={buttonRef}
      className={[
        'bitfun-notification-btn',
        activeNotification ? 'bitfun-notification-btn--has-progress' : '',
        activeNotification?.variant === 'loading' ? 'bitfun-notification-btn--loading' : '',
        navFooterHoverIconSwap && !activeNotification ? 'bitfun-notification-btn--nav-hover-icon' : '',
        className,
      ].filter(Boolean).join(' ')}
      onClick={handleActivate}
      type="button"
      aria-label={t('nav.notifications')}
      data-testid="notification-button"
    >
      {activeNotification ? (
        <>
          <div className="bitfun-notification-btn__progress" data-bf-component="notification-button" data-bf-part="progress">
            {activeNotification.variant === 'loading' ? (
              <div className="bitfun-notification-btn__loading-icon" data-bf-component="notification-button" data-bf-part="loadingIcon">
                <svg width="12" height="12" viewBox="0 0 24 24" fill="none"
                  stroke="currentColor" strokeWidth="2.5"
                  className="bitfun-notification-btn__spinner">
                  <path d="M12 2 A 10 10 0 0 1 22 12" strokeLinecap="round" />
                </svg>
              </div>
            ) : (
              <div className="bitfun-notification-btn__progress-icon" data-bf-component="notification-button" data-bf-part="progressIcon">
                <svg width="12" height="12" viewBox="0 0 24 24" fill="none"
                  stroke="currentColor" strokeWidth="2">
                  <circle cx="12" cy="12" r="10" opacity="0.2" />
                  <path d="M12 2 A 10 10 0 0 1 22 12" strokeLinecap="round"
                    style={{
                      strokeDasharray: `${(activeNotification.progress || 0) * 0.628} 62.8`,
                      transform: 'rotate(-90deg)',
                      transformOrigin: 'center',
                    }}
                  />
                </svg>
              </div>
            )}
            <span className="bitfun-notification-btn__progress-text" data-bf-component="notification-button" data-bf-part="progressText">
              {activeNotification.variant === 'loading'
                ? activeNotification.message
                : (() => {
                    const mode = activeNotification.progressMode ||
                      (activeNotification.textOnly ? 'text-only' : 'percentage');
                    if (mode === 'fraction' &&
                      activeNotification.current !== undefined &&
                      activeNotification.total !== undefined) {
                      return `${activeNotification.current}/${activeNotification.total}`;
                    }
                    return `${Math.round(activeNotification.progress || 0)}%`;
                  })()}
            </span>
          </div>
          <div
            className="bitfun-notification-btn__tooltip"
            data-bf-component="notification-button"
            data-bf-part="tooltip"
            style={{ transform: `translateX(calc(-50% + ${tooltipOffset}px))` }}
          >
            <div
              className="bitfun-notification-btn__tooltip-content"
              data-bf-component="notification-button"
              data-bf-part="tooltipContent"
              style={{ '--tooltip-offset': `${tooltipOffset}px` } as React.CSSProperties}
            >
              {activeNotification.title}
            </div>
          </div>
        </>
      ) : navFooterHoverIconSwap ? (
        unreadCount > 0 ? (
          <span className="bitfun-nav-panel__footer-btn-icon-swap" aria-hidden="true">
            <BellDot
              size={15}
              className="bitfun-notification-btn__icon--has-message bitfun-nav-panel__footer-btn-icon-swap-default"
            />
            <BellRing size={15} className="bitfun-nav-panel__footer-btn-icon-swap-hover" />
          </span>
        ) : (
          <span className="bitfun-nav-panel__footer-btn-icon-swap" aria-hidden="true">
            <Icon name="bell" size="sm" className="bitfun-nav-panel__footer-btn-icon-swap-default" />
            <BellRing size={15} className="bitfun-nav-panel__footer-btn-icon-swap-hover" />
          </span>
        )
      ) : (
        unreadCount > 0
          ? <BellDot size={14} className="bitfun-notification-btn__icon--has-message" />
          : <Icon name="bell" size="sm" />
      )}
    </button>
    </Tooltip>
  );
};

export default NotificationButton;
