/**
 * Model thinking display component.
 * Default expanded while this is still the active last step.
 * If the component mounts after later content already appeared
 * (for example after a parent remount), start collapsed directly
 * to avoid a visible expand-then-collapse flash.
 * Applies typewriter effect during streaming.
 */

import React, { useState, useEffect, useRef, useCallback, useMemo } from 'react';
import { ChevronRight } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import type { FlowThinkingItem } from '../types/flow-chat';
import { useTypewriter } from '../hooks/useTypewriter';
import { useReportTypewriterReveal } from '../hooks/typewriterRevealGateContext';
import { useToolCardHeightContract } from './useToolCardHeightContract';
import { Markdown } from '@/component-library/components/Markdown/Markdown';
import './ModelThinkingDisplay.scss';

interface ModelThinkingDisplayProps {
  thinkingItem: FlowThinkingItem;
  /** Whether this is the last item in the current round. */
  isLastItem?: boolean;
  displayContext?: 'default' | 'subagent-projection';
}

export const ModelThinkingDisplay: React.FC<ModelThinkingDisplayProps> = ({
  thinkingItem,
  isLastItem = true,
  displayContext = 'default',
}) => {
  const { t } = useTranslation('flow-chat');
  const { content, isStreaming, status } = thinkingItem;
  const wrapperRef = useRef<HTMLDivElement>(null);
  const contentRef = useRef<HTMLDivElement>(null);
  const shouldFollowTailRef = useRef(true);
  const tailFollowPauseVersionRef = useRef(0);
  const tailFollowUserPauseUntilMsRef = useRef(0);
  const touchScrollStartYRef = useRef<number | null>(null);

  const isActive = isStreaming || status === 'streaming';
  const { displayText: displayContent, isRevealing } = useTypewriter(content, isActive);
  useReportTypewriterReveal(thinkingItem.id, isRevealing);
  const shouldDefaultExpanded =
    displayContext === 'subagent-projection'
      ? isActive || isLastItem
      : isLastItem;

  const [isExpanded, setIsExpanded] = useState(shouldDefaultExpanded);
  const userToggledRef = useRef(false);
  const { applyExpandedState } = useToolCardHeightContract({
    toolId: thinkingItem.id,
    toolName: 'thinking',
    getCardHeight: () => {
      const contentScrollHeight = contentRef.current?.scrollHeight ?? null;
      const wrapperHeight = wrapperRef.current?.getBoundingClientRect().height ?? null;
      return contentScrollHeight ?? wrapperHeight;
    },
  });

  useEffect(() => {
    if (userToggledRef.current) return;
    if (isExpanded !== shouldDefaultExpanded) {
      applyExpandedState(isExpanded, shouldDefaultExpanded, setIsExpanded, {
        reason: 'auto',
      });
    }
  }, [applyExpandedState, isExpanded, shouldDefaultExpanded]);

  useEffect(() => {
    if (userToggledRef.current) return;
    if (!shouldDefaultExpanded && isExpanded) {
      applyExpandedState(isExpanded, false, setIsExpanded, {
        reason: 'auto',
      });
    }
  }, [applyExpandedState, isExpanded, shouldDefaultExpanded]);

  // Keep rendering the typewriter output while it drains after the stream
  // ends. Snapping to full `content` here would make the drain invisible
  // while `isRevealing` still holds the reveal gate, delaying the round
  // footer for no visible reason.
  const renderedContent = isRevealing ? displayContent : content;
  // Cover the whole reveal with Markdown streaming mode so the Prism upgrade
  // does not land mid-drain.
  const isVisuallyStreaming = isActive || isRevealing;

  const getThinkingScrollGap = useCallback((el: HTMLElement) => (
    el.scrollHeight - el.scrollTop - el.clientHeight
  ), []);

  const scrollThinkingToBottom = useCallback((expectedPauseVersion?: number) => {
    const el = contentRef.current;
    if (!el) return;
    if (
      expectedPauseVersion !== undefined &&
      expectedPauseVersion !== tailFollowPauseVersionRef.current
    ) {
      return;
    }
    if (!shouldFollowTailRef.current) {
      return;
    }

    el.scrollTop = el.scrollHeight;
    shouldFollowTailRef.current = true;
  }, []);

  const pauseTailFollowForUserScroll = useCallback(() => {
    shouldFollowTailRef.current = false;
    tailFollowPauseVersionRef.current += 1;
    tailFollowUserPauseUntilMsRef.current = performance.now() + 700;
  }, []);

  // Auto-scroll to bottom while content grows.
  useEffect(() => {
    if (isExpanded && contentRef.current) {
      const el = contentRef.current;
      const gap = getThinkingScrollGap(el);
      const wasNearBottom = gap < 80;
      const userPauseActive = performance.now() <= tailFollowUserPauseUntilMsRef.current;
      if (wasNearBottom && !userPauseActive) {
        shouldFollowTailRef.current = true;
      }
      const shouldScroll = shouldFollowTailRef.current || (wasNearBottom && !userPauseActive);
      if (shouldScroll) {
        const scheduledPauseVersion = tailFollowPauseVersionRef.current;
        requestAnimationFrame(() => {
          scrollThinkingToBottom(scheduledPauseVersion);
        });
      }
    }
  }, [
    displayContent,
    getThinkingScrollGap,
    isExpanded,
    scrollThinkingToBottom,
  ]);

  useEffect(() => {
    const el = contentRef.current;
    if (!el || !isExpanded) {
      return;
    }

    const observer = new ResizeObserver(() => {
      if (isActive && shouldFollowTailRef.current) {
        const scheduledPauseVersion = tailFollowPauseVersionRef.current;
        requestAnimationFrame(() => {
          scrollThinkingToBottom(scheduledPauseVersion);
        });
      }
    });

    observer.observe(el);
    const markdownEl = el.querySelector('.thinking-markdown');
    if (markdownEl instanceof HTMLElement) {
      observer.observe(markdownEl);
    }

    return () => observer.disconnect();
  }, [isActive, isExpanded, scrollThinkingToBottom]);

  // Scroll-state detection for fade gradients.
  const [scrollState, setScrollState] = useState({ hasScroll: false, atTop: true, atBottom: true });

  const checkScrollState = useCallback(() => {
    const el = contentRef.current;
    if (!el) return;
    const gap = getThinkingScrollGap(el);
    const nextScrollState = {
      hasScroll: el.scrollHeight > el.clientHeight,
      atTop: el.scrollTop <= 5,
      atBottom: gap <= 5,
    };
    if (
      nextScrollState.atBottom &&
      performance.now() > tailFollowUserPauseUntilMsRef.current
    ) {
      shouldFollowTailRef.current = true;
    }
    setScrollState({
      hasScroll: nextScrollState.hasScroll,
      atTop: nextScrollState.atTop,
      atBottom: nextScrollState.atBottom,
    });
  }, [getThinkingScrollGap]);

  useEffect(() => {
    if (isExpanded) {
      const timer = setTimeout(checkScrollState, 50);
      return () => clearTimeout(timer);
    }
  }, [isExpanded, checkScrollState]);

  const contentLengthText = useMemo(() => {
    if (!content || content.length === 0) return t('toolCards.think.thinkingComplete');
    return t('toolCards.think.thinkingCharacters', { count: content.length });
  }, [content, t]);

  const handleToggleClick = () => {
    const nextExpanded = !isExpanded;
    userToggledRef.current = true;
    applyExpandedState(isExpanded, nextExpanded, setIsExpanded);
  };

  const handleContentWheelCapture = useCallback((event: React.WheelEvent<HTMLDivElement>) => {
    if (event.deltaY < 0) {
      pauseTailFollowForUserScroll();
    }
  }, [pauseTailFollowForUserScroll]);

  const handleContentTouchStart = useCallback((event: React.TouchEvent<HTMLDivElement>) => {
    touchScrollStartYRef.current = event.touches[0]?.clientY ?? null;
  }, []);

  const handleContentTouchMove = useCallback((event: React.TouchEvent<HTMLDivElement>) => {
    const startY = touchScrollStartYRef.current;
    const currentY = event.touches[0]?.clientY;
    if (startY === null || currentY === undefined) {
      return;
    }

    if (currentY - startY > 6) {
      touchScrollStartYRef.current = currentY;
      pauseTailFollowForUserScroll();
    }
  }, [pauseTailFollowForUserScroll]);

  const handleContentTouchEnd = useCallback(() => {
    touchScrollStartYRef.current = null;
  }, []);

  const handleContentKeyDown = useCallback((event: React.KeyboardEvent<HTMLDivElement>) => {
    if (
      event.key === 'ArrowUp' ||
      event.key === 'PageUp' ||
      event.key === 'Home' ||
      (event.key === ' ' && event.shiftKey)
    ) {
      pauseTailFollowForUserScroll();
    }
  }, [pauseTailFollowForUserScroll]);

  const headerLabel = (isExpanded
    ? (isActive ? t('toolCards.think.thinking') : t('toolCards.think.thinkingProcess'))
    : contentLengthText).replace(/ /g, '\u00A0');

  const wrapperClassName = [
    'flow-thinking-item',
    isExpanded ? 'expanded' : 'collapsed',
  ].filter(Boolean).join(' ');

  return (
    <div
      ref={wrapperRef}
      data-testid="chat-thinking-panel"
      data-tool-card-id={thinkingItem.id}
      data-status={status}
      data-streaming={isActive ? 'true' : 'false'}
      data-expanded={isExpanded ? 'true' : 'false'}
      className={wrapperClassName}
    >
      <div
        data-testid="chat-thinking-toggle"
        className="thinking-collapsed-header"
        onClick={handleToggleClick}
      >
        <ChevronRight size={14} className="thinking-chevron" />
        <span className="thinking-label">{headerLabel}</span>
      </div>

      <div className={`thinking-expand-container ${isExpanded ? 'thinking-expand-container--open' : ''}`}>
        <div className={`thinking-content-wrapper ${scrollState.hasScroll ? 'has-scroll' : ''} ${scrollState.atTop ? 'at-top' : ''} ${scrollState.atBottom ? 'at-bottom' : ''}`}>
          <div
            ref={contentRef}
            data-testid="chat-thinking-content"
            data-status={status}
            data-streaming={isActive ? 'true' : 'false'}
            className={`thinking-content expanded`}
            onScroll={checkScrollState}
            onWheelCapture={handleContentWheelCapture}
            onTouchStart={handleContentTouchStart}
            onTouchMove={handleContentTouchMove}
            onTouchEnd={handleContentTouchEnd}
            onKeyDown={handleContentKeyDown}
          >
            <Markdown
              content={renderedContent}
              isStreaming={isVisuallyStreaming}
              className="thinking-markdown"
            />
          </div>
        </div>
      </div>
    </div>
  );
};
