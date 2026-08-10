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
import {
  contentEndScrollTop,
  endAlignedTailOffsetPx,
  FLOWCHAT_AT_CONTENT_END_THRESHOLD_PX as AT_CONTENT_END_THRESHOLD_PX,
  FLOWCHAT_TURN_TOP_GAP_PX,
  isViewportAtTail,
  tailSpacerPxForViewport,
  turnTopAlignmentEntersReservedBlank,
} from './flowChatTailFollow';
import { VirtualItemRenderer } from './VirtualItemRenderer';
import { getLeadingVirtualItemIndexDelta } from './virtualMessageListLayout';
import { resolveVisibleFlowChatTurnIds } from './flowChatVisibleTurns';
import {
  recordHistoryPagingEvent,
  warnHistoryPagingRefusedWithPendingTurns,
} from '../../services/historySessionDiagnostics';
import './VirtualMessageList.scss';

const VIRTUOSO_FIRST_ITEM_INDEX_BASE = 1_000_000;
const SEARCH_NAVIGATION_MAX_ATTEMPTS = 24;
const FLOW_CHAT_VIRTUOSO_OVERSCAN = { main: 600, reverse: 600 } as const;
/** Consecutive quiet frames that mark the opening viewport as settled. */
const OPEN_REVEAL_QUIET_FRAMES = 2;
/** Hard cap so the transcript is always revealed, settled or not. */
const OPEN_REVEAL_MAX_FRAMES = 40;
/**
 * Quiet period after the last scroll event that stands in for `scrollend`.
 *
 * Only used where the event is missing. It has to outlast the gap between
 * frames of a momentum scroll without making the snap back feel detached from
 * the gesture that caused it.
 */
const SCROLL_SETTLE_FALLBACK_MS = 140;
/**
 * Resize callbacks over which a viewport resting at the end is re-aligned after
 * the scroller's own box changes.
 *
 * One correction is not enough. A width change reflows every item, and a height
 * change makes Virtuoso render a different number of them; either way it
 * re-measures and re-estimates over the following passes, so the content end
 * keeps moving after the first callback. The window closes on its own so that
 * streaming content growth, which arrives through the same observer, never
 * inherits it.
 */
const TAIL_REALIGN_RESIZE_CALLBACKS = 6;
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
  tailSpacerPx: number;
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
    {/*
      The gap the first Turn sits below. Every other Turn is top-aligned to the
      same gap explicitly, so this height is the shared constant rather than a
      style of its own.
    */}
    <div
      className="message-list-header"
      data-bf-component="virtual-message-list"
      data-bf-part="header"
      style={{
        height: `${FLOWCHAT_TURN_TOP_GAP_PX}px`,
        minHeight: `${FLOWCHAT_TURN_TOP_GAP_PX}px`,
      }}
    />
    {context.previousHistoryBoundaryStatusNode}
  </>
);

const FlowChatVirtuosoFooter = ({ context }: ContextProp<FlowChatVirtuosoContext>) => (
  <>
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
    {/*
      Resident tail reservation of roughly one viewport. Its height tracks the
      viewport and nothing else — it must never react to a measured content
      change, or it becomes the compensation scheme this replaced.
    */}
    <div
      className="message-list-tail-spacer"
      data-bf-component="virtual-message-list"
      data-bf-part="tailSpacer"
      aria-hidden="true"
      style={{
        height: `${context.tailSpacerPx}px`,
        minHeight: `${context.tailSpacerPx}px`,
      }}
    />
  </>
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
  const [viewportHeightPx, setViewportHeightPx] = useState(0);
  /** Last scroller box the resize observer saw, to tell it apart from a content change. */
  const observedViewportBoxRef = useRef({ width: 0, height: 0 });
  /** Remaining resize callbacks over which to keep a resting viewport at the end. */
  const tailRealignCallbacksRef = useRef(0);
  /** Synchronous mirror of `isAtBottom`, read by a resize for the pre-resize answer. */
  const isAtTailRef = useRef(true);
  const [isAtBottom, setIsAtBottom] = useState(true);
  const [isOpenViewportSettled, setIsOpenViewportSettled] = useState(false);
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
    const leadingDelta = getLeadingVirtualItemIndexDelta(
      virtuosoIndexState.virtualItems,
      virtualItems,
      getVirtualItemStableKey,
    );
    virtuosoIndexState.firstItemIndex = Math.max(
      0,
      virtuosoIndexState.firstItemIndex + leadingDelta,
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

  const tailSpacerPx = tailSpacerPxForViewport(viewportHeightPx, bottomLayoutInsetPx);
  const tailSpacerPxRef = useRef(tailSpacerPx);
  useLayoutEffect(() => {
    tailSpacerPxRef.current = tailSpacerPx;
  }, [tailSpacerPx]);
  const getTailSpacerPx = useCallback(() => tailSpacerPxRef.current, []);

  /*
   * The paging diagnostics need a few session facts, but `activeSession` is a
   * fresh object on every streaming flush: depending on it would rebuild
   * `requestHistoryBoundary` and therefore Virtuoso's `rangeChanged` many times
   * a second. Hold the session itself by reference — one assignment per render,
   * no allocation — and read the fields only on the rare diagnostic path.
   */
  const activeSessionRef = useRef(activeSession);
  activeSessionRef.current = activeSession;

  const isOpenViewportSettledRef = useRef(isOpenViewportSettled);
  isOpenViewportSettledRef.current = isOpenViewportSettled;
  const isOpeningViewport = useCallback(() => !isOpenViewportSettledRef.current, []);

  const getRenderedUserMessageElement = useCallback((turnId: string) => (
    Array.from(
      scrollerElementRef.current?.querySelectorAll<HTMLElement>(
        '.virtual-item-wrapper[data-item-type="user-message"]',
      ) ?? [],
    ).find(element => element.dataset.turnId === turnId) ?? null
  ), []);

  const readContentEndScrollTop = useCallback((scroller: HTMLElement) => (
    contentEndScrollTop({
      scrollHeight: scroller.scrollHeight,
      clientHeight: scroller.clientHeight,
      tailSpacerPx: tailSpacerPxRef.current,
    })
  ), []);

  /**
   * The content-end scroll, issued *through Virtuoso*.
   *
   * Writing the scroller directly is cheaper, but it leaves any pending
   * `scrollToIndex` retry in place — Virtuoso re-aims at its original target
   * for up to a second after the list changes under it. A caller correcting
   * one of its own `scrollToIndex` calls has to replace that retry rather than
   * outrun it, and only another `scrollToIndex` does.
   */
  const scrollVirtuosoToContentEnd = useCallback((behavior: 'auto' | 'smooth') => {
    const lastIndex = Math.max(0, virtualItems.length - 1);
    virtuosoRef.current?.scrollToIndex({
      index: lastIndex,
      align: 'end',
      offset: endAlignedTailOffsetPx(lastIndex, virtualItems.length, tailSpacerPxRef.current),
      behavior,
    });
  }, [virtualItems.length]);

  const scrollToContentEnd = useCallback((behavior: ScrollBehavior) => {
    const scroller = scrollerElementRef.current;
    if (scroller) {
      scroller.scrollTo({ top: readContentEndScrollTop(scroller), behavior });
      return;
    }
    scrollVirtuosoToContentEnd(normalizeVirtuosoBehavior(behavior));
  }, [readContentEndScrollTop, scrollVirtuosoToContentEnd]);

  const scrollTurnToTop = useCallback((turnId: string) => {
    const targetIndex = virtualItems.findIndex(item => (
      item.turnId === turnId && item.type === 'user-message'
    ));
    if (targetIndex < 0 || !virtuosoRef.current) return false;
    virtuosoRef.current.scrollToIndex({
      index: targetIndex,
      align: 'start',
      offset: -FLOWCHAT_TURN_TOP_GAP_PX,
      behavior: 'auto',
    });
    return true;
  }, [virtualItems]);

  // Must agree with `scrollTurnToTop` down to the pixel: this is the offset the
  // follow loop re-asserts every frame, so a disagreement is a fight.
  const resolveTurnTopScrollTop = useCallback((turnId: string) => {
    const scroller = scrollerElementRef.current;
    const element = getRenderedUserMessageElement(turnId);
    if (!scroller || !element) return null;
    return Math.max(
      0,
      scroller.scrollTop
        + element.getBoundingClientRect().top
        - scroller.getBoundingClientRect().top
        - FLOWCHAT_TURN_TOP_GAP_PX,
    );
  }, [getRenderedUserMessageElement]);

  const {
    isFollowingOutput,
    enterFollowOutput,
    exitFollowOutput,
    scheduleFollowToLatest,
    handleUserScrollIntent,
    handleScroll,
    handleScrollSettled,
    handleViewportResize,
    getFollowTargetScrollTop,
  } = useFlowChatFollowOutput({
    activeSessionId: activeSessionId ?? undefined,
    latestTurnId,
    virtualItemCount: virtualItems.length,
    isStreaming: isStreamingOutput,
    isViewportActive,
    scrollerRef: scrollerElementRef,
    getTailSpacerPx,
    scrollToContentEnd,
    scrollTurnToTop,
    resolveTurnTopScrollTop,
    isOpeningViewport,
  });

  const isFollowingOutputRef = useRef(isFollowingOutput);
  isFollowingOutputRef.current = isFollowingOutput;

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


  /*
   * Opening reveal.
   *
   * A session mounts against an unmeasured transcript: for the first frames the
   * real content is a few hundred pixels of estimate, so the end of content
   * genuinely *is* the top, and the settle then walks the viewport down as items
   * measure and history pages in. Every step of that walk is correct and every
   * step is visible, which reads as a flash. Hold the transcript hidden — laid
   * out and measurable, just not painted — until it stops moving.
   */
  useLayoutEffect(() => {
    if (!scrollerElement || isOpenViewportSettled) return;

    let frame = 0;
    let quietFrames = 0;
    let rafId: number | null = null;
    const lastVirtualIndex = virtualItems.length - 1;

    const check = () => {
      frame += 1;
      /*
       * Geometry stability is not a settle signal on its own: before Virtuoso
       * renders anything, `scrollHeight` and the content end sit unchanged at
       * their unmeasured values, which is indistinguishable from having
       * finished. Require the last item to actually be rendered with its end
       * inside the viewport — that is the thing the reveal is waiting for.
       */
      const lastItem = scrollerElement.querySelector<HTMLElement>(
        `.virtual-item-wrapper[data-virtual-index="${lastVirtualIndex}"]`,
      );
      const contentEnd = readContentEndScrollTop(scrollerElement);
      const inPosition = Math.abs(scrollerElement.scrollTop - contentEnd) <= AT_CONTENT_END_THRESHOLD_PX;
      const tailVisible = lastItem !== null
        && lastItem.getBoundingClientRect().bottom
          <= scrollerElement.getBoundingClientRect().bottom + AT_CONTENT_END_THRESHOLD_PX;
      quietFrames = tailVisible && inPosition ? quietFrames + 1 : 0;

      if (quietFrames >= OPEN_REVEAL_QUIET_FRAMES || frame >= OPEN_REVEAL_MAX_FRAMES) {
        setIsOpenViewportSettled(true);
        return;
      }
      rafId = requestAnimationFrame(check);
    };
    rafId = requestAnimationFrame(check);

    return () => {
      if (rafId !== null) cancelAnimationFrame(rafId);
    };
  }, [isOpenViewportSettled, readContentEndScrollTop, scrollerElement, virtualItems.length]);

  /*
   * "At the bottom" is a band, not a point. Its upper edge is the end of real
   * content and its lower edge is whatever the follow rule owns, so a pinned
   * Turn and a held collapse gap both count as being at the end — neither is a
   * reason to offer a jump to the latest output. Below the band is reserved
   * blank, which is: without the lower edge, parking a viewport deep in the
   * spacer read as "at the bottom" and hid the only way back.
   */
  const updateIsAtBottom = useCallback(() => {
    const scroller = scrollerElementRef.current;
    if (!scroller) return;
    const contentEnd = readContentEndScrollTop(scroller);
    const atTail = isViewportAtTail({
      scrollTop: scroller.scrollTop,
      contentEndScrollTop: contentEnd,
      // Nothing owns an offset outside follow, so the band collapses onto the
      // content end.
      followTargetScrollTop: getFollowTargetScrollTop() ?? contentEnd,
      thresholdPx: AT_CONTENT_END_THRESHOLD_PX,
    });
    // Mirrored to a ref because a resize needs the answer from before it, and
    // the state update does not land until the next render.
    isAtTailRef.current = atTail;
    setIsAtBottom(atTail);
  }, [getFollowTargetScrollTop, readContentEndScrollTop]);

  /*
   * The band's lower edge is whatever the follow rule owns, so it moves when
   * ownership changes — and that can happen with the viewport perfectly still.
   * A snap back completes at rest by construction, and a jump to latest that
   * lands on a pin the viewport is already on writes nothing. Neither produces
   * a scroll event, so without this the affordance stays as the last scroll
   * left it: visible, over a viewport that is already at the tail, and inert
   * because clicking it has nothing left to do.
   */
  useEffect(() => {
    updateIsAtBottom();
  }, [isFollowingOutput, updateIsAtBottom]);

  /*
   * Snap out of the reserved blank once a gesture is over.
   *
   * `scrollend` is the accurate signal — it knows when momentum has actually
   * died, which a quiet timer can only approximate — but it is recent enough
   * that the WebKit-backed builds may not have it. Where it is missing, a quiet
   * period after the last scroll event stands in.
   */
  useEffect(() => {
    if (!scrollerElement) return;

    if ('onscrollend' in window) {
      const handleScrollEnd = () => handleScrollSettled();
      scrollerElement.addEventListener('scrollend', handleScrollEnd, { passive: true });
      return () => scrollerElement.removeEventListener('scrollend', handleScrollEnd);
    }

    let settleTimer: number | null = null;
    const handleScrollTick = () => {
      if (settleTimer !== null) window.clearTimeout(settleTimer);
      settleTimer = window.setTimeout(() => {
        settleTimer = null;
        handleScrollSettled();
      }, SCROLL_SETTLE_FALLBACK_MS);
    };
    scrollerElement.addEventListener('scroll', handleScrollTick, { passive: true });
    return () => {
      if (settleTimer !== null) window.clearTimeout(settleTimer);
      scrollerElement.removeEventListener('scroll', handleScrollTick);
    };
  }, [handleScrollSettled, scrollerElement]);

  useEffect(() => {
    if (!scrollerElement) return;
    const handleNativeScroll = () => {
      updateIsAtBottom();
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
  }, [
    handleScroll,
    notifyUserScrollIntent,
    scheduleVisibleTurnInfoUpdate,
    scrollerElement,
    updateIsAtBottom,
  ]);

  useEffect(() => {
    if (!scrollerElement) return;
    const observer = new ResizeObserver(() => {
      const nextViewportBox = {
        width: scrollerElement.clientWidth,
        height: scrollerElement.clientHeight,
      };
      /*
       * This observer watches the content too. Content growth moves the follow
       * target away from a resting viewport and can never strand it, so only a
       * change to the scroller's own box opens the re-alignment window — a
       * width change reflows the transcript, a height change moves the content
       * end directly, and both keep settling for a few callbacks afterwards.
       */
      const previousViewportBox = observedViewportBoxRef.current;
      const viewportBoxChanged = nextViewportBox.width !== previousViewportBox.width
        || nextViewportBox.height !== previousViewportBox.height;
      if (viewportBoxChanged) {
        observedViewportBoxRef.current = nextViewportBox;
        tailRealignCallbacksRef.current = TAIL_REALIGN_RESIZE_CALLBACKS;
      }
      setViewportHeightPx(nextViewportBox.height);

      if (tailRealignCallbacksRef.current > 0) {
        tailRealignCallbacksRef.current -= 1;
        handleViewportResize({
          // Non-zero only on the callback that carries the change itself; the
          // rest of the window is there for the reflow settling afterwards.
          viewportHeightDeltaPx: viewportBoxChanged
            ? nextViewportBox.height - previousViewportBox.height
            : 0,
          // The band check from before this resize, so this must run ahead of
          // `updateIsAtBottom` below.
          wasAtTail: isAtTailRef.current,
        });
      }

      scheduleFollowToLatest();
      scheduleVisibleTurnInfoUpdate();
      updateIsAtBottom();
    });
    const content = scrollerElement.firstElementChild;
    if (content) observer.observe(content);
    observer.observe(scrollerElement);
    return () => observer.disconnect();
  }, [
    handleViewportResize,
    scheduleFollowToLatest,
    scheduleVisibleTurnInfoUpdate,
    scrollerElement,
    updateIsAtBottom,
  ]);

  /**
   * Top-align a Turn, without scrolling into the reserved blank to do it.
   *
   * The branch is on what is *knowable*, not on where the Turn is. A rendered
   * Turn has a resolvable offset, so the clamp is decided before anything
   * moves and the requested behaviour survives. An unrendered one is known
   * only to Virtuoso, so it is placed instantly and the answer read back —
   * both writes land in the same task, so the correction costs a second scroll
   * but not a second visible movement.
   */
  const navigateToTurnWithStatus = useCallback((
    turnId: string,
    options?: TurnNavigationOptions,
  ): FlowChatTurnNavigationStatus => {
    const targetIndex = virtualItems.findIndex(item => (
      item.turnId === turnId && item.type === 'user-message'
    ));
    if (targetIndex < 0 || !virtuosoRef.current) return 'rejected';
    exitFollowOutput('scroll-to-turn');

    const behavior = normalizeVirtuosoBehavior(options?.behavior ?? 'auto');
    const alignTurnToTop = () => virtuosoRef.current?.scrollToIndex({
      index: targetIndex,
      align: 'start',
      offset: -FLOWCHAT_TURN_TOP_GAP_PX,
      behavior,
    });
    const scroller = scrollerElementRef.current;
    const renderedTurnTopScrollTop = resolveTurnTopScrollTop(turnId);

    if (scroller && renderedTurnTopScrollTop !== null) {
      if (turnTopAlignmentEntersReservedBlank({
        turnTopScrollTop: renderedTurnTopScrollTop,
        contentEndScrollTop: readContentEndScrollTop(scroller),
      })) {
        scrollVirtuosoToContentEnd(behavior);
      } else {
        alignTurnToTop();
      }
      return 'settled';
    }

    // Placed instantly on purpose: an animated scroll has not arrived yet, so
    // there would be nothing to read back. The requested behaviour is spent on
    // the placement, which is what turn-rail navigation already asks for.
    virtuosoRef.current.scrollToIndex({
      index: targetIndex,
      align: 'start',
      offset: -FLOWCHAT_TURN_TOP_GAP_PX,
      behavior: 'auto',
    });
    if (scroller && turnTopAlignmentEntersReservedBlank({
      turnTopScrollTop: scroller.scrollTop,
      // Re-read: placing the viewport renders items, which re-measures the
      // transcript and moves the content end with it.
      contentEndScrollTop: readContentEndScrollTop(scroller),
    })) {
      scrollVirtuosoToContentEnd('auto');
    }
    return 'settled';
  }, [
    exitFollowOutput,
    readContentEndScrollTop,
    resolveTurnTopScrollTop,
    scrollVirtuosoToContentEnd,
    virtualItems,
  ]);

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
    // Deliberately does not exit follow-output. This is the session-open
    // placement, which wants the same position the tail follow is settling on;
    // releasing ownership here strands the viewport wherever this one early
    // shot landed, before item measurement and history paging have finished.
    virtuosoRef.current.scrollToIndex({
      index: targetIndex,
      align: 'end',
      offset: endAlignedTailOffsetPx(targetIndex, virtualItems.length, tailSpacerPxRef.current),
      behavior: 'auto',
    });
    return true;
  }, [virtualItems]);

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
    // Preserving a pre-prepend anchor only makes sense while the user owns the
    // viewport. When follow-output owns it the follow target already defines
    // the position, and restoring the anchor fights it for exactly one frame —
    // which reads as a flash while a session pages in its history.
    if (isFollowingOutputRef.current) {
      historyPrependAnchorRef.current = null;
      return true;
    }
    const scrollerRect = scroller.getBoundingClientRect();
    const anchor = Array.from(
      scroller.querySelectorAll<HTMLElement>(
        '.virtual-item-wrapper[data-item-type="user-message"]',
      ),
    ).find(element => element.getBoundingClientRect().bottom > scrollerRect.top);
    if (!anchor?.dataset.turnId) {
      // Returning false cancels the load that was already fetched, so record
      // why the anchor could not be taken.
      const { sessionId } = activeSessionRef.current ?? {};
      if (sessionId) {
        recordHistoryPagingEvent(sessionId, 'anchor_capture_failed', {
          renderedUserMessages: scroller.querySelectorAll(
            '.virtual-item-wrapper[data-item-type="user-message"]',
          ).length,
          renderedItems: scroller.querySelectorAll('.virtual-item-wrapper').length,
          scrollTop: Math.round(scroller.scrollTop),
        });
      }
      return false;
    }
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
    const latchedExhausted = exhaustedBoundaryRef.current[direction];
    if (
      !onHistoryWindowBoundaryIntent ||
      boundaryRequestRef.current[direction] ||
      latchedExhausted
    ) {
      /*
       * The user has reached the head of the loaded window and we are declining
       * to fetch more. That is correct once history really is exhausted, and a
       * silent data loss when it is not — the boundary status stays idle either
       * way, so the transcript looks like it simply has no earlier Turns.
       */
      const session = activeSessionRef.current;
      if (latchedExhausted && session?.sessionId && session.isPartial === true) {
        warnHistoryPagingRefusedWithPendingTurns(session.sessionId, {
          direction,
          reason: 'latched-exhausted-while-partial',
          isPartial: true,
          latchedExhausted: true,
          loadedTurnCount: session.dialogTurns.length,
          totalTurnCount: session.totalTurnCount ?? 0,
        });
      }
      return;
    }
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
    if (scroller) {
      setViewportHeightPx(scroller.clientHeight);
      // Seed the box so the observer's first callback is not read as a resize.
      observedViewportBoxRef.current = {
        width: scroller.clientWidth,
        height: scroller.clientHeight,
      };
    }
  }, []);

  const scrollToPhysicalBottom = useCallback(() => {
    enterFollowOutput('jump-to-latest');
    updateIsAtBottom();
  }, [enterFollowOutput, updateIsAtBottom]);

  const scrollToLatestEndPosition = useCallback(() => {
    onUserScrollIntent?.();
    enterFollowOutput('jump-to-latest');
    // Entering follow can leave the viewport exactly where it is, which
    // produces no scroll event to recompute the band from.
    updateIsAtBottom();
  }, [enterFollowOutput, onUserScrollIntent, updateIsAtBottom]);

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
    tailSpacerPx,
    previousHistoryBoundaryStatusNode,
    nextHistoryBoundaryStatusNode,
    runtimeStatusSessionId: activeSessionId,
  }), [
    activeSessionId,
    bottomLayoutInsetPx,
    nextHistoryBoundaryStatusNode,
    previousHistoryBoundaryStatusNode,
    tailSpacerPx,
  ]);
  // Session open bottom-aligns the last item. Virtuoso reveals the whole footer
  // for that alignment, which now contains the tail spacer, so the spacer's
  // share is cancelled here. Virtuoso samples this alongside `footerHeight`
  // when it finally scrolls, so the two stay consistent.
  const lastItemIndex = Math.max(0, virtualItems.length - 1);
  const initialTopMostItemIndex = useMemo(() => {
    const value = {
      index: lastItemIndex,
      align: 'end' as const,
      offset: endAlignedTailOffsetPx(lastItemIndex, virtualItems.length, tailSpacerPx),
    };
    return value;
  }, [lastItemIndex, tailSpacerPx, virtualItems.length]);
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
      data-open-viewport-settled={isOpenViewportSettled ? 'true' : 'false'}
    >
      <Virtuoso
        key={activeSessionId ?? 'no-active-session'}
        ref={virtuosoRef}
        data={virtualItems}
        firstItemIndex={virtuosoFirstItemIndex}
        initialTopMostItemIndex={initialTopMostItemIndex}
        computeItemKey={computeVirtuosoItemKey}
        itemContent={renderVirtuosoItem}
        followOutput={false}
        alignToBottom={false}
        overscan={FLOW_CHAT_VIRTUOSO_OVERSCAN}
        increaseViewportBy={FLOW_CHAT_VIRTUOSO_VIEWPORT_INCREASE}
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
