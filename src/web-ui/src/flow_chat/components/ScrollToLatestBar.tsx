/**
 * Scroll-to-latest bar.
 * Minimal divider style with a soft fade.
 */

import React, { useLayoutEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { RetainedMountBoundary } from '@/shared/presence';
import {
  CHAT_INPUT_DROP_ZONE_BOTTOM_PX,
  SCROLL_TO_LATEST_INPUT_CLEARANCE_PX,
} from '../utils/flowChatScrollLayout';
import './ScrollToLatestBar.scss';

interface ScrollToLatestBarProps {
  visible: boolean;
  onClick: () => void;
  /** Measured height of the ChatInput container in pixels (0 if unknown). */
  inputHeight?: number;
  className?: string;
  focusReturnRef?: React.RefObject<HTMLElement | null>;
}

export const ScrollToLatestBar: React.FC<ScrollToLatestBarProps> = ({
  visible,
  onClick,
  inputHeight = 0,
  className = '',
  focusReturnRef,
}) => {
  const { t } = useTranslation('flow-chat');
  const barRef = useRef<HTMLDivElement>(null);

  useLayoutEffect(() => {
    if (!visible && barRef.current?.contains(document.activeElement)) {
      const focusTarget = focusReturnRef?.current;
      if (focusTarget) {
        focusTarget.focus({ preventScroll: true });
      } else {
        (document.activeElement as HTMLElement).blur();
      }
    }
  }, [focusReturnRef, visible]);

  // Dynamically compute bar height and button position based on measured ChatInput height.
  //
  // IMPORTANT: __content is position:absolute within the bar, so its `bottom` is
  // relative to the bar—not to the viewport. If bottom > barHeight the content
  // overflows the bar and is clipped by virtual-message-list's overflow:hidden.
  // Therefore we always set barHeight >= contentBottom + button clearance together.
  //
  // Layout constants: shared with VirtualMessageList footer (flowChatScrollLayout).
  const ABOVE_BTN = 24; // gradient fade above the control
  let dynamicStyle: React.CSSProperties = {};
  let contentStyle: React.CSSProperties | undefined;

  if (inputHeight > 0) {
    const contentBottom =
      inputHeight + CHAT_INPUT_DROP_ZONE_BOTTOM_PX + SCROLL_TO_LATEST_INPUT_CLEARANCE_PX;
    const barHeight = contentBottom + ABOVE_BTN;

    dynamicStyle = { height: `${barHeight}px` };
    contentStyle = { bottom: `${contentBottom}px` };
  }

  return (
    <RetainedMountBoundary present={visible}>
      <div
        ref={barRef}
        data-bf-component="scroll-to-latest-bar"
        data-bf-part="root"
        data-bf-input="active"
        data-visible={visible ? 'true' : 'false'}
        className={`scroll-to-latest-bar ${className}`}
        style={dynamicStyle}
        onClick={visible ? onClick : undefined}
        role="button"
        tabIndex={visible ? 0 : -1}
        onKeyDown={(e) => {
          if (visible && (e.key === 'Enter' || e.key === ' ')) {
            e.preventDefault();
            onClick();
          }
        }}
        aria-hidden={!visible}
        {...(!visible ? { inert: '' } : {})}
        aria-label={t('scroll.toLatest')}
      >
        <div data-bf-component="scroll-to-latest-bar" data-bf-part="gradient" className="scroll-to-latest-bar__gradient" />

        <div data-bf-component="scroll-to-latest-bar" data-bf-part="content" className="scroll-to-latest-bar__content" style={contentStyle}>
          <button data-bf-component="scroll-to-latest-bar" data-bf-part="button" className="scroll-to-latest-bar__btn" aria-hidden="true" tabIndex={-1}>
            <svg width="16" height="16" viewBox="0 0 16 16" fill="none" xmlns="http://www.w3.org/2000/svg">
              <path d="M8 3.5V12.5M8 12.5L4 8.5M8 12.5L12 8.5" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"/>
            </svg>
          </button>
        </div>
      </div>
    </RetainedMountBoundary>
  );
};

ScrollToLatestBar.displayName = 'ScrollToLatestBar';
