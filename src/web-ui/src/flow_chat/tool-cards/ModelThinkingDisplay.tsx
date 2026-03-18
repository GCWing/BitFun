/**
 * Model thinking display component.
 * Default expanded. Collapses when isLastItem becomes false
 * (i.e. the next atomic step has started).
 * Applies typewriter effect during streaming.
 */

import React, { useState, useEffect, useRef, useCallback, useMemo } from 'react';
import { ChevronRight } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import type { FlowThinkingItem } from '../types/flow-chat';
import { useTypewriter } from '../hooks/useTypewriter';
import './ModelThinkingDisplay.scss';

interface ModelThinkingDisplayProps {
  thinkingItem: FlowThinkingItem;
  /** Whether this is the last item in the current round. */
  isLastItem?: boolean;
}

export const ModelThinkingDisplay: React.FC<ModelThinkingDisplayProps> = ({ thinkingItem, isLastItem = true }) => {
  const { t } = useTranslation('flow-chat');
  const { content, isStreaming, status } = thinkingItem;
  const contentRef = useRef<HTMLDivElement>(null);

  const isActive = isStreaming || status === 'streaming';
  const displayContent = useTypewriter(content, isActive);

  const [isExpanded, setIsExpanded] = useState(true);
  const userToggledRef = useRef(false);

  useEffect(() => {
    if (userToggledRef.current) return;
    if (!isLastItem) {
      setIsExpanded(false);
    }
  }, [isLastItem]);

  // Auto-scroll to bottom while content grows.
  useEffect(() => {
    if (isExpanded && contentRef.current) {
      const el = contentRef.current;
      const gap = el.scrollHeight - el.scrollTop - el.clientHeight;
      if (gap < 80) {
        requestAnimationFrame(() => {
          if (contentRef.current) {
            contentRef.current.scrollTop = contentRef.current.scrollHeight;
          }
        });
      }
    }
  }, [displayContent, isExpanded]);

  // Scroll-state detection for fade gradients.
  const [scrollState, setScrollState] = useState({ hasScroll: false, atTop: true, atBottom: true });

  const checkScrollState = useCallback(() => {
    const el = contentRef.current;
    if (!el) return;
    setScrollState({
      hasScroll: el.scrollHeight > el.clientHeight,
      atTop: el.scrollTop <= 5,
      atBottom: el.scrollTop + el.clientHeight >= el.scrollHeight - 5,
    });
  }, []);

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
    window.dispatchEvent(new CustomEvent('tool-card-toggle'));
    userToggledRef.current = true;
    setIsExpanded(prev => !prev);
  };

  const headerLabel = isExpanded
    ? (isActive ? t('toolCards.think.thinking') : t('toolCards.think.thinkingProcess'))
    : contentLengthText;

  const wrapperClassName = [
    'flow-thinking-item',
    isExpanded ? 'expanded' : 'collapsed',
  ].filter(Boolean).join(' ');

  const renderedContent = isActive ? displayContent : content;

  return (
    <div className={wrapperClassName}>
      <div
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
            className={`thinking-content expanded`}
            onScroll={checkScrollState}
          >
            {renderedContent.split('\n').map((line: string, index: number) => (
              <div key={index} className="thinking-line">
                {line || '\u00A0'}
              </div>
            ))}
          </div>
        </div>
      </div>
    </div>
  );
};
