/**
 * Model thinking display component.
 * Shows internal model reasoning.
 * - Streaming: muted text, incremental output.
 * - Completed: auto-collapses, click to expand.
 */

import React, { useState, useEffect, useRef, useCallback, useMemo } from 'react';
import { ChevronRight } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import type { FlowThinkingItem } from '../types/flow-chat';
import './ModelThinkingDisplay.scss';

// Idle timeout after content stops growing (ms).
const CONTENT_IDLE_TIMEOUT = 500;

interface ModelThinkingDisplayProps {
  thinkingItem: FlowThinkingItem;
}

export const ModelThinkingDisplay: React.FC<ModelThinkingDisplayProps> = ({ thinkingItem }) => {
  const { t } = useTranslation('flow-chat');
  const { content, isStreaming, isCollapsed, status } = thinkingItem;
  const [isExpanded, setIsExpanded] = useState(false);
  const [scrollState, setScrollState] = useState({ hasScroll: false, atTop: true, atBottom: true });
  const contentRef = useRef<HTMLDivElement>(null);
  
  // Time-based heuristic to detect content growth.
  const [isContentGrowing, setIsContentGrowing] = useState(true);
  const lastContentRef = useRef(content);
  const timeoutRef = useRef<NodeJS.Timeout | null>(null);
  
  useEffect(() => {
    if (content !== lastContentRef.current) {
      lastContentRef.current = content;
      setIsContentGrowing(true);
      
      if (timeoutRef.current) {
        clearTimeout(timeoutRef.current);
      }
      
      timeoutRef.current = setTimeout(() => {
        setIsContentGrowing(false);
      }, CONTENT_IDLE_TIMEOUT);
    }
    
    return () => {
      if (timeoutRef.current) {
        clearTimeout(timeoutRef.current);
      }
    };
  }, [content]);
  
  useEffect(() => {
    if (status === 'completed' || !isStreaming) {
      setIsContentGrowing(false);
    }
  }, [status, isStreaming]);

  // Auto-collapse when streaming ends and the item is still expanded.
  useEffect(() => {
    if (!isStreaming && !isCollapsed && status === 'completed') {
      // Give the user a moment to see the full content.
      const timer = setTimeout(() => {
        // Parent state controls collapse; keep a local expanded flag here.
        setIsExpanded(false);
      }, 500);
      return () => clearTimeout(timer);
    }
  }, [isStreaming, isCollapsed, status]);

  const checkScrollState = useCallback(() => {
    const el = contentRef.current;
    if (!el) return;
    
    const hasScroll = el.scrollHeight > el.clientHeight;
    const atTop = el.scrollTop <= 5;
    const atBottom = el.scrollTop + el.clientHeight >= el.scrollHeight - 5;
    
    setScrollState({ hasScroll, atTop, atBottom });
  }, []);

  useEffect(() => {
    if (isExpanded) {
      // Delay to wait for DOM layout.
      const timer = setTimeout(checkScrollState, 50);
      return () => clearTimeout(timer);
    }
  }, [isExpanded, checkScrollState]);

  const contentLengthText = useMemo(() => {
    if (!content || content.length === 0) return t('toolCards.think.thinkingComplete');
    return t('toolCards.think.thinkingCharacters', { count: content.length });
  }, [content, t]);

  if (isStreaming || status === 'streaming') {
    const hasContent = content && content.length > 0;
    // Only show shimmer when content is actively growing.
    const isActivelyStreaming = status === 'streaming' && isContentGrowing;
    return (
      <div className="flow-thinking-item streaming">
        <div className="thinking-header">
          <span className="thinking-label">{t('toolCards.think.thinking')}</span>
        </div>
        <div className={`thinking-content streaming ${hasContent && isActivelyStreaming ? 'thinking-content--has-content' : ''}`}>
          {content}
        </div>
      </div>
    );
  }

  const handleToggleClick = () => {
    // Notify VirtualMessageList to prevent auto-scroll on user toggle.
    window.dispatchEvent(new CustomEvent('tool-card-toggle'));
    setIsExpanded(!isExpanded);
  };

  return (
    <div className={`flow-thinking-item collapsed ${isExpanded ? 'expanded' : ''}`}>
      <div 
        className="thinking-collapsed-header"
        onClick={handleToggleClick}
      >
        <ChevronRight size={14} className="thinking-chevron" />
        <span className="thinking-label">
          {isExpanded ? t('toolCards.think.thinkingProcess') : contentLengthText}
        </span>
      </div>

      <div className={`thinking-expand-container ${isExpanded ? 'thinking-expand-container--open' : ''}`}>
        <div className={`thinking-content-wrapper ${scrollState.hasScroll ? 'has-scroll' : ''} ${scrollState.atTop ? 'at-top' : ''} ${scrollState.atBottom ? 'at-bottom' : ''}`}>
          <div 
            ref={contentRef}
            className="thinking-content expanded"
            onScroll={checkScrollState}
          >
            {content.split('\n').map((line: string, index: number) => (
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

