import React, { useEffect, useMemo, useRef, useState } from 'react';
import { Button } from '@bitfun/ui';
import { useTranslation } from 'react-i18next';
import type { FlowChatState, FlowItem, FlowTextItem, FlowThinkingItem, FlowToolItem } from '../../types/flow-chat';
import { FlowTextBlock } from '../FlowTextBlock';
import { ModelThinkingDisplay } from '../../tool-cards/ModelThinkingDisplay';
import { FlowToolCard } from '../FlowToolCard';
import { taskCollapseStateManager } from '../../store/TaskCollapseStateManager';
import { SmoothHeightCollapse } from '../modern/SmoothHeightCollapse';
import { FLOWCHAT_COLLAPSE_DURATION_MS } from '../modern/flowChatCollapseMotion';
import { FlowChatStore } from '../../store/FlowChatStore';
import { getSubagentProjectionState } from '../../utils/subagentProjection';
import { ensureBtwSessionAvailable } from '../../services/btwSessionPane';
import { RuntimeStatusSlot } from '../modern/RuntimeStatusSlot';
import { useRuntimeStatusStore } from '../../store/runtimeStatusStore';
import './SubagentProjectionView.scss';

interface SubagentProjectionViewProps {
  parentTaskToolId: string;
  parentToolIds?: Set<string>;
  parentSessionId?: string;
  directSubagentSessionId?: string;
  directSubagentDialogTurnId?: string;
  subagentSessionId?: string;
  items?: FlowItem[];
  turnId?: string;
  sessionId?: string;
  className?: string;
  compactText?: boolean;
  liveItemsMode?: 'full-turn' | 'last-round';
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
    <div data-bf-component="subagent-projection" data-bf-part="truncated" className="subagent-projection-text--truncated">
      <FlowTextBlock
        textItem={truncatedItem}
        className={className}
        replayStreamingOnMount={false}
      />
      <div data-bf-component="subagent-projection" data-bf-part="hint" className="subagent-projection-text__hint">
        <span data-bf-component="subagent-projection" data-bf-part="message" className="subagent-projection-text__message">
          {t('subagent.showingLines', { shown: SUBAGENT_TEXT_TRUNCATE_LINES, total: lines.length })}
        </span>
        <Button
          type="button"
          variant="outline"
          size="sm"
          className="subagent-projection-text__expand-btn"
          onClick={() => setIsExpanded(true)}
        >
          {t('subagent.showAll')}
        </Button>
      </div>
    </div>
  );
});

function renderProjectedItem(
  item: FlowItem,
  sessionId: string | undefined,
  turnId: string | undefined,
  compactText: boolean,
  isLastVisibleItem: boolean,
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
          isLastItem={isLastVisibleItem}
          displayContext="subagent-projection"
        />
      );
    case 'tool':
      return (
        <div data-bf-component="subagent-projection" data-bf-part="item" key={item.id} className="flowchat-flow-item" data-flow-item-id={item.id} data-flow-item-type="tool">
          <FlowToolCard
            toolItem={item as FlowToolItem}
            sessionId={sessionId}
            turnId={turnId}
            displayContext="subagent-projection"
            isLastItem={isLastVisibleItem}
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
  directSubagentDialogTurnId,
  subagentSessionId,
  items: itemsProp,
  turnId,
  sessionId,
  className = '',
  compactText = true,
  liveItemsMode = 'last-round',
}) => {
  const containerRef = useRef<HTMLDivElement>(null);
  const userScrolledUpRef = useRef(false);
  const lastScrollTopRef = useRef(0);
  const [isCollapsed, setIsCollapsed] = useState(() =>
    taskCollapseStateManager.isCollapsed(parentTaskToolId)
  );
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
        directSubagentDialogTurnId,
      },
      { itemsMode: liveItemsMode },
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
          directSubagentDialogTurnId,
        },
        { itemsMode: liveItemsMode },
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
  }, [directSubagentDialogTurnId, directSubagentSessionId, liveItemsMode, parentSessionId, parentToolIds]);

  const liveItems = useMemo(
    () => itemsProp ?? projectionState?.items ?? [],
    [itemsProp, projectionState]
  );
  const resolvedSubagentSessionId = subagentSessionId
    ?? projectionState?.session?.sessionId
    ?? directSubagentSessionId;
  const items = liveItems;
  const runtimeStatus = useRuntimeStatusStore(state => (
    resolvedSubagentSessionId
      ? state.bySessionId.get(resolvedSubagentSessionId)
      : undefined
  ));

  useEffect(() => {
    if (!resolvedSubagentSessionId || itemsProp !== undefined) {
      return;
    }

    const flowChatStore = FlowChatStore.getInstance();
    const state = flowChatStore.getState();
    const session = state.sessions.get(resolvedSubagentSessionId);
    const ownerSessionId = parentSessionId ?? sessionId;

    const shouldEnsureSession =
      !session ||
      (
        session.isHistorical &&
        (session.historyState === 'metadata-only' || session.historyState === 'failed')
      );

    if (!shouldEnsureSession) {
      return;
    }

    if (!ownerSessionId) {
      return;
    }

    ensureBtwSessionAvailable({
      childSessionId: resolvedSubagentSessionId,
      parentSessionId: ownerSessionId,
      workspacePath: state.sessions.get(ownerSessionId)?.workspacePath,
      sessionKind: 'subagent',
      parentToolCallId: parentToolIds?.values().next().value,
      remoteConnectionId: state.sessions.get(ownerSessionId)?.remoteConnectionId,
      remoteSshHost: state.sessions.get(ownerSessionId)?.remoteSshHost,
      includeInternal: true,
    });
  }, [items.length, itemsProp, parentSessionId, parentToolIds, resolvedSubagentSessionId, sessionId]);

  // Tail position, not active status, controls live completion retention.
  // Otherwise a newer settled action can collapse while an older item still
  // carries a stale active status.
  const lastVisibleItemId = items[items.length - 1]?.id;

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
    (items.length > 0 || projectionState?.isRunning === true || Boolean(runtimeStatus));

  if (!shouldRenderProjection) {
    return null;
  }

  return (
    <div data-bf-component="subagent-projection" data-bf-part="root" data-bf-state={isCollapsed ? 'collapsed' : 'expanded'}
      className={`subagent-projection-wrapper ${isCollapsed ? 'subagent-projection-wrapper--collapsed' : 'subagent-projection-wrapper--expanded'} ${className}`.trim()}
      data-subagent-session-id={resolvedSubagentSessionId}
    >
      <SmoothHeightCollapse
        isOpen={!isCollapsed}
        className="subagent-projection-collapse"
        durationMs={FLOWCHAT_COLLAPSE_DURATION_MS}
       data-bf-component="subagent-projection" data-bf-part="collapse">
        <div
          ref={containerRef}
          data-bf-component="subagent-projection"
          data-bf-part="container"
          className={`subagent-projection-container ${isCollapsed ? 'subagent-projection-container--collapsed' : 'subagent-projection-container--expanded'}`}
          data-parent-tool-id={parentTaskToolId}
        >
          <div data-bf-component="subagent-projection" data-bf-part="content" className="subagent-projection-content">
            {items.map(item => renderProjectedItem(
              item,
              sessionId ?? resolvedSubagentSessionId,
              turnId,
              compactText,
              item.id === lastVisibleItemId,
            ))}
            <RuntimeStatusSlot sessionId={resolvedSubagentSessionId} />
          </div>
        </div>
      </SmoothHeightCollapse>
    </div>
  );
};

export default SubagentProjectionView;
