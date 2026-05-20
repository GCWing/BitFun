import React, { useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { FlowChatState, FlowItem, FlowTextItem, FlowThinkingItem, FlowToolItem } from '../../types/flow-chat';
import { FlowTextBlock } from '../FlowTextBlock';
import { ModelThinkingDisplay } from '../../tool-cards/ModelThinkingDisplay';
import { FlowToolCard } from '../FlowToolCard';
import { taskCollapseStateManager } from '../../store/TaskCollapseStateManager';
import { SmoothHeightCollapse } from '../modern/SmoothHeightCollapse';
import { FlowChatStore } from '../../store/FlowChatStore';
import { getSubagentProjectionState } from '../../utils/subagentProjection';
import './SubagentProjectionView.scss';

interface SubagentProjectionViewProps {
  parentTaskToolId: string;
  parentToolIds?: Set<string>;
  parentSessionId?: string;
  directSubagentSessionId?: string;
  subagentSessionId?: string;
  items?: FlowItem[];
  isRunning?: boolean;
  turnId?: string;
  sessionId?: string;
  className?: string;
  compactText?: boolean;
}

const SUBAGENT_TEXT_TRUNCATE_LINES = 50;

const SubagentProjectionTextBlock = React.memo<{ textItem: FlowTextItem; className?: string }>(({ textItem, className = '' }) => {
  const [isExpanded, setIsExpanded] = useState(false);
  const { t } = useTranslation('flow-chat');

  const content = typeof textItem.content === 'string'
    ? textItem.content
    : String(textItem.content || '');

  const isStreaming = textItem.isStreaming &&
    (textItem.status === 'streaming' || textItem.status === 'running');

  const lines = content.split('\n');
  const shouldTruncate = !isStreaming && !isExpanded && lines.length > SUBAGENT_TEXT_TRUNCATE_LINES;

  if (!shouldTruncate) {
    return (
      <FlowTextBlock
        textItem={textItem}
        className={className}
        replayStreamingOnMount={false}
      />
    );
  }

  const truncatedItem: FlowTextItem = {
    ...textItem,
    content: lines.slice(0, SUBAGENT_TEXT_TRUNCATE_LINES).join('\n'),
    isStreaming: false,
  };

  return (
    <div className="subagent-projection-text--truncated">
      <FlowTextBlock
        textItem={truncatedItem}
        className={className}
        replayStreamingOnMount={false}
      />
      <div className="subagent-projection-text__hint">
        <span className="subagent-projection-text__message">
          {t('subagent.showingLines', { shown: SUBAGENT_TEXT_TRUNCATE_LINES, total: lines.length })}
        </span>
        <button
          type="button"
          className="subagent-projection-text__expand-btn"
          onClick={() => setIsExpanded(true)}
        >
          {t('subagent.showAll')}
        </button>
      </div>
    </div>
  );
});

function renderProjectedItem(
  item: FlowItem,
  sessionId: string | undefined,
  turnId: string | undefined,
  compactText: boolean,
  isLastActiveItem: boolean,
): React.ReactNode {
  switch (item.type) {
    case 'text':
      return (
        <SubagentProjectionTextBlock
          key={item.id}
          textItem={item as FlowTextItem}
          className={compactText ? 'flow-text-block--subagent-compact' : ''}
        />
      );
    case 'thinking':
      return (
        <ModelThinkingDisplay
          key={item.id}
          thinkingItem={item as FlowThinkingItem}
          isLastItem={isLastActiveItem}
          displayContext="subagent-projection"
        />
      );
    case 'tool':
      return (
        <div key={item.id} className="flowchat-flow-item" data-flow-item-id={item.id} data-flow-item-type="tool">
          <FlowToolCard
            toolItem={item as FlowToolItem}
            sessionId={sessionId}
            turnId={turnId}
            displayContext="subagent-projection"
          />
        </div>
      );
    default:
      return null;
  }
}

export const SubagentProjectionView: React.FC<SubagentProjectionViewProps> = ({
  parentTaskToolId,
  parentToolIds,
  parentSessionId,
  directSubagentSessionId,
  subagentSessionId,
  items: itemsProp,
  isRunning: isRunningProp,
  turnId,
  sessionId,
  className = '',
  compactText = true,
}) => {
  const containerRef = useRef<HTMLDivElement>(null);
  const contentRef = useRef<HTMLDivElement>(null);
  const userScrolledUpRef = useRef(false);
  const lastScrollTopRef = useRef(0);
  const previousRoundIdRef = useRef<string | null>(null);
  const measuredHeightRef = useRef(0);
  const retainedItemsRef = useRef<FlowItem[]>([]);
  const floorOwnerRoundIdRef = useRef<string | null>(null);
  const [isCollapsed, setIsCollapsed] = useState(() =>
    taskCollapseStateManager.isCollapsed(parentTaskToolId)
  );
  const [heightFloorPx, setHeightFloorPx] = useState(0);
  const [projectionState, setProjectionState] = useState(() => {
    if (!parentToolIds || parentToolIds.size === 0) {
      return null;
    }

    return getSubagentProjectionState(
      FlowChatStore.getInstance().getState(),
      {
        parentSessionId,
        parentToolIds,
        directSubagentSessionId,
      },
      { itemsMode: 'last-round' },
    );
  });

  useEffect(() => {
    setIsCollapsed(taskCollapseStateManager.isCollapsed(parentTaskToolId));

    const unsubscribe = taskCollapseStateManager.addListener((toolId, collapsed) => {
      if (toolId === parentTaskToolId) {
        setIsCollapsed(collapsed);
      }
    });

    return unsubscribe;
  }, [parentTaskToolId]);

  useEffect(() => {
    if (!parentToolIds || parentToolIds.size === 0) {
      setProjectionState(null);
      return;
    }

    const flowChatStore = FlowChatStore.getInstance();

    const readProjectionState = (state: FlowChatState) => {
      return getSubagentProjectionState(
        state,
        {
          parentSessionId,
          parentToolIds,
          directSubagentSessionId,
        },
        { itemsMode: 'last-round' },
      );
    };

    let previous = readProjectionState(flowChatStore.getState());
    setProjectionState(previous);

    const unsubscribe = flowChatStore.subscribe((state) => {
      const next = readProjectionState(state);
      if (
        previous?.session === next.session &&
        previous?.turn === next.turn &&
        previous?.round === next.round &&
        previous?.items === next.items &&
        previous?.isRunning === next.isRunning
      ) {
        return;
      }
      previous = next;
      setProjectionState(next);
    });

    return unsubscribe;
  }, [directSubagentSessionId, parentSessionId, parentToolIds]);

  const liveItems = useMemo(
    () => itemsProp ?? projectionState?.items ?? [],
    [itemsProp, projectionState]
  );
  const projectedRoundId = projectionState?.round?.id ?? null;
  const isRunning = isRunningProp ?? projectionState?.isRunning ?? false;
  const resolvedSubagentSessionId = subagentSessionId
    ?? projectionState?.session?.sessionId
    ?? directSubagentSessionId;
  const pendingRoundSwitchBridgePx = (() => {
    const previousRoundId = previousRoundIdRef.current;
    const measuredHeight = Math.ceil(measuredHeightRef.current);

    if (
      !isRunning ||
      liveItems.length > 0 ||
      retainedItemsRef.current.length === 0 ||
      !previousRoundId ||
      !projectedRoundId ||
      previousRoundId === projectedRoundId ||
      measuredHeight <= 0
    ) {
      return 0;
    }

    return Math.max(heightFloorPx, measuredHeight);
  })();
  const effectiveHeightFloorPx = Math.max(heightFloorPx, pendingRoundSwitchBridgePx);

  useLayoutEffect(() => {
    const currentRoundId = projectedRoundId;
    const previousRoundId = previousRoundIdRef.current;

    if (
      isRunning &&
      previousRoundId &&
      currentRoundId &&
      previousRoundId !== currentRoundId &&
      measuredHeightRef.current > 0
    ) {
      floorOwnerRoundIdRef.current = currentRoundId;
      setHeightFloorPx(previous => Math.max(previous, Math.ceil(measuredHeightRef.current)));
    }

    if (!isRunning) {
      floorOwnerRoundIdRef.current = null;
      setHeightFloorPx(0);
    }

    previousRoundIdRef.current = currentRoundId;
  }, [heightFloorPx, isRunning, liveItems.length, parentTaskToolId, projectedRoundId]);

  const items = useMemo(() => {
    if (liveItems.length > 0) {
      retainedItemsRef.current = liveItems;
      return liveItems;
    }

    if (isRunning && effectiveHeightFloorPx > 0 && retainedItemsRef.current.length > 0) {
      return retainedItemsRef.current;
    }

    if (!isRunning) {
      retainedItemsRef.current = [];
    }

    return liveItems;
  }, [effectiveHeightFloorPx, isRunning, liveItems]);
  const lastActiveItemId = useMemo(() => {
    for (let index = items.length - 1; index >= 0; index -= 1) {
      const item = items[index];
      if (item.status !== 'completed' && item.status !== 'cancelled' && item.status !== 'error') {
        return item.id;
      }
      if (item.type === 'thinking' && (item as FlowThinkingItem).isStreaming) {
        return item.id;
      }
      if (item.type === 'text' && (item as FlowTextItem).isStreaming) {
        return item.id;
      }
    }

    return items.length > 0 ? items[items.length - 1]?.id ?? null : null;
  }, [items]);

  useEffect(() => {
    const content = contentRef.current;
    if (!content) {
      return;
    }

    const updateMeasuredHeight = () => {
      const nextHeight = Math.ceil(content.getBoundingClientRect().height);
      measuredHeightRef.current = nextHeight;
      const currentFloorOwnerRoundId = floorOwnerRoundIdRef.current;
      const isShowingLiveItems = liveItems.length > 0;
      const canReleaseHeightFloor =
        heightFloorPx > 0 &&
        isShowingLiveItems &&
        currentFloorOwnerRoundId != null &&
        projectedRoundId === currentFloorOwnerRoundId &&
        nextHeight >= heightFloorPx - 12;

      if (canReleaseHeightFloor) {
        floorOwnerRoundIdRef.current = null;
        setHeightFloorPx(0);
      }
    };

    updateMeasuredHeight();

    if (typeof ResizeObserver === 'undefined') {
      const rafId = requestAnimationFrame(updateMeasuredHeight);
      return () => cancelAnimationFrame(rafId);
    }

    const observer = new ResizeObserver(() => {
      updateMeasuredHeight();
    });
    observer.observe(content);

    return () => observer.disconnect();
  }, [heightFloorPx, isRunning, items, liveItems.length, parentTaskToolId, projectedRoundId]);

  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;

    const handleScroll = () => {
      const currentScrollTop = container.scrollTop;
      const maxScrollTop = container.scrollHeight - container.clientHeight;

      if (currentScrollTop < lastScrollTopRef.current && maxScrollTop > 0) {
        if (lastScrollTopRef.current - currentScrollTop > 20) {
          userScrolledUpRef.current = true;
        }
      }

      if (maxScrollTop > 0 && maxScrollTop - currentScrollTop < 30) {
        userScrolledUpRef.current = false;
      }

      lastScrollTopRef.current = currentScrollTop;
    };

    container.addEventListener('scroll', handleScroll, { passive: true });
    return () => container.removeEventListener('scroll', handleScroll);
  }, [isCollapsed]);

  const scrollSignal = useMemo(() => {
    return items.map((item) => {
      const itemAny = item as any;
      const contentLength = typeof itemAny.content === 'string' ? itemAny.content.length : 0;
      const paramsLength = itemAny.partialParams ? JSON.stringify(itemAny.partialParams).length : 0;
      return `${item.id}:${item.status}:${contentLength}:${paramsLength}`;
    }).join('|');
  }, [items]);

  useEffect(() => {
    const container = containerRef.current;
    if (!container || isCollapsed) return;

    const rafId = requestAnimationFrame(() => {
      requestAnimationFrame(() => {
        if (!userScrolledUpRef.current) {
          container.scrollTop = container.scrollHeight;
          lastScrollTopRef.current = container.scrollTop;
        }
      });
    });

    return () => cancelAnimationFrame(rafId);
  }, [isCollapsed, scrollSignal]);

  const shouldRenderProjection =
    Boolean(resolvedSubagentSessionId) &&
    (items.length > 0 || (isRunning && effectiveHeightFloorPx > 0));

  if (!shouldRenderProjection) {
    return null;
  }

  return (
    <div
      className={`subagent-projection-wrapper ${isCollapsed ? 'subagent-projection-wrapper--collapsed' : 'subagent-projection-wrapper--expanded'} ${className}`.trim()}
      data-subagent-session-id={resolvedSubagentSessionId}
    >
      <SmoothHeightCollapse isOpen={!isCollapsed} className="subagent-projection-collapse">
        <div
          ref={containerRef}
          className={`subagent-projection-container ${isCollapsed ? 'subagent-projection-container--collapsed' : 'subagent-projection-container--expanded'}`}
          data-parent-tool-id={parentTaskToolId}
          style={effectiveHeightFloorPx > 0 ? { minHeight: `${effectiveHeightFloorPx}px` } : undefined}
        >
          <div ref={contentRef} className="subagent-projection-content">
            {items.map(item => renderProjectedItem(
              item,
              sessionId ?? resolvedSubagentSessionId,
              turnId,
              compactText,
              item.id === lastActiveItemId,
            ))}
          </div>
        </div>
      </SmoothHeightCollapse>
    </div>
  );
};

export default SubagentProjectionView;
