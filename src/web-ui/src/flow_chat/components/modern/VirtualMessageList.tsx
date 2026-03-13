/**
 * Virtualized message list.
 * Renders a flattened DialogTurn stream (user messages + model rounds).
 * Keeps the conceptual model: Session → DialogTurn → ModelRound → FlowItem.
 *
 * Stick-to-bottom behavior:
 * 1) Default stick-to-bottom; any content change (including collapse) scrolls to bottom.
 * 2) Scrolling up past a threshold exits stick-to-bottom.
 * 3) Returning to bottom or clicking the button re-enables stick-to-bottom.
 * 4) MutationObserver watches height changes to handle collapse/expand.
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

// Scroll-up threshold for entering history view.
const SCROLL_UP_THRESHOLD = 50;

/**
 * Methods exposed by VirtualMessageList.
 */
export interface VirtualMessageListRef {
  /** Scroll to a specific turn (1-based). */
  scrollToTurn: (turnIndex: number) => void;
  /** Scroll to a specific virtual item index (0-based). */
  scrollToIndex: (index: number) => void;
  /** Scroll to bottom. */
  scrollToBottom: () => void;
}

export const VirtualMessageList = forwardRef<VirtualMessageListRef>((_, ref) => {
  const virtuosoRef = useRef<VirtuosoHandle>(null);
  const virtualItems = useVirtualItems();
  const activeSession = useActiveSession();
  // Stick-to-bottom state: true auto-follows new content and height changes.
  // false means the user is viewing history and auto-scroll is disabled.
  const [stickToBottom, setStickToBottom] = useState(true);
  const stickToBottomRef = useRef(true);
  // When we programmatically navigate (e.g. jump to a marker), Virtuoso can briefly
  // report "at bottom" due to thresholds, which would auto-reenable stick-to-bottom.
  // Suppress that behavior for a short window after navigation.
  const suppressAutoEnableStickToBottomRef = useRef(false);
  const suppressAutoEnableTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  
  // Sync ref with state.
  useEffect(() => {
    stickToBottomRef.current = stickToBottom;
  }, [stickToBottom]);

  useEffect(() => {
    return () => {
      if (suppressAutoEnableTimerRef.current) {
        clearTimeout(suppressAutoEnableTimerRef.current);
      }
    };
  }, []);

  const beginNavigation = useCallback(() => {
    suppressAutoEnableStickToBottomRef.current = true;
    if (suppressAutoEnableTimerRef.current) {
      clearTimeout(suppressAutoEnableTimerRef.current);
    }
    // Long enough to cover the scroll + DOM focus + atBottom recalculation.
    suppressAutoEnableTimerRef.current = setTimeout(() => {
      suppressAutoEnableStickToBottomRef.current = false;
    }, 900);
  }, []);
  
  // Track whether we're at bottom (for button visibility).
  const [isAtBottom, setIsAtBottom] = useState(true);
  
  // Last scroll position to detect direction.
  const lastScrollTopRef = useRef(0);
  
  // Initialization flag to avoid false positives on first render.
  const isInitializedRef = useRef(false);
  
  // Scroller ref for observing height changes.
  const scrollerElementRef = useRef<HTMLElement | null>(null);
  
  // Track user scrolling to prevent auto-scroll during interaction.
  const isUserScrollingRef = useRef(false);
  const userScrollingTimeoutRef = useRef<NodeJS.Timeout | null>(null);
  
  // Previous virtualItems length to detect new tail items and post-layout scrolling.
  const prevLengthRef = useRef(0);
  // Set true during programmatic scroll-to-bottom to avoid mis-detecting user scroll.
  const programmaticScrollToBottomRef = useRef(false);
  // Visible item range (updated by rangeChanged) for layout bookkeeping.
  const lastRangeRef = useRef<ListRange>({ startIndex: 0, endIndex: 0 });
  // When list length increases, delay scroll-to-bottom and suppress followOutput to avoid jitter.
  const suppressFollowOutputRef = useRef(false);
  
  // Delay initialization until layout stabilizes.
  useEffect(() => {
    const timer = setTimeout(() => {
      isInitializedRef.current = true;
    }, 100);
    return () => clearTimeout(timer);
  }, []);
  
  useEffect(() => {
    return () => {
      if (userScrollingTimeoutRef.current) {
        clearTimeout(userScrollingTimeoutRef.current);
      }
    };
  }, []);
  
  // Treat tool card toggle as user interaction to prevent auto-scroll.
  useEffect(() => {
    const handleToolCardToggle = () => {
      isUserScrollingRef.current = true;
      
      if (userScrollingTimeoutRef.current) {
        clearTimeout(userScrollingTimeoutRef.current);
      }
      
      // Clear after 300ms (slightly longer than scroll) to cover animation.
      userScrollingTimeoutRef.current = setTimeout(() => {
        isUserScrollingRef.current = false;
      }, 300);
    };
    
    window.addEventListener('tool-card-toggle', handleToolCardToggle);
    return () => {
      window.removeEventListener('tool-card-toggle', handleToolCardToggle);
    };
  }, []);
  
  // ChatInput expansion state.
  const isInputActive = useChatInputState(state => state.isActive);
  const isInputExpanded = useChatInputState(state => state.isExpanded);

  const handleScrollerRef = useCallback((ref: HTMLElement | Window | null) => {
    if (ref && ref instanceof HTMLElement) {
      scrollerElementRef.current = ref;
    }
  }, []);
  
  // Observe scroller height changes; auto-scroll in stick-to-bottom mode.
  // Performance: throttle MutationObserver callbacks.
  useEffect(() => {
    const scroller = scrollerElementRef.current;
    if (!scroller) return;
    
    let lastScrollHeight = scroller.scrollHeight;
    let rafId: number | null = null;
    let throttleTimer: NodeJS.Timeout | null = null;
    let pendingCheck = false;
    
    // Throttle interval: at most once every 50ms during streaming.
    const THROTTLE_MS = 50;
    
    const checkAndScroll = () => {
      pendingCheck = false;
      
      // Skip auto-scroll while the user is scrolling.
      if (isUserScrollingRef.current) {
        // Update lastScrollHeight without scrolling.
        lastScrollHeight = scroller.scrollHeight;
        return;
      }
      
      // When delaying scroll-to-bottom on new items, skip followOutput here to avoid jitter.
      if (suppressFollowOutputRef.current) {
        lastScrollHeight = scroller.scrollHeight;
        return;
      }
      
      const newScrollHeight = scroller.scrollHeight;
      
      // Height changed.
      if (newScrollHeight !== lastScrollHeight) {
        const heightDecreased = newScrollHeight < lastScrollHeight;
        lastScrollHeight = newScrollHeight;
        
        // In stick-to-bottom mode, scroll to bottom.
        // When height decreases (collapse), scroll immediately.
        if (stickToBottomRef.current && virtuosoRef.current) {
          // Use requestAnimationFrame to avoid animation conflicts.
          if (rafId) cancelAnimationFrame(rafId);
          rafId = requestAnimationFrame(() => {
            if (stickToBottomRef.current && virtuosoRef.current && !isUserScrollingRef.current) {
              virtuosoRef.current.scrollTo({ 
                top: 999999999, 
                behavior: heightDecreased ? 'auto' : 'auto'
              });
            }
          });
        }
      }
    };
    
    // Throttled mutation handler to reduce high-frequency callbacks.
    const throttledCheckAndScroll = () => {
      if (pendingCheck) return; // Pending check already scheduled.
      
      pendingCheck = true;
      
      if (throttleTimer) {
        return;
      }
      
      checkAndScroll();
      
      throttleTimer = setTimeout(() => {
        throttleTimer = null;
        if (pendingCheck) {
          checkAndScroll();
        }
      }, THROTTLE_MS);
    };
    
    // Observe DOM changes via MutationObserver.
    // Optimization: listen to childList only; streaming updates trigger childList changes.
    const observer = new MutationObserver(throttledCheckAndScroll);
    
    observer.observe(scroller, { 
      childList: true, 
      subtree: true,
      // Remove attributes and characterData to reduce callbacks.
      // attributes: true,
      // characterData: true 
    });
    
    checkAndScroll();
    
    return () => {
      observer.disconnect();
      if (rafId) cancelAnimationFrame(rafId);
      if (throttleTimer) clearTimeout(throttleTimer);
    };
  }, []); // Empty deps: set up once on mount.

  // Compute all user message items and their turn info.
  const userMessageItems = React.useMemo(() => {
    return virtualItems
      .map((item, index) => ({ item, index }))
      .filter(({ item }) => item.type === 'user-message');
  }, [virtualItems]);

  // Update visible turn info when the range changes.
  const handleRangeChanged = useCallback((range: ListRange) => {
    lastRangeRef.current = range;
    const setVisibleTurnInfo = useModernFlowChatStore.getState().setVisibleTurnInfo;
    
    if (userMessageItems.length === 0) {
      setVisibleTurnInfo(null);
      return;
    }

    // Find the first visible user message.
    const visibleUserMessage = userMessageItems.find(({ index }) => 
      index >= range.startIndex && index <= range.endIndex
    );

    // If none are visible, find the closest one before the range.
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

  // Initialize visible turn info.
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

  // Scroll to a specific turn.
  const scrollToTurn = useCallback((turnIndex: number) => {
    if (virtuosoRef.current && turnIndex >= 1 && turnIndex <= userMessageItems.length) {
      const targetItem = userMessageItems[turnIndex - 1];
      if (targetItem) {
        beginNavigation();
        // Immediately exit stick-to-bottom to avoid a race where followOutput / delayed
        // scroll-to-bottom pulls us back down before state updates propagate.
        stickToBottomRef.current = false;

        if (targetItem.index === 0) {
          virtuosoRef.current.scrollTo({
            top: 0,
            behavior: 'smooth',
          });
        } else {
          virtuosoRef.current.scrollToIndex({
            index: targetItem.index,
            align: 'center',
            behavior: 'smooth',
          });
        }
        setStickToBottom(false); // Exit stick-to-bottom on manual navigation.
      }
    }
  }, [userMessageItems]);

  const scrollToIndex = useCallback((index: number) => {
    if (!virtuosoRef.current) return;
    if (index < 0 || index >= virtualItems.length) return;

    beginNavigation();
    stickToBottomRef.current = false;

    if (index === 0) {
      virtuosoRef.current.scrollTo({ top: 0, behavior: 'auto' });
    } else {
      virtuosoRef.current.scrollToIndex({
        index,
        align: 'center',
        behavior: 'auto',
      });
    }

    setStickToBottom(false);
  }, [virtualItems.length]);

  // Scroll to bottom.
  const scrollToBottom = useCallback(() => {
    if (virtuosoRef.current && virtualItems.length > 0) {
      suppressAutoEnableStickToBottomRef.current = false;
      if (suppressAutoEnableTimerRef.current) {
        clearTimeout(suppressAutoEnableTimerRef.current);
        suppressAutoEnableTimerRef.current = null;
      }
      stickToBottomRef.current = true;
      virtuosoRef.current.scrollTo({
        top: 999999999,
        behavior: 'smooth',
      });
      setStickToBottom(true); // Restore stick-to-bottom mode.
    }
  }, [virtualItems.length]);

  // When list length increases in stick-to-bottom mode, delay scroll-to-bottom
  // to avoid followOutput scrolling before layout is ready.
  useEffect(() => {
    const currentLength = virtualItems.length;
    const prevLength = prevLengthRef.current;
    prevLengthRef.current = currentLength;

    if (!stickToBottom || currentLength === 0 || currentLength <= prevLength) return;
    if (!virtuosoRef.current) return;

    // Suppress followOutput and do a single delayed scroll-to-bottom.
    suppressFollowOutputRef.current = true;
    let delayedScrollTimeoutId: ReturnType<typeof setTimeout> | null = null;
    const rafId1 = requestAnimationFrame(() => {
      requestAnimationFrame(() => {
        if (!virtuosoRef.current || !stickToBottomRef.current) {
          suppressFollowOutputRef.current = false;
          return;
        }
        programmaticScrollToBottomRef.current = true;
        // One delayed scroll-to-bottom; skip scrollToIndex to avoid jitter.
        delayedScrollTimeoutId = setTimeout(() => {
          if (virtuosoRef.current && stickToBottomRef.current) {
            virtuosoRef.current.scrollTo({ top: 999999999, behavior: 'auto' });
          }
          setTimeout(() => { programmaticScrollToBottomRef.current = false; }, 300);
          // Delay releasing suppression to avoid another followOutput trigger.
          setTimeout(() => { suppressFollowOutputRef.current = false; }, 150);
        }, 100);
      });
    });
    return () => {
      cancelAnimationFrame(rafId1);
      if (delayedScrollTimeoutId !== null) clearTimeout(delayedScrollTimeoutId);
    };
  }, [virtualItems.length, stickToBottom]);

  // Expose methods to parent components.
  useImperativeHandle(ref, () => ({
    scrollToTurn,
    scrollToIndex,
    scrollToBottom,
  }), [scrollToTurn, scrollToIndex, scrollToBottom]);
  
  // Processing state from the state machine.
  const activeSessionState = useActiveSessionState();
  const isProcessing = activeSessionState.isProcessing;
  const processingPhase = activeSessionState.processingPhase;
  
  // Get the last item content for time-based checks.
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
  
  // Time-based heuristic: detect whether content is growing.
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
  
  // Reset content-growth state when not processing.
  useEffect(() => {
    if (!lastItemInfo.isTurnProcessing && !isProcessing) {
      setIsContentGrowing(false);
    }
  }, [lastItemInfo.isTurnProcessing, isProcessing]);
  
  // Breathing indicator visibility.
  const showBreathingIndicator = React.useMemo(() => {
    const { lastItem, isTurnProcessing } = lastItemInfo;
    
    if (!isTurnProcessing && !isProcessing) {
      return false;
    }
    
    if (processingPhase === 'tool_confirming') {
      return false;
    }
    
    if (!lastItem) {
      return true;
    }
    
    if ((lastItem.type === 'text' || lastItem.type === 'thinking')) {
      const hasContent = 'content' in lastItem && lastItem.content;
      if (hasContent && isContentGrowing) {
        return false;
      }
    }
    
    if (lastItem.type === 'tool') {
      const toolStatus = lastItem.status;
      if (toolStatus === 'running' || toolStatus === 'streaming' || toolStatus === 'preparing') {
        return false;
      }
    }
    
    return isTurnProcessing || isProcessing;
  }, [isProcessing, processingPhase, lastItemInfo, isContentGrowing]);

  // Reserve space while processing to avoid layout jumps.
  const reserveSpaceForIndicator = React.useMemo(() => {
    if (!lastItemInfo.isTurnProcessing && !isProcessing) return false;
    if (processingPhase === 'tool_confirming') return false;
    return true;
  }, [lastItemInfo.isTurnProcessing, isProcessing, processingPhase]);
  
  // Diagnostic logging (development only).
  // React.useEffect(() => {
  //   if (process.env.NODE_ENV === 'development') {
  //     console.log('[VirtualMessageList] Processing state:', {
  //       sessionId: activeSession?.sessionId,
  //       isProcessing,
  //       processingPhase,
  //       showBreathingIndicator,
  //       stickToBottom,
  //       status: activeSession?.status
  //     });
  //   }
  // }, [activeSession?.sessionId, isProcessing, processingPhase, showBreathingIndicator, stickToBottom, activeSession?.status]);

  // Listen to bottom state changes.
  const handleAtBottomStateChange = useCallback((atBottom: boolean) => {
    if (!isInitializedRef.current) return;
    
    setIsAtBottom(atBottom);
    
    if (atBottom && !stickToBottomRef.current && !suppressAutoEnableStickToBottomRef.current) {
      stickToBottomRef.current = true;
      setStickToBottom(true);
    }
  }, []);

  // Listen to scroll and detect intentional upward scrolling.
  const handleScroll = useCallback((scrolling: boolean) => {
    // Mark user scrolling to avoid MutationObserver interference.
    if (scrolling) {
      isUserScrollingRef.current = true;
      
      if (userScrollingTimeoutRef.current) {
        clearTimeout(userScrollingTimeoutRef.current);
      }
      
      // Clear the marker 200ms after scroll end.
      userScrollingTimeoutRef.current = setTimeout(() => {
        isUserScrollingRef.current = false;
      }, 200);
    }
    
    if (!scrolling) return;
    if (!isInitializedRef.current) return;
    
    if (virtuosoRef.current) {
      virtuosoRef.current.getState((state) => {
        // Skip when programmatic scroll-to-bottom is in progress.
        if (programmaticScrollToBottomRef.current) return;
        const currentScrollTop = state.scrollTop;
        const scrollDelta = currentScrollTop - lastScrollTopRef.current;
        
        // Exit stick-to-bottom only after scrolling up past threshold.
        if (scrollDelta < -SCROLL_UP_THRESHOLD) {
          stickToBottomRef.current = false;
          setStickToBottom(false);
        }
        
        lastScrollTopRef.current = currentScrollTop;
      });
    }
  }, []);

  // followOutput: honor stick-to-bottom; suppress during delayed scroll to avoid jitter.
  const handleFollowOutput = useCallback(() => {
    if (suppressFollowOutputRef.current) return false;
    return stickToBottomRef.current ? 'smooth' : false;
  }, []);

  // Empty state.
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
        // Auto-follow based on stick-to-bottom.
        followOutput={handleFollowOutput}
        
        alignToBottom={false}
        initialTopMostItemIndex={0}
        
        // Overscan (lower to reduce memory).
        overscan={50}
        
        atBottomThreshold={50}
        
        atBottomStateChange={handleAtBottomStateChange}
        
        isScrolling={handleScroll}
        
        rangeChanged={handleRangeChanged}
        
        defaultItemHeight={100}
        
        // Increase viewport by a smaller amount to limit memory.
        increaseViewportBy={{ top: 100, bottom: 200 }}
        
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
        visible={(!stickToBottom || !isAtBottom) && virtualItems.length > 0}
        onClick={scrollToBottom}
        isInputActive={isInputActive}
        isInputExpanded={isInputExpanded}
      />
    </div>
  );
});

VirtualMessageList.displayName = 'VirtualMessageList';
