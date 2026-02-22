/**
 * NotificationButton — global notification indicator for TitleBar.
 *
 * Extracted from StatusBar. Shows bell icon (with dot on unread),
 * or active task progress indicator. Clicking opens NotificationCenter.
 */

import React, { useRef, useEffect, useState } from 'react';
import { Bell, BellDot } from 'lucide-react';
import {
  useUnreadCount,
  useLatestTaskNotification,
} from '../../../shared/notification-system/hooks/useNotificationState';
import { notificationService } from '../../../shared/notification-system/services/NotificationService';
import './NotificationButton.scss';

interface NotificationButtonProps {
  className?: string;
}

const NotificationButton: React.FC<NotificationButtonProps> = ({ className = '' }) => {
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

  return (
    <button
      ref={buttonRef}
      className={[
        'bitfun-notification-btn',
        activeNotification ? 'bitfun-notification-btn--has-progress' : '',
        activeNotification?.variant === 'loading' ? 'bitfun-notification-btn--loading' : '',
        className,
      ].filter(Boolean).join(' ')}
      onClick={() => notificationService.toggleCenter()}
      title="Notifications"
      type="button"
    >
      {activeNotification ? (
        <>
          <div className="bitfun-notification-btn__progress">
            {activeNotification.variant === 'loading' ? (
              <div className="bitfun-notification-btn__loading-icon">
                <svg width="12" height="12" viewBox="0 0 24 24" fill="none"
                  stroke="currentColor" strokeWidth="2.5"
                  className="bitfun-notification-btn__spinner">
                  <path d="M12 2 A 10 10 0 0 1 22 12" strokeLinecap="round" />
                </svg>
              </div>
            ) : (
              <div className="bitfun-notification-btn__progress-icon">
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
            <span className="bitfun-notification-btn__progress-text">
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
            style={{ transform: `translateX(calc(-50% + ${tooltipOffset}px))` }}
          >
            <div
              className="bitfun-notification-btn__tooltip-content"
              style={{ '--tooltip-offset': `${tooltipOffset}px` } as React.CSSProperties}
            >
              {activeNotification.title}
            </div>
          </div>
        </>
      ) : (
        unreadCount > 0
          ? <BellDot size={14} className="bitfun-notification-btn__icon--has-message" />
          : <Bell size={14} />
      )}
    </button>
  );
};

export default NotificationButton;
