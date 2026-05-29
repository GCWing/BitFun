/**
 * Streaming text block component.
 * Applies a typewriter effect during streaming to smooth out
 * the batched content updates from EventBatcher (~100ms).
 * Supports a streaming cursor indicator.
 */

import React, { useState, useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { MarkdownRenderer } from '@/component-library';
import { DotMatrixLoader } from '@/component-library';
import type { FlowTextItem } from '../types/flow-chat';
import { useFlowChatContext } from './modern/FlowChatContext';
import { useTypewriter } from '../hooks/useTypewriter';
import './FlowTextBlock.scss';

// Idle timeout (ms) after content stops growing.
const CONTENT_IDLE_TIMEOUT = 500;

interface FlowTextBlockProps {
  textItem: FlowTextItem;
  className?: string;
  replayStreamingOnMount?: boolean;
}

const RuntimeStatusBlock: React.FC<Pick<FlowTextBlockProps, 'textItem' | 'className'>> = ({ textItem, className = '' }) => {
  const { t } = useTranslation('flow-chat/processing-hints');
  const rawHints = t('items', { returnObjects: true });
  const hints = Array.isArray(rawHints)
    ? rawHints.filter((item): item is string => typeof item === 'string')
    : [];
  const hintIndex = hints.length > 0
    ? Math.abs(textItem.id.split('').reduce((acc, ch) => acc + ch.charCodeAt(0), 0)) % hints.length
    : 0;
  const hint = hints[hintIndex] ?? '';

  return (
    <div className={`flow-text-block flow-text-block--runtime-status ${className}`}>
      <DotMatrixLoader size="medium" className="flow-text-block__runtime-status-icon" />
      {hint && <span className="flow-text-block__runtime-status-text">{hint}</span>}
    </div>
  );
};

/**
 * Use React.memo to avoid unnecessary re-renders.
 * Re-render only when key textItem fields change.
 */
export const FlowTextBlock = React.memo<FlowTextBlockProps>(({
  textItem,
  className = '',
  replayStreamingOnMount = true
}) => {
  const { onFileViewRequest, onTabOpen, onHttpLinkClick, onOpenVisualization } = useFlowChatContext();

  // Normalize content to a string.
  const content = typeof textItem.content === 'string'
    ? textItem.content
    : String(textItem.content || '');

  const isStreaming = textItem.isStreaming &&
    (textItem.status === 'streaming' || textItem.status === 'running');
  const displayContent = useTypewriter(content, isStreaming, {
    replayOnMount: replayStreamingOnMount,
  });
  
  // Heuristic: if content does not change for a while, streaming is done.
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
    if (textItem.status === 'completed' || !textItem.isStreaming) {
      setIsContentGrowing(false);
    }
  }, [textItem.status, textItem.isStreaming]);
  
  const isActivelyStreaming = textItem.isStreaming &&
    (textItem.status === 'streaming' || textItem.status === 'running') &&
    isContentGrowing;

  if (textItem.runtimeStatus) {
    return <RuntimeStatusBlock textItem={textItem} className={className} />;
  }

  return (
    <div className={`flow-text-block ${className} ${isActivelyStreaming ? 'streaming flow-text-block--streaming' : ''}`}>
      {textItem.isMarkdown ? (
        <MarkdownRenderer
          content={displayContent}
          // Pass the raw streaming flag (not the idle-gated
          // `isActivelyStreaming`) so the code-block render path inside
          // Markdown stays stable across bursty AI output. Otherwise
          // `isContentGrowing` toggles every >500ms idle and forces the
          // fallback <pre> / Prism highlighter to swap back and forth,
          // which makes line numbers and the code body visibly shake
          // until the stream finally completes.
          isStreaming={isStreaming}
          onFileViewRequest={onFileViewRequest}
          onTabOpen={onTabOpen}
          onHttpLinkClick={onHttpLinkClick}
          onOpenVisualization={(visualization) => {
            onOpenVisualization?.(visualization?.type, visualization?.data);
          }}
        />
      ) : (
        <div className="text-content">
          {displayContent}
        </div>
      )}
    </div>
  );
}, (prevProps, nextProps) => {
  const prev = prevProps.textItem;
  const next = nextProps.textItem;
  return (
    prev.id === next.id &&
    prev.content === next.content &&
    prev.isStreaming === next.isStreaming &&
    prev.status === next.status &&
    prevProps.className === nextProps.className &&
    prevProps.replayStreamingOnMount === nextProps.replayStreamingOnMount
  );
});
