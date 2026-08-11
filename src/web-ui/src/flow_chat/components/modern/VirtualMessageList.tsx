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
import { findRenderedTurnAnchorElement } from './flowChatViewportAnchor';
import { useFlowChatViewportAnchor } from './useFlowChatViewportAnchor';
import {
  contentEndScrollTop,
  FLOWCHAT_AT_CONTENT_END_THRESHOLD_PX as AT_CONTENT_END_THRESHOLD_PX,
  FLOWCHAT_TURN_TOP_GAP_PX,
  isViewportAtTail,
  tailSpacerPxForViewport,
  turnTopAlignmentEntersReservedBlank,
} from './flowChatTailFollow';
import { useFlowChatVirtualizer } from './useFlowChatVirtualizer';
import { historyBoundariesForVisibleRange } from './flowChatHistoryBoundary';
import { VirtualItemRenderer } from './VirtualItemRenderer';
import {
  estimateVirtualMessageItemHeight,
} from './virtualMessageListLayout';
import { resolveVisibleFlowChatTurnIds } from './flowChatVisibleTurns';
import { warnHistoryPagingRefusedWithPendingTurns } from '../../services/historySessionDiagnostics';
import './VirtualMessageList.scss';

const SEARCH_NAVIGATION_MAX_ATTEMPTS = 24;
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
 * change makes the virtualizer render a different number of them; either way it
 * re-measures over the following passes, so the content end keeps moving after
 * the first callback. The window closes on its own so that
 * streaming content growth, which arrives through the same observer, never
 * inherits it.
 */
const TAIL_REALIGN_RESIZE_CALLBACKS = 6;
const HISTORY_WINDOW_DIRECTIONS: readonly SessionHistoryWindowDirection[] = ['before', 'after'];
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

type PreparedTurnNavigation = {
  turnId: string;
  behavior: ScrollBehavior;
};

/**
 * The gap the first Turn sits below.
 *
 * Every other Turn is top-aligned to the same gap explicitly, so this height is
 * the shared constant rather than a style of its own. It is also what the
 * virtualizer's `scrollPaddingStart` reserves, so an aim that is re-taken while
 * items measure keeps the same gap.
 *
 * Anything else rendered here is above the items in the scroll range, which is
 * why the virtualizer measures this element rather than assuming its height.
 */
const FlowChatListHeader = forwardRef<HTMLDivElement, {
  previousHistoryBoundaryStatusNode: React.ReactNode;
}>(({ previousHistoryBoundaryStatusNode }, ref) => (
  <div ref={ref} className="message-list-header-block">
    <div
      className="message-list-header"
      data-bf-component="virtual-message-list"
      data-bf-part="header"
      style={{
        height: `${FLOWCHAT_TURN_TOP_GAP_PX}px`,
        minHeight: `${FLOWCHAT_TURN_TOP_GAP_PX}px`,
      }}
    />
    {previousHistoryBoundaryStatusNode}
  </div>
));
FlowChatListHeader.displayName = 'FlowChatListHeader';

const FlowChatListFooter = ({
  bottomLayoutInsetPx,
  tailSpacerPx,
  nextHistoryBoundaryStatusNode,
  runtimeStatusSessionId,
}: {
  bottomLayoutInsetPx: number;
  tailSpacerPx: number;
  nextHistoryBoundaryStatusNode: React.ReactNode;
  runtimeStatusSessionId: string | null;
}) => (
  <>
    <div
      className="message-list-footer"
      data-bf-component="virtual-message-list"
      data-bf-part="footer"
      style={{
        height: `${bottomLayoutInsetPx}px`,
        minHeight: `${bottomLayoutInsetPx}px`,
      }}
    >
      {nextHistoryBoundaryStatusNode}
      <RuntimeStatusSlot sessionId={runtimeStatusSessionId} placement="footer" />
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
        height: `${tailSpacerPx}px`,
        minHeight: `${tailSpacerPx}px`,
      }}
    />
  </>
);

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

/**
 * Whether a pointer press landed on the scroller's scrollbar rather than on the
 * transcript.
 *
 * `clientWidth` stops at the scrollbar, so anything past the content box's
 * trailing edge is the bar or its track. Measured on WebView2: a 10px gutter,
 * a press on the transcript at `clientX` 1497 against a content box ending at
 * 1641, and presses on the bar at 1643-1647.
 *
 * Chromium does dispatch `pointerdown` for a scrollbar press. WebKit-backed
 * builds draw overlay scrollbars that take no layout width, leaving no gutter
 * to test against, so this returns false there and the drag stays unnoticed.
 */
function isScrollbarPress(event: PointerEvent, scroller: HTMLElement): boolean {
  return event.clientX > scroller.getBoundingClientRect().left + scroller.clientWidth;
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
  /**
   * The newest Turn the session has, which is what "a new Turn" means.
   *
   * Deliberately a fact about the ledger and not about the projection.
   * `virtualItems.at(-1)` answers where the presentation currently *ends*, and
   * a history window re-cut moves that to a Turn which has existed for hours:
   * measured, navigating to Turn 2 landed correctly and was then overwritten
   * twice, because each window loaded on the way ended somewhere new and each
   * of those read as a submission, pinning the window's last Turn to the top.
   *
   * Whether the Turn can be *acted on* is a second question, and it belongs
   * with the response rather than the identity — qualifying the identity by
   * visibility instead makes a Turn that merely came into view look new, which
   * is the same bug wearing the opposite sign.
   */
  const latestTurnId = activeSession?.dialogTurns.at(-1)?.id ?? null;
  const scrollerElementRef = useRef<HTMLElement | null>(null);
  const headerElementRef = useRef<HTMLDivElement | null>(null);
  const [scrollerElement, setScrollerElement] = useState<HTMLElement | null>(null);
  const [viewportHeightPx, setViewportHeightPx] = useState(0);
  /** Last scroller box the resize observer saw, to tell it apart from a content change. */
  const observedViewportBoxRef = useRef({ width: 0, height: 0 });
  /** Remaining resize callbacks over which to keep a resting viewport at the end. */
  const tailRealignCallbacksRef = useRef(0);
  /** Synchronous mirror of `isAtBottom`, read by a resize for the pre-resize answer. */
  const isAtTailRef = useRef(true);
  /** A pointer is held on the scrollbar, so the scrolling it causes is intent. */
  const isScrollbarPressRef = useRef(false);
  const [isAtBottom, setIsAtBottom] = useState(true);
  const [isOpenViewportSettled, setIsOpenViewportSettled] = useState(false);
  const preparedTurnNavigationRef = useRef<PreparedTurnNavigation | null>(null);
  const boundaryRequestRef = useRef<Record<SessionHistoryWindowDirection, Promise<void> | null>>({
    before: null,
    after: null,
  });
  const exhaustedBoundaryRef = useRef<Record<SessionHistoryWindowDirection, boolean>>({
    before: false,
    after: false,
  });
  /**
   * Whether a boundary may be asked about again.
   *
   * Prepend compensation puts the viewport back on the reader's content, but
   * the virtualizer places its rows from a scroll offset it only refreshes on
   * the next frame, so for one commit the visible range is still read against
   * the head. Asking from that commit pages again and produces another one just
   * like it: measured, a single junction paged a transcript back to its first
   * Turn while the reader held still.
   *
   * A direction is armed by the visible range leaving it. react-virtuoso had
   * this for free — the range it reported was absolute, so a prepend moved the
   * local start index by the number of items added and the rule stopped
   * applying by itself.
   */
  const boundaryArmedRef = useRef<Record<SessionHistoryWindowDirection, boolean>>({
    before: true,
    after: true,
  });
  /** Assigned below, once the boundary evaluation it stands for exists. */
  const evaluateHistoryBoundariesRef = useRef<() => void>(() => {});
  const searchNavigationRequestIdRef = useRef(0);
  const visibleTurnUpdateFrameRef = useRef<number | null>(null);

  const virtualizer = useFlowChatVirtualizer({
    items: virtualItems,
    scrollerRef: scrollerElementRef,
    headerRef: headerElementRef,
    getItemKey: getVirtualItemStableKey,
    estimateItemHeightPx: estimateVirtualMessageItemHeight,
    scrollPaddingStartPx: FLOWCHAT_TURN_TOP_GAP_PX,
  });

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
   * `requestHistoryBoundary` and therefore the rendered-range effect many times
   * a second. Hold the session itself by reference — one assignment per render,
   * no allocation — and read the fields only on the rare diagnostic path.
   */
  const activeSessionRef = useRef(activeSession);
  activeSessionRef.current = activeSession;

  const isOpenViewportSettledRef = useRef(isOpenViewportSettled);
  isOpenViewportSettledRef.current = isOpenViewportSettled;
  const isOpeningViewport = useCallback(() => !isOpenViewportSettledRef.current, []);

  const getRenderedUserMessageElement = useCallback((turnId: string) => (
    findRenderedTurnAnchorElement(scrollerElementRef.current, turnId)
  ), []);

  const readContentEndScrollTop = useCallback((scroller: HTMLElement) => (
    contentEndScrollTop({
      scrollHeight: scroller.scrollHeight,
      clientHeight: scroller.clientHeight,
      tailSpacerPx: tailSpacerPxRef.current,
    })
  ), []);

  /**
   * The content-end scroll, issued *through the virtualizer*.
   *
   * Writing the scroller directly is cheaper, but it leaves any pending re-aim
   * in place — the virtualizer keeps chasing its last target for as long as the
   * measurements under it move. A caller correcting one of its own scrolls has
   * to replace that chase rather than outrun it, and only another scroll issued
   * through the virtualizer does.
   *
   * The target is read off live geometry rather than aligned to the last item,
   * because the end of *real content* is above the resident tail spacer and no
   * item knows where that is.
   */
  const scrollToContentEndThroughVirtualizer = useCallback((behavior: 'auto' | 'smooth') => {
    const scroller = scrollerElementRef.current;
    if (!scroller) return;
    virtualizer.scrollToOffset(readContentEndScrollTop(scroller), behavior);
  }, [readContentEndScrollTop, virtualizer]);

  const scrollToContentEnd = useCallback((behavior: ScrollBehavior) => {
    const scroller = scrollerElementRef.current;
    if (scroller) {
      scroller.scrollTo({ top: readContentEndScrollTop(scroller), behavior });
      return;
    }
    scrollToContentEndThroughVirtualizer(behavior === 'smooth' ? 'smooth' : 'auto');
  }, [readContentEndScrollTop, scrollToContentEndThroughVirtualizer]);

  const scrollTurnToTop = useCallback((turnId: string) => {
    const targetIndex = virtualItems.findIndex(item => (
      item.turnId === turnId && item.type === 'user-message'
    ));
    if (targetIndex < 0) return false;
    virtualizer.scrollItemIntoView(targetIndex, { align: 'start' });
    return true;
  }, [virtualItems, virtualizer]);

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
    isSnapBackInFlight,
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

  /*
   * A snap back counts, and it is the one that had to be measured to be
   * believed. Ownership passes to follow-output only once the animation lands,
   * so for its whole duration the viewport belongs to nobody and the anchor
   * treats the animation's own movement as a displacement: the snap travelled
   * 0.7px, the anchor wrote it back, the write cancelled the animation, the
   * cancellation read as a gesture coming to rest, and the snap was issued
   * again — 958 times over 20 seconds, arriving nowhere.
   */
  const isViewportOwnedElsewhere = useCallback(() => (
    isFollowingOutputRef.current || isOpeningViewport() || isSnapBackInFlight()
  ), [isOpeningViewport, isSnapBackInFlight]);

  const viewportAnchor = useFlowChatViewportAnchor({
    scrollerRef: scrollerElementRef,
    isViewportOwnedElsewhere,
  });

  /**
   * Keep the viewport on the same content when history is prepended.
   *
   * This is the half of react-virtuoso's `firstItemIndex` that nothing replaced.
   * Keying measurements on item identity means a prepend invalidates none of
   * them, which is the other half — but the scroll offset is still a number,
   * and items arriving above it push the reader's content down by their height
   * while it stays where it was.
   *
   * Everything downstream assumes this does not happen, and all three of the
   * failures measured here were that assumption breaking:
   *
   * - The virtualizer re-windows from its own scroll offset, which lags by a
   *   frame, so it renders the head. The paging rule reads that as the reader
   *   having arrived at the head and pages again, and again.
   * - The anchored Turn falls outside that window, so the anchor cannot find
   *   its element, drops the anchor and corrects nothing — measured, 655px of
   *   history arrived and `scrollTop` held at 23.
   * - With the reader left at the head, the boundary never re-arms.
   *
   * The amount is a delta and not a total: the height of exactly the items that
   * arrived above, read from the virtualizer's own placement of the item that
   * used to be first. Their heights are estimates until they measure, so this
   * lands close rather than exactly, and the anchor — which can now find its
   * Turn — takes it the rest of the way.
   */
  const firstItemKeyRef = useRef<string | null>(null);
  useLayoutEffect(() => {
    const previousFirstKey = firstItemKeyRef.current;
    const nextFirstKey = virtualItems[0] ? getVirtualItemStableKey(virtualItems[0]) : null;
    firstItemKeyRef.current = nextFirstKey;

    const scroller = scrollerElementRef.current;
    // Follow-output re-asserts its own target every frame; a second writer in
    // the same commit would only be overwritten by it.
    if (!scroller || isFollowingOutputRef.current) return;
    if (previousFirstKey === null || previousFirstKey === nextFirstKey) return;
    // Absent means the head was trimmed rather than extended, and there is no
    // prepended height to account for.
    const movedTo = virtualItems.findIndex(
      item => getVirtualItemStableKey(item) === previousFirstKey,
    );
    if (movedTo <= 0) return;
    const arrived = virtualizer.getItemBounds(movedTo);
    const head = virtualizer.getItemBounds(0);
    if (!arrived || !head) return;
    const prependedPx = arrived.startPx - head.startPx;
    if (prependedPx <= 0) return;
    scroller.scrollTop += prependedPx;
  }, [presentationMode, virtualItems, virtualizer]);

  useLayoutEffect(() => {
    viewportAnchor.openSettleWindow();
  }, [viewportAnchor, virtualItems]);

  const notifyUserScrollIntent = useCallback(() => {
    viewportAnchor.markUserScrollIntent();
    handleUserScrollIntent();
    onUserScrollIntent?.();
  }, [handleUserScrollIntent, onUserScrollIntent, viewportAnchor]);

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
       * Geometry stability is not a settle signal on its own: before the virtualizer
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
      const handleScrollEnd = () => {
        handleScrollSettled();
      };
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
      /*
       * A scroll under a scrollbar press is the one case where a plain scroll
       * event does carry intent — the press is what qualifies it. Left
       * unqualified, a drag never released the viewport: follow-output kept
       * writing its target every frame against the thumb (measured: a 100px
       * oscillation, every frame, for as long as the drag lasted), and a drag
       * that came to rest in the reserved blank was skipped by the snap back
       * because follow still nominally owned it.
       */
      if (isScrollbarPressRef.current) notifyUserScrollIntent();
      updateIsAtBottom();
      handleScroll();
      /*
       * Re-anchor to wherever the user has arrived, and only there. The rule
       * for "was this scroll the user's" lives with the anchor.
       */
      viewportAnchor.captureAnchorForScroll();
      scheduleVisibleTurnInfoUpdate();
      /*
       * Paging is a question about where the reader is, so a scroll is its
       * primary input. Through a ref: this listener must not be torn down and
       * re-attached every time the transcript changes.
       */
      evaluateHistoryBoundariesRef.current();
    };
    const handleWheel = () => notifyUserScrollIntent();
    const handleTouchMove = () => notifyUserScrollIntent();
    const handleKeyDown = (event: KeyboardEvent) => {
      if (['ArrowUp', 'ArrowDown', 'PageUp', 'PageDown', 'Home', 'End', ' '].includes(event.key)) {
        notifyUserScrollIntent();
      }
    };
    /*
     * Arming rather than releasing outright: `scrollbar-gutter: stable` keeps
     * the gutter reserved whether or not a bar is drawn in it, so a press there
     * is only intent once something actually scrolls. A click on the thumb that
     * moves nothing leaves the viewport where it was.
     */
    const handlePointerDown = (event: PointerEvent) => {
      isScrollbarPressRef.current = isScrollbarPress(event, scrollerElement);
    };
    const handlePointerRelease = () => {
      isScrollbarPressRef.current = false;
    };
    scrollerElement.addEventListener('scroll', handleNativeScroll, { passive: true });
    scrollerElement.addEventListener('wheel', handleWheel, { passive: true });
    scrollerElement.addEventListener('touchmove', handleTouchMove, { passive: true });
    scrollerElement.addEventListener('keydown', handleKeyDown);
    scrollerElement.addEventListener('pointerdown', handlePointerDown, { passive: true });
    // On the window: a drag can be released anywhere, including outside it.
    window.addEventListener('pointerup', handlePointerRelease, { passive: true });
    window.addEventListener('pointercancel', handlePointerRelease, { passive: true });
    return () => {
      scrollerElement.removeEventListener('scroll', handleNativeScroll);
      scrollerElement.removeEventListener('wheel', handleWheel);
      scrollerElement.removeEventListener('touchmove', handleTouchMove);
      scrollerElement.removeEventListener('keydown', handleKeyDown);
      scrollerElement.removeEventListener('pointerdown', handlePointerDown);
      window.removeEventListener('pointerup', handlePointerRelease);
      window.removeEventListener('pointercancel', handlePointerRelease);
    };
  }, [
    handleScroll,
    notifyUserScrollIntent,
    scheduleVisibleTurnInfoUpdate,
    scrollerElement,
    updateIsAtBottom,
    viewportAnchor,
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

      /*
       * Before paint, and ahead of everything below: this observer is the one
       * callback the browser delivers between the transcript changing height
       * and the frame being painted, which is the only place a correction can
       * still be invisible. A viewport box change is not a content shift, so a
       * genuine resize re-anchors instead of correcting.
       */
      if (viewportBoxChanged) {
        viewportAnchor.captureAnchor();
      } else {
        viewportAnchor.openSettleWindow();
      }

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
    /*
     * The virtualizer's own first child is a viewport-sized box — it stays at
     * the scroller's height no matter how much transcript there is, so
     * observing it never reported a content change at all. The item list is
     * the element that grows, and its padding is where the virtualizer parks
     * item space, which the content box does not include.
     */
    const content = scrollerElement.querySelector('[data-testid="flowchat-item-list"]')
      ?? scrollerElement.firstElementChild;
    if (content) observer.observe(content, { box: 'border-box' });
    observer.observe(scrollerElement);
    return () => observer.disconnect();
  }, [
    handleViewportResize,
    scheduleFollowToLatest,
    scheduleVisibleTurnInfoUpdate,
    scrollerElement,
    updateIsAtBottom,
    viewportAnchor,
  ]);

  /**
   * Top-align a Turn, without scrolling into the reserved blank to do it.
   *
   * The branch is on what is *knowable*, not on where the Turn is. A rendered
   * Turn has a resolvable offset, so the clamp is decided before anything
   * moves and the requested behaviour survives. An unrendered one is known
   * only to the virtualizer, so it is placed instantly and the answer read back —
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
    if (targetIndex < 0) return 'rejected';
    exitFollowOutput('scroll-to-turn');

    const behavior = options?.behavior === 'smooth' ? 'smooth' : 'auto';
    const alignTurnToTop = () => virtualizer.scrollItemIntoView(targetIndex, {
      align: 'start',
      behavior,
    });
    const scroller = scrollerElementRef.current;
    const renderedTurnTopScrollTop = resolveTurnTopScrollTop(turnId);

    if (scroller && renderedTurnTopScrollTop !== null) {
      if (turnTopAlignmentEntersReservedBlank({
        turnTopScrollTop: renderedTurnTopScrollTop,
        contentEndScrollTop: readContentEndScrollTop(scroller),
      })) {
        scrollToContentEndThroughVirtualizer(behavior);
      } else {
        alignTurnToTop();
      }
      return 'settled';
    }

    // Placed instantly on purpose: an animated scroll has not arrived yet, so
    // there would be nothing to read back. The requested behaviour is spent on
    // the placement, which is what turn-rail navigation already asks for.
    virtualizer.scrollItemIntoView(targetIndex, { align: 'start' });
    if (scroller && turnTopAlignmentEntersReservedBlank({
      turnTopScrollTop: scroller.scrollTop,
      // Re-read: placing the viewport renders items, which re-measures the
      // transcript and moves the content end with it.
      contentEndScrollTop: readContentEndScrollTop(scroller),
    })) {
      scrollToContentEndThroughVirtualizer('auto');
    }
    return 'settled';
  }, [
    exitFollowOutput,
    readContentEndScrollTop,
    resolveTurnTopScrollTop,
    scrollToContentEndThroughVirtualizer,
    virtualItems,
    virtualizer,
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
    if (index < 0 || index >= virtualItems.length) return;
    exitFollowOutput('scroll-to-index');
    virtualizer.scrollItemIntoView(index, { align: 'center' });
  }, [exitFollowOutput, virtualItems.length, virtualizer]);

  const scrollToTurnEnd = useCallback((turnId: string) => {
    let targetIndex = -1;
    for (let index = virtualItems.length - 1; index >= 0; index -= 1) {
      if (virtualItems[index]?.turnId === turnId) {
        targetIndex = index;
        break;
      }
    }
    const scroller = scrollerElementRef.current;
    const bounds = targetIndex < 0 ? null : virtualizer.getItemBounds(targetIndex);
    if (!bounds || !scroller) return false;
    // Deliberately does not exit follow-output. This is the session-open
    // placement, which wants the same position the tail follow is settling on;
    // releasing ownership here strands the viewport wherever this one early
    // shot landed, before item measurement and history paging have finished.
    //
    // Aligned by hand rather than by asking for the last item's end: the
    // virtualizer's own end alignment runs to the bottom of the scroll range,
    // and the resident tail spacer lives down there.
    virtualizer.scrollToOffset(bounds.endPx - scroller.clientHeight);
    return true;
  }, [virtualItems, virtualizer]);

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
    virtualizer.scrollItemIntoView(target.virtualItemIndex, { align: 'center' });
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
  }, [clearSearchMatch, exitFollowOutput, virtualizer]);

  useEffect(() => () => setFlowChatSearchHighlight(null), []);

  const requestHistoryBoundary = useCallback((direction: SessionHistoryWindowDirection) => {
    /*
     * Two refusals, both asking whether the ask describes the reader.
     *
     * While the follow rule owns the viewport, the position the ask was derived
     * from is our own placement — as true of a history window being opened as
     * of the live tail, so the test is ownership and not which presentation is
     * on screen. Ownership ends the moment the reader scrolls, which is exactly
     * when the ask starts meaning something. Measured on session open: five
     * pages landed in 890ms, each one displacing the viewport and so requesting
     * the next, until history ran out.
     *
     * The second is the arming latch; see `boundaryArmedRef`.
     */
    if (isFollowingOutputRef.current) return;
    if (!boundaryArmedRef.current[direction]) return;
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
    // Disarmed for as long as this page is the reason the window sits at the
    // boundary. Only the window moving off it arms the direction again.
    boundaryArmedRef.current[direction] = false;
    const request = Promise.resolve(onHistoryWindowBoundaryIntent(direction)).then(
      normalizeBoundaryResult,
    ).then(result => {
      if (result === 'exhausted') {
        exhaustedBoundaryRef.current[direction] = true;
      } else if (result === 'applied') {
        exhaustedBoundaryRef.current[direction] = false;
      }
      // Nothing was prepended, so the window sitting at the boundary is still
      // the reader's own position rather than a consequence of this page.
      if (result !== 'applied') {
        boundaryArmedRef.current[direction] = true;
      }
    }).finally(() => {
      boundaryRequestRef.current[direction] = null;
    });
    boundaryRequestRef.current[direction] = request;
  }, [onHistoryWindowBoundaryIntent]);

  /**
   * Whether either end of the loaded transcript is on screen, and act on it.
   *
   * Reads live geometry rather than the rendered window. The window carries
   * overscan and, for a transcript short enough to render whole, reports the
   * first and last item as present wherever the viewport is — which makes it
   * mute on the only question being asked. Measured: a 21-item transcript
   * rendered 21 rows from index 0 no matter where the reader stood, so the
   * head boundary read as reached forever and the tail never re-armed.
   *
   * A scroll moves the viewport across the window without changing it, so this
   * has to be a callback that both the scroll handler and the item-change
   * effect can invoke, rather than a value either of them derives.
   */
  const evaluateHistoryBoundaries = useCallback(() => {
    const range = virtualizer.getVisibleItemRange();
    if (!range) return;
    const atBoundary = new Set(historyBoundariesForVisibleRange(
      range,
      virtualItems.length,
      presentationMode,
    ));
    for (const direction of HISTORY_WINDOW_DIRECTIONS) {
      // Off the boundary: whatever the last page added has been absorbed, and
      // arriving there again will be the reader's own doing.
      if (!atBoundary.has(direction)) {
        boundaryArmedRef.current[direction] = true;
        continue;
      }
      requestHistoryBoundary(direction);
    }
  }, [presentationMode, requestHistoryBoundary, virtualItems.length, virtualizer]);

  evaluateHistoryBoundariesRef.current = evaluateHistoryBoundaries;

  useEffect(() => {
    scheduleVisibleTurnInfoUpdate();
    evaluateHistoryBoundaries();
  }, [
    evaluateHistoryBoundaries,
    /*
     * Follow-output releasing the viewport is a reason to ask again, not only a
     * reason the last ask was declined.
     */
    isFollowingOutput,
    scheduleVisibleTurnInfoUpdate,
    virtualizer.rows,
  ]);

  useLayoutEffect(() => {
    scheduleVisibleTurnInfoUpdate();
  }, [scheduleVisibleTurnInfoUpdate, virtualItems]);

  useEffect(() => {
    if (userMessageItems.length === 0) {
      useModernFlowChatStore.getState().setVisibleTurnInfo(null);
    }
  }, [userMessageItems.length]);

  const handleScrollerRef = useCallback((element: HTMLElement | null) => {
    const scroller = element;
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
      <div
        ref={handleScrollerRef}
        className="virtual-message-list__scroller"
        data-flowchat-scroller="true"
        data-testid="flowchat-scroller"
      >
        <FlowChatListHeader
          ref={headerElementRef}
          previousHistoryBoundaryStatusNode={previousHistoryBoundaryStatusNode}
        />
        {/*
          The window of rendered items, with the rest of the transcript standing
          in as padding above and below. Items stay in normal flow so that one
          growing reflows the ones under it in the same layout pass.
        */}
        <div
          className="virtual-message-list__items"
          data-testid="flowchat-item-list"
          style={{
            paddingTop: `${virtualizer.paddingTopPx}px`,
            paddingBottom: `${virtualizer.paddingBottomPx}px`,
          }}
        >
          {virtualizer.rows.map(row => (
            <VirtualItemRenderer
              key={row.key}
              item={virtualItems[row.index]}
              index={row.index}
              measureRef={virtualizer.measureRowElement}
            />
          ))}
        </div>
        <FlowChatListFooter
          bottomLayoutInsetPx={bottomLayoutInsetPx}
          tailSpacerPx={tailSpacerPx}
          nextHistoryBoundaryStatusNode={nextHistoryBoundaryStatusNode}
          runtimeStatusSessionId={activeSessionId}
        />
      </div>

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
