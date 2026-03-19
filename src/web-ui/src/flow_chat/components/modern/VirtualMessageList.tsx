/**
 * Virtualized message list.
 * Renders a flattened DialogTurn stream (user messages + model rounds).
 *
 * Scroll policy (simplified):
 * - While the agent is processing → always auto-scroll to bottom (smooth).
 * - While idle → user scrolls freely; no auto-scroll interference.
 * - "Scroll to latest" bar appears when not at bottom AND not processing.
 */

import React, { useRef, useState, useCallback, useEffect, forwardRef, useImperativeHandle } from 'react';
import { Virtuoso, VirtuosoHandle, ListRange } from 'react-virtuoso';
import { useActiveSessionState } from '../../hooks/useActiveSessionState';
import { VirtualItemRenderer } from './VirtualItemRenderer';
import { ScrollToLatestBar } from '../ScrollToLatestBar';
import { ProcessingIndicator } from './ProcessingIndicator';
import { ScrollAnchor } from './ScrollAnchor';
import { useVirtualItems, useActiveSession, useModernFlowChatStore } from '../../store/modernFlowChatStore';
import { useChatInputState } from '../../store/chatInputStateStore';
import './VirtualMessageList.scss';

/**
 * Methods exposed by VirtualMessageList.
 */
export interface VirtualMessageListRef {
  scrollToTurn: (turnIndex: number) => void;
  scrollToIndex: (index: number) => void;
  scrollToBottom: () => void;
}

export const VirtualMessageList = forwardRef<VirtualMessageListRef>((_, ref) => {
  const virtuosoRef = useRef<VirtuosoHandle>(null);
  const virtualItems = useVirtualItems();
  const activeSession = useActiveSession();

  const [isAtBottom, setIsAtBottom] = useState(true);

  const scrollerElementRef = useRef<HTMLElement | null>(null);

  const isInputActive = useChatInputState(state => state.isActive);
  const isInputExpanded = useChatInputState(state => state.isExpanded);

  const activeSessionState = useActiveSessionState();
  const isProcessing = activeSessionState.isProcessing;
  const processingPhase = activeSessionState.processingPhase;

  const handleScrollerRef = useCallback((el: HTMLElement | Window | null) => {
    if (el && el instanceof HTMLElement) {
      scrollerElementRef.current = el;
    }
  }, []);

  // ── User-message index map (for turn navigation & range reporting) ───
  const userMessageItems = React.useMemo(() => {
    return virtualItems
      .map((item, index) => ({ item, index }))
      .filter(({ item }) => item.type === 'user-message');
  }, [virtualItems]);

  // ── Visible turn info (range-changed callback) ───────────────────────
  const handleRangeChanged = useCallback((range: ListRange) => {
    const setVisibleTurnInfo = useModernFlowChatStore.getState().setVisibleTurnInfo;

    if (userMessageItems.length === 0) {
      setVisibleTurnInfo(null);
      return;
    }

    const visibleUserMessage = userMessageItems.find(({ index }) =>
      index >= range.startIndex && index <= range.endIndex
    );
    const targetMessage = visibleUserMessage ||
      [...userMessageItems].reverse().find(({ index }) => index < range.startIndex);

    if (targetMessage) {
      const turnIndex = userMessageItems.indexOf(targetMessage) + 1;
      const userMessage = targetMessage.item.type === 'user-message'
        ? targetMessage.item.data
        : null;

      setVisibleTurnInfo({
        turnIndex,
        totalTurns: userMessageItems.length,
        userMessage: userMessage?.content || '',
        turnId: targetMessage.item.turnId,
      });
    }
  }, [userMessageItems]);

  useEffect(() => {
    const setVisibleTurnInfo = useModernFlowChatStore.getState().setVisibleTurnInfo;

    if (userMessageItems.length > 0) {
      const firstMessage = userMessageItems[0];
      const userMessage = firstMessage.item.type === 'user-message'
        ? firstMessage.item.data
        : null;

      setVisibleTurnInfo({
        turnIndex: 1,
        totalTurns: userMessageItems.length,
        userMessage: userMessage?.content || '',
        turnId: firstMessage.item.turnId,
      });
    } else {
      setVisibleTurnInfo(null);
    }
  }, [userMessageItems.length]);

  // ── Navigation helpers ────────────────────────────────────────────────
  const scrollToTurn = useCallback((turnIndex: number) => {
    if (!virtuosoRef.current) return;
    if (turnIndex < 1 || turnIndex > userMessageItems.length) return;

    const targetItem = userMessageItems[turnIndex - 1];
    if (!targetItem) return;

    if (targetItem.index === 0) {
      virtuosoRef.current.scrollTo({ top: 0, behavior: 'smooth' });
    } else {
      virtuosoRef.current.scrollToIndex({
        index: targetItem.index,
        behavior: 'smooth',
        align: 'center',
      });
    }
  }, [userMessageItems]);

  const scrollToIndex = useCallback((index: number) => {
    if (!virtuosoRef.current) return;
    if (index < 0 || index >= virtualItems.length) return;

    if (index === 0) {
      virtuosoRef.current.scrollTo({ top: 0, behavior: 'auto' });
    } else {
      virtuosoRef.current.scrollToIndex({ index, align: 'center', behavior: 'auto' });
    }
  }, [virtualItems.length]);

  const scrollToBottom = useCallback(() => {
    if (virtuosoRef.current && virtualItems.length > 0) {
      virtuosoRef.current.scrollTo({ top: 999999999, behavior: 'smooth' });
    }
  }, [virtualItems.length]);

  useImperativeHandle(ref, () => ({
    scrollToTurn,
    scrollToIndex,
    scrollToBottom,
  }), [scrollToTurn, scrollToIndex, scrollToBottom]);

  // ── Initial scroll to bottom when processing starts ──────────────────
  // Note: followOutput handles continuous auto-scroll, so we only need
  // an initial scroll here. The 300ms interval was removed because it
  // conflicted with followOutput and caused visual jitter.
  useEffect(() => {
    if (!isProcessing) return;

    if (virtuosoRef.current) {
      virtuosoRef.current.scrollTo({ top: 999999999, behavior: 'auto' });
    }
  }, [isProcessing]);

  const handleFollowOutput = useCallback(() => {
    return isProcessing ? 'smooth' as const : false;
  }, [isProcessing]);

  const handleAtBottomStateChange = useCallback((atBottom: boolean) => {
    setIsAtBottom(atBottom);
  }, []);

  // ── Last-item info for breathing indicator ────────────────────────────
  const lastItemInfo = React.useMemo(() => {
    const dialogTurns = activeSession?.dialogTurns;
    const lastDialogTurn = dialogTurns && dialogTurns.length > 0
      ? dialogTurns[dialogTurns.length - 1]
      : undefined;
    const modelRounds = lastDialogTurn?.modelRounds;
    const lastModelRound = modelRounds && modelRounds.length > 0
      ? modelRounds[modelRounds.length - 1]
      : undefined;
    const items = lastModelRound?.items;
    const lastItem = items && items.length > 0
      ? items[items.length - 1]
      : undefined;

    const content = lastItem && 'content' in lastItem ? (lastItem as any).content : '';
    const isTurnProcessing = lastDialogTurn?.status === 'processing' ||
                              lastDialogTurn?.status === 'image_analyzing';

    return { lastItem, lastDialogTurn, content, isTurnProcessing };
  }, [activeSession]);

  const [isContentGrowing, setIsContentGrowing] = useState(true);
  const lastContentRef = useRef(lastItemInfo.content);
  const contentTimeoutRef = useRef<NodeJS.Timeout | null>(null);

  useEffect(() => {
    const currentContent = lastItemInfo.content;

    if (currentContent !== lastContentRef.current) {
      lastContentRef.current = currentContent;
      setIsContentGrowing(true);

      if (contentTimeoutRef.current) {
        clearTimeout(contentTimeoutRef.current);
      }

      contentTimeoutRef.current = setTimeout(() => {
        setIsContentGrowing(false);
      }, 500);
    }

    return () => {
      if (contentTimeoutRef.current) {
        clearTimeout(contentTimeoutRef.current);
      }
    };
  }, [lastItemInfo.content]);

  useEffect(() => {
    if (!lastItemInfo.isTurnProcessing && !isProcessing) {
      setIsContentGrowing(false);
    }
  }, [lastItemInfo.isTurnProcessing, isProcessing]);

  const showBreathingIndicator = React.useMemo(() => {
    const { lastItem, isTurnProcessing } = lastItemInfo;

    if (!isTurnProcessing && !isProcessing) return false;
    if (processingPhase === 'tool_confirming') return false;
    if (!lastItem) return true;

    if ((lastItem.type === 'text' || lastItem.type === 'thinking')) {
      const hasContent = 'content' in lastItem && lastItem.content;
      if (hasContent && isContentGrowing) return false;
    }

    if (lastItem.type === 'tool') {
      const toolStatus = lastItem.status;
      if (toolStatus === 'running' || toolStatus === 'streaming' || toolStatus === 'preparing') {
        return false;
      }
    }

    return isTurnProcessing || isProcessing;
  }, [isProcessing, processingPhase, lastItemInfo, isContentGrowing]);

  const reserveSpaceForIndicator = React.useMemo(() => {
    if (!lastItemInfo.isTurnProcessing && !isProcessing) return false;
    if (processingPhase === 'tool_confirming') return false;
    return true;
  }, [lastItemInfo.isTurnProcessing, isProcessing, processingPhase]);

  // ── Render ────────────────────────────────────────────────────────────
  if (virtualItems.length === 0) {
    return (
      <div className="virtual-message-list virtual-message-list--empty">
        <div className="empty-state">
          <p>No messages yet</p>
        </div>
      </div>
    );
  }

  return (
    <div className="virtual-message-list">
      <Virtuoso
        ref={virtuosoRef}
        data={virtualItems}
        computeItemKey={(index, item) =>
          `${item.type}-${item.turnId}-${'data' in item && item.data && typeof item.data === 'object' && 'id' in item.data ? item.data.id : index}`
        }
        itemContent={(index, item) => (
          <VirtualItemRenderer
            item={item}
            index={index}
          />
        )}
        followOutput={handleFollowOutput}

        alignToBottom={false}
        initialTopMostItemIndex={0}

        overscan={{ main: 1200, reverse: 1200 }}

        atBottomThreshold={50}
        atBottomStateChange={handleAtBottomStateChange}

        rangeChanged={handleRangeChanged}

        defaultItemHeight={200}

        increaseViewportBy={{ top: 1200, bottom: 1200 }}

        scrollerRef={handleScrollerRef}

        components={{
          Header: () => <div className="message-list-header" />,
          Footer: () => (
            <>
              <ProcessingIndicator visible={showBreathingIndicator} reserveSpace={reserveSpaceForIndicator} />
              <div className="message-list-footer" />
            </>
          ),
        }}
      />

      <ScrollAnchor
        virtuosoRef={virtuosoRef}
        scrollerRef={scrollerElementRef}
      />

      <ScrollToLatestBar
        visible={!isAtBottom && !isProcessing && virtualItems.length > 0}
        onClick={scrollToBottom}
        isInputActive={isInputActive}
        isInputExpanded={isInputExpanded}
      />
    </div>
  );
});

VirtualMessageList.displayName = 'VirtualMessageList';
