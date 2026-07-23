/**
 * Scroll-to-bottom button.
 * Shows when the user scrolls up; click to return to latest messages.
 */

import React from 'react';
import { useTranslation } from 'react-i18next';
import { Tooltip } from '@/component-library';
import './ScrollToBottomButton.scss';

interface ScrollToBottomButtonProps {
  visible: boolean;
  onClick: () => void;
  unreadCount?: number; // Optional: show unread message count.
  className?: string;
}

export const ScrollToBottomButton: React.FC<ScrollToBottomButtonProps> = ({
  visible,
  onClick,
  unreadCount,
  className = ''
}) => {
  const { t } = useTranslation('flow-chat');
  
  if (!visible) return null;

  return (
    <Tooltip content={t('scroll.toBottom')}>
      <button
        className={`scroll-to-bottom-button ${className}`}
        onClick={onClick}
        aria-label={unreadCount ? t('scroll.toBottomWithCount', { count: unreadCount }) : t('scroll.toBottom')}
      >
        <svg
          className="scroll-icon"
          width="20"
          height="20"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="3"
          strokeLinecap="round"
          strokeLinejoin="round"
        >
          <polyline points="6 9 12 15 18 9" />
        </svg>
        {unreadCount !== undefined && unreadCount > 0 && (
          <span className="unread-badge">{unreadCount > 99 ? '99+' : unreadCount}</span>
        )}
      </button>
    </Tooltip>
  );
};

