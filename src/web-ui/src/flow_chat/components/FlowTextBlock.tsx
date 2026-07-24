/**
 * Streaming text block component.
 * Applies an adaptive typewriter during streaming to smoothly drain
 * batched EventBatcher text updates. Supports a streaming cursor indicator.
 */

import React, { useState, useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { MarkdownRenderer } from '@/component-library';
import { DotMatrixLoader } from '@/component-library';
import type { MarkdownTraceContext } from '@/component-library';
import type { FlowTextItem } from '../types/flow-chat';
import { useFlowChatContext } from './modern/FlowChatContext';
import { useTypewriter } from '../hooks/useTypewriter';
import { useReportTypewriterReveal } from '../hooks/typewriterRevealGateContext';
import { isStartupRenderTraceEnabled } from '@/shared/utils/startupTrace';
import './FlowTextBlock.scss';

// Idle timeout (ms) after content stops growing.
const CONTENT_IDLE_TIMEOUT = 500;

interface FlowTextBlockProps {
  textItem: FlowTextItem;
  className?: string;
  replayStreamingOnMount?: boolean;
  traceContext?: MarkdownTraceContext;
  testId?: string;
  testAttributes?: Record<`data-${string}`, string | number | boolean | undefined>;
}

const RuntimeStatusBlock: React.FC<Pick<FlowTextBlockProps, 'textItem' | 'className' | 'testId' | 'testAttributes'>> = ({
  textItem,
  className = '',
  testId,
  testAttributes,
}) => {
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
    <div
      className={`flow-text-block flow-text-block--runtime-status ${className}`}
      data-testid={testId}
      {...testAttributes}
    >
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
  replayStreamingOnMount = true,
  traceContext,
  testId,
  testAttributes,
}) => {
  const {
    onFileViewRequest,
    onTabOpen,
    onHttpLinkClick,
    onOpenVisualization,
    activeSessionOverride,
  } = useFlowChatContext();
  const markdownBasePath = activeSessionOverride?.workspacePath
    || activeSessionOverride?.config?.workspacePath;
  const markdownRemoteConnectionId = activeSessionOverride?.remoteConnectionId
    || activeSessionOverride?.config?.remoteConnectionId;

  // Normalize content to a string.
  const content = typeof textItem.content === 'string'
    ? textItem.content
    : String(textItem.content || '');

  const isStreaming = textItem.isStreaming &&
    (textItem.status === 'streaming' || textItem.status === 'running');
  const { displayText: displayContent, isRevealing } = useTypewriter(content, isStreaming, {
    replayOnMount: replayStreamingOnMount,
  });
  useReportTypewriterReveal(textItem.id, isRevealing);
  // Keep streaming render mode until the typewriter finishes draining so the
  // Markdown path does not flash when the model completes early.
  const isVisuallyStreaming = isStreaming || isRevealing;
  // Leave Markdown streaming mode one frame after visual settle so footer /
  // list layout commits first; avoids a same-frame Prism upgrade flash.
  const [markdownStreaming, setMarkdownStreaming] = useState(isVisuallyStreaming);
  useEffect(() => {
    if (isVisuallyStreaming) {
      setMarkdownStreaming(true);
      return;
    }
    let cancelled = false;
    const frameId = requestAnimationFrame(() => {
      requestAnimationFrame(() => {
        if (!cancelled) {
          setMarkdownStreaming(false);
        }
      });
    });
    return () => {
      cancelled = true;
      cancelAnimationFrame(frameId);
    };
  }, [isVisuallyStreaming]);
  
  // Heuristic: if content does not change for a while, streaming is done.
  const [isContentGrowing, setIsContentGrowing] = useState(isStreaming);
  const lastContentRef = useRef(content);
  const timeoutRef = useRef<NodeJS.Timeout | null>(null);
  
  useEffect(() => {
    const clearGrowthTimeout = () => {
      if (timeoutRef.current) {
        clearTimeout(timeoutRef.current);
        timeoutRef.current = null;
      }
    };

    if (!isStreaming) {
      lastContentRef.current = content;
      clearGrowthTimeout();
      setIsContentGrowing((wasGrowing) => (wasGrowing ? false : wasGrowing));
      return clearGrowthTimeout;
    }

    if (content !== lastContentRef.current) {
      lastContentRef.current = content;
      setIsContentGrowing(true);
      clearGrowthTimeout();
      
      timeoutRef.current = setTimeout(() => {
        setIsContentGrowing(false);
      }, CONTENT_IDLE_TIMEOUT);
    }
    
    return clearGrowthTimeout;
  }, [content, isStreaming]);
  
  // Keep streaming chrome while either the model is actively emitting or the
  // typewriter is still revealing leftover characters.
  const isActivelyStreaming = (isStreaming && isContentGrowing) || isRevealing;
  const markdownTraceContext = isStartupRenderTraceEnabled() ? traceContext : undefined;

  if (textItem.runtimeStatus) {
    return (
      <RuntimeStatusBlock
        textItem={textItem}
        className={className}
        testId={testId}
        testAttributes={testAttributes}
      />
    );
  }

  return (
    <div
      className={`flow-text-block ${className} ${isActivelyStreaming ? 'streaming flow-text-block--streaming' : ''}`}
      data-testid={testId}
      data-flow-item-id={textItem.id}
      data-status={textItem.status}
      data-streaming={isVisuallyStreaming ? 'true' : 'false'}
      {...testAttributes}
    >
      {textItem.isMarkdown ? (
        <MarkdownRenderer
          content={displayContent}
          basePath={markdownBasePath}
          remoteConnectionId={markdownRemoteConnectionId}
          // Prefer deferred visual streaming so Prism upgrade does not share a
          // frame with footer insertion / list scroll settlement.
          isStreaming={markdownStreaming}
          onFileViewRequest={onFileViewRequest}
          onTabOpen={onTabOpen}
          onHttpLinkClick={onHttpLinkClick}
          onOpenVisualization={(visualization) => {
            onOpenVisualization?.(visualization?.type, visualization?.data);
          }}
          traceContext={markdownTraceContext}
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
    prevProps.replayStreamingOnMount === nextProps.replayStreamingOnMount &&
    prevProps.traceContext === nextProps.traceContext &&
    prevProps.testId === nextProps.testId &&
    prevProps.testAttributes === nextProps.testAttributes
  );
});
