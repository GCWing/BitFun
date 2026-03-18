/**
 * Streaming text block component.
 * Applies a typewriter effect during streaming to smooth out
 * the batched content updates from EventBatcher (~100ms).
 * Supports a streaming cursor indicator.
 */

import React, { useState, useEffect, useRef } from 'react';
import { MarkdownRenderer } from '@/component-library';
import type { FlowTextItem } from '../types/flow-chat';
import { useFlowChatContext } from './modern/FlowChatContext';
import { useTypewriter } from '../hooks/useTypewriter';
import './FlowTextBlock.scss';

// Idle timeout (ms) after content stops growing.
const CONTENT_IDLE_TIMEOUT = 500;

interface FlowTextBlockProps {
  textItem: FlowTextItem;
  className?: string;
}

/**
 * Use React.memo to avoid unnecessary re-renders.
 * Re-render only when key textItem fields change.
 */
export const FlowTextBlock = React.memo<FlowTextBlockProps>(({
  textItem,
  className = ''
}) => {
  const { onFileViewRequest, onTabOpen, onOpenVisualization } = useFlowChatContext();

  // Normalize content to a string.
  const content = typeof textItem.content === 'string'
    ? textItem.content
    : String(textItem.content || '');

  const isStreaming = textItem.isStreaming &&
    (textItem.status === 'streaming' || textItem.status === 'running');
  const displayContent = useTypewriter(content, isStreaming);
  
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
  const hasContent = content.length > 0;

  return (
    <div className={`flow-text-block ${className} ${isActivelyStreaming ? 'streaming' : ''}`}>
      {textItem.isMarkdown ? (
        <MarkdownRenderer
          content={displayContent}
          isStreaming={isActivelyStreaming}
          onFileViewRequest={onFileViewRequest}
          onTabOpen={onTabOpen}
          onOpenVisualization={(visualization) => {
            onOpenVisualization?.(visualization?.type, visualization?.data);
          }}
        />
      ) : (
        <div className={`text-content ${isActivelyStreaming && hasContent ? 'text-content--streaming' : ''}`}>
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
    prevProps.className === nextProps.className
  );
});
