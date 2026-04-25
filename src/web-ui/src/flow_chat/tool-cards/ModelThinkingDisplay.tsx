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
import { useToolCardHeightContract } from './useToolCardHeightContract';
import { Markdown } from '@/component-library/components/Markdown/Markdown';
import { aiExperienceConfigService } from '@/infrastructure/config/services/AIExperienceConfigService';
import './ModelThinkingDisplay.scss';

interface ModelThinkingDisplayProps {
  thinkingItem: FlowThinkingItem;
  /** Whether this is the last item in the current round. */
  isLastItem?: boolean;
}

export const ModelThinkingDisplay: React.FC<ModelThinkingDisplayProps> = ({ thinkingItem, isLastItem = true }) => {
  const { t } = useTranslation('flow-chat');
  const { content, isStreaming, status } = thinkingItem;
  const wrapperRef = useRef<HTMLDivElement>(null);
  const contentRef = useRef<HTMLDivElement>(null);

  const isActive = isStreaming || status === 'streaming';
  const displayContent = useTypewriter(content, isActive);

  const [isExpanded, setIsExpanded] = useState(isLastItem);
  const [thinkingDisplaySettings, setThinkingDisplaySettings] = useState(() => {
    const settings = aiExperienceConfigService.getSettings();
    return {
      showThinkingProcess: settings.show_thinking_process,
      showCompletedThinkingItem: settings.show_completed_thinking_item,
    };
  });
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
    let cancelled = false;
    aiExperienceConfigService.getSettingsAsync().then(settings => {
      if (cancelled) return;
      setThinkingDisplaySettings({
        showThinkingProcess: settings.show_thinking_process,
        showCompletedThinkingItem: settings.show_completed_thinking_item,
      });
    });

    const unsubscribe = aiExperienceConfigService.addChangeListener(settings => {
      setThinkingDisplaySettings({
        showThinkingProcess: settings.show_thinking_process,
        showCompletedThinkingItem: settings.show_completed_thinking_item,
      });
    });

    return () => {
      cancelled = true;
      unsubscribe();
    };
  }, []);

  useEffect(() => {
    if (userToggledRef.current) return;
    if (!isLastItem && isExpanded) {
      applyExpandedState(isExpanded, false, setIsExpanded, {
        reason: 'auto',
      });
    }
  }, [applyExpandedState, isExpanded, isLastItem]);

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
    const nextExpanded = !isExpanded;
    userToggledRef.current = true;
    applyExpandedState(isExpanded, nextExpanded, setIsExpanded);
  };

  const headerLabel = (isExpanded
    ? (isActive ? t('toolCards.think.thinking') : t('toolCards.think.thinkingProcess'))
    : contentLengthText).replace(/ /g, '\u00A0');

  const wrapperClassName = [
    'flow-thinking-item',
    isExpanded ? 'expanded' : 'collapsed',
  ].filter(Boolean).join(' ');

  const renderedContent = isActive ? displayContent : content;

  if (
    !thinkingDisplaySettings.showThinkingProcess ||
    (!isActive && !thinkingDisplaySettings.showCompletedThinkingItem)
  ) {
    return null;
  }

  return (
    <div ref={wrapperRef} data-tool-card-id={thinkingItem.id} className={wrapperClassName}>
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
            <Markdown
              content={renderedContent}
              isStreaming={isActive}
              className="thinking-markdown"
            />
          </div>
        </div>
      </div>
    </div>
  );
};
