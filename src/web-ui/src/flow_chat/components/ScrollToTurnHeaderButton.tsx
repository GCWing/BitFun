/**
 * Scroll-to-current-turn-header button.
 * Shows at the top of the message list when the current turn's user message
 * has scrolled out of view above the viewport.
 */

import React from 'react';
import { useTranslation } from 'react-i18next';
import { Tooltip } from '@/component-library';
import './ScrollToTurnHeaderButton.scss';

interface ScrollToTurnHeaderButtonProps {
  visible: boolean;
  onClick: () => void;
  turnLabel?: string;
  className?: string;
}

export const ScrollToTurnHeaderButton: React.FC<ScrollToTurnHeaderButtonProps> = ({
  visible,
  onClick,
  turnLabel,
  className = ''
}) => {
  const { t } = useTranslation('flow-chat');

  return (
    <div
      className={`scroll-to-turn-header-trigger ${visible ? 'scroll-to-turn-header-trigger--visible' : ''} ${className}`}
      aria-hidden={!visible}
    >
      <div className="scroll-to-turn-header-trigger__gradient" />
      <div className="scroll-to-turn-header-trigger__content">
        <Tooltip content={turnLabel || t('scroll.toCurrentTurn', { defaultValue: 'Jump to current turn' })}>
          <button
            className="scroll-to-turn-header-trigger__btn"
            onClick={onClick}
            aria-label={turnLabel || t('scroll.toCurrentTurn', { defaultValue: 'Jump to current turn' })}
            tabIndex={visible ? 0 : -1}
          >
            <svg
              width="16"
              height="16"
              viewBox="0 0 16 16"
              fill="none"
              xmlns="http://www.w3.org/2000/svg"
            >
              <path
                d="M8 12.5V3.5M8 3.5L4 7.5M8 3.5L12 7.5"
                stroke="currentColor"
                strokeWidth="1.5"
                strokeLinecap="round"
                strokeLinejoin="round"
              />
            </svg>
          </button>
        </Tooltip>
      </div>
    </div>
  );
};

ScrollToTurnHeaderButton.displayName = 'ScrollToTurnHeaderButton';
