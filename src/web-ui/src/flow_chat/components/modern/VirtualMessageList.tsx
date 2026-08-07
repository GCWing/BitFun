/**
 * Virtualized FlowChat transcript with natural browser scroll range.
 *
 * The list never manufactures tail space for turn alignment or layout
 * preservation. Navigation is best-effort within the physical content range,
 * card collapses reflow naturally, and useFlowChatFollowOutput is the only
 * continuous writer that follows streaming output.
 */

import React, {
  forwardRef,
  useCallback,
  useEffect,
  useImperativeHandle,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from 'react';
import {
  Virtuoso,
  type Components,
  type ContextProp,
  type ListRange,
  type VirtuosoHandle,
} from 'react-virtuoso';
import { Loader2 } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { useActiveSessionState } from '../../hooks/useActiveSessionState';
import { useScrollToTurnHeader } from '../../hooks/useScrollToTurnHeader';
import { useVisibleTaskInfo } from '../../hooks/useVisibleTaskInfo';
import type { SessionHistoryWindowDirection } from '../../store/FlowChatStore';
import {
  useActiveSession,
  useModernFlowChatStore,
  useVirtualItems,
  type VirtualItem,
} from '../../store/modernFlowChatStore';
import { useChatInputState } from '../../store/chatInputStateStore';
import type { ActiveTurnRenderRange } from '../../types/flow-chat';
import { computeFlowChatInputStackFooterPx } from '../../utils/flowChatScrollLayout';
import { ScrollToLatestBar } from '../ScrollToLatestBar';
import { ScrollToTurnHeaderButton } from '../ScrollToTurnHeaderButton';
import {
  findElementWithDataValue,
  findFlowChatSearchTextRanges,
  getFlowChatSearchTextRoot,
  setFlowChatSearchHighlight,
} from './flowChatSearchDom';
import { RuntimeStatusSlot } from './RuntimeStatusSlot';
import { StickyTaskIndicator } from '../StickyTaskIndicator';
import { useFlowChatFollowOutput } from './useFlowChatFollowOutput';
import { VirtualItemRenderer } from './VirtualItemRenderer';
import { getLeadingVirtualItemIndexDelta } from './virtualMessageListLayout';
import { resolveVisibleFlowChatTurnIds } from './flowChatVisibleTurns';
import './VirtualMessageList.scss';

const VIRTUOSO_FIRST_ITEM_INDEX_BASE = 1_000_000;
const SEARCH_NAVIGATION_MAX_ATTEMPTS = 24;
const FLOW_CHAT_VIRTUOSO_OVERSCAN = { main: 600, reverse: 600 } as const;
const FLOW_CHAT_VIRTUOSO_VIEWPORT_INCREASE = { top: 600, bottom: 600 } as const;
const IDLE_HISTORY_WINDOW_BOUNDARY_STATE: Record<
  SessionHistoryWindowDirection,
  'idle' | 'loading' | 'error'
> = { before: 'idle', after: 'idle' };

export type FlowChatTurnNavigationStatus = 'rejected' | 'pending' | 'settled';

export interface TurnNavigationOptions {
  behavior?: ScrollBehavior;
}

export type HistoryWindowBoundaryIntentResult =
  | 'applied'
  | 'exhausted'
  | 'not-ready'
  | 'cancelled';

type HistoryWindowBoundaryIntentResponse =
  | HistoryWindowBoundaryIntentResult
  | boolean
  | void;

export interface HistoryWindowBoundaryIntentOptions {
  prepareViewportForPresentationCommit?: () => boolean | void | Promise<boolean | void>;
  cancelViewportPresentationCommit?: () => void;
}

export interface VirtualMessageListRef {
  scrollToTurn: (turnIndex: number) => void;
  scrollToIndex: (index: number) => void;
  scrollToSearchMatch: (target: {
    virtualItemIndex: number;
    query: string;
    flowItemId?: string;
    occurrenceIndex?: number;
    expandableIds?: readonly string[];
  }) => void;
  clearSearchMatch: () => void;
  scrollToPhysicalBottom: () => void;
  scrollToTurnEnd: (turnId: string) => boolean;
  isTurnRenderedInViewport: (turnId: string) => boolean;
  isTurnTextRenderedInViewport: (turnId: string) => boolean;
  scrollToLatestEndPosition: () => void;
  navigateToTurn: (turnId: string, options?: TurnNavigationOptions) => boolean;
  navigateToTurnWithStatus: (
    turnId: string,
    options?: TurnNavigationOptions,
  ) => FlowChatTurnNavigationStatus;
  prepareTurnNavigation: (
    turnId: string,
    options?: TurnNavigationOptions,
  ) => FlowChatTurnNavigationStatus;
}

export interface VirtualMessageListProps {
  items?: VirtualItem[];
  isViewportActive?: boolean;
  presentationMode?: 'tail' | 'history-window';
  viewportMode?: 'live-tail' | 'history-reading';
  historyWindow?: ActiveTurnRenderRange | null;
  presentationRevision?: number;
  historyBoundaryState?: Record<SessionHistoryWindowDirection, 'idle' | 'loading' | 'error'>;
  onHistoryWindowBoundaryIntent?: (
    direction: SessionHistoryWindowDirection,
    options?: HistoryWindowBoundaryIntentOptions,
  ) => HistoryWindowBoundaryIntentResponse | Promise<HistoryWindowBoundaryIntentResponse>;
  onRequestJumpToLatest?: () => void;
  onUserScrollIntent?: () => void;
}

type FlowChatVirtuosoContext = {
  bottomLayoutInsetPx: number;
  previousHistoryBoundaryStatusNode: React.ReactNode;
  nextHistoryBoundaryStatusNode: React.ReactNode;
  runtimeStatusSessionId: string | null;
};

type PreparedTurnNavigation = {
  turnId: string;
  behavior: ScrollBehavior;
};

type HistoryPrependAnchor = {
  turnId: string;
  offsetFromScrollerTop: number;
};

const FlowChatVirtuosoHeader = ({ context }: ContextProp<FlowChatVirtuosoContext>) => (
  <>
    <div
      className="message-list-header"
      data-bf-component="virtual-message-list"
      data-bf-part="header"
    />
    {context.previousHistoryBoundaryStatusNode}
  </>
);

const FlowChatVirtuosoFooter = ({ context }: ContextProp<FlowChatVirtuosoContext>) => (
  <div
    className="message-list-footer"
    data-bf-component="virtual-message-list"
    data-bf-part="footer"
    style={{
      height: `${context.bottomLayoutInsetPx}px`,
      minHeight: `${context.bottomLayoutInsetPx}px`,
    }}
  >
    {context.nextHistoryBoundaryStatusNode}
    <RuntimeStatusSlot sessionId={context.runtimeStatusSessionId} placement="footer" />
  </div>
);

const FLOW_CHAT_VIRTUOSO_COMPONENTS: Components<VirtualItem, FlowChatVirtuosoContext> = {
  Header: FlowChatVirtuosoHeader,
  Footer: FlowChatVirtuosoFooter,
};

const FlowChatHistoryPagingSentinel = ({
  state,
  label,
}: {
  state: 'idle' | 'loading' | 'error';
  label: string;
}) => (
  <div
    className="virtual-message-list__history-paging-sentinel"
    data-history-paging-sentinel={state}
    data-history-boundary-status={state === 'loading' ? 'preparing' : state === 'error' ? 'not-ready' : undefined}
    aria-hidden={state === 'idle'}
    role={state === 'idle' ? undefined : 'status'}
    aria-live={state === 'idle' ? undefined : 'polite'}
  >
    {state === 'loading' ? (
      <Loader2 size={14} aria-hidden className="virtual-message-list__history-paging-spinner" />
    ) : null}
    <span>{label}</span>
  </div>
);

function normalizeBoundaryResult(
  result: HistoryWindowBoundaryIntentResponse,
): HistoryWindowBoundaryIntentResult {
  if (result === true) return 'applied';
  if (result === false || result === undefined) return 'not-ready';
  return result;
}

function normalizeVirtuosoBehavior(behavior: ScrollBehavior): 'auto' | 'smooth' {
  return behavior === 'smooth' ? 'smooth' : 'auto';
}

function getVirtualItemStableKey(item: VirtualItem): string {
  switch (item.type) {
    case 'user-message':
    case 'user-steering-message':
      return `${item.type}:${item.turnId}:${item.data.id}`;
    case 'model-round':
      return `${item.type}:${item.turnId}:${item.data.id}`;
    case 'explore-group':
      return `${item.type}:${item.turnId}:${item.data.groupId}`;
    case 'turn-completion-notice':
      return `${item.type}:${item.turnId}:${item.data.reasonCode}`;
    case 'turn-failure-notice':
    case 'image-analyzing':
      return `${item.type}:${item.turnId}`;
  }
}

function isElementVisibleInScroller(element: HTMLElement, scroller: HTMLElement): boolean {
  const elementRect = element.getBoundingClientRect();
  const scrollerRect = scroller.getBoundingClientRect();
  return elementRect.bottom > scrollerRect.top && elementRect.top < scrollerRect.bottom;
}

const VirtualMessageListSession = forwardRef<VirtualMessageListRef, VirtualMessageListProps>(({
  items,
  isViewportActive = true,
  presentationMode = 'tail',
  viewportMode = presentationMode === 'history-window' ? 'history-reading' : 'live-tail',
  historyWindow: _historyWindow = null,
  presentationRevision: _presentationRevision = 0,
  historyBoundaryState = IDLE_HISTORY_WINDOW_BOUNDARY_STATE,
  onHistoryWindowBoundaryIntent,
  onRequestJumpToLatest,
  onUserScrollIntent,
}, ref) => {
  const { t } = useTranslation('flow-chat');
  const canonicalVirtualItems = useVirtualItems();
  const virtualItems = items ?? canonicalVirtualItems;
  const activeSession = useActiveSession();
  const activeSessionState = useActiveSessionState();
  const activeSessionId = activeSession?.sessionId ?? null;
  const latestTurnId = virtualItems.at(-1)?.turnId ?? null;
  const virtuosoRef = useRef<VirtuosoHandle>(null);
  const scrollerElementRef = useRef<HTMLElement | null>(null);
  const [scrollerElement, setScrollerElement] = useState<HTMLElement | null>(null);
  const [isAtBottom, setIsAtBottom] = useState(true);
  const preparedTurnNavigationRef = useRef<PreparedTurnNavigation | null>(null);
  const historyPrependAnchorRef = useRef<HistoryPrependAnchor | null>(null);
  const boundaryRequestRef = useRef<Record<SessionHistoryWindowDirection, Promise<void> | null>>({
    before: null,
    after: null,
  });
  const exhaustedBoundaryRef = useRef<Record<SessionHistoryWindowDirection, boolean>>({
    before: false,
    after: false,
  });
  const searchNavigationRequestIdRef = useRef(0);
  const visibleTurnUpdateFrameRef = useRef<number | null>(null);

  const virtuosoIndexStateRef = useRef({
    sessionId: activeSessionId,
    firstItemIndex: VIRTUOSO_FIRST_ITEM_INDEX_BASE,
    virtualItems,
  });
  const virtuosoIndexState = virtuosoIndexStateRef.current;
  if (virtuosoIndexState.sessionId !== activeSessionId) {
    virtuosoIndexState.sessionId = activeSessionId;
    virtuosoIndexState.firstItemIndex = VIRTUOSO_FIRST_ITEM_INDEX_BASE;
    virtuosoIndexState.virtualItems = virtualItems;
  } else if (virtuosoIndexState.virtualItems !== virtualItems) {
    virtuosoIndexState.firstItemIndex = Math.max(
      0,
      virtuosoIndexState.firstItemIndex + getLeadingVirtualItemIndexDelta(
        virtuosoIndexState.virtualItems,
        virtualItems,
        getVirtualItemStableKey,
      ),
    );
    virtuosoIndexState.virtualItems = virtualItems;
  }
  const virtuosoFirstItemIndex = virtuosoIndexState.firstItemIndex;

  const userMessageItems = useMemo(() => virtualItems
    .map((item, index) => ({ item, index }))
    .filter(({ item }) => item.type === 'user-message'), [virtualItems]);

  const isStreamingOutput = useMemo(() => {
    if (viewportMode === 'history-reading') return false;
    if (activeSessionState.isProcessing) return true;
    const latestTurn = activeSession?.dialogTurns.at(-1);
    return Boolean(
      latestTurn && (
        latestTurn.status === 'processing' ||
        latestTurn.status === 'finishing' ||
        latestTurn.status === 'image_analyzing' ||
        latestTurn.modelRounds.some(round => round.isStreaming)
      )
    );
  }, [activeSession, activeSessionState.isProcessing, viewportMode]);

  const isInputActive = useChatInputState(state => state.isActive);
  const isInputExpanded = useChatInputState(state => state.isExpanded);
  const inputHeight = useChatInputState(state => state.inputHeight);
  const bottomLayoutInsetPx = computeFlowChatInputStackFooterPx(inputHeight);

  const scrollToTail = useCallback((behavior: ScrollBehavior) => {
    const scroller = scrollerElementRef.current;
    if (scroller) {
      scroller.scrollTo({
        top: Math.max(0, scroller.scrollHeight - scroller.clientHeight),
        behavior,
      });
      return;
    }
    virtuosoRef.current?.scrollToIndex({
      index: Math.max(0, virtualItems.length - 1),
      align: 'end',
      behavior: normalizeVirtuosoBehavior(behavior),
    });
  }, [virtualItems.length]);

  const {
    enterFollowOutput,
    exitFollowOutput,
    scheduleFollowToLatest,
    handleUserScrollIntent,
    handleScroll,
  } = useFlowChatFollowOutput({
    activeSessionId: activeSessionId ?? undefined,
    latestTurnId,
    virtualItemCount: virtualItems.length,
    isStreaming: isStreamingOutput,
    isViewportActive,
    scrollerRef: scrollerElementRef,
    scrollToTail,
  });

  const notifyUserScrollIntent = useCallback(() => {
    handleUserScrollIntent();
    onUserScrollIntent?.();
  }, [handleUserScrollIntent, onUserScrollIntent]);

  const updateVisibleTurnInfoFromViewport = useCallback(() => {
    const scroller = scrollerElementRef.current;
    if (!scroller) return;
    const scrollerRect = scroller.getBoundingClientRect();
    const viewportEntries = Array.from(
      scroller.querySelectorAll<HTMLElement>('.virtual-item-wrapper[data-turn-id]'),
    ).map(element => {
      const rect = element.getBoundingClientRect();
      return {
        turnId: element.dataset.turnId ?? null,
        itemType: element.dataset.itemType ?? null,
        top: rect.top,
        bottom: rect.bottom,
      };
    });
    const visibleTurnIds = resolveVisibleFlowChatTurnIds(
      viewportEntries,
      scrollerRect.top,
      scrollerRect.bottom,
    );
    const currentTurnId = visibleTurnIds[0] ?? null;
    const currentTurn = currentTurnId
      ? userMessageItems.find(({ item }) => item.turnId === currentTurnId)
      : undefined;
    const store = useModernFlowChatStore.getState();

    if (!currentTurn || currentTurn.item.type !== 'user-message') {
      if (store.visibleTurnInfo !== null) store.setVisibleTurnInfo(null);
      return;
    }

    const nextVisibleTurnInfo = {
      turnIndex: userMessageItems.indexOf(currentTurn) + 1,
      totalTurns: userMessageItems.length,
      userMessage: currentTurn.item.data.content ?? '',
      turnId: currentTurn.item.turnId,
      visibleTurnIds,
    };
    const previous = store.visibleTurnInfo;
    const unchanged = previous?.turnId === nextVisibleTurnInfo.turnId
      && previous.turnIndex === nextVisibleTurnInfo.turnIndex
      && previous.totalTurns === nextVisibleTurnInfo.totalTurns
      && previous.userMessage === nextVisibleTurnInfo.userMessage
      && previous.visibleTurnIds.length === visibleTurnIds.length
      && previous.visibleTurnIds.every((turnId, index) => turnId === visibleTurnIds[index]);
    if (!unchanged) store.setVisibleTurnInfo(nextVisibleTurnInfo);
  }, [userMessageItems]);

  const scheduleVisibleTurnInfoUpdate = useCallback(() => {
    if (visibleTurnUpdateFrameRef.current !== null) return;
    visibleTurnUpdateFrameRef.current = requestAnimationFrame(() => {
      visibleTurnUpdateFrameRef.current = null;
      updateVisibleTurnInfoFromViewport();
    });
  }, [updateVisibleTurnInfoFromViewport]);

  useEffect(() => () => {
    if (visibleTurnUpdateFrameRef.current !== null) {
      cancelAnimationFrame(visibleTurnUpdateFrameRef.current);
      visibleTurnUpdateFrameRef.current = null;
    }
  }, []);

  useEffect(() => {
    if (!scrollerElement) return;
    const handleNativeScroll = () => {
      const distanceFromBottom = Math.max(
        0,
        scrollerElement.scrollHeight - scrollerElement.clientHeight - scrollerElement.scrollTop,
      );
      setIsAtBottom(distanceFromBottom <= 50);
      handleScroll();
      scheduleVisibleTurnInfoUpdate();
    };
    const handleWheel = () => notifyUserScrollIntent();
    const handleTouchMove = () => notifyUserScrollIntent();
    const handleKeyDown = (event: KeyboardEvent) => {
      if (['ArrowUp', 'ArrowDown', 'PageUp', 'PageDown', 'Home', 'End', ' '].includes(event.key)) {
        notifyUserScrollIntent();
      }
    };
    scrollerElement.addEventListener('scroll', handleNativeScroll, { passive: true });
    scrollerElement.addEventListener('wheel', handleWheel, { passive: true });
    scrollerElement.addEventListener('touchmove', handleTouchMove, { passive: true });
    scrollerElement.addEventListener('keydown', handleKeyDown);
    return () => {
      scrollerElement.removeEventListener('scroll', handleNativeScroll);
      scrollerElement.removeEventListener('wheel', handleWheel);
      scrollerElement.removeEventListener('touchmove', handleTouchMove);
      scrollerElement.removeEventListener('keydown', handleKeyDown);
    };
  }, [handleScroll, notifyUserScrollIntent, scheduleVisibleTurnInfoUpdate, scrollerElement]);

  useEffect(() => {
    if (!scrollerElement) return;
    const observer = new ResizeObserver(() => {
      scheduleFollowToLatest();
      scheduleVisibleTurnInfoUpdate();
    });
    const content = scrollerElement.firstElementChild;
    if (content) observer.observe(content);
    observer.observe(scrollerElement);
    return () => observer.disconnect();
  }, [scheduleFollowToLatest, scheduleVisibleTurnInfoUpdate, scrollerElement]);

  const getRenderedUserMessageElement = useCallback((turnId: string) => (
    Array.from(
      scrollerElementRef.current?.querySelectorAll<HTMLElement>(
        '.virtual-item-wrapper[data-item-type="user-message"]',
      ) ?? [],
    ).find(element => element.dataset.turnId === turnId) ?? null
  ), []);

  const navigateToTurnWithStatus = useCallback((
    turnId: string,
    options?: TurnNavigationOptions,
  ): FlowChatTurnNavigationStatus => {
    const targetIndex = virtualItems.findIndex(item => (
      item.turnId === turnId && item.type === 'user-message'
    ));
    if (targetIndex < 0 || !virtuosoRef.current) return 'rejected';
    exitFollowOutput('scroll-to-turn');
    virtuosoRef.current.scrollToIndex({
      index: targetIndex,
      align: 'start',
      behavior: normalizeVirtuosoBehavior(options?.behavior ?? 'auto'),
    });
    return 'settled';
  }, [exitFollowOutput, virtualItems]);

  const navigateToTurn = useCallback((turnId: string, options?: TurnNavigationOptions) => (
    navigateToTurnWithStatus(turnId, options) !== 'rejected'
  ), [navigateToTurnWithStatus]);

  const prepareTurnNavigation = useCallback((
    turnId: string,
    options?: TurnNavigationOptions,
  ): FlowChatTurnNavigationStatus => {
    if (!turnId || !activeSessionId) return 'rejected';
    exitFollowOutput('scroll-to-turn');
    preparedTurnNavigationRef.current = {
      turnId,
      behavior: options?.behavior ?? 'auto',
    };
    return 'pending';
  }, [activeSessionId, exitFollowOutput]);

  useLayoutEffect(() => {
    const prepared = preparedTurnNavigationRef.current;
    if (!prepared) return;
    const status = navigateToTurnWithStatus(prepared.turnId, { behavior: prepared.behavior });
    if (status === 'settled') preparedTurnNavigationRef.current = null;
  }, [navigateToTurnWithStatus, virtualItems]);

  const scrollToTurn = useCallback((turnIndex: number) => {
    const target = userMessageItems[turnIndex - 1];
    if (target) navigateToTurn(target.item.turnId, { behavior: 'smooth' });
  }, [navigateToTurn, userMessageItems]);

  const scrollToIndex = useCallback((index: number) => {
    if (!virtuosoRef.current || index < 0 || index >= virtualItems.length) return;
    exitFollowOutput('scroll-to-index');
    virtuosoRef.current.scrollToIndex({ index, align: 'center', behavior: 'auto' });
  }, [exitFollowOutput, virtualItems.length]);

  const scrollToTurnEnd = useCallback((turnId: string) => {
    let targetIndex = -1;
    for (let index = virtualItems.length - 1; index >= 0; index -= 1) {
      if (virtualItems[index]?.turnId === turnId) {
        targetIndex = index;
        break;
      }
    }
    if (targetIndex < 0 || !virtuosoRef.current) return false;
    exitFollowOutput('scroll-to-turn');
    virtuosoRef.current.scrollToIndex({ index: targetIndex, align: 'end', behavior: 'auto' });
    return true;
  }, [exitFollowOutput, virtualItems]);

  const isTurnRenderedInViewport = useCallback((turnId: string) => {
    const scroller = scrollerElementRef.current;
    const element = getRenderedUserMessageElement(turnId);
    return Boolean(scroller && element && isElementVisibleInScroller(element, scroller));
  }, [getRenderedUserMessageElement]);

  const isTurnTextRenderedInViewport = useCallback((turnId: string) => {
    const scroller = scrollerElementRef.current;
    const element = getRenderedUserMessageElement(turnId);
    return Boolean(
      scroller &&
      element &&
      element.textContent?.trim() &&
      isElementVisibleInScroller(element, scroller)
    );
  }, [getRenderedUserMessageElement]);

  const clearSearchMatch = useCallback(() => {
    searchNavigationRequestIdRef.current += 1;
    setFlowChatSearchHighlight(null);
  }, []);

  const scrollToSearchMatch = useCallback((target: {
    virtualItemIndex: number;
    query: string;
    flowItemId?: string;
    occurrenceIndex?: number;
    expandableIds?: readonly string[];
  }) => {
    clearSearchMatch();
    exitFollowOutput('scroll-to-index');
    const requestId = searchNavigationRequestIdRef.current;
    virtuosoRef.current?.scrollToIndex({
      index: target.virtualItemIndex,
      align: 'center',
      behavior: 'auto',
    });
    let attempts = 0;
    const resolve = () => {
      if (searchNavigationRequestIdRef.current !== requestId) return;
      attempts += 1;
      const scroller = scrollerElementRef.current;
      const wrapper = Array.from(
        scroller?.querySelectorAll<HTMLElement>('.virtual-item-wrapper') ?? [],
      ).find(element => Number(element.dataset.virtualIndex) === target.virtualItemIndex);
      if (!scroller || !wrapper) {
        if (attempts < SEARCH_NAVIGATION_MAX_ATTEMPTS) requestAnimationFrame(resolve);
        return;
      }
      for (const expandableId of target.expandableIds ?? []) {
        const expandable = findElementWithDataValue(wrapper, 'data-tool-card-id', expandableId);
        if (expandable?.dataset.expanded === 'false') {
          expandable.querySelector<HTMLElement>(
            '[data-testid="chat-explore-group-toggle"], [data-testid="chat-thinking-toggle"]',
          )?.click();
          if (attempts < SEARCH_NAVIGATION_MAX_ATTEMPTS) requestAnimationFrame(resolve);
          return;
        }
      }
      const root = getFlowChatSearchTextRoot(wrapper, target.flowItemId);
      const ranges = findFlowChatSearchTextRanges(root, target.query);
      const rangeIndex = Math.min(target.occurrenceIndex ?? 0, Math.max(0, ranges.length - 1));
      const range = ranges[rangeIndex] ?? null;
      if (!range) return;
      setFlowChatSearchHighlight(range, ranges.filter((_, index) => index !== rangeIndex));
      const rangeRect = range.getBoundingClientRect();
      const scrollerRect = scroller.getBoundingClientRect();
      scroller.scrollTop = Math.max(
        0,
        Math.min(
          scroller.scrollHeight - scroller.clientHeight,
          scroller.scrollTop + rangeRect.top - scrollerRect.top -
            Math.max(0, (scroller.clientHeight - rangeRect.height) / 2),
        ),
      );
    };
    requestAnimationFrame(resolve);
  }, [clearSearchMatch, exitFollowOutput]);

  useEffect(() => () => setFlowChatSearchHighlight(null), []);

  const captureHistoryPrependAnchor = useCallback(() => {
    const scroller = scrollerElementRef.current;
    if (!scroller) return false;
    const scrollerRect = scroller.getBoundingClientRect();
    const anchor = Array.from(
      scroller.querySelectorAll<HTMLElement>(
        '.virtual-item-wrapper[data-item-type="user-message"]',
      ),
    ).find(element => element.getBoundingClientRect().bottom > scrollerRect.top);
    if (!anchor?.dataset.turnId) return false;
    historyPrependAnchorRef.current = {
      turnId: anchor.dataset.turnId,
      offsetFromScrollerTop: anchor.getBoundingClientRect().top - scrollerRect.top,
    };
    return true;
  }, []);

  useLayoutEffect(() => {
    const anchor = historyPrependAnchorRef.current;
    const scroller = scrollerElementRef.current;
    if (!anchor || !scroller) return;
    const element = getRenderedUserMessageElement(anchor.turnId);
    if (!element) return;
    const correction = element.getBoundingClientRect().top -
      scroller.getBoundingClientRect().top - anchor.offsetFromScrollerTop;
    scroller.scrollTop += correction;
    historyPrependAnchorRef.current = null;
  }, [getRenderedUserMessageElement, virtualItems]);

  const requestHistoryBoundary = useCallback((direction: SessionHistoryWindowDirection) => {
    if (
      !onHistoryWindowBoundaryIntent ||
      boundaryRequestRef.current[direction] ||
      exhaustedBoundaryRef.current[direction]
    ) return;
    const request = Promise.resolve(onHistoryWindowBoundaryIntent(direction, direction === 'before' ? {
      prepareViewportForPresentationCommit: captureHistoryPrependAnchor,
      cancelViewportPresentationCommit: () => {
        historyPrependAnchorRef.current = null;
      },
    } : undefined)).then(normalizeBoundaryResult).then(result => {
      if (result === 'exhausted') {
        exhaustedBoundaryRef.current[direction] = true;
      } else if (result === 'applied') {
        exhaustedBoundaryRef.current[direction] = false;
      }
      if (result !== 'applied') historyPrependAnchorRef.current = null;
    }).finally(() => {
      boundaryRequestRef.current[direction] = null;
    });
    boundaryRequestRef.current[direction] = request;
  }, [captureHistoryPrependAnchor, onHistoryWindowBoundaryIntent]);

  const handleRangeChanged = useCallback((range: ListRange) => {
    const localStart = Math.max(0, range.startIndex - virtuosoFirstItemIndex);
    const localEnd = Math.max(localStart, range.endIndex - virtuosoFirstItemIndex);
    scheduleVisibleTurnInfoUpdate();
    if (localStart <= 1) requestHistoryBoundary('before');
    if (localEnd >= virtualItems.length - 2 && presentationMode === 'history-window') {
      requestHistoryBoundary('after');
    }
  }, [presentationMode, requestHistoryBoundary, scheduleVisibleTurnInfoUpdate, virtualItems.length, virtuosoFirstItemIndex]);

  useLayoutEffect(() => {
    scheduleVisibleTurnInfoUpdate();
  }, [scheduleVisibleTurnInfoUpdate, virtualItems]);

  useEffect(() => {
    if (userMessageItems.length === 0) {
      useModernFlowChatStore.getState().setVisibleTurnInfo(null);
    }
  }, [userMessageItems.length]);

  const handleScrollerRef = useCallback((element: HTMLElement | Window | null) => {
    const scroller = element instanceof HTMLElement ? element : null;
    scrollerElementRef.current = scroller;
    setScrollerElement(scroller);
  }, []);

  const scrollToPhysicalBottom = useCallback(() => {
    enterFollowOutput('jump-to-latest');
  }, [enterFollowOutput]);

  const scrollToLatestEndPosition = useCallback(() => {
    onUserScrollIntent?.();
    enterFollowOutput('jump-to-latest');
  }, [enterFollowOutput, onUserScrollIntent]);

  useImperativeHandle(ref, () => ({
    scrollToTurn,
    scrollToIndex,
    scrollToSearchMatch,
    clearSearchMatch,
    scrollToPhysicalBottom,
    scrollToTurnEnd,
    isTurnRenderedInViewport,
    isTurnTextRenderedInViewport,
    scrollToLatestEndPosition,
    navigateToTurn,
    navigateToTurnWithStatus,
    prepareTurnNavigation,
  }), [
    clearSearchMatch,
    isTurnRenderedInViewport,
    isTurnTextRenderedInViewport,
    navigateToTurn,
    navigateToTurnWithStatus,
    prepareTurnNavigation,
    scrollToIndex,
    scrollToLatestEndPosition,
    scrollToPhysicalBottom,
    scrollToSearchMatch,
    scrollToTurn,
    scrollToTurnEnd,
  ]);

  const visibleTurnInfo = useModernFlowChatStore(state => state.visibleTurnInfo);
  const handleJumpToCurrentTurn = useCallback(() => {
    if (visibleTurnInfo?.turnId) {
      navigateToTurn(visibleTurnInfo.turnId, { behavior: 'smooth' });
    }
  }, [navigateToTurn, visibleTurnInfo?.turnId]);
  const { shouldShowButton: shouldShowTurnHeaderButton, handleClick: handleTurnHeaderClick } =
    useScrollToTurnHeader({
      scrollerRef: scrollerElementRef,
      currentTurnId: visibleTurnInfo?.turnId ?? null,
      currentTurnIndex: visibleTurnInfo?.turnIndex ?? 0,
      visibleTurnInfo,
      onJumpToCurrentTurn: handleJumpToCurrentTurn,
    });
  const { visibleTaskInfo, scrollToTask } = useVisibleTaskInfo({
    scrollerRef: scrollerElementRef,
    virtualItems,
  });

  const previousHistoryBoundaryStatusNode = useMemo(() => (
    historyBoundaryState.before !== 'idle' ? (
      <FlowChatHistoryPagingSentinel
        state={historyBoundaryState.before}
        label={historyBoundaryState.before === 'error'
          ? t('historyState.olderHistoryNotReady')
          : t('historyState.preparingOlderHistory')}
      />
    ) : null
  ), [historyBoundaryState.before, t]);
  const nextHistoryBoundaryStatusNode = useMemo(() => (
    presentationMode === 'history-window' && historyBoundaryState.after !== 'idle' ? (
      <FlowChatHistoryPagingSentinel
        state={historyBoundaryState.after}
        label={t('historyState.loadingDescription')}
      />
    ) : null
  ), [historyBoundaryState.after, presentationMode, t]);
  const virtuosoContext = useMemo<FlowChatVirtuosoContext>(() => ({
    bottomLayoutInsetPx,
    previousHistoryBoundaryStatusNode,
    nextHistoryBoundaryStatusNode,
    runtimeStatusSessionId: activeSessionId,
  }), [activeSessionId, bottomLayoutInsetPx, nextHistoryBoundaryStatusNode, previousHistoryBoundaryStatusNode]);
  const computeVirtuosoItemKey = useCallback((_: number, item: VirtualItem) => (
    `${activeSessionId ?? 'no-active-session'}:${getVirtualItemStableKey(item)}`
  ), [activeSessionId]);
  const renderVirtuosoItem = useCallback((index: number, item: VirtualItem) => (
    <VirtualItemRenderer item={item} index={index - virtuosoFirstItemIndex} />
  ), [virtuosoFirstItemIndex]);

  if (virtualItems.length === 0) {
    return (
      <div
        data-bf-component="virtual-message-list"
        data-bf-part="root"
        data-bf-state="empty"
        className="virtual-message-list virtual-message-list--empty"
        data-testid="flowchat-message-list-empty"
      >
        <div className="empty-state" data-bf-component="virtual-message-list" data-bf-part="empty">
          <p data-bf-component="virtual-message-list" data-bf-part="emptyMessage">No messages yet</p>
        </div>
      </div>
    );
  }

  return (
    <div
      data-bf-component="virtual-message-list"
      data-bf-part="root"
      className="virtual-message-list"
      data-testid="flowchat-message-list"
      data-presentation-mode={presentationMode}
      data-viewport-mode={viewportMode}
      data-streaming-output={isStreamingOutput ? 'true' : 'false'}
    >
      <Virtuoso
        key={activeSessionId ?? 'no-active-session'}
        ref={virtuosoRef}
        data={virtualItems}
        firstItemIndex={virtuosoFirstItemIndex}
        initialTopMostItemIndex={{ index: Math.max(0, virtualItems.length - 1), align: 'end' }}
        computeItemKey={computeVirtuosoItemKey}
        itemContent={renderVirtuosoItem}
        followOutput={false}
        alignToBottom={false}
        overscan={FLOW_CHAT_VIRTUOSO_OVERSCAN}
        increaseViewportBy={FLOW_CHAT_VIRTUOSO_VIEWPORT_INCREASE}
        atBottomThreshold={50}
        atBottomStateChange={setIsAtBottom}
        rangeChanged={handleRangeChanged}
        scrollerRef={handleScrollerRef}
        context={virtuosoContext}
        components={FLOW_CHAT_VIRTUOSO_COMPONENTS}
      />

      <ScrollToTurnHeaderButton
        visible={shouldShowTurnHeaderButton}
        onClick={handleTurnHeaderClick}
        turnLabel={visibleTurnInfo ? `Turn ${visibleTurnInfo.turnIndex}` : undefined}
      />
      <StickyTaskIndicator
        visible={Boolean(visibleTaskInfo)}
        taskInfo={visibleTaskInfo}
        onClick={scrollToTask}
      />
      <ScrollToLatestBar
        visible={(viewportMode === 'history-reading' || !isAtBottom) && virtualItems.length > 0}
        onClick={viewportMode === 'history-reading' && onRequestJumpToLatest
          ? onRequestJumpToLatest
          : scrollToLatestEndPosition}
        isInputActive={isInputActive}
        isInputExpanded={isInputExpanded}
        inputHeight={inputHeight}
      />
    </div>
  );
});

VirtualMessageListSession.displayName = 'VirtualMessageListSession';

export const VirtualMessageList = forwardRef<VirtualMessageListRef, VirtualMessageListProps>((props, ref) => {
  const activeSession = useActiveSession();
  return <VirtualMessageListSession key={activeSession?.sessionId ?? 'no-active-session'} ref={ref} {...props} />;
});

VirtualMessageList.displayName = 'VirtualMessageList';
