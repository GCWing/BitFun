/**
 * Virtualized message list.
 * Renders a flattened DialogTurn stream (user messages + model rounds).
 *
 * Scroll policy (simplified):
 * - The list preserves the current viewport by default.
 * - A new turn first pins the latest user message near the top for reading.
 * - Follow mode starts explicitly via "jump to latest", or automatically once
 *   the latest turn's streaming output grows enough to consume the sticky tail space.
 * - User upward scroll intent exits follow and cancels any pending auto-follow arm.
 * - "Scroll to latest" bar appears whenever the list is not at bottom.
 */

import React, { useRef, useState, useCallback, useEffect, useLayoutEffect, forwardRef, useImperativeHandle } from 'react';
import {
  Virtuoso,
  VirtuosoHandle,
  type Components,
  type ContextProp,
} from 'react-virtuoso';
import { useTranslation } from 'react-i18next';
import { useActiveSessionState } from '../../hooks/useActiveSessionState';
import { VirtualItemRenderer } from './VirtualItemRenderer';
import { ScrollToLatestBar } from '../ScrollToLatestBar';
import { ScrollToTurnHeaderButton } from '../ScrollToTurnHeaderButton';
import { useScrollToTurnHeader } from '../../hooks/useScrollToTurnHeader';
import { useVisibleTaskInfo } from '../../hooks/useVisibleTaskInfo';
import { StickyTaskIndicator } from '../StickyTaskIndicator';
import { RuntimeStatusSlot } from './RuntimeStatusSlot';
import { ScrollAnchor } from './ScrollAnchor';
import { useFlowChatFollowOutput } from './useFlowChatFollowOutput';
import type { FlowChatPinTurnToTopMode } from '../../events/flowchatNavigation';
import { useVirtualItems, useActiveSession, useModernFlowChatStore, type VisibleTurnInfo, type VirtualItem } from '../../store/modernFlowChatStore';
import { useChatInputState } from '../../store/chatInputStateStore';
import {
  computeFlowChatInputStackFooterPx,
  FLOWCHAT_MESSAGE_TAIL_CLEARANCE_PX,
} from '../../utils/flowChatScrollLayout';
import {
  findDialogTurn,
  shouldUseStickyLatestPin,
} from '../../utils/flowChatTurnScrollPolicy';
import { flowChatStore } from '../../store/FlowChatStore';
import { startupTrace } from '@/shared/utils/startupTrace';
import { flowChatDiagnostics } from '@/infrastructure/diagnostics/flowChatDiagnostics';
import {
  estimateVirtualMessageItemHeight,
  getVirtualMessageDefaultItemHeight,
  INITIAL_HISTORY_RENDER_MIN_ESTIMATED_HEIGHT_PX,
  INITIAL_HISTORY_RENDER_MIN_TURN_COUNT,
  mapInitialHistoryExpansionScrollTop,
  selectInitialHistoryRenderWindow,
} from './virtualMessageListLayout';
import {
  activeSessionHistoryProjectionHandoff,
  type HistoryProjectionHandoffSnapshot,
} from './historyProjectionHandoff';
import {
  findElementWithDataValue,
  findFlowChatSearchTextRanges,
  getFlowChatSearchTextRoot,
  setFlowChatSearchHighlight,
} from './flowChatSearchDom';
import {
  areBottomReservationStatesEqual,
  clampPinReservationPxToViewport,
  COMPENSATION_EPSILON_PX,
  consumeBottomReservationForContentGrowth,
  createInitialBottomReservationState,
  ensureCollapseReservationForScrollTop,
  getCanceledUnsettledStickyPinGrowthPx,
  getReservationTotalPx,
  isTurnPinRequestIdentityCurrent,
  protectCurrentCollapseReservation,
  reconcileUnsignaledShrinkReservation,
  releasePinReservationForUserNavigation,
  resolveAutoCollapseAnchorScrollTop,
  resolveProvisionalStickyPinReservationPx,
  sanitizeBottomReservationState,
  settleCollapseReservationForPreservedViewport,
  settleRetainedCollapseReservationForAnchor,
  shouldBypassShrinkCompensationInTailFollow,
  shouldPreserveCollapseReservationAfterIntent,
  shouldClearExpiredProvisionalStickyPin,
  shouldSuppressFollowingTailNegativeScrollBy,
  shouldSyncPhysicalBottom,
  transferCollapseReservationToPin,
  type BottomReservationState,
  type PinBottomReservation,
} from './flowChatScrollStability';
import {
  canHandoffPinnedItemToTail,
  FlowChatViewportCoordinator,
  type FlowChatViewportRangeHost,
} from './FlowChatViewportCoordinator';
import {
  FLOWCHAT_AUTO_COLLAPSE_SETTLE_FRAMES,
  FLOWCHAT_COLLAPSE_DURATION_MS,
  FLOWCHAT_COLLAPSE_INTENT_TTL_MS,
} from './flowChatCollapseMotion';
import './VirtualMessageList.scss';

const PINNED_TURN_VIEWPORT_OFFSET_PX = 57; // Keep in sync with `.message-list-header`.
const TOUCH_SCROLL_INTENT_EXIT_THRESHOLD_PX = 6;
const USER_UPWARD_SCROLL_INTENT_WINDOW_MS = 800;
const LATEST_END_ANCHOR_STABILIZATION_MAX_ATTEMPTS = 120;
const LATEST_END_ANCHOR_STABILIZATION_MIN_ATTEMPTS = 12;
const LATEST_END_ANCHOR_STABLE_VISIBLE_FRAMES = 8;
const LATEST_END_ANCHOR_VISIBILITY_MARGIN_PX = 4;
const LATEST_END_ANCHOR_STABLE_EPSILON_PX = 1;
const LATEST_END_ANCHOR_STATIC_FAST_PATH_TOLERANCE_PX = 96;
const VIRTUOSO_FIRST_ITEM_INDEX_BASE = 1_000_000;
const PARTIAL_HISTORY_INITIAL_TAIL_TURN_BUDGET = 16;
const PARTIAL_HISTORY_FULL_PROJECTION_TOP_THRESHOLD_PX = 1200;
const HISTORY_PROJECTION_HANDOFF_MAX_DURATION_MS = 5000;
const SESSION_OPEN_HANDOFF_ITEM_BUDGET = 24;
const PREVIOUS_HISTORY_BOUNDARY_STATUS_DURATION_MS = 2500;
const SEARCH_NAVIGATION_MAX_ATTEMPTS = 24;
const COLLAPSE_INTENT_TTL_MS = FLOWCHAT_COLLAPSE_INTENT_TTL_MS;
const AUTO_COLLAPSE_SETTLE_FRAMES = FLOWCHAT_AUTO_COLLAPSE_SETTLE_FRAMES;
const STICKY_PIN_GROWTH_SETTLE_MS = 300;
const RETAINED_COLLAPSE_QUIET_SETTLE_MS = 120;
const RETAINED_COLLAPSE_QUIET_SETTLE_FRAMES = 2;
const RETAINED_COLLAPSE_RELEASE_QUIET_MS = COLLAPSE_INTENT_TTL_MS;

type FlowChatVirtuosoContext = {
  footerRef: React.RefCallback<HTMLDivElement>;
  previousHistoryBoundaryStatusNode: React.ReactNode;
  runtimeStatusSessionId: string | null;
};

const FlowChatVirtuosoHeader = ({ context }: ContextProp<FlowChatVirtuosoContext>) => (
  <>
    <div className="message-list-header" />
    {context.previousHistoryBoundaryStatusNode}
  </>
);

const FlowChatVirtuosoFooter = ({ context }: ContextProp<FlowChatVirtuosoContext>) => (
  <div
    ref={context.footerRef}
    className="message-list-footer"
  >
    <RuntimeStatusSlot sessionId={context.runtimeStatusSessionId} placement="footer" />
  </div>
);

const FLOW_CHAT_VIRTUOSO_COMPONENTS: Components<VirtualItem, FlowChatVirtuosoContext> = {
  Header: FlowChatVirtuosoHeader,
  Footer: FlowChatVirtuosoFooter,
};
const FLOW_CHAT_VIRTUOSO_OVERSCAN = { main: 600, reverse: 600 } as const;
const FLOW_CHAT_VIRTUOSO_VIEWPORT_INCREASE = { top: 600, bottom: 600 } as const;

type LatestEndAnchorResolveReason =
  | 'raf'
  | 'range-changed'
  | 'resize-observer'
  | 'transition-finish';

type InitialHistoryTransitionState = {
  key: string;
  sessionId: string;
  isPartial: boolean;
  contextRestoreState: string;
  usesInitialHistoryRenderBudget: boolean;
};

// Read `FLOWCHAT_SCROLL_STABILITY.md` before changing collapse compensation logic.

/**
 * Methods exposed by VirtualMessageList.
 */
export type FlowChatTurnPinRequestStatus = 'rejected' | 'pending' | 'settled';

export interface VirtualMessageListRef {
  scrollToTurn: (turnIndex: number) => void;
  scrollToIndex: (index: number) => void;
  // Materializes a virtual item, reveals collapsed search content, and centers the exact text range.
  scrollToSearchMatch: (target: {
    virtualItemIndex: number;
    query: string;
    flowItemId?: string;
    occurrenceIndex?: number;
    expandableIds?: readonly string[];
  }) => void;
  clearSearchMatch: () => void;
  // Clears pin reservation first, then scrolls to the physical bottom.
  scrollToPhysicalBottomAndClearPin: () => void;
  // Clears pin reservation first, then keeps the target turn visible near the natural tail.
  scrollToTurnEndAndClearPin: (turnId: string) => boolean;
  // Checks the current rendered DOM instead of the possibly stale visible-turn store.
  isTurnRenderedInViewport: (turnId: string) => boolean;
  // Checks whether the current rendered DOM has visible, readable text for the turn.
  isTurnTextRenderedInViewport: (turnId: string) => boolean;
  // Preserves any existing pin reservation and behaves like an End-key scroll.
  scrollToLatestEndPosition: () => void;
  // Aligns the target turn's user message to the viewport top.
  pinTurnToTop: (turnId: string, options?: { behavior?: ScrollBehavior; pinMode?: FlowChatPinTurnToTopMode }) => boolean;
  // Detailed status for callers that must distinguish immediate feedback from deferred virtual-list settling.
  pinTurnToTopWithStatus: (turnId: string, options?: { behavior?: ScrollBehavior; pinMode?: FlowChatPinTurnToTopMode }) => FlowChatTurnPinRequestStatus;
}

export interface VirtualMessageListProps {
  onUserScrollIntent?: () => void;
}

interface PendingCollapseIntentState {
  active: boolean;
  anchorScrollTop: number;
  toolId: string | null;
  toolName: string | null;
  expiresAtMs: number;
  distanceFromBottomBeforeCollapse: number;
  baseTotalCompensationPx: number;
  cumulativeShrinkPx: number;
}

interface RetainedCollapseAnchorState {
  anchorScrollTop: number;
  toolId: string | null;
  toolName: string | null;
}

function createInactiveCollapseIntentState(): PendingCollapseIntentState {
  return {
    active: false,
    anchorScrollTop: 0,
    toolId: null,
    toolName: null,
    expiresAtMs: 0,
    distanceFromBottomBeforeCollapse: 0,
    baseTotalCompensationPx: 0,
    cumulativeShrinkPx: 0,
  };
}

interface LatestEndAnchorRequestState {
  turnId: string;
  targetIndex: number;
  attempts: number;
  visibleFrames: number;
  stableVisibleFrames: number;
  lastScrollHeight: number | null;
  lastScrollTop: number | null;
  lastTargetTop: number | null;
  lastTargetBottom: number | null;
}

interface ScrollerGeometrySnapshot {
  scrollTop: number;
  scrollHeight: number;
  clientHeight: number;
}

interface PendingTurnPinState {
  generation: number;
  sessionId: string;
  turnId: string;
  behavior: ScrollBehavior;
  pinMode: FlowChatPinTurnToTopMode;
  expiresAtMs: number;
  attempts: number;
}

interface PendingStaticTurnPinState {
  turnId: string;
  behavior: ScrollBehavior;
}

function sanitizeReservationPx(value: number): number {
  return Number.isFinite(value) ? Math.max(0, value) : 0;
}

function isEditableElement(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) {
    return false;
  }

  return (
    target.isContentEditable ||
    target.closest('input, textarea, select, [contenteditable="true"]') !== null
  );
}

function isUpwardScrollIntentKey(event: KeyboardEvent): boolean {
  if (event.defaultPrevented || event.altKey || event.ctrlKey || event.metaKey) {
    return false;
  }

  return (
    event.key === 'ArrowUp' ||
    event.key === 'PageUp' ||
    event.key === 'Home' ||
    (event.key === ' ' && event.shiftKey)
  );
}

function isScrollIntentKey(event: KeyboardEvent): boolean {
  if (event.defaultPrevented || event.altKey || event.ctrlKey || event.metaKey) {
    return false;
  }

  return (
    event.key === 'ArrowUp' ||
    event.key === 'ArrowDown' ||
    event.key === 'PageUp' ||
    event.key === 'PageDown' ||
    event.key === 'Home' ||
    event.key === 'End' ||
    event.key === ' '
  );
}

function isPointerOnScrollbarGutter(
  scroller: HTMLElement,
  clientX: number,
  clientY: number,
): boolean {
  const rect = scroller.getBoundingClientRect();
  const verticalScrollbarWidth = Math.max(0, scroller.offsetWidth - scroller.clientWidth);
  const horizontalScrollbarHeight = Math.max(0, scroller.offsetHeight - scroller.clientHeight);

  const isWithinVerticalScrollbar = (
    verticalScrollbarWidth > 0 &&
    clientX >= rect.right - verticalScrollbarWidth &&
    clientX <= rect.right &&
    clientY >= rect.top &&
    clientY <= rect.bottom
  );

  const isWithinHorizontalScrollbar = (
    horizontalScrollbarHeight > 0 &&
    clientY >= rect.bottom - horizontalScrollbarHeight &&
    clientY <= rect.bottom &&
    clientX >= rect.left &&
    clientX <= rect.right
  );

  return isWithinVerticalScrollbar || isWithinHorizontalScrollbar;
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
      return `${item.type}:${item.turnId}`;
    case 'image-analyzing':
      return `${item.type}:${item.turnId}`;
  }
}

function getPrependedVirtualItemCount(previousItems: VirtualItem[], nextItems: VirtualItem[]): number {
  if (previousItems.length === 0 || nextItems.length <= previousItems.length) {
    return 0;
  }

  const prependedCount = nextItems.length - previousItems.length;
  for (let index = 0; index < previousItems.length; index += 1) {
    if (getVirtualItemStableKey(previousItems[index]) !== getVirtualItemStableKey(nextItems[prependedCount + index])) {
      return 0;
    }
  }

  return prependedCount;
}

const VirtualMessageListSession = forwardRef<VirtualMessageListRef, VirtualMessageListProps>(({
  onUserScrollIntent,
}, ref) => {
  const { t } = useTranslation('flow-chat');
  const virtuosoRef = useRef<VirtuosoHandle>(null);
  const virtualItems = useVirtualItems();
  const activeSession = useActiveSession();
  const virtuosoIndexStateRef = useRef<{
    sessionId: string | null;
    firstItemIndex: number;
    virtualItems: VirtualItem[];
  }>({
    sessionId: null,
    firstItemIndex: VIRTUOSO_FIRST_ITEM_INDEX_BASE,
    virtualItems: [],
  });
  const virtuosoIndexState = virtuosoIndexStateRef.current;
  const activeSessionId = activeSession?.sessionId ?? null;
  if (virtuosoIndexState.sessionId !== activeSessionId) {
    virtuosoIndexState.sessionId = activeSessionId;
    virtuosoIndexState.firstItemIndex = VIRTUOSO_FIRST_ITEM_INDEX_BASE;
    virtuosoIndexState.virtualItems = virtualItems;
  } else if (virtuosoIndexState.virtualItems !== virtualItems) {
    const prependedCount = getPrependedVirtualItemCount(virtuosoIndexState.virtualItems, virtualItems);
    if (prependedCount > 0) {
      virtuosoIndexState.firstItemIndex = Math.max(0, virtuosoIndexState.firstItemIndex - prependedCount);
    }
    virtuosoIndexState.virtualItems = virtualItems;
  }
  const virtuosoFirstItemIndex = virtuosoIndexState.firstItemIndex;
  const toVirtuosoIndex = useCallback((localIndex: number) => virtuosoFirstItemIndex + localIndex, [virtuosoFirstItemIndex]);

  const [isAtBottom, setIsAtBottom] = useState(true);
  const [scrollerElement, setScrollerElement] = useState<HTMLElement | null>(null);
  const [bottomReservationState, setBottomReservationState] = useState<BottomReservationState>(
    () => createInitialBottomReservationState()
  );
  const [pendingTurnPin, setPendingTurnPin] = useState<PendingTurnPinState | null>(null);
  const [historyProjectionHandoff, setHistoryProjectionHandoff] = useState<HistoryProjectionHandoffSnapshot | null>(null);
  const [expandedInitialHistoryRenderKey, setExpandedInitialHistoryRenderKey] = useState<string | null>(null);
  const [staticAnchorWindowTurnId, setStaticAnchorWindowTurnId] = useState<string | null>(null);
  const [previousHistoryBoundaryStatus, setPreviousHistoryBoundaryStatus] = useState<{
    sessionId: string;
    reason: string;
    state: 'preparing' | 'not-ready';
  } | null>(null);

  const scrollerElementRef = useRef<HTMLElement | null>(null);
  const footerElementRef = useRef<HTMLDivElement | null>(null);
  const activeSessionIdRef = useRef<string | null>(null);
  const historyProjectionHandoffRef = useRef<HistoryProjectionHandoffSnapshot | null>(null);
  const historyProjectionHandoffReleaseFrameRef = useRef<number | null>(null);
  const latestVisibleHistoryTailSnapshotRef = useRef<HistoryProjectionHandoffSnapshot | null>(null);
  const previousInitialHistoryTransitionStateRef = useRef<InitialHistoryTransitionState | null>(null);
  const fullHistoryProjectionIntentFrameRef = useRef<number | null>(null);
  const pendingFullHistoryProjectionReasonRef = useRef<string | null>(null);
  const previousHistoryBoundaryStatusTimerRef = useRef<number | null>(null);
  const sessionOpenHandoffSessionIdRef = useRef<string | null>(null);
  const previousActiveSessionIdForOpenHandoffRef = useRef<string | null | undefined>(undefined);
  const viewportCoordinatorRef = useRef(new FlowChatViewportCoordinator());
  const pendingStaticAnchorTurnIdRef = useRef<string | null>(null);
  const bottomReservationStateRef = useRef<BottomReservationState>(createInitialBottomReservationState());
  const restoreScrollerMethodsRef = useRef<(() => void) | null>(null);
  const previousMeasuredHeightRef = useRef<number | null>(null);
  const previousScrollTopRef = useRef(0);
  const previousScrollerGeometryRef = useRef<ScrollerGeometrySnapshot | null>(null);
  const markedInitialHistoryRenderWindowKeyRef = useRef<string | null>(null);
  const autoScrolledInitialHistoryRenderKeyRef = useRef<string | null>(null);
  const useStaticInitialHistoryListRef = useRef(false);
  const staticInitialHistoryUserLeftBottomRef = useRef(false);
  const pendingInitialHistoryExpansionRef = useRef<{
    scrollTop: number;
    scrollHeight: number;
    omittedEstimatedHeightPx: number;
    wasAtBottom: boolean;
  } | null>(null);
  const pendingStaticTurnPinRef = useRef<PendingStaticTurnPinState | null>(null);
  const pendingStaticLatestScrollBehaviorRef = useRef<('auto' | 'smooth') | null>(null);
  const initialHistoryRenderWindowCheckFrameRef = useRef<number | null>(null);
  const measureFrameRef = useRef<number | null>(null);
  const visibleTurnMeasureFrameRef = useRef<number | null>(null);
  const pinReservationReconcileFrameRef = useRef<number | null>(null);
  const turnPinStabilizationFrameRef = useRef<number | null>(null);
  const turnPinRequestGenerationRef = useRef(0);
  const latestEndAnchorStabilizationFrameRef = useRef<number | null>(null);
  const searchNavigationRequestIdRef = useRef(0);
  const staticInitialHistoryBottomGuardFrameRef = useRef<number | null>(null);
  const staticInitialHistoryBottomGuardUntilMsRef = useRef(0);
  const latestEndAnchorRequestRef = useRef<LatestEndAnchorRequestState | null>(null);
  const resolveLatestEndAnchorStabilizationRef = useRef<((reason: LatestEndAnchorResolveReason) => boolean) | null>(null);
  const resizeObserverRef = useRef<ResizeObserver | null>(null);
  const mutationObserverRef = useRef<MutationObserver | null>(null);
  const touchScrollIntentStartYRef = useRef<number | null>(null);
  const scrollbarPointerInteractionActiveRef = useRef(false);
  // Timestamp until which we treat any upward scroll as user-initiated. Set by
  // wheel/touch/keyboard/scrollbar handlers BEFORE the browser actually moves
  // the scroller. Used by the handleScroll "shrink-clamp restore" intercept
  // (below) to distinguish a genuine user upward scroll from a browser auto
  // clamp caused by content shrinking near the bottom in follow mode.
  const userInitiatedUpwardScrollUntilMsRef = useRef(0);
  const pendingCollapseIntentRef = useRef<PendingCollapseIntentState>(
    createInactiveCollapseIntentState(),
  );
  const retainedCollapseAnchorRef = useRef<RetainedCollapseAnchorState | null>(null);
  const collapseIntentFinalizeTimerRef = useRef<number | null>(null);
  const collapseIntentAnimationTimerRef = useRef<number | null>(null);
  const collapseIntentSettleFrameRef = useRef<number | null>(null);
  const retainedCollapseSettleTimerRef = useRef<number | null>(null);
  const retainedCollapseSettleFrameRef = useRef<number | null>(null);
  const retainedCollapseReleaseTimerRef = useRef<number | null>(null);
  const retainedCollapseSettleGenerationRef = useRef(0);
  const pendingStickyPinGrowthRef = useRef<{
    targetTurnId: string | null;
    amountPx: number;
  }>({
    targetTurnId: null,
    amountPx: 0,
  });
  const stickyPinGrowthSettleTimerRef = useRef<number | null>(null);
  const settlePendingStickyPinGrowthRef = useRef<(reason: string) => void>(() => {});
  const maybeHandoffPinnedTurnToTailRef = useRef<(reason: string) => boolean>(() => false);
  const finalizeCollapseIntentRef = useRef<(
    reason: string,
    options?: { expectedExpiresAtMs?: number; suppressHandoff?: boolean },
  ) => boolean>(() => false);
  const exitPinnedViewportForUserIntentRef = useRef<(reason: string) => void>((reason) => {
    viewportCoordinatorRef.current.release(reason);
  });
  const followOutputControllerRef = useRef<{
    handleUserScrollIntent: () => void;
    handleScroll: () => void;
    scheduleFollowToLatest: (reason: string) => void;
  }>({
    handleUserScrollIntent: () => {},
    handleScroll: () => {},
    scheduleFollowToLatest: () => {},
  });
  const deferredFollowReasonRef = useRef<string | null>(null);
  // Mirror of `isFollowingOutput` for use inside listeners that are registered
  // once per mount. When follow mode is active we deliberately bypass collapse
  // pre-compensation and semantic-anchor restoration so the continuous follow
  // loop can keep tracking the bottom without fighting layout stabilization.
  const isFollowingOutputRef = useRef(false);
  const isStreamingOutputRef = useRef(false);
  const previousIsStreamingOutputRef = useRef(false);
  const transientTurnPinStabilizationRef = useRef<PendingTurnPinState | null>(null);
  activeSessionIdRef.current = activeSessionId;

  const isInputActive = useChatInputState(state => state.isActive);
  const isInputExpanded = useChatInputState(state => state.isExpanded);
  const inputHeight = useChatInputState(state => state.inputHeight);

  const inputStackFooterPxRef = useRef(0);
  const previousInputStackFooterPxRef = useRef(0);
  // Snapshot scrollTop during render, before React commits the new (smaller)
  // footer height.  The browser has not yet clamped, so this is the accurate
  // pre-clamp value.  Used by the useLayoutEffect below to restore scrollTop
  // after footer-shrink compensation.
  const footerShrinkPreScrollTopRef = useRef(0);
  const inputStackFooterPx = computeFlowChatInputStackFooterPx(inputHeight, isInputActive);
  // Snapshot pre-commit scrollTop before we update the footer px ref.
  // The scrollerElement may not be set on first mount; that's fine —
  // useLayoutEffect checks scroller !== null before restoring.
  if (scrollerElement) {
    footerShrinkPreScrollTopRef.current = scrollerElement.scrollTop;
  }
  inputStackFooterPxRef.current = inputStackFooterPx;

  const activeSessionState = useActiveSessionState();
  const isProcessing = activeSessionState.isProcessing;

  const getFooterHeightPx = useCallback((compensationPx: number) => {
    return inputStackFooterPxRef.current + compensationPx;
  }, []);

  const getTotalBottomCompensationPx = useCallback((state: BottomReservationState = bottomReservationStateRef.current) => {
    return getReservationTotalPx(state.collapse) + getReservationTotalPx(state.pin);
  }, []);

  const clearRetainedCollapseSettlement = useCallback(() => {
    retainedCollapseSettleGenerationRef.current += 1;
    if (retainedCollapseSettleTimerRef.current !== null) {
      window.clearTimeout(retainedCollapseSettleTimerRef.current);
      retainedCollapseSettleTimerRef.current = null;
    }
    if (retainedCollapseSettleFrameRef.current !== null) {
      cancelAnimationFrame(retainedCollapseSettleFrameRef.current);
      retainedCollapseSettleFrameRef.current = null;
    }
    if (retainedCollapseReleaseTimerRef.current !== null) {
      window.clearTimeout(retainedCollapseReleaseTimerRef.current);
      retainedCollapseReleaseTimerRef.current = null;
    }
  }, []);

  const applyFooterHeightToElement = useCallback((footer: HTMLDivElement, compensationPx: number) => {
    const heightPx = getFooterHeightPx(compensationPx);
    footer.style.height = `${heightPx}px`;
    footer.style.minHeight = `${heightPx}px`;
  }, [getFooterHeightPx]);

  const handleFooterElementRef = useCallback((element: HTMLDivElement | null) => {
    footerElementRef.current = element;
    if (!element) {
      return;
    }

    // Virtuoso may replace its Footer during a measurement commit. Reapply
    // the ref-owned reservation synchronously so the physical scroll range
    // never falls back to a stale React render for one frame.
    applyFooterHeightToElement(element, getTotalBottomCompensationPx());
    void element.offsetHeight;
    void scrollerElementRef.current?.scrollHeight;
  }, [applyFooterHeightToElement, getTotalBottomCompensationPx]);

  const snapshotMeasuredContentHeight = useCallback((
    scroller: HTMLElement,
    reservationState: BottomReservationState = bottomReservationStateRef.current,
  ) => {
    const compensationPx = getTotalBottomCompensationPx(reservationState);
    return Math.max(0, scroller.scrollHeight - compensationPx - inputStackFooterPxRef.current);
  }, [getTotalBottomCompensationPx]);

  const recordScrollerGeometry = useCallback((scroller: HTMLElement) => {
    previousScrollerGeometryRef.current = {
      scrollTop: scroller.scrollTop,
      scrollHeight: scroller.scrollHeight,
      clientHeight: scroller.clientHeight,
    };
  }, []);

  const getEffectiveBottomScrollTop = useCallback((scroller: HTMLElement) => {
    return Math.max(
      0,
      scroller.scrollHeight - scroller.clientHeight - getTotalBottomCompensationPx(),
    );
  }, [getTotalBottomCompensationPx]);

  const isGeometryAtEffectiveBottom = useCallback((geometry: ScrollerGeometrySnapshot | null) => {
    if (!geometry) {
      return false;
    }

    const effectiveBottomScrollTop = Math.max(
      0,
      geometry.scrollHeight - geometry.clientHeight - getTotalBottomCompensationPx(),
    );
    return Math.abs(effectiveBottomScrollTop - geometry.scrollTop) <= LATEST_END_ANCHOR_STABLE_EPSILON_PX;
  }, [getTotalBottomCompensationPx]);

  const recordStaticInitialHistoryBottomState = useCallback((scroller: HTMLElement) => {
    if (!useStaticInitialHistoryListRef.current) {
      staticInitialHistoryUserLeftBottomRef.current = false;
      return;
    }

    const distanceFromBottom = Math.max(
      0,
      getEffectiveBottomScrollTop(scroller) - scroller.scrollTop,
    );
    staticInitialHistoryUserLeftBottomRef.current =
      distanceFromBottom > LATEST_END_ANCHOR_STABLE_EPSILON_PX;
  }, [getEffectiveBottomScrollTop]);

  const recordScrollerGeometryIfLayoutStable = useCallback((scroller: HTMLElement) => {
    const previousGeometry = previousScrollerGeometryRef.current;
    if (
      previousGeometry &&
      (
        Math.abs(scroller.clientHeight - previousGeometry.clientHeight) > COMPENSATION_EPSILON_PX ||
        Math.abs(scroller.scrollHeight - previousGeometry.scrollHeight) > COMPENSATION_EPSILON_PX
      )
    ) {
      return;
    }
    recordScrollerGeometry(scroller);
  }, [recordScrollerGeometry]);

  const notifyUserScrollIntent = useCallback((reason = 'user-scroll-intent') => {
    if (flowChatDiagnostics.isEnabled()) {
      flowChatDiagnostics.trace({
        hypothesis: 'A',
        location: 'VirtualMessageList.notifyUserScrollIntent',
        message: 'User scroll intent requested viewport ownership release',
        data: () => {
          const scroller = scrollerElementRef.current;
          return {
            reason,
            coordinatorMode: viewportCoordinatorRef.current.getMode(),
            isFollowingOutput: isFollowingOutputRef.current,
            isStreamingOutput: isStreamingOutputRef.current,
            reservation: bottomReservationStateRef.current,
            pendingCollapseIntent: pendingCollapseIntentRef.current.active,
            scrollTop: scroller?.scrollTop ?? null,
            scrollHeight: scroller?.scrollHeight ?? null,
            clientHeight: scroller?.clientHeight ?? null,
          };
        },
      });
    }
    clearRetainedCollapseSettlement();
    retainedCollapseAnchorRef.current = null;
    exitPinnedViewportForUserIntentRef.current(reason);
    followOutputControllerRef.current.handleUserScrollIntent();
    onUserScrollIntent?.();
  }, [clearRetainedCollapseSettlement, onUserScrollIntent]);

  const syncPhysicalBottomAfterViewportResize = useCallback((scroller: HTMLElement): boolean => {
    const previousGeometry = previousScrollerGeometryRef.current;
    if (!previousGeometry) {
      return false;
    }

    const viewportGeometryChanged =
      Math.abs(scroller.scrollHeight - previousGeometry.scrollHeight) > COMPENSATION_EPSILON_PX ||
      Math.abs(scroller.clientHeight - previousGeometry.clientHeight) > COMPENSATION_EPSILON_PX;
    if (
      !viewportGeometryChanged ||
      pendingCollapseIntentRef.current.active ||
      retainedCollapseAnchorRef.current !== null
    ) {
      return false;
    }

    const previousMaxScrollTop = Math.max(0, previousGeometry.scrollHeight - previousGeometry.clientHeight);
    const wasAtPhysicalBottom =
      Math.abs(previousMaxScrollTop - previousGeometry.scrollTop) <= LATEST_END_ANCHOR_STABLE_EPSILON_PX;
    if (!wasAtPhysicalBottom) {
      return false;
    }

    const maxScrollTop = Math.max(0, scroller.scrollHeight - scroller.clientHeight);
    const ownsElementAnchor = viewportCoordinatorRef.current.ownsElementAnchor();
    const willWrite = shouldSyncPhysicalBottom({
      viewportGeometryChanged,
      collapseProtectionActive: pendingCollapseIntentRef.current.active,
      wasAtPhysicalBottom,
      ownsElementAnchor,
    }) && Math.abs(maxScrollTop - scroller.scrollTop) > COMPENSATION_EPSILON_PX;
    if (ownsElementAnchor) {
      viewportCoordinatorRef.current.restoreElementAnchor(scroller, 'viewport-resize');
      previousScrollTopRef.current = scroller.scrollTop;
      recordScrollerGeometry(scroller);
      return true;
    }

    if (willWrite) {
      scroller.scrollTop = maxScrollTop;
      staticInitialHistoryUserLeftBottomRef.current = false;
    }
    previousScrollTopRef.current = scroller.scrollTop;
    previousMeasuredHeightRef.current = snapshotMeasuredContentHeight(scroller);
    recordScrollerGeometry(scroller);
    return true;
  }, [recordScrollerGeometry, snapshotMeasuredContentHeight]);

  const cancelStaticInitialHistoryBottomGuard = useCallback(() => {
    staticInitialHistoryBottomGuardUntilMsRef.current = 0;
    if (staticInitialHistoryBottomGuardFrameRef.current !== null) {
      cancelAnimationFrame(staticInitialHistoryBottomGuardFrameRef.current);
      staticInitialHistoryBottomGuardFrameRef.current = null;
    }
  }, []);

  const startStaticInitialHistoryBottomGuard = useCallback((durationMs = 2500) => {
    const scroller = scrollerElementRef.current;
    if (!scroller) {
      return;
    }

    if (
      staticInitialHistoryUserLeftBottomRef.current ||
      pendingStaticTurnPinRef.current ||
      staticAnchorWindowTurnId
    ) {
      return;
    }

    const effectiveBottomScrollTop = getEffectiveBottomScrollTop(scroller);
    if (Math.abs(effectiveBottomScrollTop - scroller.scrollTop) > LATEST_END_ANCHOR_STABLE_EPSILON_PX) {
      scroller.scrollTop = effectiveBottomScrollTop;
      staticInitialHistoryUserLeftBottomRef.current = false;
      previousScrollTopRef.current = effectiveBottomScrollTop;
      previousMeasuredHeightRef.current = snapshotMeasuredContentHeight(scroller);
      recordScrollerGeometry(scroller);
    }

    staticInitialHistoryBottomGuardUntilMsRef.current = Math.max(
      staticInitialHistoryBottomGuardUntilMsRef.current,
      performance.now() + durationMs,
    );
    if (staticInitialHistoryBottomGuardFrameRef.current !== null) {
      return;
    }

    const tick = () => {
      staticInitialHistoryBottomGuardFrameRef.current = null;
      const now = performance.now();
      if (now > staticInitialHistoryBottomGuardUntilMsRef.current) {
        return;
      }

      const currentScroller = scrollerElementRef.current;
      if (
        !currentScroller ||
        !useStaticInitialHistoryListRef.current ||
        staticInitialHistoryUserLeftBottomRef.current ||
        pendingStaticTurnPinRef.current ||
        staticAnchorWindowTurnId ||
        now <= userInitiatedUpwardScrollUntilMsRef.current
      ) {
        staticInitialHistoryBottomGuardUntilMsRef.current = 0;
        return;
      }

      const currentEffectiveBottomScrollTop = getEffectiveBottomScrollTop(currentScroller);
      const distanceFromBottom = Math.max(
        0,
        currentEffectiveBottomScrollTop - currentScroller.scrollTop,
      );
      if (distanceFromBottom > LATEST_END_ANCHOR_STABLE_EPSILON_PX) {
        currentScroller.scrollTop = currentEffectiveBottomScrollTop;
        staticInitialHistoryUserLeftBottomRef.current = false;
        previousScrollTopRef.current = currentEffectiveBottomScrollTop;
        previousMeasuredHeightRef.current = snapshotMeasuredContentHeight(currentScroller);
      }
      recordScrollerGeometry(currentScroller);

      staticInitialHistoryBottomGuardFrameRef.current = requestAnimationFrame(tick);
    };

    staticInitialHistoryBottomGuardFrameRef.current = requestAnimationFrame(tick);
  }, [
    getEffectiveBottomScrollTop,
    recordScrollerGeometry,
    snapshotMeasuredContentHeight,
    staticAnchorWindowTurnId,
  ]);

  const updateBottomReservationState = useCallback((
    updater: BottomReservationState | ((prev: BottomReservationState) => BottomReservationState),
  ) => {
    const previous = bottomReservationStateRef.current;
    const rawNext = typeof updater === 'function' ? updater(previous) : updater;
    const next = sanitizeBottomReservationState(rawNext);
    if (
      flowChatDiagnostics.isEnabled() &&
      !areBottomReservationStatesEqual(previous, next)
    ) {
      flowChatDiagnostics.trace({
        hypothesis: 'A',
        location: 'VirtualMessageList.updateBottomReservationState',
        message: 'Bottom reservation state changed',
        data: () => ({
          before: previous,
          after: next,
          coordinatorMode: viewportCoordinatorRef.current.getMode(),
          isFollowingOutput: isFollowingOutputRef.current,
          isStreamingOutput: isStreamingOutputRef.current,
        }),
      });
    }
    bottomReservationStateRef.current = next;
    setBottomReservationState(prev => {
      return areBottomReservationStatesEqual(next, prev) ? prev : next;
    });
  }, []);

  const clearTurnPinRequest = useCallback(() => {
    turnPinRequestGenerationRef.current += 1;
    transientTurnPinStabilizationRef.current = null;

    if (turnPinStabilizationFrameRef.current !== null) {
      cancelAnimationFrame(turnPinStabilizationFrameRef.current);
      turnPinStabilizationFrameRef.current = null;
    }

    setPendingTurnPin(null);
  }, []);

  const cancelLatestEndAnchorStabilization = useCallback(() => {
    if (latestEndAnchorStabilizationFrameRef.current !== null) {
      cancelAnimationFrame(latestEndAnchorStabilizationFrameRef.current);
      latestEndAnchorStabilizationFrameRef.current = null;
    }
    latestEndAnchorRequestRef.current = null;
  }, []);

  const activateTransientTurnPinStabilization = useCallback((request: PendingTurnPinState) => {
    if (request.pinMode !== 'transient') {
      return;
    }

    transientTurnPinStabilizationRef.current = {
      ...request,
      behavior: 'auto',
      attempts: request.attempts + 1,
    };
  }, []);

  const resetBottomReservations = useCallback(() => {
    updateBottomReservationState(createInitialBottomReservationState());
  }, [updateBottomReservationState]);

  const consumeBottomCompensation = useCallback((
    amountPx: number,
    options?: {
      consumeStickyPinFloor?: boolean;
      preserveCollapseReservation?: boolean;
    },
  ) => {
    if (amountPx <= COMPENSATION_EPSILON_PX) {
      return bottomReservationStateRef.current;
    }

    let resolvedNextState = bottomReservationStateRef.current;
    updateBottomReservationState(prev => {
      const nextState = consumeBottomReservationForContentGrowth(
        prev,
        amountPx,
        options?.consumeStickyPinFloor === true,
        options?.preserveCollapseReservation === true,
      );
      resolvedNextState = nextState;
      return nextState;
    });
    return resolvedNextState;
  }, [updateBottomReservationState]);

  const applyFooterCompensationNow = useCallback((compensation: number | BottomReservationState) => {
    const footer = footerElementRef.current;
    const scroller = scrollerElementRef.current;
    if (!footer || !scroller) return;

    const compensationPx = typeof compensation === 'number'
      ? compensation
      : getTotalBottomCompensationPx(compensation);
    applyFooterHeightToElement(footer, compensationPx);
    void footer.offsetHeight;
    void scroller.scrollHeight;
    if (flowChatDiagnostics.isEnabled()) {
      flowChatDiagnostics.trace({
        hypothesis: 'C',
        location: 'VirtualMessageList.applyFooterCompensationNow',
        message: 'Footer compensation applied synchronously',
        data: () => ({
          compensationPx,
          renderedHeight: footer.getBoundingClientRect().height,
          scrollTop: scroller.scrollTop,
          scrollHeight: scroller.scrollHeight,
          clientHeight: scroller.clientHeight,
          coordinatorMode: viewportCoordinatorRef.current.getMode(),
        }),
      });
    }
  }, [applyFooterHeightToElement, getTotalBottomCompensationPx]);

  const preserveCollapseAnchorScrollTop = useCallback((
    scroller: HTMLElement,
    targetScrollTop: number,
    source: string,
  ) => {
    const currentState = bottomReservationStateRef.current;
    const nextState = ensureCollapseReservationForScrollTop(currentState, {
      targetScrollTop,
      scrollHeight: scroller.scrollHeight,
      clientHeight: scroller.clientHeight,
    });
    const reservationChanged = !areBottomReservationStatesEqual(currentState, nextState);
    if (reservationChanged) {
      updateBottomReservationState(nextState);
      applyFooterCompensationNow(nextState);
    }

    const scrollTopBefore = scroller.scrollTop;
    const maxScrollTop = Math.max(0, scroller.scrollHeight - scroller.clientHeight);
    const restoredScrollTop = Math.min(maxScrollTop, Math.max(scrollTopBefore, targetScrollTop));
    const scrollTopChanged = restoredScrollTop - scrollTopBefore > COMPENSATION_EPSILON_PX;
    if (scrollTopChanged) {
      scroller.scrollTop = restoredScrollTop;
    }

    if (flowChatDiagnostics.isEnabled() && (reservationChanged || scrollTopChanged)) {
      flowChatDiagnostics.trace({
        hypothesis: 'E',
        location: 'VirtualMessageList.preserveCollapseAnchorScrollTop',
        message: 'Collapse anchor range restored synchronously',
        data: () => ({
          source,
          targetScrollTop,
          scrollTopBefore,
          scrollTopAfter: scroller.scrollTop,
          scrollHeight: scroller.scrollHeight,
          clientHeight: scroller.clientHeight,
          reservationBefore: currentState,
          reservationAfter: bottomReservationStateRef.current,
          coordinatorMode: viewportCoordinatorRef.current.getMode(),
        }),
      });
    }

    previousScrollTopRef.current = scroller.scrollTop;
    previousMeasuredHeightRef.current = snapshotMeasuredContentHeight(
      scroller,
      bottomReservationStateRef.current,
    );
    recordScrollerGeometry(scroller);
    return reservationChanged || scrollTopChanged;
  }, [
    applyFooterCompensationNow,
    recordScrollerGeometry,
    snapshotMeasuredContentHeight,
    updateBottomReservationState,
  ]);

  const settleRetainedCollapseRange = useCallback((reason: string, generation: number) => {
    if (generation !== retainedCollapseSettleGenerationRef.current) {
      return;
    }
    const retainedAnchor = retainedCollapseAnchorRef.current;
    const scroller = scrollerElementRef.current;
    if (!retainedAnchor || !scroller || pendingCollapseIntentRef.current.active) {
      return;
    }

    const targetScrollTop = Math.max(retainedAnchor.anchorScrollTop, scroller.scrollTop);
    retainedAnchor.anchorScrollTop = targetScrollTop;
    const currentState = bottomReservationStateRef.current;
    const nextState = settleRetainedCollapseReservationForAnchor(currentState, {
      targetScrollTop,
      scrollHeight: scroller.scrollHeight,
      clientHeight: scroller.clientHeight,
    });
    const reservationChanged = !areBottomReservationStatesEqual(currentState, nextState);
    if (reservationChanged) {
      updateBottomReservationState(nextState);
      applyFooterCompensationNow(nextState);
    }
    preserveCollapseAnchorScrollTop(scroller, targetScrollTop, `quiet-settle:${reason}`);

    const releaseGeneration = retainedCollapseSettleGenerationRef.current;
    retainedCollapseReleaseTimerRef.current = window.setTimeout(() => {
      retainedCollapseReleaseTimerRef.current = null;
      if (releaseGeneration !== retainedCollapseSettleGenerationRef.current) {
        return;
      }
      retainedCollapseAnchorRef.current = null;
      if (flowChatDiagnostics.isEnabled()) {
        flowChatDiagnostics.trace({
          hypothesis: 'F',
          location: 'VirtualMessageList.releaseSettledCollapseAnchor',
          message: 'Settled collapse anchor released after layout remained quiet',
          data: () => ({
            reason,
            retainedAnchor,
            reservation: bottomReservationStateRef.current,
            scrollTop: scroller.scrollTop,
            scrollHeight: scroller.scrollHeight,
            clientHeight: scroller.clientHeight,
            coordinatorMode: viewportCoordinatorRef.current.getMode(),
          }),
        });
      }
    }, RETAINED_COLLAPSE_RELEASE_QUIET_MS);

    if (flowChatDiagnostics.isEnabled()) {
      flowChatDiagnostics.trace({
        hypothesis: 'F',
        location: 'VirtualMessageList.settleRetainedCollapseRange',
        message: 'Retained collapse range settled to current anchor geometry',
        data: () => ({
          reason,
          retainedAnchor,
          targetScrollTop,
          reservationBefore: currentState,
          reservationAfter: bottomReservationStateRef.current,
          releasedPx: Math.max(
            0,
            getReservationTotalPx(currentState.collapse) -
              getReservationTotalPx(bottomReservationStateRef.current.collapse),
          ),
          scrollTop: scroller.scrollTop,
          scrollHeight: scroller.scrollHeight,
          clientHeight: scroller.clientHeight,
          coordinatorMode: viewportCoordinatorRef.current.getMode(),
        }),
      });
    }
  }, [
    applyFooterCompensationNow,
    preserveCollapseAnchorScrollTop,
    updateBottomReservationState,
  ]);

  const scheduleRetainedCollapseSettlement = useCallback((reason: string) => {
    if (
      retainedCollapseAnchorRef.current === null ||
      pendingCollapseIntentRef.current.active
    ) {
      return;
    }

    clearRetainedCollapseSettlement();
    const generation = retainedCollapseSettleGenerationRef.current;
    retainedCollapseSettleTimerRef.current = window.setTimeout(() => {
      retainedCollapseSettleTimerRef.current = null;
      const settleAfterFrames = (remainingFrames: number) => {
        retainedCollapseSettleFrameRef.current = requestAnimationFrame(() => {
          if (generation !== retainedCollapseSettleGenerationRef.current) {
            retainedCollapseSettleFrameRef.current = null;
            return;
          }
          if (remainingFrames > 1) {
            settleAfterFrames(remainingFrames - 1);
            return;
          }
          retainedCollapseSettleFrameRef.current = null;
          settleRetainedCollapseRange(reason, generation);
        });
      };
      settleAfterFrames(RETAINED_COLLAPSE_QUIET_SETTLE_FRAMES);
    }, RETAINED_COLLAPSE_QUIET_SETTLE_MS);
  }, [clearRetainedCollapseSettlement, settleRetainedCollapseRange]);

  const retainCollapseRangeForQuietSettlement = useCallback((
    anchor: RetainedCollapseAnchorState,
    reason: string,
  ) => {
    const scroller = scrollerElementRef.current;
    const currentState = bottomReservationStateRef.current;
    if (currentState.collapse.px <= COMPENSATION_EPSILON_PX) {
      clearRetainedCollapseSettlement();
      retainedCollapseAnchorRef.current = null;
      return;
    }
    const previousRetainedAnchor = retainedCollapseAnchorRef.current;
    const nextRetainedAnchor: RetainedCollapseAnchorState = {
      anchorScrollTop: Math.max(
        anchor.anchorScrollTop,
        previousRetainedAnchor?.anchorScrollTop ?? 0,
        scroller?.scrollTop ?? 0,
      ),
      toolId: anchor.toolId ?? previousRetainedAnchor?.toolId ?? null,
      toolName: anchor.toolName ?? previousRetainedAnchor?.toolName ?? null,
    };
    retainedCollapseAnchorRef.current = nextRetainedAnchor;

    const nextState = scroller
      ? ensureCollapseReservationForScrollTop(currentState, {
        targetScrollTop: nextRetainedAnchor.anchorScrollTop,
        scrollHeight: scroller.scrollHeight,
        clientHeight: scroller.clientHeight,
      })
      : protectCurrentCollapseReservation(currentState);
    if (!areBottomReservationStatesEqual(currentState, nextState)) {
      updateBottomReservationState(nextState);
      applyFooterCompensationNow(nextState);
    }
    if (scroller) {
      preserveCollapseAnchorScrollTop(
        scroller,
        nextRetainedAnchor.anchorScrollTop,
        `retain:${reason}`,
      );
    }
    scheduleRetainedCollapseSettlement(reason);

    if (flowChatDiagnostics.isEnabled()) {
      flowChatDiagnostics.trace({
        hypothesis: 'E',
        location: 'VirtualMessageList.retainCollapseRangeForQuietSettlement',
        message: 'Collapse range retained until delayed measurements settle',
        data: () => ({
          reason,
          retainedAnchor: nextRetainedAnchor,
          reservationBefore: currentState,
          reservationAfter: bottomReservationStateRef.current,
          scrollTop: scroller?.scrollTop ?? null,
          scrollHeight: scroller?.scrollHeight ?? null,
          clientHeight: scroller?.clientHeight ?? null,
        }),
      });
    }
  }, [
    applyFooterCompensationNow,
    clearRetainedCollapseSettlement,
    preserveCollapseAnchorScrollTop,
    scheduleRetainedCollapseSettlement,
    updateBottomReservationState,
  ]);

  const clearPendingStickyPinGrowth = useCallback((_reason: string) => {
    if (stickyPinGrowthSettleTimerRef.current !== null) {
      window.clearTimeout(stickyPinGrowthSettleTimerRef.current);
      stickyPinGrowthSettleTimerRef.current = null;
    }
    pendingStickyPinGrowthRef.current = {
      targetTurnId: null,
      amountPx: 0,
    };
  }, []);

  const settlePendingStickyPinGrowth = useCallback((reason: string) => {
    stickyPinGrowthSettleTimerRef.current = null;
    const pending = pendingStickyPinGrowthRef.current;
    if (pending.amountPx <= COMPENSATION_EPSILON_PX || !pending.targetTurnId) {
      return;
    }

    const currentState = bottomReservationStateRef.current;
    const pinReservation = currentState.pin;
    if (
      pinReservation.mode !== 'sticky-latest' ||
      pinReservation.targetTurnId !== pending.targetTurnId ||
      !viewportCoordinatorRef.current.ownsElementAnchor()
    ) {
      clearPendingStickyPinGrowth(`${reason}:pin-owner-changed`);
      return;
    }

    if (pendingCollapseIntentRef.current.active) {
      stickyPinGrowthSettleTimerRef.current = window.setTimeout(() => {
        settlePendingStickyPinGrowthRef.current('collapse-settled-retry');
      }, 50);
      return;
    }

    const consumedPx = Math.min(pending.amountPx, pinReservation.floorPx);
    pendingStickyPinGrowthRef.current = {
      targetTurnId: null,
      amountPx: 0,
    };
    if (consumedPx <= COMPENSATION_EPSILON_PX) {
      clearPendingStickyPinGrowth(`${reason}:nothing-consumable`);
      return;
    }

    const nextState = sanitizeBottomReservationState({
      ...currentState,
      pin: {
        ...pinReservation,
        px: pinReservation.px - consumedPx,
        floorPx: pinReservation.floorPx - consumedPx,
      },
    });
    updateBottomReservationState(nextState);
    applyFooterCompensationNow(nextState);
    const scroller = scrollerElementRef.current;
    if (scroller) {
      viewportCoordinatorRef.current.restoreElementAnchor(scroller, 'sticky-pin-growth-settled');
      previousScrollTopRef.current = scroller.scrollTop;
      previousMeasuredHeightRef.current = snapshotMeasuredContentHeight(scroller, nextState);
      recordScrollerGeometry(scroller);
    }
    maybeHandoffPinnedTurnToTailRef.current(`sticky-pin-growth-settled:${reason}`);
  }, [
    applyFooterCompensationNow,
    clearPendingStickyPinGrowth,
    recordScrollerGeometry,
    snapshotMeasuredContentHeight,
    updateBottomReservationState,
  ]);
  settlePendingStickyPinGrowthRef.current = settlePendingStickyPinGrowth;

  const queuePendingStickyPinGrowth = useCallback((targetTurnId: string, amountPx: number) => {
    const sanitizedAmountPx = sanitizeReservationPx(amountPx);
    if (sanitizedAmountPx <= COMPENSATION_EPSILON_PX) {
      return;
    }

    const previousPending = pendingStickyPinGrowthRef.current;
    const nextPending = previousPending.targetTurnId === targetTurnId
      ? {
        targetTurnId,
        amountPx: previousPending.amountPx + sanitizedAmountPx,
      }
      : {
        targetTurnId,
        amountPx: sanitizedAmountPx,
      };
    pendingStickyPinGrowthRef.current = nextPending;
    if (stickyPinGrowthSettleTimerRef.current !== null) {
      window.clearTimeout(stickyPinGrowthSettleTimerRef.current);
    }
    stickyPinGrowthSettleTimerRef.current = window.setTimeout(() => {
      settlePendingStickyPinGrowthRef.current('settle-timeout');
    }, STICKY_PIN_GROWTH_SETTLE_MS);
  }, []);

  useEffect(() => () => {
    if (stickyPinGrowthSettleTimerRef.current !== null) {
      window.clearTimeout(stickyPinGrowthSettleTimerRef.current);
      stickyPinGrowthSettleTimerRef.current = null;
    }
  }, []);

  const ensureSemanticAnchorRange = useCallback((
    options: Parameters<FlowChatViewportRangeHost['ensureBottomRange']>[0],
  ) => {
    const additionalPx = sanitizeReservationPx(options.additionalPx);
    const footer = footerElementRef.current;
    const scroller = scrollerElementRef.current;
    if (additionalPx <= COMPENSATION_EPSILON_PX || !footer || !scroller) {
      return false;
    }

    const previousState = bottomReservationStateRef.current;
    const nextState: BottomReservationState = options.mode === 'pinned-item'
      ? {
        ...previousState,
        pin: {
          ...previousState.pin,
          px: previousState.pin.px + additionalPx,
          floorPx: previousState.pin.mode === 'sticky-latest'
            ? previousState.pin.floorPx + additionalPx
            : previousState.pin.floorPx,
        },
      }
      : {
        ...previousState,
        collapse: {
          ...previousState.collapse,
          px: previousState.collapse.px + additionalPx,
        },
      };

    updateBottomReservationState(nextState);
    applyFooterCompensationNow(nextState);
    previousMeasuredHeightRef.current = snapshotMeasuredContentHeight(scroller, nextState);
    return true;
  }, [
    applyFooterCompensationNow,
    snapshotMeasuredContentHeight,
    updateBottomReservationState,
  ]);

  useLayoutEffect(() => {
    const coordinator = viewportCoordinatorRef.current;
    coordinator.setRangeHost({
      ensureBottomRange: ensureSemanticAnchorRange,
    });
    return () => {
      coordinator.setRangeHost(null);
    };
  }, [ensureSemanticAnchorRange]);

  // When the input collapses (e.g. user sends a message), preserve its previous
  // space as a reservation before synchronizing the ref-owned Footer height.
  // The Virtuoso Footer has no React-owned height, so this layout effect is the
  // single writer for input-stack height changes and cannot expose a smaller
  // intermediate Footer to the browser.
  //
  // Detect the footer shrink in a useLayoutEffect (fires synchronously after
  // commit, before paint) and extend the bottom collapse reservation by the
  // shrink amount so the total footer height stays unchanged.  The reservation
  // is later consumed organically by the grow branch of measureHeightChange as
  // streaming content arrives.
  useLayoutEffect(() => {
    const prevFooterPx = previousInputStackFooterPxRef.current;
    const currFooterPx = inputStackFooterPx;
    previousInputStackFooterPxRef.current = currFooterPx;

    const shrinkAmount = prevFooterPx - currFooterPx;
    if (shrinkAmount <= COMPENSATION_EPSILON_PX) {
      applyFooterCompensationNow(bottomReservationStateRef.current);
      return;
    }

    const scroller = scrollerElementRef.current;
    // Use the pre-commit scrollTop captured during render. It also protects
    // against any unrelated virtualizer measurement committed in this render.
    const preClampScrollTop = footerShrinkPreScrollTopRef.current;

    const baseState = bottomReservationStateRef.current;
    const currentCollapsePx = getReservationTotalPx(baseState.collapse);
    const nextReservationState: BottomReservationState = {
      ...baseState,
      collapse: {
        ...baseState.collapse,
        px: currentCollapsePx + shrinkAmount,
      },
    };
    updateBottomReservationState(nextReservationState);
    applyFooterCompensationNow(nextReservationState);

    // Restore scrollTop to the pre-clamp position if the browser already
    // clamped it during the commit.  After extending the footer via
    // compensation, the total scrollHeight is back to its pre-shrink value,
    // so restoring the old scrollTop is safe.
    if (scroller && preClampScrollTop !== undefined) {
      const maxScrollTop = Math.max(
        0,
        scroller.scrollHeight - scroller.clientHeight,
      );
      scroller.scrollTop = Math.min(preClampScrollTop, maxScrollTop);
    }

    // If no streaming is in progress, schedule a consumption pass to drain
    // the compensation added above. Without streaming, the grow branch of
    // measureHeightChange never fires, so the residual collapse.px would
    // persist as permanent footer whitespace (issue #1176).
    if (!isStreamingOutputRef.current) {
      requestAnimationFrame(() => {
        const scrollerNow = scrollerElementRef.current;
        if (!scrollerNow) return;
        // Do not drain if a collapse intent is still protecting an ongoing
        // CSS transition or delayed virtualizer measurement.
        const intent = pendingCollapseIntentRef.current;
        if (intent.active || retainedCollapseAnchorRef.current) return;
        const collapsePx = getReservationTotalPx(bottomReservationStateRef.current.collapse);
        if (collapsePx <= COMPENSATION_EPSILON_PX) return;
        const distanceFromBottom = Math.max(
          0,
          scrollerNow.scrollHeight - scrollerNow.clientHeight - scrollerNow.scrollTop,
        );
        if (distanceFromBottom <= COMPENSATION_EPSILON_PX) {
          // User is at the bottom — safe to drain all collapse compensation.
          const drained: BottomReservationState = {
            ...bottomReservationStateRef.current,
            collapse: {
              ...bottomReservationStateRef.current.collapse,
              px: 0,
              floorPx: 0,
            },
          };
          updateBottomReservationState(drained);
          applyFooterCompensationNow(drained);
        }
      });
    }
  }, [inputStackFooterPx, updateBottomReservationState, applyFooterCompensationNow]);

  const measureHeightChange = useCallback(() => {
    const scroller = scrollerElementRef.current;
    if (!scroller) return;

    const currentScrollTop = scroller.scrollTop;
    const previousScrollTop = previousScrollTopRef.current;
    const previousScrollerGeometry = previousScrollerGeometryRef.current;
    const viewportGeometryChanged = Boolean(
      previousScrollerGeometry &&
      (
        Math.abs(scroller.scrollHeight - previousScrollerGeometry.scrollHeight) > COMPENSATION_EPSILON_PX ||
        Math.abs(scroller.clientHeight - previousScrollerGeometry.clientHeight) > COMPENSATION_EPSILON_PX
      )
    );
    const wasAtPhysicalBottom = Boolean(
      previousScrollerGeometry &&
      Math.abs(
        Math.max(0, previousScrollerGeometry.scrollHeight - previousScrollerGeometry.clientHeight) -
        previousScrollerGeometry.scrollTop
      ) <= LATEST_END_ANCHOR_STABLE_EPSILON_PX
    );
    const currentTotalCompensation = getTotalBottomCompensationPx();
    const effectiveScrollHeight = Math.max(
      0,
      scroller.scrollHeight - currentTotalCompensation - inputStackFooterPxRef.current,
    );
    const previousMeasuredHeight = previousMeasuredHeightRef.current;
    previousMeasuredHeightRef.current = effectiveScrollHeight;

    if (previousMeasuredHeight === null) {
      previousScrollTopRef.current = currentScrollTop;
      recordScrollerGeometry(scroller);
      return;
    }

    const heightDelta = effectiveScrollHeight - previousMeasuredHeight;
    if (Math.abs(heightDelta) <= COMPENSATION_EPSILON_PX) {
      previousScrollTopRef.current = currentScrollTop;
      recordScrollerGeometry(scroller);
      return;
    }

    if (
      heightDelta < -COMPENSATION_EPSILON_PX &&
      retainedCollapseAnchorRef.current !== null &&
      !pendingCollapseIntentRef.current.active
    ) {
      scheduleRetainedCollapseSettlement('delayed-negative-measurement');
    }

    if (flowChatDiagnostics.isEnabled()) {
      flowChatDiagnostics.trace({
        hypothesis: 'C',
        location: 'VirtualMessageList.measureHeightChange',
        message: 'Effective content height changed',
        data: () => ({
          heightDelta,
          previousMeasuredHeight,
          effectiveScrollHeight,
          currentScrollTop,
          previousScrollTop,
          scrollHeight: scroller.scrollHeight,
          clientHeight: scroller.clientHeight,
          reservation: bottomReservationStateRef.current,
          coordinatorMode: viewportCoordinatorRef.current.getMode(),
          isFollowingOutput: isFollowingOutputRef.current,
          isStreamingOutput: isStreamingOutputRef.current,
          collapseIntentActive: pendingCollapseIntentRef.current.active,
          retainedCollapseAnchor: retainedCollapseAnchorRef.current,
          wasAtPhysicalBottom,
          viewportGeometryChanged,
        }),
      });
    }

    const ownsElementAnchor = viewportCoordinatorRef.current.ownsElementAnchor();
    const hasCollapseAnchorProtection = (
      pendingCollapseIntentRef.current.active ||
      retainedCollapseAnchorRef.current !== null
    );
    if (shouldSyncPhysicalBottom({
      viewportGeometryChanged,
      collapseProtectionActive: hasCollapseAnchorProtection,
      wasAtPhysicalBottom,
      ownsElementAnchor,
    })) {
      if (wasAtPhysicalBottom) {
        const maxScrollTop = Math.max(0, scroller.scrollHeight - scroller.clientHeight);
        if (Math.abs(maxScrollTop - scroller.scrollTop) > COMPENSATION_EPSILON_PX) {
          if (flowChatDiagnostics.isEnabled()) {
            flowChatDiagnostics.trace({
              hypothesis: 'C',
              location: 'VirtualMessageList.measureHeightChange',
              message: 'Content measurement synchronized the physical bottom',
              data: () => ({
                scrollTopBefore: scroller.scrollTop,
                requestedScrollTop: maxScrollTop,
                heightDelta,
              }),
            });
          }
          scroller.scrollTop = maxScrollTop;
        }
      }
      previousScrollTopRef.current = scroller.scrollTop;
      previousMeasuredHeightRef.current = snapshotMeasuredContentHeight(scroller);
      recordScrollerGeometry(scroller);
      return;
    }

    const distanceFromBottom = Math.max(
      0,
      scroller.scrollHeight - scroller.clientHeight - scroller.scrollTop
    );

    // Content grew: consume temporary footer padding first.
    if (heightDelta > 0) {
      const collapseIntent0 = pendingCollapseIntentRef.current;
      const collapseProtectionActive = collapseIntent0.active;
      const previousReservationState = bottomReservationStateRef.current;
      const pinReservation = previousReservationState.pin;
      const canSettleStickyPinGrowth = (
        pinReservation.mode === 'sticky-latest' &&
        Boolean(pinReservation.targetTurnId) &&
        viewportCoordinatorRef.current.ownsElementAnchor()
      );
      const nextReservationState = consumeBottomCompensation(heightDelta, {
        consumeStickyPinFloor: false,
        preserveCollapseReservation: collapseProtectionActive,
      });
      const immediatelyConsumedPx = Math.max(
        0,
        getTotalBottomCompensationPx(previousReservationState) -
        getTotalBottomCompensationPx(nextReservationState),
      );
      const unsettledGrowthPx = Math.max(0, heightDelta - immediatelyConsumedPx);
      if (canSettleStickyPinGrowth && pinReservation.targetTurnId) {
        queuePendingStickyPinGrowth(
          pinReservation.targetTurnId,
          Math.min(unsettledGrowthPx, nextReservationState.pin.floorPx),
        );
      }
      applyFooterCompensationNow(nextReservationState);
      viewportCoordinatorRef.current.restoreElementAnchor(scroller, 'measure-grow');
      previousScrollTopRef.current = scroller.scrollTop;
      recordScrollerGeometry(scroller);
      return;
    }

    // Content shrank: preserve the current visual anchor by extending the footer
    // when the user does not already have enough distance from the bottom.
    //
    // Unsignaled shrinks in streaming tail-follow remain owned by the follow
    // loop. A known collapse intent is different: its synchronous Footer
    // reservation keeps both the physical bottom and the card header stable,
    // so it must continue through the reconciliation path below.
    const collapseIntent = pendingCollapseIntentRef.current;
    const retainedCollapseAnchor = retainedCollapseAnchorRef.current;
    if (retainedCollapseAnchor) {
      preserveCollapseAnchorScrollTop(
        scroller,
        retainedCollapseAnchor.anchorScrollTop,
        'measure-retained-shrink',
      );
      return;
    }
    if (shouldBypassShrinkCompensationInTailFollow({
      isFollowingOutput: isFollowingOutputRef.current,
      isStreamingOutput: isStreamingOutputRef.current,
      hasActiveCollapseIntent: collapseIntent.active,
    })) {
      if (flowChatDiagnostics.isEnabled()) {
        flowChatDiagnostics.trace({
          hypothesis: 'D',
          location: 'VirtualMessageList.measureHeightChange',
          message: 'Unsignaled shrink delegated to streaming tail follow',
          data: () => ({ heightDelta, currentScrollTop, previousScrollTop }),
        });
      }
      previousScrollTopRef.current = currentScrollTop;
      recordScrollerGeometry(scroller);
      return;
    }

    const pendingStickyPinGrowth = pendingStickyPinGrowthRef.current;
    const canceledPendingGrowthPx = getCanceledUnsettledStickyPinGrowthPx({
      pendingGrowthPx: pendingStickyPinGrowth.amountPx,
      shrinkPx: -heightDelta,
      hasActiveCollapseIntent: collapseIntent.active,
    });
    if (canceledPendingGrowthPx > COMPENSATION_EPSILON_PX) {
      pendingStickyPinGrowthRef.current = {
        targetTurnId: pendingStickyPinGrowth.amountPx - canceledPendingGrowthPx > COMPENSATION_EPSILON_PX
          ? pendingStickyPinGrowth.targetTurnId
          : null,
        amountPx: Math.max(0, pendingStickyPinGrowth.amountPx - canceledPendingGrowthPx),
      };
      if (
        pendingStickyPinGrowthRef.current.amountPx <= COMPENSATION_EPSILON_PX &&
        stickyPinGrowthSettleTimerRef.current !== null
      ) {
        window.clearTimeout(stickyPinGrowthSettleTimerRef.current);
        stickyPinGrowthSettleTimerRef.current = null;
      }
    }
    const shrinkAmount = Math.max(0, -heightDelta - canceledPendingGrowthPx);
    if (shrinkAmount <= COMPENSATION_EPSILON_PX) {
      viewportCoordinatorRef.current.restoreElementAnchor(scroller, 'measure-unsettled-growth-reverted');
      previousScrollTopRef.current = scroller.scrollTop;
      recordScrollerGeometry(scroller);
      return;
    }
    const hasValidCollapseIntent = collapseIntent.active;
    // For unsignaled shrinks, the visible gap to the bottom determines the
    // required compensation. We no longer ratchet up via Math.max with the
    // previous collapse.px: stale compensation from an earlier protected
    // collapse is intentionally allowed to shrink when the current shrink
    // needs less, preventing permanent whitespace accumulation (issue #1176).
    const fallbackRequiredCollapseCompensation = Math.max(0, shrinkAmount - distanceFromBottom);
    const cumulativeShrinkPx = hasValidCollapseIntent
      ? collapseIntent.cumulativeShrinkPx + shrinkAmount
      : 0;
    const resolvedIntentCompensation = hasValidCollapseIntent
      ? collapseIntent.baseTotalCompensationPx + Math.max(0, cumulativeShrinkPx - collapseIntent.distanceFromBottomBeforeCollapse)
      : 0;
    const reconciledUnsignaledState = hasValidCollapseIntent
      ? null
      : reconcileUnsignaledShrinkReservation(
        bottomReservationStateRef.current,
        fallbackRequiredCollapseCompensation,
      );
    const nextTotalCompensation = hasValidCollapseIntent
      ? Math.max(currentTotalCompensation, resolvedIntentCompensation)
      : getTotalBottomCompensationPx(
        reconciledUnsignaledState ?? bottomReservationStateRef.current,
      );
    if (hasValidCollapseIntent) {
      pendingCollapseIntentRef.current = {
        ...collapseIntent,
        cumulativeShrinkPx,
      };
    }

    if (!hasValidCollapseIntent && fallbackRequiredCollapseCompensation <= COMPENSATION_EPSILON_PX) {
      // If the user is already far enough from the bottom, this shrink does not
      // need protection. Reusing stale bottom compensation here makes the
      // scroll listener restore an older anchor during upward scroll and causes
      // the visible "wall hit" jitter.
      previousScrollTopRef.current = currentScrollTop;
      recordScrollerGeometry(scroller);
      return;
    }

    const nextReservationState: BottomReservationState = reconciledUnsignaledState ?? {
      ...bottomReservationStateRef.current,
      collapse: {
        ...bottomReservationStateRef.current.collapse,
        px: Math.max(0, nextTotalCompensation - getReservationTotalPx(bottomReservationStateRef.current.pin)),
      },
    };
    if (flowChatDiagnostics.isEnabled()) {
      flowChatDiagnostics.trace({
        hypothesis: hasValidCollapseIntent ? 'E' : 'C',
        location: 'VirtualMessageList.measureHeightChange',
        message: 'Content shrink resolved bottom compensation',
        data: () => ({
          heightDelta,
          shrinkAmount,
          distanceFromBottom,
          hasValidCollapseIntent,
          fallbackRequiredCollapseCompensation,
          currentTotalCompensation,
          nextTotalCompensation,
          reservationBefore: bottomReservationStateRef.current,
          reservationAfter: nextReservationState,
        }),
      });
    }
    updateBottomReservationState(nextReservationState);
    if (nextTotalCompensation > COMPENSATION_EPSILON_PX) {
      const anchorTarget =
        hasValidCollapseIntent
          ? collapseIntent.anchorScrollTop
          : previousScrollTop;

      applyFooterCompensationNow(nextReservationState);
      if (hasValidCollapseIntent) {
        preserveCollapseAnchorScrollTop(
          scroller,
          collapseIntent.anchorScrollTop,
          'measure-active-collapse',
        );
      } else if (!(isFollowingOutputRef.current && isStreamingOutputRef.current)) {
        viewportCoordinatorRef.current.restoreScrollPositionOnce(
          scroller,
          anchorTarget,
          hasValidCollapseIntent ? 'transition-shrink' : 'instant-shrink',
        );
      }
    }

    previousScrollTopRef.current = scroller.scrollTop;
    recordScrollerGeometry(scroller);
  }, [
    applyFooterCompensationNow,
    consumeBottomCompensation,
    getTotalBottomCompensationPx,
    queuePendingStickyPinGrowth,
    preserveCollapseAnchorScrollTop,
    recordScrollerGeometry,
    snapshotMeasuredContentHeight,
    scheduleRetainedCollapseSettlement,
    updateBottomReservationState,
  ]);

  const scheduleHeightMeasure = useCallback((frames: number = 1) => {
    if (measureFrameRef.current !== null) {
      cancelAnimationFrame(measureFrameRef.current);
      measureFrameRef.current = null;
    }

    const run = (remainingFrames: number) => {
      measureFrameRef.current = requestAnimationFrame(() => {
        if (remainingFrames > 1) {
          run(remainingFrames - 1);
          return;
        }

        measureFrameRef.current = null;
        measureHeightChange();
      });
    };

    run(Math.max(1, frames));
  }, [measureHeightChange]);

  const userMessageItems = React.useMemo(() => {
    return virtualItems
      .map((item, index) => ({ item, index }))
      .filter(({ item }) => item.type === 'user-message');
  }, [virtualItems]);

  const latestTurnId = userMessageItems[userMessageItems.length - 1]?.item.turnId ?? null;
  const latestUserMessageIndex = userMessageItems[userMessageItems.length - 1]?.index ?? 0;
  const isTurnPinRequestCurrent = useCallback((request: PendingTurnPinState) => {
    const currentSessionId = activeSessionIdRef.current;
    const currentTargetTurnId = request.pinMode === 'sticky-latest'
      ? latestTurnId
      : request.turnId;
    if (!currentSessionId || !currentTargetTurnId) {
      return false;
    }

    return isTurnPinRequestIdentityCurrent(request, {
      generation: turnPinRequestGenerationRef.current,
      sessionId: currentSessionId,
      turnId: currentTargetTurnId,
    });
  }, [latestTurnId]);
  const hasPendingHistoryCompletion = activeSession?.sessionId
    ? flowChatStore.hasPendingSessionHistoryCompletion(activeSession.sessionId)
    : false;
  const hasPartialHistoryInitialViewport =
    activeSession?.historyState === 'ready' &&
    activeSession.contextRestoreState === 'pending' &&
    (activeSession.dialogTurns.length ?? 0) <= PARTIAL_HISTORY_INITIAL_TAIL_TURN_BUDGET;
  const useInitialHistoryRenderBudget = hasPendingHistoryCompletion || hasPartialHistoryInitialViewport;
  const useStaticInitialHistoryList = useInitialHistoryRenderBudget;
  useStaticInitialHistoryListRef.current = useStaticInitialHistoryList;
  useEffect(() => {
    setStaticAnchorWindowTurnId(null);
  }, [activeSession?.sessionId]);
  const latestTurnAutoFollowStateRef = useRef<{
    turnId: string | null;
    sawPositiveFloor: boolean;
  }>({
    turnId: latestTurnId,
    sawPositiveFloor: false,
  });
  const hasPrimedMountedStreamingTurnFollowRef = useRef(false);
  const previousLatestTurnIdForFollowRef = useRef<string | null>(latestTurnId);
  const previousSessionIdForFollowRef = useRef<string | undefined>(activeSession?.sessionId);

  const visibleTurnInfoByTurnId = React.useMemo(() => {
    const infoMap = new Map<string, VisibleTurnInfo>();

    userMessageItems.forEach(({ item }, index) => {
      if (item.type !== 'user-message') return;

      infoMap.set(item.turnId, {
        turnIndex: index + 1,
        totalTurns: userMessageItems.length,
        userMessage: item.data?.content || '',
        turnId: item.turnId,
      });
    });

    return infoMap;
  }, [userMessageItems]);

  const measureVisibleTurn = useCallback(() => {
    const setVisibleTurnInfo = useModernFlowChatStore.getState().setVisibleTurnInfo;
    const currentVisibleTurnInfo = useModernFlowChatStore.getState().visibleTurnInfo;

    if (userMessageItems.length === 0) {
      if (currentVisibleTurnInfo !== null) {
        setVisibleTurnInfo(null);
      }
      return;
    }

    const scroller = scrollerElementRef.current;
    if (!scroller) {
      const fallbackInfo = visibleTurnInfoByTurnId.get(userMessageItems[0]?.item.turnId ?? '') ?? null;
      if (
        currentVisibleTurnInfo?.turnId !== fallbackInfo?.turnId ||
        currentVisibleTurnInfo?.turnIndex !== fallbackInfo?.turnIndex ||
        currentVisibleTurnInfo?.totalTurns !== fallbackInfo?.totalTurns ||
        currentVisibleTurnInfo?.userMessage !== fallbackInfo?.userMessage
      ) {
        setVisibleTurnInfo(fallbackInfo);
      }
      return;
    }

    const scrollerRect = scroller.getBoundingClientRect();
    const viewportTop = scrollerRect.top + PINNED_TURN_VIEWPORT_OFFSET_PX;
    const viewportBottom = scrollerRect.bottom;
    const renderedItems = Array.from(
      scroller.querySelectorAll<HTMLElement>('.virtual-item-wrapper[data-turn-id]')
    );

    const topVisibleItem = renderedItems.find(node => {
      const rect = node.getBoundingClientRect();
      return rect.bottom > viewportTop && rect.top < viewportBottom;
    });

    const nextTurnId = topVisibleItem?.dataset.turnId ?? userMessageItems[0]?.item.turnId ?? null;
    const nextInfo = nextTurnId ? (visibleTurnInfoByTurnId.get(nextTurnId) ?? null) : null;

    if (
      currentVisibleTurnInfo?.turnId === nextInfo?.turnId &&
      currentVisibleTurnInfo?.turnIndex === nextInfo?.turnIndex &&
      currentVisibleTurnInfo?.totalTurns === nextInfo?.totalTurns &&
      currentVisibleTurnInfo?.userMessage === nextInfo?.userMessage
    ) {
      return;
    }

    setVisibleTurnInfo(nextInfo);
  }, [userMessageItems, visibleTurnInfoByTurnId]);

  const scheduleVisibleTurnMeasure = useCallback((frames: number = 1) => {
    if (visibleTurnMeasureFrameRef.current !== null) {
      cancelAnimationFrame(visibleTurnMeasureFrameRef.current);
      visibleTurnMeasureFrameRef.current = null;
    }

    const run = (remainingFrames: number) => {
      visibleTurnMeasureFrameRef.current = requestAnimationFrame(() => {
        if (remainingFrames > 1) {
          run(remainingFrames - 1);
          return;
        }

        visibleTurnMeasureFrameRef.current = null;
        measureVisibleTurn();
      });
    };

    run(Math.max(1, frames));
  }, [measureVisibleTurn]);

  const escapeTurnIdSelector = useCallback((turnId: string) => (
    typeof CSS !== 'undefined' && typeof CSS.escape === 'function'
      ? CSS.escape(turnId)
      : turnId.replace(/["\\]/g, '\\$&')
  ), []);

  const getRenderedTurnElements = useCallback((turnId: string, options?: { includeHistoryProjectionHandoff?: boolean }) => {
    const scroller = scrollerElementRef.current;
    if (!scroller) return [];

    const escapedTurnId = escapeTurnIdSelector(turnId);
    const nodes = Array.from(scroller.querySelectorAll<HTMLElement>(
      `.virtual-item-wrapper[data-turn-id="${escapedTurnId}"]`,
    ));

    if (options?.includeHistoryProjectionHandoff === false) {
      return nodes.filter(node => !node.closest('[data-history-projection-handoff="true"]'));
    }

    return nodes;
  }, [escapeTurnIdSelector]);

  const getRenderedUserMessageElement = useCallback((turnId: string) => {
    const scroller = scrollerElementRef.current;
    if (!scroller) return null;

    const escapedTurnId = escapeTurnIdSelector(turnId);
    return scroller.querySelector<HTMLElement>(
      `.virtual-item-wrapper[data-item-type="user-message"][data-turn-id="${escapedTurnId}"]`,
    );
  }, [escapeTurnIdSelector]);

  const getRenderedVirtualItemElement = useCallback((itemIndex: number) => {
    const scroller = scrollerElementRef.current;
    if (!scroller) return null;

    return scroller.querySelector<HTMLElement>(
      `.virtual-item-wrapper[data-virtual-index="${itemIndex}"]`,
    );
  }, []);

  const isTurnRenderedInViewport = useCallback((turnId: string) => {
    const scroller = scrollerElementRef.current;
    if (!scroller) {
      return false;
    }

    const scrollerRect = scroller.getBoundingClientRect();
    const nodes = getRenderedTurnElements(turnId);

    return nodes.some(node => {
      const rect = node.getBoundingClientRect();
      return rect.bottom > scrollerRect.top && rect.top < scrollerRect.bottom;
    });
  }, [getRenderedTurnElements]);

  const isTurnTextRenderedInViewport = useCallback((turnId: string) => {
    const scroller = scrollerElementRef.current;
    if (!scroller) {
      return false;
    }

    const scrollerRect = scroller.getBoundingClientRect();
    const nodes = getRenderedTurnElements(turnId);

    return nodes.some(node => {
      const rect = node.getBoundingClientRect();
      const visible = rect.bottom > scrollerRect.top && rect.top < scrollerRect.bottom;
      return visible && (node.innerText?.trim().length ?? 0) > 0;
    });
  }, [getRenderedTurnElements]);

  const isTurnTextRenderedInViewportOutsideHandoff = useCallback((turnId: string) => {
    const scroller = scrollerElementRef.current;
    if (!scroller) {
      return false;
    }

    const scrollerRect = scroller.getBoundingClientRect();
    const nodes = getRenderedTurnElements(turnId, { includeHistoryProjectionHandoff: false });

    return nodes.some(node => {
      const rect = node.getBoundingClientRect();
      const visible = rect.bottom > scrollerRect.top && rect.top < scrollerRect.bottom;
      return visible && (node.innerText?.trim().length ?? 0) > 0;
    });
  }, [getRenderedTurnElements]);

  const hasVisibleRenderedVirtualItem = useCallback(() => {
    const scroller = scrollerElementRef.current;
    if (!scroller) {
      return false;
    }

    const scrollerRect = scroller.getBoundingClientRect();
    const nodes = Array.from(
      scroller.querySelectorAll<HTMLElement>('.virtual-item-wrapper[data-turn-id]'),
    );

    return nodes.some(node => {
      const rect = node.getBoundingClientRect();
      const style = window.getComputedStyle(node);
      return (
        rect.bottom > scrollerRect.top &&
        rect.top < scrollerRect.bottom &&
        rect.width > 0 &&
        rect.height > 0 &&
        style.visibility !== 'hidden' &&
        style.display !== 'none' &&
        (node.innerText?.trim().length ?? 0) > 0
      );
    });
  }, []);

  const isHistoryProjectionHandoffTargetReady = useCallback((snapshot: HistoryProjectionHandoffSnapshot) => {
    if (snapshot.targetTurnId) {
      return isTurnTextRenderedInViewportOutsideHandoff(snapshot.targetTurnId);
    }

    return hasVisibleRenderedVirtualItem();
  }, [
    hasVisibleRenderedVirtualItem,
    isTurnTextRenderedInViewportOutsideHandoff,
  ]);

  const clearHistoryProjectionHandoff = useCallback((reason: string) => {
    if (historyProjectionHandoffReleaseFrameRef.current !== null) {
      cancelAnimationFrame(historyProjectionHandoffReleaseFrameRef.current);
      historyProjectionHandoffReleaseFrameRef.current = null;
    }

    const snapshot = historyProjectionHandoffRef.current;
    historyProjectionHandoffRef.current = null;
    setHistoryProjectionHandoff(null);

    if (snapshot) {
      startupTrace.markPhase('flowchat_history_projection_handoff_cleared', {
        sessionId: snapshot.sessionId,
        reason,
        handoffReason: snapshot.reason,
        durationMs: Math.round(performance.now() - snapshot.createdAtMs),
      });
    }
  }, []);

  const scheduleHistoryProjectionHandoffRelease = useCallback((frames = 1) => {
    if (historyProjectionHandoffReleaseFrameRef.current !== null) {
      cancelAnimationFrame(historyProjectionHandoffReleaseFrameRef.current);
      historyProjectionHandoffReleaseFrameRef.current = null;
    }

    const run = (remainingFrames: number) => {
      historyProjectionHandoffReleaseFrameRef.current = requestAnimationFrame(() => {
        historyProjectionHandoffReleaseFrameRef.current = null;

        const snapshot = historyProjectionHandoffRef.current;
        if (!snapshot) {
          return;
        }

        if (remainingFrames > 1) {
          run(remainingFrames - 1);
          return;
        }

        if (activeSessionIdRef.current !== snapshot.sessionId) {
          clearHistoryProjectionHandoff('session-changed');
          return;
        }

        if (isHistoryProjectionHandoffTargetReady(snapshot)) {
          clearHistoryProjectionHandoff('target-content-ready');
          return;
        }

        if (performance.now() - snapshot.createdAtMs >= HISTORY_PROJECTION_HANDOFF_MAX_DURATION_MS) {
          clearHistoryProjectionHandoff('timeout');
          return;
        }

        run(1);
      });
    };

    run(Math.max(1, frames));
  }, [clearHistoryProjectionHandoff, isHistoryProjectionHandoffTargetReady]);

  const buildPinReservation = useCallback((
    turnId: string,
    pinMode: FlowChatPinTurnToTopMode,
    requiredTailSpacePx: number,
    maxPinReservationPx: number,
    currentPinReservation: PinBottomReservation = bottomReservationStateRef.current.pin,
  ): PinBottomReservation => {
    const resolvedRequiredTailSpacePx = clampPinReservationPxToViewport(
      requiredTailSpacePx,
      maxPinReservationPx,
    );
    const nextFloorPx = pinMode === 'sticky-latest'
      ? resolvedRequiredTailSpacePx
      : 0;
    // Only preserve a sticky pin after a real floor has been measured.
    // Provisional fallback reservations should shrink on the next resolve.
    const shouldPreserveCurrentPx = (
      currentPinReservation.mode === pinMode &&
      currentPinReservation.targetTurnId === turnId &&
      (
        pinMode === 'transient' ||
        currentPinReservation.floorPx > COMPENSATION_EPSILON_PX
      )
    );
    const preservedPx = shouldPreserveCurrentPx
      ? clampPinReservationPxToViewport(currentPinReservation.px, maxPinReservationPx)
      : 0;
    const shouldRetainTarget = (
      pinMode === 'sticky-latest' ||
      resolvedRequiredTailSpacePx > COMPENSATION_EPSILON_PX ||
      shouldPreserveCurrentPx
    );

    return {
      kind: 'pin',
      px: Math.max(nextFloorPx, resolvedRequiredTailSpacePx, preservedPx),
      floorPx: nextFloorPx,
      mode: pinMode,
      targetTurnId: shouldRetainTarget ? turnId : null,
    };
  }, []);

  const resolveTurnPinMetrics = useCallback((turnId: string, ignoredTailSpacePx: number = 0) => {
    const scroller = scrollerElementRef.current;
    if (!scroller) return null;

    const targetElement = getRenderedUserMessageElement(turnId);
    if (!targetElement) return null;

    const scrollerRect = scroller.getBoundingClientRect();
    const targetRect = targetElement.getBoundingClientRect();
    const viewportTop = scrollerRect.top + PINNED_TURN_VIEWPORT_OFFSET_PX;
    const desiredScrollTop = Math.max(0, scroller.scrollTop + (targetRect.top - viewportTop));
    const effectiveScrollHeight = Math.max(0, scroller.scrollHeight - Math.max(0, ignoredTailSpacePx));
    const rawMaxScrollTop = effectiveScrollHeight - scroller.clientHeight;
    const maxScrollTop = Math.max(0, rawMaxScrollTop);
    // When content is shorter than the viewport, the clamped max scroll range is 0
    // even though we still need to reserve the underflow gap before the target can pin.
    const rawMissingTailSpace = Math.max(0, desiredScrollTop - rawMaxScrollTop);
    const missingTailSpace = clampPinReservationPxToViewport(
      rawMissingTailSpace,
      scroller.clientHeight,
    );
    if (
      flowChatDiagnostics.isEnabled() &&
      rawMissingTailSpace - missingTailSpace > COMPENSATION_EPSILON_PX
    ) {
      flowChatDiagnostics.trace({
        hypothesis: 'F',
        location: 'VirtualMessageList.resolveTurnPinMetrics',
        message: 'Pin reservation exceeded the viewport and was clamped',
        data: () => ({
          turnId,
          rawMissingTailSpace,
          missingTailSpace,
          clientHeight: scroller.clientHeight,
          desiredScrollTop,
          rawMaxScrollTop,
          effectiveScrollHeight,
          ignoredTailSpacePx,
        }),
      });
    }

    return {
      targetElement,
      viewportTop,
      desiredScrollTop,
      maxScrollTop,
      missingTailSpace,
    };
  }, [getRenderedUserMessageElement]);

  const reconcileStickyPinReservation = useCallback(() => {
    const scroller = scrollerElementRef.current;
    const currentState = bottomReservationStateRef.current;
    const pinReservation = currentState.pin;
    if (!scroller || pinReservation.mode !== 'sticky-latest' || !pinReservation.targetTurnId) {
      return false;
    }

    const collapseIntent = pendingCollapseIntentRef.current;
    const hasActiveCollapseProtection = (
      collapseIntent.active
    );
    // During a collapse animation, let collapse compensation own the footer space.
    // Recomputing sticky pin floor from intermediate DOM heights causes the two
    // reservations to fight each other and reintroduces visible vertical jitter.
    if (hasActiveCollapseProtection) {
      return false;
    }

    const resolvedMetrics = resolveTurnPinMetrics(
      pinReservation.targetTurnId,
      pinReservation.px,
    );
    if (!resolvedMetrics) {
      return false;
    }

    const requiredFloorPx = clampPinReservationPxToViewport(
      resolvedMetrics.missingTailSpace,
      scroller.clientHeight,
    );
    const boundedCurrentPx = clampPinReservationPxToViewport(
      pinReservation.px,
      scroller.clientHeight,
    );
    const boundedCurrentFloorPx = clampPinReservationPxToViewport(
      pinReservation.floorPx,
      scroller.clientHeight,
    );
    const currentReservationExceedsViewport = (
      pinReservation.px - boundedCurrentPx > COMPENSATION_EPSILON_PX ||
      pinReservation.floorPx - boundedCurrentFloorPx > COMPENSATION_EPSILON_PX
    );
    // A live target rect is transient while Virtuoso is reconciling item
    // measurements. It is safe to increase the range from that snapshot, but
    // reducing the floor can remove scroll range for a frame and strand the
    // semantic anchor at the physical bottom. Real content growth drains the
    // floor through measureHeightChange instead; the viewport cap is the only
    // synchronous reduction allowed here.
    if (
      requiredFloorPx <= pinReservation.floorPx + COMPENSATION_EPSILON_PX &&
      !currentReservationExceedsViewport
    ) {
      return false;
    }

    const nextFloorPx = Math.max(requiredFloorPx, boundedCurrentFloorPx);
    const nextPinReservation: PinBottomReservation = {
      ...pinReservation,
      px: clampPinReservationPxToViewport(
        Math.max(nextFloorPx, boundedCurrentPx),
        scroller.clientHeight,
      ),
      floorPx: nextFloorPx,
    };

    if (
      Math.abs(nextPinReservation.px - pinReservation.px) <= COMPENSATION_EPSILON_PX &&
      Math.abs(nextPinReservation.floorPx - pinReservation.floorPx) <= COMPENSATION_EPSILON_PX
    ) {
      return false;
    }

    const nextState: BottomReservationState = {
      ...currentState,
      pin: nextPinReservation,
    };
    updateBottomReservationState(nextState);
    applyFooterCompensationNow(nextState);
    viewportCoordinatorRef.current.restoreElementAnchor(scroller, 'pin-reservation-grow');
    previousMeasuredHeightRef.current = snapshotMeasuredContentHeight(scroller, nextState);
    return true;
  }, [
    applyFooterCompensationNow,
    resolveTurnPinMetrics,
    snapshotMeasuredContentHeight,
    updateBottomReservationState,
  ]);

  const schedulePinReservationReconcile = useCallback((frames: number = 1) => {
    if (pinReservationReconcileFrameRef.current !== null) {
      cancelAnimationFrame(pinReservationReconcileFrameRef.current);
      pinReservationReconcileFrameRef.current = null;
    }

    const run = (remainingFrames: number) => {
      pinReservationReconcileFrameRef.current = requestAnimationFrame(() => {
        if (remainingFrames > 1) {
          run(remainingFrames - 1);
          return;
        }

        pinReservationReconcileFrameRef.current = null;
        reconcileStickyPinReservation();
      });
    };

    run(Math.max(1, frames));
  }, [reconcileStickyPinReservation]);

  const clearExpiredProvisionalStickyPin = useCallback((request: PendingTurnPinState) => {
    const currentState = bottomReservationStateRef.current;
    if (!shouldClearExpiredProvisionalStickyPin({
      requestTurnId: request.turnId,
      requestPinMode: request.pinMode,
      pinReservation: currentState.pin,
      ownsElementAnchor: viewportCoordinatorRef.current.ownsElementAnchor(),
    })) {
      return false;
    }

    const nextState: BottomReservationState = {
      ...currentState,
      pin: {
        kind: 'pin',
        px: 0,
        floorPx: 0,
        mode: 'transient',
        targetTurnId: null,
      },
    };
    viewportCoordinatorRef.current.release('unresolved-turn-pin-expired');
    updateBottomReservationState(nextState);
    applyFooterCompensationNow(nextState);

    const scroller = scrollerElementRef.current;
    if (scroller) {
      previousScrollTopRef.current = scroller.scrollTop;
      previousMeasuredHeightRef.current = snapshotMeasuredContentHeight(scroller, nextState);
      recordScrollerGeometry(scroller);
    }
    return true;
  }, [
    applyFooterCompensationNow,
    recordScrollerGeometry,
    snapshotMeasuredContentHeight,
    updateBottomReservationState,
  ]);

  const scrollStaticTurnToTop = useCallback((turnId: string, behavior: ScrollBehavior) => {
    const scroller = scrollerElementRef.current;
    const targetElement = getRenderedUserMessageElement(turnId);
    if (!scroller || !targetElement) {
      return false;
    }

    const targetRect = targetElement.getBoundingClientRect();
    const scrollerRect = scroller.getBoundingClientRect();
    const targetScrollTop = Math.max(
      0,
      scroller.scrollTop + targetRect.top - scrollerRect.top - PINNED_TURN_VIEWPORT_OFFSET_PX,
    );
    cancelStaticInitialHistoryBottomGuard();
    scroller.scrollTo({
      top: targetScrollTop,
      behavior,
    });
    previousScrollTopRef.current = targetScrollTop;
    previousMeasuredHeightRef.current = snapshotMeasuredContentHeight(scroller);
    recordScrollerGeometry(scroller);
    recordStaticInitialHistoryBottomState(scroller);
    setIsAtBottom(false);
    scheduleVisibleTurnMeasure(2);
    return true;
  }, [
    cancelStaticInitialHistoryBottomGuard,
    getRenderedUserMessageElement,
    recordScrollerGeometry,
    recordStaticInitialHistoryBottomState,
    scheduleVisibleTurnMeasure,
    snapshotMeasuredContentHeight,
  ]);

  useLayoutEffect(() => {
    const pending = pendingStaticTurnPinRef.current;
    if (!pending || pending.turnId !== staticAnchorWindowTurnId) {
      return;
    }

    if (scrollStaticTurnToTop(pending.turnId, pending.behavior)) {
      pendingStaticTurnPinRef.current = null;
    }
  }, [
    scrollStaticTurnToTop,
    staticAnchorWindowTurnId,
    virtualItems,
  ]);

  const tryResolvePendingTurnPin = useCallback((request: PendingTurnPinState) => {
    if (!isTurnPinRequestCurrent(request)) {
      return false;
    }

    const scroller = scrollerElementRef.current;
    const virtuoso = virtuosoRef.current;

    if (!scroller || !virtuoso) {
      startupTrace.markPhase('flowchat_turn_pin_resolve', {
        result: 'missing_scroller_or_virtuoso',
        turnId: request.turnId,
        pinMode: request.pinMode,
        attempt: request.attempts,
        hasScroller: Boolean(scroller),
        hasVirtuoso: Boolean(virtuoso),
      });
      return false;
    }

    const targetItem = userMessageItems.find(({ item }) => item.turnId === request.turnId);
    if (!targetItem) {
      startupTrace.markPhase('flowchat_turn_pin_resolve', {
        result: 'missing_target_item',
        turnId: request.turnId,
        pinMode: request.pinMode,
        attempt: request.attempts,
        userMessageCount: userMessageItems.length,
      });
      return false;
    }

    const currentPinReservation = bottomReservationStateRef.current.pin;
    // Existing pin tail space is synthetic footer reservation, not real content.
    // Ignore it when resolving a new pin target so maxScrollTop is computed against
    // the effective content height instead of the previous pin reservation.
    let ignoredTailSpacePx = 0;
    if (currentPinReservation.px > COMPENSATION_EPSILON_PX) {
      ignoredTailSpacePx = currentPinReservation.px;
    }
    const resolvedMetrics = resolveTurnPinMetrics(request.turnId, ignoredTailSpacePx);
    if (!resolvedMetrics) {
      // A far smooth scroll can leave the target virtualized long enough for the
      // deferred pin to expire. Materialize it first, then let the existing live
      // geometry pass perform the exact alignment and stabilization.
      const fallbackBehavior: ScrollBehavior = 'auto';
      const provisionalPinPx = request.pinMode === 'sticky-latest'
        ? resolveProvisionalStickyPinReservationPx({
          scrollHeight: scroller.scrollHeight,
          clientHeight: scroller.clientHeight,
          currentPinPx: currentPinReservation.px,
        })
        : 0;

      if (request.pinMode === 'sticky-latest' && provisionalPinPx > COMPENSATION_EPSILON_PX) {
        // Reserve enough tail space before the target is rendered so the first
        // fallback scroll does not briefly land on the physical bottom.
        const nextReservationState: BottomReservationState = {
          ...bottomReservationStateRef.current,
          pin: {
            kind: 'pin',
            px: provisionalPinPx,
            floorPx: 0,
            mode: request.pinMode,
            targetTurnId: request.turnId,
          },
        };
        updateBottomReservationState(nextReservationState);
        applyFooterCompensationNow(nextReservationState);
        previousMeasuredHeightRef.current = snapshotMeasuredContentHeight(scroller, nextReservationState);
      }

      virtuoso.scrollToIndex({
        index: toVirtuosoIndex(targetItem.index),
        align: 'start',
        behavior: fallbackBehavior,
      });
      startupTrace.markPhase('flowchat_turn_pin_resolve', {
        result: 'target_not_rendered_fallback_scroll_to_index',
        turnId: request.turnId,
        pinMode: request.pinMode,
        attempt: request.attempts,
        targetIndex: targetItem.index,
        scrollTop: scroller.scrollTop,
        scrollHeight: scroller.scrollHeight,
        clientHeight: scroller.clientHeight,
      });
      return false;
    }

    const nextReservationState: BottomReservationState = {
      ...bottomReservationStateRef.current,
      pin: buildPinReservation(
        request.turnId,
        request.pinMode,
        resolvedMetrics.missingTailSpace,
        scroller.clientHeight,
      ),
    };
    updateBottomReservationState(nextReservationState);
    applyFooterCompensationNow(nextReservationState);

    const resolvedMaxScrollTop = Math.max(0, scroller.scrollHeight - scroller.clientHeight);
    const targetScrollTop = Math.min(resolvedMetrics.desiredScrollTop, resolvedMaxScrollTop);
    if (Math.abs(scroller.scrollTop - targetScrollTop) > COMPENSATION_EPSILON_PX) {
      scroller.scrollTop = targetScrollTop;
    }

    // Some turn jumps align correctly at first, then drift on the next frame as
    // Virtuoso finishes layout stabilization. Re-check the live DOM before we
    // decide the pin has truly settled.
    const verifyPinAlignment = (frameLabel: string) => {
      const liveTargetElement = getRenderedUserMessageElement(request.turnId);
      const liveRect = liveTargetElement?.getBoundingClientRect();
      const viewportTop = liveTargetElement
        ? scroller.getBoundingClientRect().top + PINNED_TURN_VIEWPORT_OFFSET_PX
        : null;
      const deltaToViewportTop = liveRect && viewportTop != null
        ? liveRect.top - viewportTop
        : null;

      const stickyPinStillTargetsRequest = (
        bottomReservationStateRef.current.pin.mode === 'sticky-latest' &&
        bottomReservationStateRef.current.pin.targetTurnId === request.turnId
      );
      // Sticky latest pins should keep correcting post-layout drift until the
      // target stabilizes, while transient jumps still back off if the user
      // has already moved away from the requested position.
      const shouldRealign = (
        frameLabel !== 'immediate' &&
        deltaToViewportTop != null &&
        Math.abs(deltaToViewportTop) > 1.5 &&
        (
          request.pinMode === 'transient'
            ? Math.abs(scroller.scrollTop - targetScrollTop) <= 2
            : stickyPinStillTargetsRequest
        )
      );
      if (!shouldRealign) {
        return;
      }

      const correctedMaxScrollTop = Math.max(0, scroller.scrollHeight - scroller.clientHeight);
      const correctedScrollTop = Math.min(
        correctedMaxScrollTop,
        Math.max(0, scroller.scrollTop + deltaToViewportTop),
      );
      if (Math.abs(correctedScrollTop - scroller.scrollTop) <= COMPENSATION_EPSILON_PX) {
        return;
      }

      scroller.scrollTop = correctedScrollTop;
      previousScrollTopRef.current = correctedScrollTop;
      previousMeasuredHeightRef.current = snapshotMeasuredContentHeight(
        scroller,
        bottomReservationStateRef.current,
      );
      recordScrollerGeometry(scroller);
      scheduleVisibleTurnMeasure(2);
      schedulePinReservationReconcile(2);
    };
    verifyPinAlignment('immediate');
    // The observed drift lands after the initial alignment, so sample two
    // follow-up frames and realign only if the target actually shifts.
    requestAnimationFrame(() => {
      verifyPinAlignment('raf-1');
      requestAnimationFrame(() => {
        verifyPinAlignment('raf-2');
      });
    });

    previousScrollTopRef.current = targetScrollTop;
    previousMeasuredHeightRef.current = snapshotMeasuredContentHeight(scroller, nextReservationState);
    recordScrollerGeometry(scroller);

    const alignedRect = resolvedMetrics.targetElement.getBoundingClientRect();
    const alignedWithinTolerance = Math.abs(alignedRect.top - resolvedMetrics.viewportTop) <= 1.5;
    if (alignedWithinTolerance) {
      viewportCoordinatorRef.current.pinElement(resolvedMetrics.targetElement);
    }
    startupTrace.markPhase('flowchat_turn_pin_resolve', {
      result: alignedWithinTolerance ? 'aligned' : 'not_aligned_after_scroll',
      turnId: request.turnId,
      pinMode: request.pinMode,
      attempt: request.attempts,
      targetIndex: targetItem.index,
      targetScrollTop,
      scrollTop: scroller.scrollTop,
      targetTop: alignedRect.top,
      viewportTop: resolvedMetrics.viewportTop,
    });

    return alignedWithinTolerance;
  }, [
    buildPinReservation,
    applyFooterCompensationNow,
    getRenderedUserMessageElement,
    isTurnPinRequestCurrent,
    recordScrollerGeometry,
    resolveTurnPinMetrics,
    schedulePinReservationReconcile,
    scheduleVisibleTurnMeasure,
    snapshotMeasuredContentHeight,
    toVirtuosoIndex,
    updateBottomReservationState,
    userMessageItems,
  ]);

  const reconcileTransientTurnPinStabilization = useCallback(() => {
    const request = transientTurnPinStabilizationRef.current;
    if (!request) {
      return;
    }

    if (request.pinMode !== 'transient' || performance.now() > request.expiresAtMs) {
      transientTurnPinStabilizationRef.current = null;
      return;
    }

    const nextRequest: PendingTurnPinState = {
      ...request,
      behavior: 'auto',
      attempts: request.attempts + 1,
    };
    transientTurnPinStabilizationRef.current = nextRequest;

    if (tryResolvePendingTurnPin(nextRequest)) {
      scheduleVisibleTurnMeasure(2);
    }
  }, [scheduleVisibleTurnMeasure, tryResolvePendingTurnPin]);

  const scheduleTransientTurnPinStabilization = useCallback((frames: number = 1) => {
    if (!transientTurnPinStabilizationRef.current) {
      return;
    }

    if (turnPinStabilizationFrameRef.current !== null) {
      cancelAnimationFrame(turnPinStabilizationFrameRef.current);
      turnPinStabilizationFrameRef.current = null;
    }

    const run = (remainingFrames: number) => {
      turnPinStabilizationFrameRef.current = requestAnimationFrame(() => {
        if (remainingFrames > 1) {
          run(remainingFrames - 1);
          return;
        }

        turnPinStabilizationFrameRef.current = null;
        reconcileTransientTurnPinStabilization();
      });
    };

    run(Math.max(1, frames));
  }, [reconcileTransientTurnPinStabilization]);

  const drainCollapseReservationPreservingPinnedItem = useCallback((reason: string) => {
    const currentState = bottomReservationStateRef.current;
    if (getReservationTotalPx(currentState.collapse) <= COMPENSATION_EPSILON_PX) {
      return currentState;
    }

    const scroller = scrollerElementRef.current;
    const ownsPinnedItem = viewportCoordinatorRef.current.getMode() === 'pinned-item';
    const stickyPinTarget = (
      ownsPinnedItem &&
      currentState.pin.mode === 'sticky-latest' &&
      currentState.pin.targetTurnId
    )
      ? currentState.pin.targetTurnId
      : null;
    const resolvedPinMetrics = scroller && stickyPinTarget
      ? resolveTurnPinMetrics(
        stickyPinTarget,
        getTotalBottomCompensationPx(currentState),
      )
      : null;
    if (stickyPinTarget && !resolvedPinMetrics) {
      return null;
    }

    const nextPinReservation = stickyPinTarget && resolvedPinMetrics && scroller
      ? buildPinReservation(
        stickyPinTarget,
        'sticky-latest',
        resolvedPinMetrics.missingTailSpace,
        scroller.clientHeight,
        currentState.pin,
      )
      : currentState.pin;
    const nextState = transferCollapseReservationToPin(currentState, nextPinReservation);
    updateBottomReservationState(nextState);
    applyFooterCompensationNow(nextState);
    if (scroller && stickyPinTarget) {
      viewportCoordinatorRef.current.restoreElementAnchor(
        scroller,
        `collapse-drain:${reason}`,
      );
    }
    return nextState;
  }, [
    applyFooterCompensationNow,
    buildPinReservation,
    getTotalBottomCompensationPx,
    resolveTurnPinMetrics,
    updateBottomReservationState,
  ]);

  const clearCollapseIntentScheduling = useCallback(() => {
    if (collapseIntentFinalizeTimerRef.current !== null) {
      window.clearTimeout(collapseIntentFinalizeTimerRef.current);
      collapseIntentFinalizeTimerRef.current = null;
    }
    if (collapseIntentAnimationTimerRef.current !== null) {
      window.clearTimeout(collapseIntentAnimationTimerRef.current);
      collapseIntentAnimationTimerRef.current = null;
    }
    if (collapseIntentSettleFrameRef.current !== null) {
      cancelAnimationFrame(collapseIntentSettleFrameRef.current);
      collapseIntentSettleFrameRef.current = null;
    }
  }, []);

  const finalizeCollapseIntent = useCallback((
    reason: string,
    options?: { expectedExpiresAtMs?: number; suppressHandoff?: boolean },
  ) => {
    const intent = pendingCollapseIntentRef.current;
    if (
      !intent.active ||
      (
        options?.expectedExpiresAtMs !== undefined &&
        intent.expiresAtMs !== options.expectedExpiresAtMs
      )
    ) {
      return false;
    }

    if (flowChatDiagnostics.isEnabled()) {
      flowChatDiagnostics.trace({
        hypothesis: 'E',
        location: 'VirtualMessageList.finalizeCollapseIntent',
        message: 'Collapse intent finalization started',
        data: () => ({
          reason,
          intent,
          coordinatorMode: viewportCoordinatorRef.current.getMode(),
          reservation: bottomReservationStateRef.current,
          isFollowingOutput: isFollowingOutputRef.current,
          isStreamingOutput: isStreamingOutputRef.current,
        }),
      });
    }

    clearCollapseIntentScheduling();
    pendingCollapseIntentRef.current = createInactiveCollapseIntentState();
    const coordinatorMode = viewportCoordinatorRef.current.getMode();
    const shouldRetainForQuietSettlement = (
      isFollowingOutputRef.current &&
      (coordinatorMode === 'following-tail' || coordinatorMode === 'pinned-item')
    );
    if (shouldRetainForQuietSettlement) {
      retainCollapseRangeForQuietSettlement(intent, reason);
    }
    if (coordinatorMode === 'following-tail' && isFollowingOutputRef.current) {
      if (deferredFollowReasonRef.current) {
        const deferredReason = deferredFollowReasonRef.current;
        deferredFollowReasonRef.current = null;
        followOutputControllerRef.current.scheduleFollowToLatest(
          `${deferredReason}-after-collapse`,
        );
      }
      return true;
    }

    const preserveReservation = shouldPreserveCollapseReservationAfterIntent({
      isFollowingOutput: isFollowingOutputRef.current,
      isStreamingOutput: isStreamingOutputRef.current,
      isPreservingElement: coordinatorMode === 'preserving-element',
      hasProtectedCollapseRange:
        bottomReservationStateRef.current.collapse.floorPx > COMPENSATION_EPSILON_PX,
    });
    const scroller = scrollerElementRef.current;
    const nextState = preserveReservation
      ? coordinatorMode === 'preserving-element'
        ? scroller
          ? settleCollapseReservationForPreservedViewport(
            bottomReservationStateRef.current,
            {
              scrollTop: scroller.scrollTop,
              scrollHeight: scroller.scrollHeight,
              clientHeight: scroller.clientHeight,
            },
          )
          : protectCurrentCollapseReservation(bottomReservationStateRef.current)
        : bottomReservationStateRef.current
      : drainCollapseReservationPreservingPinnedItem(reason);
    if (nextState === null) {
      pendingCollapseIntentRef.current = intent;
      collapseIntentFinalizeTimerRef.current = window.setTimeout(() => {
        collapseIntentFinalizeTimerRef.current = null;
        finalizeCollapseIntentRef.current('collapse-intent-target-retry', {
          expectedExpiresAtMs: intent.expiresAtMs,
          suppressHandoff: options?.suppressHandoff,
        });
      }, 50);
      return false;
    }

    if (!areBottomReservationStatesEqual(bottomReservationStateRef.current, nextState)) {
      updateBottomReservationState(nextState);
      applyFooterCompensationNow(nextState);
    }
    if (coordinatorMode === 'preserving-element' && scroller) {
      viewportCoordinatorRef.current.restoreElementAnchor(
        scroller,
        `collapse-finalized:${reason}`,
      );
      const restoredState = protectCurrentCollapseReservation(
        bottomReservationStateRef.current,
      );
      if (!areBottomReservationStatesEqual(bottomReservationStateRef.current, restoredState)) {
        updateBottomReservationState(restoredState);
        applyFooterCompensationNow(restoredState);
      }
      previousScrollTopRef.current = scroller.scrollTop;
      previousMeasuredHeightRef.current = snapshotMeasuredContentHeight(scroller);
      recordScrollerGeometry(scroller);
    }
    if (coordinatorMode === 'preserving-element') {
      viewportCoordinatorRef.current.settleElementPreservation(
        `collapse-finalized:${reason}`,
      );
    }

    if (deferredFollowReasonRef.current) {
      const deferredReason = deferredFollowReasonRef.current;
      deferredFollowReasonRef.current = null;
      followOutputControllerRef.current.scheduleFollowToLatest(`${deferredReason}-after-collapse`);
    }
    if (!options?.suppressHandoff) {
      maybeHandoffPinnedTurnToTailRef.current(`collapse-finalized:${reason}`);
    }
    return true;
  }, [
    applyFooterCompensationNow,
    clearCollapseIntentScheduling,
    drainCollapseReservationPreservingPinnedItem,
    recordScrollerGeometry,
    retainCollapseRangeForQuietSettlement,
    snapshotMeasuredContentHeight,
    updateBottomReservationState,
  ]);
  finalizeCollapseIntentRef.current = finalizeCollapseIntent;

  const scheduleCollapseIntentFinalization = useCallback((
    intent: PendingCollapseIntentState,
    reason: string | null | undefined,
  ) => {
    clearCollapseIntentScheduling();
    const expectedExpiresAtMs = intent.expiresAtMs;
    collapseIntentFinalizeTimerRef.current = window.setTimeout(() => {
      collapseIntentFinalizeTimerRef.current = null;
      finalizeCollapseIntentRef.current('collapse-intent-timeout', {
        expectedExpiresAtMs,
      });
    }, Math.max(0, expectedExpiresAtMs - performance.now()));

    if (reason !== 'auto') {
      return;
    }

    // Auto collapses animate for FLOWCHAT_COLLAPSE_DURATION_MS. Finalize only
    // after that window plus a short settle so reservation tracks the moving
    // height instead of ending while the box is still closing.
    const settle = (remainingFrames: number) => {
      collapseIntentSettleFrameRef.current = requestAnimationFrame(() => {
        if (remainingFrames > 1) {
          settle(remainingFrames - 1);
          return;
        }
        collapseIntentSettleFrameRef.current = null;
        finalizeCollapseIntentRef.current('auto-collapse-layout-settled', {
          expectedExpiresAtMs,
        });
      });
    };
    collapseIntentAnimationTimerRef.current = window.setTimeout(() => {
      collapseIntentAnimationTimerRef.current = null;
      settle(AUTO_COLLAPSE_SETTLE_FRAMES);
    }, FLOWCHAT_COLLAPSE_DURATION_MS);
  }, [clearCollapseIntentScheduling]);

  useEffect(() => clearCollapseIntentScheduling, [clearCollapseIntentScheduling]);

  const handleScrollerRef = useCallback((el: HTMLElement | Window | null) => {
    restoreScrollerMethodsRef.current?.();
    restoreScrollerMethodsRef.current = null;

    if (el && el instanceof HTMLElement) {
      const originalScrollTo = el.scrollTo;
      const originalScrollBy = el.scrollBy;
      const wrapScrollMethod = (
        method: 'scrollTo' | 'scrollBy',
        original: typeof el.scrollTo,
      ): typeof el.scrollTo => {
        return ((...args: unknown[]) => {
          let requestedTop: number | null = null;
          const firstArg = args[0];
          if (typeof firstArg === 'number' && typeof args[1] === 'number') {
            requestedTop = args[1];
          } else if (
            firstArg !== null &&
            typeof firstArg === 'object' &&
            'top' in firstArg &&
            typeof firstArg.top === 'number'
          ) {
            requestedTop = firstArg.top;
          }
          const previousGeometry = previousScrollerGeometryRef.current;
          const previousMaxScrollTop = previousGeometry
            ? Math.max(0, previousGeometry.scrollHeight - previousGeometry.clientHeight)
            : null;
          const wasAtPhysicalBottomBeforeScrollBy = Boolean(
            previousGeometry &&
            previousMaxScrollTop !== null &&
            Math.abs(previousMaxScrollTop - previousGeometry.scrollTop) <= LATEST_END_ANCHOR_STABLE_EPSILON_PX
          );
          const hasSemanticAnchor = (
            method === 'scrollBy' &&
            viewportCoordinatorRef.current.ownsElementAnchor()
          );
          const suppressVirtualizerCompensation = (
            hasSemanticAnchor ||
            (
              method === 'scrollBy' &&
              shouldSuppressFollowingTailNegativeScrollBy({
                requestedTop,
                isFollowingOutput: isFollowingOutputRef.current,
                isStreamingOutput: isStreamingOutputRef.current,
                wasAtPhysicalBottom: wasAtPhysicalBottomBeforeScrollBy,
              })
            )
          );
          const diagnosticsEnabled = flowChatDiagnostics.isEnabled();
          const scrollTopBefore = diagnosticsEnabled ? el.scrollTop : null;
          if (!suppressVirtualizerCompensation) {
            Reflect.apply(original, el, args);
          }
          if (diagnosticsEnabled) {
            flowChatDiagnostics.trace({
              hypothesis: 'D',
              location: `VirtualMessageList.scroller.${method}`,
              message: suppressVirtualizerCompensation
                ? 'Virtualizer scroll request suppressed'
                : 'Virtualizer scroll request applied',
              data: () => ({
                method,
                requestedTop,
                suppressVirtualizerCompensation,
                hasSemanticAnchor,
                wasAtPhysicalBottomBeforeScrollBy,
                isFollowingOutput: isFollowingOutputRef.current,
                isStreamingOutput: isStreamingOutputRef.current,
                coordinatorMode: viewportCoordinatorRef.current.getMode(),
                scrollTopBefore,
                scrollTopAfter: el.scrollTop,
                scrollHeight: el.scrollHeight,
                clientHeight: el.clientHeight,
              }),
            });
          }
        }) as typeof el.scrollTo;
      };
      const wrappedScrollTo = typeof originalScrollTo === 'function'
        ? wrapScrollMethod('scrollTo', originalScrollTo)
        : null;
      const wrappedScrollBy = typeof originalScrollBy === 'function'
        ? wrapScrollMethod('scrollBy', originalScrollBy)
        : null;
      if (wrappedScrollTo) {
        el.scrollTo = wrappedScrollTo;
      }
      if (wrappedScrollBy) {
        el.scrollBy = wrappedScrollBy;
      }
      restoreScrollerMethodsRef.current = () => {
        if (wrappedScrollTo && el.scrollTo === wrappedScrollTo) {
          el.scrollTo = originalScrollTo;
        }
        if (wrappedScrollBy && el.scrollBy === wrappedScrollBy) {
          el.scrollBy = originalScrollBy;
        }
      };
      scrollerElementRef.current = el;
      setScrollerElement(el);
      return;
    }

    scrollerElementRef.current = null;
    setScrollerElement(null);
  }, []);

  const clearPreviousHistoryBoundaryStatus = useCallback(() => {
    if (previousHistoryBoundaryStatusTimerRef.current !== null) {
      window.clearTimeout(previousHistoryBoundaryStatusTimerRef.current);
      previousHistoryBoundaryStatusTimerRef.current = null;
    }
    setPreviousHistoryBoundaryStatus(null);
  }, []);

  const showPreviousHistoryBoundaryStatus = useCallback((
    sessionId: string,
    reason: string,
    state: 'preparing' | 'not-ready'
  ) => {
    if (previousHistoryBoundaryStatusTimerRef.current !== null) {
      window.clearTimeout(previousHistoryBoundaryStatusTimerRef.current);
      previousHistoryBoundaryStatusTimerRef.current = null;
    }

    setPreviousHistoryBoundaryStatus({
      sessionId,
      reason,
      state,
    });

    previousHistoryBoundaryStatusTimerRef.current = window.setTimeout(() => {
      previousHistoryBoundaryStatusTimerRef.current = null;
      setPreviousHistoryBoundaryStatus(current => {
        if (
          current?.sessionId === sessionId &&
          current.reason === reason &&
          current.state === state
        ) {
          return null;
        }
        return current;
      });
    }, PREVIOUS_HISTORY_BOUNDARY_STATUS_DURATION_MS);
  }, []);

  useEffect(() => () => {
    if (previousHistoryBoundaryStatusTimerRef.current !== null) {
      window.clearTimeout(previousHistoryBoundaryStatusTimerRef.current);
      previousHistoryBoundaryStatusTimerRef.current = null;
    }
  }, []);

  useEffect(() => {
    setPreviousHistoryBoundaryStatus(current => {
      if (current && current.sessionId !== activeSessionId) {
        return null;
      }
      return current;
    });
  }, [activeSessionId]);

  const revealPreviousHistoryWindowForUserIntent = useCallback((reason: string) => {
    const sessionId = activeSession?.sessionId;
    if (
      !sessionId ||
      activeSession.historyState !== 'ready' ||
      activeSession.isPartial !== true
    ) {
      return;
    }

    if (!flowChatStore.hasDeferredSessionHistoryProjection(sessionId)) {
      if (flowChatStore.hasPendingSessionHistoryCompletion(sessionId)) {
        const released = flowChatStore.releaseSessionHistoryCompletionAfterInitialPaint(sessionId, {
          immediate: true,
          reason,
        });
        showPreviousHistoryBoundaryStatus(sessionId, reason, 'preparing');
        startupTrace.markPhase('flowchat_previous_history_window_preparing', {
          sessionId,
          reason,
          released,
        });
      } else {
        showPreviousHistoryBoundaryStatus(sessionId, reason, 'not-ready');
        startupTrace.markPhase('flowchat_previous_history_window_not_ready', {
          sessionId,
          reason,
        });
      }
      return;
    }

    const revealed = flowChatStore.revealPreviousSessionHistoryWindow(sessionId, reason);
    if (revealed) {
      clearPreviousHistoryBoundaryStatus();
    } else {
      showPreviousHistoryBoundaryStatus(sessionId, reason, 'not-ready');
    }
    startupTrace.markPhase('flowchat_previous_history_window_requested', {
      sessionId,
      reason,
      revealed,
    });
  }, [
    activeSession?.historyState,
    activeSession?.isPartial,
    activeSession?.sessionId,
    clearPreviousHistoryBoundaryStatus,
    showPreviousHistoryBoundaryStatus,
  ]);

  const shouldRevealPreviousHistoryWindowForUserIntent = useCallback((options?: { force?: boolean }) => {
    if (options?.force === true) {
      return true;
    }

    const scroller = scrollerElementRef.current;
    if (!scroller) {
      return false;
    }

    const topThresholdPx = Math.max(
      PARTIAL_HISTORY_FULL_PROJECTION_TOP_THRESHOLD_PX,
      scroller.clientHeight * 1.5,
    );
    return scroller.scrollTop <= topThresholdPx;
  }, []);

  const schedulePreviousHistoryWindowForUserIntent = useCallback((reason: string, options?: { force?: boolean }) => {
    const scheduledSessionId = activeSession?.sessionId ?? null;
    pendingFullHistoryProjectionReasonRef.current = reason;
    if (fullHistoryProjectionIntentFrameRef.current !== null) {
      cancelAnimationFrame(fullHistoryProjectionIntentFrameRef.current);
      fullHistoryProjectionIntentFrameRef.current = null;
    }

    fullHistoryProjectionIntentFrameRef.current = requestAnimationFrame(() => {
      fullHistoryProjectionIntentFrameRef.current = requestAnimationFrame(() => {
        fullHistoryProjectionIntentFrameRef.current = null;
        const pendingReason = pendingFullHistoryProjectionReasonRef.current;
        pendingFullHistoryProjectionReasonRef.current = null;
        if (!pendingReason || activeSessionIdRef.current !== scheduledSessionId) {
          return;
        }

        if (!shouldRevealPreviousHistoryWindowForUserIntent(options)) {
          return;
        }

        revealPreviousHistoryWindowForUserIntent(pendingReason);
      });
    });
  }, [
    activeSession?.sessionId,
    revealPreviousHistoryWindowForUserIntent,
    shouldRevealPreviousHistoryWindowForUserIntent,
  ]);

  const shouldSuspendAutoFollow = useCallback(() => {
    const collapseIntent = pendingCollapseIntentRef.current;
    return collapseIntent.active;
  }, []);

  const scheduleFollowToLatestWithViewportState = useCallback((reason: string) => {
    const collapseIntentActive = shouldSuspendAutoFollow();
    if (collapseIntentActive) {
      deferredFollowReasonRef.current = reason;
      return;
    }
    deferredFollowReasonRef.current = null;
    followOutputControllerRef.current.scheduleFollowToLatest(reason);
  }, [shouldSuspendAutoFollow]);

  useLayoutEffect(() => {
    previousMeasuredHeightRef.current = null;
    previousScrollTopRef.current = 0;
    viewportCoordinatorRef.current.release('session-reset');
    clearTurnPinRequest();
    cancelLatestEndAnchorStabilization();
    cancelStaticInitialHistoryBottomGuard();
    clearCollapseIntentScheduling();
    clearRetainedCollapseSettlement();
    clearPendingStickyPinGrowth('session-reset');
    pendingCollapseIntentRef.current = createInactiveCollapseIntentState();
    retainedCollapseAnchorRef.current = null;
    previousScrollerGeometryRef.current = null;
    pendingStaticAnchorTurnIdRef.current = null;
    pendingStaticTurnPinRef.current = null;
    const currentSessionId = activeSession?.sessionId ?? null;
    const activeHandoff = historyProjectionHandoffRef.current;
    if (!activeHandoff || activeHandoff.sessionId !== currentSessionId) {
      clearHistoryProjectionHandoff('session-reset');
    }
    resetBottomReservations();
  }, [
    activeSession?.sessionId,
    cancelLatestEndAnchorStabilization,
    cancelStaticInitialHistoryBottomGuard,
    clearCollapseIntentScheduling,
    clearRetainedCollapseSettlement,
    clearPendingStickyPinGrowth,
    clearHistoryProjectionHandoff,
    clearTurnPinRequest,
    resetBottomReservations,
  ]);

  useEffect(() => {
    previousIsStreamingOutputRef.current = false;
  }, [activeSession?.sessionId]);

  useEffect(() => {
    if (virtualItems.length === 0) {
      previousMeasuredHeightRef.current = null;
      clearTurnPinRequest();
      cancelLatestEndAnchorStabilization();
      clearPendingStickyPinGrowth('empty-list');
      resetBottomReservations();
    }
  }, [
    virtualItems.length,
    cancelLatestEndAnchorStabilization,
    clearPendingStickyPinGrowth,
    clearTurnPinRequest,
    resetBottomReservations,
  ]);

  useEffect(() => {
    return () => {
      clearRetainedCollapseSettlement();
      cancelLatestEndAnchorStabilization();
      cancelStaticInitialHistoryBottomGuard();
      if (historyProjectionHandoffReleaseFrameRef.current !== null) {
        cancelAnimationFrame(historyProjectionHandoffReleaseFrameRef.current);
        historyProjectionHandoffReleaseFrameRef.current = null;
      }
    };
  }, [
    cancelLatestEndAnchorStabilization,
    cancelStaticInitialHistoryBottomGuard,
    clearRetainedCollapseSettlement,
  ]);

  useEffect(() => {
    if (!scrollerElement) {
      previousMeasuredHeightRef.current = null;
      previousScrollerGeometryRef.current = null;
      cancelStaticInitialHistoryBottomGuard();
      return;
    }

    const resizeTarget =
      scrollerElement.firstElementChild instanceof HTMLElement
        ? scrollerElement.firstElementChild
        : scrollerElement;

    previousMeasuredHeightRef.current = snapshotMeasuredContentHeight(scrollerElement);
    previousScrollTopRef.current = scrollerElement.scrollTop;
    recordScrollerGeometry(scrollerElement);

    // Batch observer-triggered work into a single rAF per frame
    let observerBatchPending = false;
    const scheduleObserverBatch = () => {
      if (observerBatchPending) return;
      observerBatchPending = true;
      requestAnimationFrame(() => {
        observerBatchPending = false;
        scheduleHeightMeasure(2);
        scheduleVisibleTurnMeasure(2);
        schedulePinReservationReconcile(2);
        scheduleTransientTurnPinStabilization(2);
        scheduleFollowToLatestWithViewportState('observer');
        scheduleHistoryProjectionHandoffRelease(1);
      });
    };

    resizeObserverRef.current?.disconnect();
    resizeObserverRef.current = new ResizeObserver(() => {
      syncPhysicalBottomAfterViewportResize(scrollerElement);
      resolveLatestEndAnchorStabilizationRef.current?.('resize-observer');
      scheduleObserverBatch();
    });
    resizeObserverRef.current.observe(resizeTarget);
    if (resizeTarget !== scrollerElement) {
      resizeObserverRef.current.observe(scrollerElement);
    }

    const handleWindowResize = () => {
      syncPhysicalBottomAfterViewportResize(scrollerElement);
      resolveLatestEndAnchorStabilizationRef.current?.('resize-observer');
      scheduleObserverBatch();
    };
    window.addEventListener('resize', handleWindowResize);
    window.visualViewport?.addEventListener('resize', handleWindowResize);

    mutationObserverRef.current?.disconnect();
    let mutationPending = false;
    mutationObserverRef.current = new MutationObserver((mutations) => {
      if (mutationPending) return;
      if (!isProcessing) {
        return;
      }
      const characterDataMutations = mutations.filter(mutation => mutation.type === 'characterData');
      const attributesMutations = mutations.filter(mutation => mutation.type === 'attributes');
      const hasSemanticMutation = (
        characterDataMutations.length > 0 ||
        attributesMutations.length > 0
      );
      if (!hasSemanticMutation) {
        return;
      }
      mutationPending = true;
      requestAnimationFrame(() => {
        mutationPending = false;
        scheduleObserverBatch();
      });
    });
    mutationObserverRef.current.observe(scrollerElement, {
      subtree: true,
      childList: true,
      characterData: true,
    });

    // The owned timer/frame finalizer does not depend on scroll events. Keep
    // this check only as a backup for browsers that throttle background timers.
    const replayDeferredFollowIfSettled = () => {
      const now = performance.now();
      const intent = pendingCollapseIntentRef.current;
      if (intent.active && intent.expiresAtMs < now) {
        finalizeCollapseIntent('collapse-intent-scroll-backup', {
          expectedExpiresAtMs: intent.expiresAtMs,
        });
      }
    };

    const handleScroll = () => {
      const now = performance.now();
      const intent = pendingCollapseIntentRef.current;
      const retainedCollapseAnchor = retainedCollapseAnchorRef.current;
      const collapseProtectionActive = intent.active || retainedCollapseAnchor !== null;

      // Reactive shrink-clamp restore: in follow + streaming mode, an upward
      // jump in scrollTop that we did NOT request from JS and that is NOT
      // attributable to a user gesture is the browser auto-clamping scrollTop
      // because `scrollHeight` shrunk below `scrollTop + clientHeight`
      // (typical cause: an unsignaled item shrink from Virtuoso re-measure
      // or a tool result finalizing). With `overflow-anchor: none` we cannot
      // ask the browser to keep the visual anchor for us.
      //
      // Unsignaled follow-mode shrink remains owned by the continuous tail loop.
      // A known active or retained collapse is different: its transaction owns
      // a bounded target and can restore a non-user clamp synchronously.
      const intentCheckScrollTop = scrollerElement.scrollTop;
      const intentCheckPreviousScrollTop = previousScrollTopRef.current;
      const intentCheckScrollDelta = intentCheckScrollTop - intentCheckPreviousScrollTop;
      const hasRecentUserUpwardIntent = now <= userInitiatedUpwardScrollUntilMsRef.current;
      if (
        flowChatDiagnostics.isEnabled() &&
        Math.abs(intentCheckScrollDelta) > COMPENSATION_EPSILON_PX
      ) {
        flowChatDiagnostics.trace({
          hypothesis: 'A',
          location: 'VirtualMessageList.handleScroll',
          message: 'Scroller position changed',
          data: () => ({
            scrollTop: intentCheckScrollTop,
            previousScrollTop: intentCheckPreviousScrollTop,
            scrollDelta: intentCheckScrollDelta,
            scrollHeight: scrollerElement.scrollHeight,
            clientHeight: scrollerElement.clientHeight,
            hasRecentUserUpwardIntent,
            scrollbarPointerInteractionActive: scrollbarPointerInteractionActiveRef.current,
            coordinatorMode: viewportCoordinatorRef.current.getMode(),
            reservation: bottomReservationStateRef.current,
            isFollowingOutput: isFollowingOutputRef.current,
            isStreamingOutput: isStreamingOutputRef.current,
            collapseProtectionActive,
          }),
        });
      }
      const collapseAnchorScrollTop = intent.active
        ? intent.anchorScrollTop
        : retainedCollapseAnchor?.anchorScrollTop ?? null;
      if (
        collapseAnchorScrollTop !== null &&
        intentCheckScrollDelta < -COMPENSATION_EPSILON_PX &&
        !hasRecentUserUpwardIntent &&
        collapseAnchorScrollTop - intentCheckScrollTop > COMPENSATION_EPSILON_PX
      ) {
        preserveCollapseAnchorScrollTop(
          scrollerElement,
          collapseAnchorScrollTop,
          intent.active ? 'scroll-active-collapse' : 'scroll-retained-collapse',
        );
        if (!intent.active) {
          scheduleRetainedCollapseSettlement('delayed-scroll-clamp');
        }
        return;
      }
      if (
        intentCheckScrollDelta < -COMPENSATION_EPSILON_PX &&
        isFollowingOutputRef.current &&
        isStreamingOutputRef.current &&
        !hasRecentUserUpwardIntent &&
        !collapseProtectionActive
      ) {
        if (flowChatDiagnostics.isEnabled()) {
          flowChatDiagnostics.trace({
            hypothesis: 'D',
            location: 'VirtualMessageList.handleScroll',
            message: 'Upward clamp delegated to the streaming follow loop',
            data: () => ({
              scrollTop: intentCheckScrollTop,
              previousScrollTop: intentCheckPreviousScrollTop,
              scrollDelta: intentCheckScrollDelta,
            }),
          });
        }
        // Follow+streaming: do not inject compensation or restore old
        // scrollTop. Let the follow loop handle the scroll naturally on the
        // next animation frame. Return here to prevent the downstream follow
        // controller (followOutputControllerRef.current.handleScroll) from
        // seeing the browser-clamp delta and misclassifying it as a user
        // upward scroll, which would incorrectly exit follow mode.
        previousScrollTopRef.current = intentCheckScrollTop;
        recordScrollerGeometry(scrollerElement);
        return;
      }

      const currentTotalCompensation = getTotalBottomCompensationPx();
      if (
        currentTotalCompensation > COMPENSATION_EPSILON_PX &&
        !intent.active
      ) {
        const nextScrollTop = scrollerElement.scrollTop;
        const scrollDelta = nextScrollTop - previousScrollTopRef.current;
        if (scrollDelta > COMPENSATION_EPSILON_PX) {
          const nextCompensationState = consumeBottomCompensation(scrollDelta);
          applyFooterCompensationNow(nextCompensationState);
          if (retainedCollapseAnchorRef.current) {
            retainedCollapseAnchorRef.current.anchorScrollTop = Math.max(
              retainedCollapseAnchorRef.current.anchorScrollTop,
              scrollerElement.scrollTop,
            );
          }
          previousMeasuredHeightRef.current = snapshotMeasuredContentHeight(
            scrollerElement,
            nextCompensationState,
          );
        }
      }

      if (viewportCoordinatorRef.current.scheduleElementAnchorRestore(scrollerElement, 'scroll-handler')) {
        previousScrollTopRef.current = scrollerElement.scrollTop;
        recordScrollerGeometry(scrollerElement);
        return;
      }
      previousScrollTopRef.current = scrollerElement.scrollTop;
      recordScrollerGeometryIfLayoutStable(scrollerElement);
      recordStaticInitialHistoryBottomState(scrollerElement);
      scheduleVisibleTurnMeasure();
      followOutputControllerRef.current.handleScroll();
      if (
        performance.now() <= userInitiatedUpwardScrollUntilMsRef.current ||
        scrollbarPointerInteractionActiveRef.current
      ) {
        schedulePreviousHistoryWindowForUserIntent('scroll-near-partial-history-boundary');
      }

      replayDeferredFollowIfSettled();
    };
    scrollerElement.addEventListener('scroll', handleScroll, { passive: true });

    const handleWheel = (event: WheelEvent) => {
      if (flowChatDiagnostics.isEnabled() && event.deltaY !== 0) {
        flowChatDiagnostics.trace({
          hypothesis: 'A',
          location: 'VirtualMessageList.handleWheel',
          message: 'Wheel scroll intent observed',
          data: () => ({
            deltaY: event.deltaY,
            scrollTop: scrollerElement.scrollTop,
            coordinatorMode: viewportCoordinatorRef.current.getMode(),
          }),
        });
      }
      if (event.deltaY !== 0) {
        notifyUserScrollIntent();
        clearTurnPinRequest();
        cancelLatestEndAnchorStabilization();
      }

      if (event.deltaY < 0) {
        staticInitialHistoryUserLeftBottomRef.current = true;
        userInitiatedUpwardScrollUntilMsRef.current =
          performance.now() + USER_UPWARD_SCROLL_INTENT_WINDOW_MS;
        schedulePreviousHistoryWindowForUserIntent('wheel-up');
        followOutputControllerRef.current.handleUserScrollIntent();
      }
    };

    const handleTouchStart = (event: TouchEvent) => {
      touchScrollIntentStartYRef.current = event.touches[0]?.clientY ?? null;
    };

    const handleTouchMove = (event: TouchEvent) => {
      const startY = touchScrollIntentStartYRef.current;
      const currentY = event.touches[0]?.clientY;
      if (startY === null || currentY === undefined) {
        return;
      }

      if (Math.abs(currentY - startY) > TOUCH_SCROLL_INTENT_EXIT_THRESHOLD_PX) {
        if (flowChatDiagnostics.isEnabled()) {
          flowChatDiagnostics.trace({
            hypothesis: 'A',
            location: 'VirtualMessageList.handleTouchMove',
            message: 'Touch scroll intent crossed the exit threshold',
            data: () => ({ deltaY: currentY - startY, scrollTop: scrollerElement.scrollTop }),
          });
        }
        notifyUserScrollIntent();
        clearTurnPinRequest();
        cancelLatestEndAnchorStabilization();
      }

      if (currentY - startY > TOUCH_SCROLL_INTENT_EXIT_THRESHOLD_PX) {
        touchScrollIntentStartYRef.current = currentY;
        staticInitialHistoryUserLeftBottomRef.current = true;
        userInitiatedUpwardScrollUntilMsRef.current =
          performance.now() + USER_UPWARD_SCROLL_INTENT_WINDOW_MS;
        schedulePreviousHistoryWindowForUserIntent('touch-scroll-up');
        followOutputControllerRef.current.handleUserScrollIntent();
      }
    };

    const resetTouchScrollIntent = () => {
      touchScrollIntentStartYRef.current = null;
    };

    const handleKeyDown = (event: KeyboardEvent) => {
      if (!isScrollIntentKey(event) || isEditableElement(event.target)) {
        return;
      }

      notifyUserScrollIntent();
      if (flowChatDiagnostics.isEnabled()) {
        flowChatDiagnostics.trace({
          hypothesis: 'A',
          location: 'VirtualMessageList.handleKeyDown',
          message: 'Keyboard scroll intent observed',
          data: () => ({ key: event.key, scrollTop: scrollerElement.scrollTop }),
        });
      }
      clearTurnPinRequest();
      cancelLatestEndAnchorStabilization();

      if (!isUpwardScrollIntentKey(event)) {
        return;
      }

      staticInitialHistoryUserLeftBottomRef.current = true;
      userInitiatedUpwardScrollUntilMsRef.current =
        performance.now() + USER_UPWARD_SCROLL_INTENT_WINDOW_MS;
      schedulePreviousHistoryWindowForUserIntent('keyboard-scroll-up', { force: event.key === 'Home' });
      followOutputControllerRef.current.handleUserScrollIntent();
    };

    const handlePointerDown = (event: PointerEvent) => {
      if (event.pointerType === 'touch' || event.button !== 0) {
        return;
      }

      if (!isPointerOnScrollbarGutter(scrollerElement, event.clientX, event.clientY)) {
        return;
      }

      scrollbarPointerInteractionActiveRef.current = true;
      if (flowChatDiagnostics.isEnabled()) {
        flowChatDiagnostics.trace({
          hypothesis: 'A',
          location: 'VirtualMessageList.handlePointerDown',
          message: 'Scrollbar pointer interaction started',
          data: () => ({ scrollTop: scrollerElement.scrollTop }),
        });
      }
      notifyUserScrollIntent();
      clearTurnPinRequest();
      cancelLatestEndAnchorStabilization();
      staticInitialHistoryUserLeftBottomRef.current = true;
      userInitiatedUpwardScrollUntilMsRef.current =
        performance.now() + USER_UPWARD_SCROLL_INTENT_WINDOW_MS;
      schedulePreviousHistoryWindowForUserIntent('scrollbar-pointer-down');
      followOutputControllerRef.current.handleUserScrollIntent();
    };

    const handlePointerMove = (event: PointerEvent) => {
      if (!scrollbarPointerInteractionActiveRef.current || event.pointerType === 'touch') {
        return;
      }

      if ((event.buttons & 1) !== 1) {
        scrollbarPointerInteractionActiveRef.current = false;
        return;
      }

      clearTurnPinRequest();
      if (flowChatDiagnostics.isEnabled()) {
        flowChatDiagnostics.trace({
          hypothesis: 'A',
          location: 'VirtualMessageList.handlePointerMove',
          message: 'Scrollbar pointer interaction moved',
          data: () => ({ scrollTop: scrollerElement.scrollTop }),
        });
      }
      notifyUserScrollIntent();
      cancelLatestEndAnchorStabilization();
      staticInitialHistoryUserLeftBottomRef.current = true;
      userInitiatedUpwardScrollUntilMsRef.current =
        performance.now() + USER_UPWARD_SCROLL_INTENT_WINDOW_MS;
      schedulePreviousHistoryWindowForUserIntent('scrollbar-pointer-move');
      followOutputControllerRef.current.handleUserScrollIntent();
    };

    const endScrollbarPointerInteraction = () => {
      scrollbarPointerInteractionActiveRef.current = false;
    };

    scrollerElement.addEventListener('wheel', handleWheel, { passive: true });
    scrollerElement.addEventListener('touchstart', handleTouchStart, { passive: true });
    scrollerElement.addEventListener('touchmove', handleTouchMove, { passive: true });
    scrollerElement.addEventListener('touchend', resetTouchScrollIntent, { passive: true });
    scrollerElement.addEventListener('touchcancel', resetTouchScrollIntent, { passive: true });
    scrollerElement.addEventListener('keydown', handleKeyDown, true);
    scrollerElement.addEventListener('pointerdown', handlePointerDown, true);
    window.addEventListener('pointermove', handlePointerMove, true);
    window.addEventListener('pointerup', endScrollbarPointerInteraction, true);
    window.addEventListener('pointercancel', endScrollbarPointerInteraction, true);

    const handleToolCardToggle = () => {
      scheduleHeightMeasure(2);
      scheduleVisibleTurnMeasure(2);
      schedulePinReservationReconcile(2);
      scheduleHistoryProjectionHandoffRelease(1);
    };

    const handleToolCardCollapseIntent = (event: Event) => {
      const detail = (event as CustomEvent<{
        toolId?: string | null;
        toolName?: string | null;
        cardHeight?: number | null;
        anchorElement?: HTMLElement | null;
        filePath?: string | null;
        reason?: string | null;
      }>).detail;
      const previousIntent = pendingCollapseIntentRef.current;
      clearRetainedCollapseSettlement();
      if (flowChatDiagnostics.isEnabled()) {
        flowChatDiagnostics.trace({
          hypothesis: 'E',
          location: 'VirtualMessageList.handleToolCardCollapseIntent',
          message: 'Tool card collapse intent received',
          data: () => ({
            reason: detail?.reason ?? null,
            toolName: detail?.toolName ?? null,
            cardHeight: detail?.cardHeight ?? null,
            hasAnchorElement: Boolean(detail?.anchorElement),
            anchorElementConnected: detail?.anchorElement?.isConnected ?? null,
            previousIntent,
            coordinatorMode: viewportCoordinatorRef.current.getMode(),
            reservation: bottomReservationStateRef.current,
            scrollTop: scrollerElement.scrollTop,
            scrollHeight: scrollerElement.scrollHeight,
            clientHeight: scrollerElement.clientHeight,
            isFollowingOutput: isFollowingOutputRef.current,
            isStreamingOutput: isStreamingOutputRef.current,
          }),
        });
      }
      // Coalesce overlapping collapses instead of finalizing the previous
      // intent. Finalizing mid-animation can briefly drop footer protection and
      // flash the pane when several cards compact in the same stream burst.
      if (detail?.anchorElement) {
        viewportCoordinatorRef.current.preserveElement(detail.anchorElement);
      } else if (!previousIntent.active) {
        viewportCoordinatorRef.current.preserveElement(null);
      }

      const baseTotalCompensationPx = previousIntent.active
        ? previousIntent.baseTotalCompensationPx
        : getTotalBottomCompensationPx();
      const currentScrollTop = scrollerElement.scrollTop;
      const previousStableScrollTop = previousScrollTopRef.current;
      const anchorScrollTop = previousIntent.active
        ? previousIntent.anchorScrollTop
        : resolveAutoCollapseAnchorScrollTop({
          currentScrollTop,
          previousStableScrollTop,
          reason: detail?.reason,
          isFollowingOutput: isFollowingOutputRef.current,
          isStreamingOutput: isStreamingOutputRef.current,
        });
      const distanceFromBottom = Math.max(
        0,
        scrollerElement.scrollHeight - scrollerElement.clientHeight - currentScrollTop
      );
      const currentTotalCompensationPx = getTotalBottomCompensationPx();
      const effectiveDistanceFromBottom = previousIntent.active
        ? previousIntent.distanceFromBottomBeforeCollapse
        : Math.max(0, distanceFromBottom - currentTotalCompensationPx);
      const estimatedShrink = Math.max(0, detail?.cardHeight ?? 0);
      const cumulativeShrinkPx = previousIntent.active
        ? previousIntent.cumulativeShrinkPx
        : 0;
      const provisionalTotalCompensationPx = Math.max(
        currentTotalCompensationPx,
        baseTotalCompensationPx + Math.max(
          0,
          cumulativeShrinkPx + estimatedShrink - effectiveDistanceFromBottom,
        ),
      );
      const nextIntent: PendingCollapseIntentState = {
        active: true,
        anchorScrollTop,
        toolId: detail?.toolId ?? previousIntent.toolId ?? null,
        toolName: detail?.toolName ?? previousIntent.toolName ?? null,
        expiresAtMs: performance.now() + COLLAPSE_INTENT_TTL_MS,
        distanceFromBottomBeforeCollapse: effectiveDistanceFromBottom,
        baseTotalCompensationPx,
        cumulativeShrinkPx,
      };
      pendingCollapseIntentRef.current = nextIntent;
      if (flowChatDiagnostics.isEnabled()) {
        flowChatDiagnostics.trace({
          hypothesis: 'E',
          location: 'VirtualMessageList.handleToolCardCollapseIntent',
          message: 'Tool card collapse reservation calculated',
          data: () => ({
            nextIntent,
            baseTotalCompensationPx,
            currentTotalCompensationPx,
            provisionalTotalCompensationPx,
            effectiveDistanceFromBottom,
            estimatedShrink,
            currentScrollTop,
            previousStableScrollTop,
            coordinatorMode: viewportCoordinatorRef.current.getMode(),
          }),
        });
      }
      if (provisionalTotalCompensationPx - baseTotalCompensationPx > COMPENSATION_EPSILON_PX) {
        const nextReservationState: BottomReservationState = {
          ...bottomReservationStateRef.current,
          collapse: {
            ...bottomReservationStateRef.current.collapse,
            px: Math.max(0, provisionalTotalCompensationPx - getReservationTotalPx(bottomReservationStateRef.current.pin)),
          },
        };
        updateBottomReservationState(nextReservationState);
        applyFooterCompensationNow(nextReservationState);
      }

      if (anchorScrollTop - currentScrollTop > COMPENSATION_EPSILON_PX) {
        let maxScrollTop = Math.max(
          0,
          scrollerElement.scrollHeight - scrollerElement.clientHeight,
        );
        let rangeExtensionPx = 0;
        if (anchorScrollTop - maxScrollTop > COMPENSATION_EPSILON_PX) {
          rangeExtensionPx = anchorScrollTop - maxScrollTop + 1;
          const currentState = bottomReservationStateRef.current;
          const extendedState: BottomReservationState = {
            ...currentState,
            collapse: {
              ...currentState.collapse,
              px: currentState.collapse.px + rangeExtensionPx,
            },
          };
          updateBottomReservationState(extendedState);
          applyFooterCompensationNow(extendedState);
          maxScrollTop = Math.max(
            0,
            scrollerElement.scrollHeight - scrollerElement.clientHeight,
          );
        }
        scrollerElement.scrollTop = Math.min(anchorScrollTop, maxScrollTop);
        previousScrollTopRef.current = scrollerElement.scrollTop;
        recordScrollerGeometry(scrollerElement);
      }

      scheduleCollapseIntentFinalization(nextIntent, detail?.reason);

      scheduleVisibleTurnMeasure(2);
      schedulePinReservationReconcile(2);
    };

    window.addEventListener('tool-card-toggle', handleToolCardToggle);
    window.addEventListener('flowchat:tool-card-collapse-intent', handleToolCardCollapseIntent as EventListener);
    scheduleVisibleTurnMeasure(2);

    return () => {
      scrollerElement.removeEventListener('scroll', handleScroll);
      scrollerElement.removeEventListener('wheel', handleWheel);
      scrollerElement.removeEventListener('touchstart', handleTouchStart);
      scrollerElement.removeEventListener('touchmove', handleTouchMove);
      scrollerElement.removeEventListener('touchend', resetTouchScrollIntent);
      scrollerElement.removeEventListener('touchcancel', resetTouchScrollIntent);
      scrollerElement.removeEventListener('keydown', handleKeyDown, true);
      scrollerElement.removeEventListener('pointerdown', handlePointerDown, true);
      window.removeEventListener('pointermove', handlePointerMove, true);
      window.removeEventListener('pointerup', endScrollbarPointerInteraction, true);
      window.removeEventListener('pointercancel', endScrollbarPointerInteraction, true);
      window.removeEventListener('tool-card-toggle', handleToolCardToggle);
      window.removeEventListener('flowchat:tool-card-collapse-intent', handleToolCardCollapseIntent as EventListener);
      window.removeEventListener('resize', handleWindowResize);
      window.visualViewport?.removeEventListener('resize', handleWindowResize);
      resizeObserverRef.current?.disconnect();
      resizeObserverRef.current = null;
      mutationObserverRef.current?.disconnect();
      mutationObserverRef.current = null;
      touchScrollIntentStartYRef.current = null;
      scrollbarPointerInteractionActiveRef.current = false;

      if (measureFrameRef.current !== null) {
        cancelAnimationFrame(measureFrameRef.current);
        measureFrameRef.current = null;
      }

      if (visibleTurnMeasureFrameRef.current !== null) {
        cancelAnimationFrame(visibleTurnMeasureFrameRef.current);
        visibleTurnMeasureFrameRef.current = null;
      }

      if (pinReservationReconcileFrameRef.current !== null) {
        cancelAnimationFrame(pinReservationReconcileFrameRef.current);
        pinReservationReconcileFrameRef.current = null;
      }

      if (turnPinStabilizationFrameRef.current !== null) {
        cancelAnimationFrame(turnPinStabilizationFrameRef.current);
        turnPinStabilizationFrameRef.current = null;
      }

      if (fullHistoryProjectionIntentFrameRef.current !== null) {
        cancelAnimationFrame(fullHistoryProjectionIntentFrameRef.current);
        fullHistoryProjectionIntentFrameRef.current = null;
      }
      cancelStaticInitialHistoryBottomGuard();
      pendingFullHistoryProjectionReasonRef.current = null;
    };
  }, [
    applyFooterCompensationNow,
    cancelLatestEndAnchorStabilization,
    cancelStaticInitialHistoryBottomGuard,
    consumeBottomCompensation,
    clearRetainedCollapseSettlement,
    clearTurnPinRequest,
    finalizeCollapseIntent,
    getTotalBottomCompensationPx,
    latestTurnId,
    notifyUserScrollIntent,
    pendingTurnPin?.pinMode,
    pendingTurnPin?.turnId,
    preserveCollapseAnchorScrollTop,
    recordScrollerGeometry,
    recordScrollerGeometryIfLayoutStable,
    recordStaticInitialHistoryBottomState,
    scheduleHeightMeasure,
    scheduleFollowToLatestWithViewportState,
    schedulePreviousHistoryWindowForUserIntent,
    scheduleHistoryProjectionHandoffRelease,
    schedulePinReservationReconcile,
    scheduleRetainedCollapseSettlement,
    scheduleTransientTurnPinStabilization,
    scheduleVisibleTurnMeasure,
    scrollerElement,
    scheduleCollapseIntentFinalization,
    snapshotMeasuredContentHeight,
    syncPhysicalBottomAfterViewportResize,
    isProcessing,
    updateBottomReservationState,
  ]);

  const resolveLatestEndAnchorStabilization = useCallback((reason: LatestEndAnchorResolveReason) => {
    const request = latestEndAnchorRequestRef.current;
    if (!request) {
      return false;
    }

    const scheduleNextResolve = () => {
      if (latestEndAnchorStabilizationFrameRef.current === null) {
        latestEndAnchorStabilizationFrameRef.current = requestAnimationFrame(() => {
          latestEndAnchorStabilizationFrameRef.current = null;
          resolveLatestEndAnchorStabilization('raf');
        });
      }
    };

    const scroller = scrollerElementRef.current;
    const virtuoso = virtuosoRef.current;
    if (!scroller) {
      request.attempts += 1;
      if (request.attempts >= LATEST_END_ANCHOR_STABILIZATION_MAX_ATTEMPTS) {
        cancelLatestEndAnchorStabilization();
        startupTrace.markPhase('flowchat_latest_end_anchor_unresolved', {
          attempts: request.attempts,
          reason,
          targetIndex: request.targetIndex,
          turnId: request.turnId,
          cause: 'missing_scroller',
        });
        return false;
      }

      scheduleNextResolve();
      return false;
    }

    let targetIndex = request.targetIndex;
    if (targetIndex < 0 || virtualItems[targetIndex]?.turnId !== request.turnId) {
      targetIndex = -1;
      for (let index = virtualItems.length - 1; index >= 0; index -= 1) {
        if (virtualItems[index]?.turnId === request.turnId) {
          targetIndex = index;
          break;
        }
      }
      request.targetIndex = targetIndex;
    }

    if (targetIndex < 0) {
      cancelLatestEndAnchorStabilization();
      return false;
    }

    request.attempts += 1;
    const shouldSnapTargetToPhysicalBottom =
      targetIndex >= virtualItems.length - 1 ||
      request.turnId === latestTurnId;
    const targetElement = getRenderedVirtualItemElement(targetIndex);
    const scrollerRect = scroller.getBoundingClientRect();
    const inputOverlayInsetPx = Math.max(
      0,
      inputStackFooterPxRef.current - FLOWCHAT_MESSAGE_TAIL_CLEARANCE_PX,
    );
    const visibleTop = scrollerRect.top + LATEST_END_ANCHOR_VISIBILITY_MARGIN_PX;
    const visibleBottom = Math.max(
      visibleTop + 1,
      scrollerRect.bottom - inputOverlayInsetPx - LATEST_END_ANCHOR_VISIBILITY_MARGIN_PX,
    );

    const readTargetEndState = (currentTargetElement: HTMLElement | null) => {
      if (!currentTargetElement) {
        return null;
      }
      const rect = currentTargetElement.getBoundingClientRect();
      const maxScrollTop = Math.max(0, scroller.scrollHeight - scroller.clientHeight);
      const endDeltaPx = rect.bottom - visibleBottom;
      const physicalBottomAnchored =
        Math.abs(maxScrollTop - scroller.scrollTop) <= LATEST_END_ANCHOR_STABLE_EPSILON_PX;
      const endAnchored = shouldSnapTargetToPhysicalBottom
        ? physicalBottomAnchored
        : (
          Math.abs(endDeltaPx) <= LATEST_END_ANCHOR_STABLE_EPSILON_PX ||
          (endDeltaPx > 0 && physicalBottomAnchored) ||
          (endDeltaPx < 0 && scroller.scrollTop <= LATEST_END_ANCHOR_STABLE_EPSILON_PX)
        );
      return {
        endAnchored,
        endDeltaPx,
        maxScrollTop,
        rect,
        visible: rect.bottom > visibleTop && rect.top < visibleBottom,
      };
    };

    const settleIfStableEndAnchored = () => {
      const currentTargetElement = getRenderedVirtualItemElement(targetIndex);
      const state = readTargetEndState(currentTargetElement);
      if (!state?.visible || !state.endAnchored) {
        request.visibleFrames = 0;
        request.stableVisibleFrames = 0;
        return false;
      }

      const scrollHeight = scroller.scrollHeight;
      const scrollTop = scroller.scrollTop;
      const geometryStable = (
        request.lastScrollHeight !== null &&
        request.lastScrollTop !== null &&
        request.lastTargetTop !== null &&
        request.lastTargetBottom !== null &&
        Math.abs(scrollHeight - request.lastScrollHeight) <= LATEST_END_ANCHOR_STABLE_EPSILON_PX &&
        Math.abs(scrollTop - request.lastScrollTop) <= LATEST_END_ANCHOR_STABLE_EPSILON_PX &&
        Math.abs(state.rect.top - request.lastTargetTop) <= LATEST_END_ANCHOR_STABLE_EPSILON_PX &&
        Math.abs(state.rect.bottom - request.lastTargetBottom) <= LATEST_END_ANCHOR_STABLE_EPSILON_PX
      );

      request.lastScrollHeight = scrollHeight;
      request.lastScrollTop = scrollTop;
      request.lastTargetTop = state.rect.top;
      request.lastTargetBottom = state.rect.bottom;
      request.visibleFrames += 1;
      request.stableVisibleFrames = geometryStable ? request.stableVisibleFrames + 1 : 1;

      if (
        request.attempts < LATEST_END_ANCHOR_STABILIZATION_MIN_ATTEMPTS ||
        request.stableVisibleFrames < LATEST_END_ANCHOR_STABLE_VISIBLE_FRAMES
      ) {
        scheduleNextResolve();
        return true;
      }

      startupTrace.markPhase('flowchat_latest_end_anchor_settled', {
        attempts: request.attempts,
        reason,
        stableVisibleFrames: request.stableVisibleFrames,
        targetIndex,
        turnId: request.turnId,
      });
      cancelLatestEndAnchorStabilization();
      scheduleVisibleTurnMeasure(1);
      return true;
    };

    if (settleIfStableEndAnchored()) {
      return true;
    }

    request.visibleFrames = 0;
    request.stableVisibleFrames = 0;
    request.lastScrollHeight = null;
    request.lastScrollTop = null;
    request.lastTargetTop = null;
    request.lastTargetBottom = null;

    if (targetElement) {
      const state = readTargetEndState(targetElement);
      let nextScrollTop = scroller.scrollTop;
      if (state) {
        nextScrollTop = shouldSnapTargetToPhysicalBottom
          ? state.maxScrollTop
          : Math.max(
            0,
            Math.min(state.maxScrollTop, scroller.scrollTop + state.endDeltaPx),
          );
      }

      if (Math.abs(nextScrollTop - scroller.scrollTop) > COMPENSATION_EPSILON_PX) {
        scroller.scrollTop = nextScrollTop;
        previousScrollTopRef.current = nextScrollTop;
        previousMeasuredHeightRef.current = snapshotMeasuredContentHeight(scroller);
        recordScrollerGeometry(scroller);
      }
    } else if (virtuoso) {
      request.visibleFrames = 0;
      virtuoso.scrollToIndex({
        index: toVirtuosoIndex(targetIndex),
        align: 'end',
        behavior: 'auto',
      });
      const shouldUseTailFallback =
        targetIndex >= virtualItems.length - 1 ||
        request.turnId === latestTurnId;
      if (shouldUseTailFallback) {
        const maxScrollTop = Math.max(0, scroller.scrollHeight - scroller.clientHeight);
        if (Math.abs(maxScrollTop - scroller.scrollTop) > COMPENSATION_EPSILON_PX) {
          scroller.scrollTop = maxScrollTop;
          previousScrollTopRef.current = maxScrollTop;
          previousMeasuredHeightRef.current = snapshotMeasuredContentHeight(scroller);
          recordScrollerGeometry(scroller);
        }
      }
    } else {
      request.visibleFrames = 0;
      const maxScrollTop = Math.max(0, scroller.scrollHeight - scroller.clientHeight);
      if (Math.abs(maxScrollTop - scroller.scrollTop) > COMPENSATION_EPSILON_PX) {
        scroller.scrollTop = maxScrollTop;
        previousScrollTopRef.current = maxScrollTop;
        previousMeasuredHeightRef.current = snapshotMeasuredContentHeight(scroller);
        recordScrollerGeometry(scroller);
      }
    }

    if (settleIfStableEndAnchored()) {
      return true;
    }

    if (request.attempts >= LATEST_END_ANCHOR_STABILIZATION_MAX_ATTEMPTS) {
      cancelLatestEndAnchorStabilization();
      scheduleVisibleTurnMeasure(1);
      startupTrace.markPhase('flowchat_latest_end_anchor_unresolved', {
        attempts: request.attempts,
        reason,
        targetIndex,
        turnId: request.turnId,
      });
      return false;
    }

    scheduleNextResolve();

    return false;
  }, [
    cancelLatestEndAnchorStabilization,
    getRenderedVirtualItemElement,
    latestTurnId,
    recordScrollerGeometry,
    scheduleVisibleTurnMeasure,
    snapshotMeasuredContentHeight,
    toVirtuosoIndex,
    virtualItems,
  ]);
  resolveLatestEndAnchorStabilizationRef.current = resolveLatestEndAnchorStabilization;

  // `rangeChanged` is affected by overscan/increaseViewportBy, so treat it as a
  // "rendered DOM changed" signal and derive the pinned turn from real DOM visibility.
  const handleRangeChanged = useCallback(() => {
    resolveLatestEndAnchorStabilization('range-changed');
    scheduleVisibleTurnMeasure(2);
    schedulePinReservationReconcile(2);
    scheduleTransientTurnPinStabilization(2);
    scheduleFollowToLatestWithViewportState('range-changed');
    // Reset the handoff release timer rather than accelerating it.
    // A session-open projection handoff needs the Virtuoso measurement
    // to settle before releasing; calling with 3 resets the countdown
    // so the handoff stays until 3 frames of stability pass.
    scheduleHistoryProjectionHandoffRelease(3);
  }, [
    resolveLatestEndAnchorStabilization,
    scheduleFollowToLatestWithViewportState,
    scheduleHistoryProjectionHandoffRelease,
    schedulePinReservationReconcile,
    scheduleTransientTurnPinStabilization,
    scheduleVisibleTurnMeasure,
  ]);

  useEffect(() => {
    if (userMessageItems.length === 0) {
      const setVisibleTurnInfo = useModernFlowChatStore.getState().setVisibleTurnInfo;
      setVisibleTurnInfo(null);
      return;
    }

    scheduleVisibleTurnMeasure(2);
    schedulePinReservationReconcile(2);
    scheduleTransientTurnPinStabilization(2);
  }, [
    activeSession?.sessionId,
    schedulePinReservationReconcile,
    scheduleTransientTurnPinStabilization,
    scheduleVisibleTurnMeasure,
    scrollerElement,
    userMessageItems,
    virtualItems.length,
  ]);

  useEffect(() => {
    if (!pendingTurnPin) return;

    if (!isTurnPinRequestCurrent(pendingTurnPin)) {
      setPendingTurnPin(prev => (
        prev?.generation === pendingTurnPin.generation ? null : prev
      ));
      return;
    }

    if (performance.now() > pendingTurnPin.expiresAtMs) {
      clearTurnPinRequest();
      if (clearExpiredProvisionalStickyPin(pendingTurnPin)) {
        followOutputControllerRef.current.scheduleFollowToLatest(
          'unresolved-turn-pin-expired',
        );
      }
      return;
    }

    const frameId = requestAnimationFrame(() => {
      const resolved = tryResolvePendingTurnPin(pendingTurnPin);
      if (resolved) {
        if (
          pendingTurnPin.pinMode === 'transient' &&
          performance.now() <= pendingTurnPin.expiresAtMs
        ) {
          activateTransientTurnPinStabilization(pendingTurnPin);
          scheduleTransientTurnPinStabilization(2);
          scheduleVisibleTurnMeasure(2);
          setPendingTurnPin(null);
          return;
        }

        clearTurnPinRequest();
        scheduleVisibleTurnMeasure(2);
        return;
      }

      setPendingTurnPin(prev => {
        if (
          !prev ||
          prev.generation !== pendingTurnPin.generation ||
          !isTurnPinRequestCurrent(pendingTurnPin)
        ) {
          return prev;
        }

        return {
          ...prev,
          attempts: prev.attempts + 1,
          behavior: 'auto',
        };
      });
    });

    return () => {
      cancelAnimationFrame(frameId);
    };
  }, [
    activateTransientTurnPinStabilization,
    clearExpiredProvisionalStickyPin,
    clearTurnPinRequest,
    isTurnPinRequestCurrent,
    pendingTurnPin,
    scheduleTransientTurnPinStabilization,
    scheduleVisibleTurnMeasure,
    tryResolvePendingTurnPin,
  ]);

  // ── Navigation helpers ────────────────────────────────────────────────
  const clearAllBottomReservationsForUserNavigation = useCallback(() => {
    const currentState = bottomReservationStateRef.current;
    const scroller = scrollerElementRef.current;
    const nextReservationState = createInitialBottomReservationState();
    const hasActiveReservation = !areBottomReservationStatesEqual(currentState, nextReservationState);

    viewportCoordinatorRef.current.release('clear-all-reservations');
    clearTurnPinRequest();
    clearCollapseIntentScheduling();
    clearPendingStickyPinGrowth('clear-all-reservations');
    pendingCollapseIntentRef.current = createInactiveCollapseIntentState();
    retainedCollapseAnchorRef.current = null;

    if (!hasActiveReservation) {
      return;
    }

    bottomReservationStateRef.current = nextReservationState;
    updateBottomReservationState(nextReservationState);
    applyFooterCompensationNow(nextReservationState);

    if (scroller) {
      previousScrollTopRef.current = scroller.scrollTop;
      previousMeasuredHeightRef.current = snapshotMeasuredContentHeight(scroller, nextReservationState);
      recordScrollerGeometry(scroller);
    }
  }, [
    applyFooterCompensationNow,
    clearTurnPinRequest,
    clearCollapseIntentScheduling,
    clearPendingStickyPinGrowth,
    recordScrollerGeometry,
    snapshotMeasuredContentHeight,
    updateBottomReservationState,
  ]);

  const clearPinReservationForUserNavigation = useCallback((
    reason = 'user-navigation',
    options?: { preserveCurrentRange?: boolean },
  ) => {
    const currentState = bottomReservationStateRef.current;
    const scroller = scrollerElementRef.current;
    const hasActivePin = (
      currentState.pin.px > COMPENSATION_EPSILON_PX ||
      currentState.pin.floorPx > COMPENSATION_EPSILON_PX ||
      currentState.pin.targetTurnId !== null ||
      currentState.pin.mode !== 'transient'
    );

    if (flowChatDiagnostics.isEnabled()) {
      flowChatDiagnostics.trace({
        hypothesis: 'A',
        location: 'VirtualMessageList.clearPinReservationForUserNavigation',
        message: 'User navigation began pin reservation release',
        data: () => ({
          reason,
          preserveCurrentRange: options?.preserveCurrentRange === true,
          hasActivePin,
          reservationBefore: currentState,
          coordinatorMode: viewportCoordinatorRef.current.getMode(),
          scrollTop: scroller?.scrollTop ?? null,
          scrollHeight: scroller?.scrollHeight ?? null,
          clientHeight: scroller?.clientHeight ?? null,
        }),
      });
    }

    clearTurnPinRequest();
    clearPendingStickyPinGrowth(reason);

    if (!hasActivePin) {
      return;
    }

    const nextReservationState = releasePinReservationForUserNavigation(currentState, {
      preserveCurrentRange: options?.preserveCurrentRange === true,
      ownsElementAnchor: viewportCoordinatorRef.current.ownsElementAnchor(),
    });
    if (flowChatDiagnostics.isEnabled()) {
      flowChatDiagnostics.trace({
        hypothesis: 'A',
        location: 'VirtualMessageList.clearPinReservationForUserNavigation',
        message: 'Pin reservation released for user navigation',
        data: () => ({
          reason,
          preserveCurrentRange: options?.preserveCurrentRange === true,
          reservationBefore: currentState,
          reservationAfter: nextReservationState,
        }),
      });
    }
    updateBottomReservationState(nextReservationState);
    applyFooterCompensationNow(nextReservationState);

    if (scroller) {
      previousScrollTopRef.current = scroller.scrollTop;
      previousMeasuredHeightRef.current = snapshotMeasuredContentHeight(scroller, nextReservationState);
      recordScrollerGeometry(scroller);
    }
  }, [
    applyFooterCompensationNow,
    clearPendingStickyPinGrowth,
    clearTurnPinRequest,
    recordScrollerGeometry,
    snapshotMeasuredContentHeight,
    updateBottomReservationState,
  ]);

  exitPinnedViewportForUserIntentRef.current = reason => {
    if (flowChatDiagnostics.isEnabled()) {
      flowChatDiagnostics.trace({
        hypothesis: 'A',
        location: 'VirtualMessageList.exitPinnedViewportForUserIntent',
        message: 'User intent exited the pinned viewport',
        data: () => ({
          reason,
          coordinatorModeBefore: viewportCoordinatorRef.current.getMode(),
          reservationBefore: bottomReservationStateRef.current,
        }),
      });
    }
    viewportCoordinatorRef.current.release(reason);
    clearPinReservationForUserNavigation(reason, { preserveCurrentRange: true });
  };

  const isStreamingOutput = React.useMemo(() => {
    if (isProcessing) {
      return true;
    }

    const dialogTurns = activeSession?.dialogTurns;
    const lastDialogTurn = dialogTurns && dialogTurns.length > 0
      ? dialogTurns[dialogTurns.length - 1]
      : undefined;

    if (!lastDialogTurn) {
      return false;
    }

    if (
      lastDialogTurn.status === 'processing' ||
      lastDialogTurn.status === 'finishing' ||
      lastDialogTurn.status === 'image_analyzing'
    ) {
      return true;
    }

    return lastDialogTurn.modelRounds.some(round => round.isStreaming);
  }, [activeSession, isProcessing]);
  const initialTopMostItemIndex = React.useMemo(() => {
    if (isStreamingOutput) {
      return toVirtuosoIndex(latestUserMessageIndex);
    }

    return {
      index: toVirtuosoIndex(Math.max(0, virtualItems.length - 1)),
      align: 'end' as const,
    };
  }, [
    isStreamingOutput,
    latestUserMessageIndex,
    toVirtuosoIndex,
    virtualItems.length,
  ]);

  useEffect(() => {
    const wasStreaming = previousIsStreamingOutputRef.current;
    previousIsStreamingOutputRef.current = isStreamingOutput;
    if (!wasStreaming || isStreamingOutput) {
      return;
    }

    const scroller = scrollerElementRef.current;
    const distanceFromBottomBefore = scroller
      ? Math.max(0, scroller.scrollHeight - scroller.clientHeight - scroller.scrollTop)
      : 0;
    const wasNearBottom = distanceFromBottomBefore <= 80;

    // Collapse compensation can be much larger than the tail space needed to
    // keep a sticky user message pinned. Transfer it to the pin reservation in
    // one DOM update so reducing the footer cannot temporarily remove the scroll
    // range beneath the semantic anchor.
    const currentReservationState = bottomReservationStateRef.current;
    const coordinatorModeBefore = viewportCoordinatorRef.current.getMode();
    const ownsElementAnchorBefore = viewportCoordinatorRef.current.ownsElementAnchor();
    const collapsePx = getReservationTotalPx(currentReservationState.collapse);
    const activeCollapseIntent = pendingCollapseIntentRef.current;
    const retainedCollapseAnchor = retainedCollapseAnchorRef.current;
    const collapseAnchorToRetain = activeCollapseIntent.active
      ? activeCollapseIntent
      : retainedCollapseAnchor;
    const shouldRetainFollowingTailCollapse = (
      coordinatorModeBefore === 'following-tail' &&
      collapsePx > COMPENSATION_EPSILON_PX &&
      collapseAnchorToRetain !== null
    );
    if (shouldRetainFollowingTailCollapse && collapseAnchorToRetain) {
      retainCollapseRangeForQuietSettlement(
        collapseAnchorToRetain,
        'stream-end',
      );
    }
    const pendingStickyPinGrowthPx = pendingStickyPinGrowthRef.current.amountPx;
    const stickyPinTarget = (
      currentReservationState.pin.mode === 'sticky-latest' &&
      currentReservationState.pin.targetTurnId
    )
      ? currentReservationState.pin.targetTurnId
      : null;
    if (
      !shouldRetainFollowingTailCollapse &&
      (
        collapsePx > COMPENSATION_EPSILON_PX ||
        pendingStickyPinGrowthPx > COMPENSATION_EPSILON_PX ||
        stickyPinTarget
      )
    ) {
      const resolvedPinMetrics = scroller && stickyPinTarget
        ? resolveTurnPinMetrics(
          stickyPinTarget,
          getTotalBottomCompensationPx(currentReservationState),
        )
        : null;
      const requiredPinPx = resolvedPinMetrics
        ? sanitizeReservationPx(resolvedPinMetrics.missingTailSpace)
        : 0;
      const nextPinReservation = stickyPinTarget && resolvedPinMetrics
        ? {
          ...currentReservationState.pin,
          px: requiredPinPx,
          floorPx: requiredPinPx,
          mode: 'sticky-latest' as const,
          targetTurnId: stickyPinTarget,
        }
        : currentReservationState.pin;
      if (!stickyPinTarget || resolvedPinMetrics) {
        const reconciledReservationState = transferCollapseReservationToPin(
          currentReservationState,
          nextPinReservation,
        );
        updateBottomReservationState(reconciledReservationState);
        applyFooterCompensationNow(reconciledReservationState);
      }
      if (scroller && ownsElementAnchorBefore) {
        viewportCoordinatorRef.current.restoreElementAnchor(scroller, 'stream-end-reservation-transfer');
      }
      // Footer height shrank: if we were following the bottom, re-pin in the
      // same turn to avoid a one-frame whole-pane jump that looks like a flash.
      if (scroller && wasNearBottom && !ownsElementAnchorBefore) {
        scroller.scrollTop = Math.max(0, scroller.scrollHeight - scroller.clientHeight);
      }
    }

    // Stream end is an ownership boundary. Preserve the current physical range
    // while removing the pin reservation, then release the semantic anchor so a
    // residual floor cannot keep the coordinator alive after cancellation.
    const shouldReleaseStreamPin = (
      coordinatorModeBefore === 'pinned-item' ||
      bottomReservationStateRef.current.pin.mode === 'sticky-latest'
    );
    if (shouldReleaseStreamPin) {
      const releasedReservationState = releasePinReservationForUserNavigation(
        bottomReservationStateRef.current,
        {
          preserveCurrentRange: true,
          ownsElementAnchor: true,
        },
      );
      updateBottomReservationState(releasedReservationState);
      applyFooterCompensationNow(releasedReservationState);
    }
    if (
      coordinatorModeBefore === 'pinned-item' ||
      (shouldReleaseStreamPin && coordinatorModeBefore === 'idle')
    ) {
      viewportCoordinatorRef.current.release('stream-end-pinned-item');
    }

    // Clear timer-owned work. A following-tail collapse anchor remains retained
    // until delayed virtualizer measurements and real range consumption settle.
    clearTurnPinRequest();
    clearCollapseIntentScheduling();
    clearPendingStickyPinGrowth('stream-end-reconciled');
    pendingCollapseIntentRef.current = createInactiveCollapseIntentState();
    if (!shouldReleaseStreamPin) {
      viewportCoordinatorRef.current.settleElementPreservation('stream-end-reconciled');
      maybeHandoffPinnedTurnToTailRef.current('stream-end');
    }

    if (flowChatDiagnostics.isEnabled()) {
      flowChatDiagnostics.trace({
        hypothesis: 'B',
        location: 'VirtualMessageList.streamEndReconciliation',
        message: 'Streaming viewport ownership reconciled',
        data: () => ({
          coordinatorModeBefore,
          coordinatorModeAfter: viewportCoordinatorRef.current.getMode(),
          ownsElementAnchorBefore,
          reservationBefore: currentReservationState,
          reservationAfter: bottomReservationStateRef.current,
          scrollTop: scroller?.scrollTop ?? null,
          scrollHeight: scroller?.scrollHeight ?? null,
          clientHeight: scroller?.clientHeight ?? null,
        }),
      });
    }

    if (scroller) {
      previousScrollTopRef.current = scroller.scrollTop;
      previousMeasuredHeightRef.current = snapshotMeasuredContentHeight(
        scroller,
        bottomReservationStateRef.current,
      );
      recordScrollerGeometry(scroller);
    }

  }, [
    applyFooterCompensationNow,
    clearCollapseIntentScheduling,
    clearPendingStickyPinGrowth,
    clearTurnPinRequest,
    getTotalBottomCompensationPx,
    isStreamingOutput,
    recordScrollerGeometry,
    retainCollapseRangeForQuietSettlement,
    resolveTurnPinMetrics,
    snapshotMeasuredContentHeight,
    updateBottomReservationState,
  ]);

  const scrollToLatestEndPositionInternal = useCallback((behavior: 'auto' | 'smooth') => {
    const scroller = scrollerElementRef.current;
    if (!scroller) return;

    if (useStaticInitialHistoryListRef.current && staticAnchorWindowTurnId) {
      pendingStaticTurnPinRef.current = null;
      pendingStaticAnchorTurnIdRef.current = null;
      pendingStaticLatestScrollBehaviorRef.current = behavior;
      setStaticAnchorWindowTurnId(null);
      return;
    }

    if (behavior === 'auto' && isStreamingOutputRef.current) {
      clearPinReservationForUserNavigation();
    }

    const compensationPx = getTotalBottomCompensationPx();
    // Auto-follow during streaming with active collapse compensation: scroll
    // to the EFFECTIVE bottom (the top edge of the footer reservation), and
    // preserve the reservation. Clearing it here would shrink `scrollHeight`
    // by the full compensation amount in one frame, which clamps `scrollTop`
    // downward and produces a visible whole-conversation "sink-down" jump.
    // The reservation drains organically as the grow branch in
    // `measureHeightChange` consumes it while new tokens stream in.
    // 'smooth' is reserved for explicit user navigation ("jump to latest"),
    // which intentionally clears reservations.
    if (behavior === 'auto' && compensationPx > COMPENSATION_EPSILON_PX) {
      const effectiveBottomTop = Math.max(
        0,
        scroller.scrollHeight - scroller.clientHeight - compensationPx,
      );
      // Only ever move DOWNWARD here. If `effectiveBottomTop` is above the
      // current scrollTop (e.g. because the bottom reservation just grew to
      // absorb an unsignaled shrink via the handleScroll auto-clamp restore),
      // pulling scrollTop upward would itself produce a visible "sink-down"
      // jump. Hold position; the reservation will be drained by future grow
      // events.
      if (effectiveBottomTop - scroller.scrollTop > COMPENSATION_EPSILON_PX) {
        scroller.scrollTo({ top: effectiveBottomTop, behavior: 'auto' });
        recordStaticInitialHistoryBottomState(scroller);
      }
      setIsAtBottom(true);
      return;
    }

    if (behavior !== 'auto') {
      clearAllBottomReservationsForUserNavigation();
    }
    scroller.scrollTo({
      top: Math.max(0, scroller.scrollHeight - scroller.clientHeight),
      behavior,
    });
    staticInitialHistoryUserLeftBottomRef.current = false;
    setIsAtBottom(true);
  }, [
    clearAllBottomReservationsForUserNavigation,
    clearPinReservationForUserNavigation,
    getTotalBottomCompensationPx,
    recordStaticInitialHistoryBottomState,
    staticAnchorWindowTurnId,
  ]);
  useLayoutEffect(() => {
    const behavior = pendingStaticLatestScrollBehaviorRef.current;
    if (!behavior || staticAnchorWindowTurnId || !useStaticInitialHistoryList) {
      return;
    }

    pendingStaticLatestScrollBehaviorRef.current = null;
    scrollToLatestEndPositionInternal(behavior);
  }, [
    scrollToLatestEndPositionInternal,
    staticAnchorWindowTurnId,
    useStaticInitialHistoryList,
  ]);

  const requestTurnPinToTop = useCallback((turnId: string, options?: { behavior?: ScrollBehavior; pinMode?: FlowChatPinTurnToTopMode }): FlowChatTurnPinRequestStatus => {
    const requestedPinMode = options?.pinMode ?? 'transient';
    const requestedBehavior = options?.behavior ?? 'auto';
    const targetTurn = findDialogTurn(activeSession?.dialogTurns, turnId);
    if (requestedPinMode === 'sticky-latest' && !shouldUseStickyLatestPin(targetTurn)) {
      return 'rejected';
    }
    const targetItem = userMessageItems.find(({ item }) => item.turnId === turnId);
    if (!targetItem) {
      return 'rejected';
    }

    if (!virtuosoRef.current) {
      if (!useStaticInitialHistoryList) {
        return 'rejected';
      }

      retainedCollapseAnchorRef.current = null;
      viewportCoordinatorRef.current.pinItem();

      pendingStaticTurnPinRef.current = {
        turnId,
        behavior: requestedBehavior,
      };

      startupTrace.markPhase('flowchat_static_turn_pin_request', {
        turnId,
        pinMode: requestedPinMode,
        targetIndex: targetItem.index,
        userMessageCount: userMessageItems.length,
      });

      if (scrollStaticTurnToTop(turnId, requestedBehavior)) {
        pendingStaticTurnPinRef.current = null;
        return 'settled';
      }

      setStaticAnchorWindowTurnId(turnId);
      return 'pending';
    }

    retainedCollapseAnchorRef.current = null;
    viewportCoordinatorRef.current.pinItem();

    if (targetItem.index === 0 && requestedPinMode === 'transient') {
      // The first turn has a deterministic destination, so bypass the deferred
      // pin pipeline and snap to the true top immediately.
      clearTurnPinRequest();
      virtuosoRef.current.scrollTo({ top: 0, behavior: 'auto' });

      return 'settled';
    }

    const requestSessionId = activeSessionIdRef.current;
    if (!requestSessionId) {
      return 'rejected';
    }

    const request: PendingTurnPinState = {
      generation: turnPinRequestGenerationRef.current + 1,
      sessionId: requestSessionId,
      turnId,
      behavior: requestedBehavior,
      pinMode: requestedPinMode,
      expiresAtMs: performance.now() + 1500,
      attempts: 0,
    };
    turnPinRequestGenerationRef.current = request.generation;

    startupTrace.markPhase('flowchat_turn_pin_request', {
      turnId,
      pinMode: requestedPinMode,
      targetIndex: targetItem.index,
      userMessageCount: userMessageItems.length,
    });

    if (tryResolvePendingTurnPin(request)) {
      if (requestedPinMode === 'transient') {
        activateTransientTurnPinStabilization(request);
        scheduleTransientTurnPinStabilization(2);
      } else {
        clearTurnPinRequest();
      }
      scheduleVisibleTurnMeasure(2);
      return 'settled';
    }

    setPendingTurnPin(request);
    return 'pending';
  }, [
    activeSession?.dialogTurns,
    activateTransientTurnPinStabilization,
    clearTurnPinRequest,
    scheduleTransientTurnPinStabilization,
    scheduleVisibleTurnMeasure,
    scrollStaticTurnToTop,
    tryResolvePendingTurnPin,
    useStaticInitialHistoryList,
    userMessageItems,
  ]);

  const performAutoFollowSync = useCallback(() => {
    if (!viewportCoordinatorRef.current.followTail()) {
      return;
    }
    scrollToLatestEndPositionInternal('auto');
  }, [scrollToLatestEndPositionInternal]);

  const {
    isFollowingOutput,
    enterFollowOutput,
    exitFollowOutput,
    armFollowOutputForNewTurn,
    resumeFollowOutputForMountedStream,
    activateArmedFollowOutput,
    cancelPendingAutoFollowArm,
    scheduleFollowToLatest,
    handleUserScrollIntent,
    handleScroll: handleFollowOutputScroll,
  } = useFlowChatFollowOutput({
    activeSessionId: activeSession?.sessionId,
    latestTurnId,
    virtualItemCount: virtualItems.length,
    isStreaming: isStreamingOutput,
    scrollerRef: scrollerElementRef,
    performUserFollowScroll: () => {
      viewportCoordinatorRef.current.followTail({ force: true });
      scrollToLatestEndPositionInternal('smooth');
    },
    performAutoFollowScroll: performAutoFollowSync,
    performLatestTurnStickyPin: () => {
      if (!latestTurnId) {
        return;
      }
      const latestTurn = findDialogTurn(activeSession?.dialogTurns, latestTurnId);
      if (!shouldUseStickyLatestPin(latestTurn)) {
        return;
      }
      retainedCollapseAnchorRef.current = null;
      viewportCoordinatorRef.current.pinItem();
      requestTurnPinToTop(latestTurnId, {
        behavior: 'auto',
        pinMode: 'sticky-latest',
      });
    },
    shouldSuspendAutoFollow,
    // Subtract the bottom-reservation footer so the follow controller treats
    // synthetic footer space as "already at the bottom". Without this, the
    // post-collapse footer (kept around to preserve the upper anchor) would be
    // classified as "user fell behind the tail" and trigger
    // `performAutoFollowScroll` -> `clearAllBottomReservationsForUserNavigation`,
    // which snaps scrollTop down by the entire compensation amount and produces
    // the visible "sink-down" jump the user reported. With this subtraction,
    // the loop and the deferred-follow path stay quiet while the grow-branch in
    // `measureHeightChange` consumes the footer organically as streaming tokens
    // refill the bottom space.
    getAutoFollowDistanceFromBottom: (scroller) => {
      const compensationPx = getTotalBottomCompensationPx();
      return Math.max(
        0,
        scroller.scrollHeight - scroller.clientHeight - scroller.scrollTop - compensationPx,
      );
    },
    onContinuousFollowFrame: undefined,
  });

  useEffect(() => {
    if (hasPrimedMountedStreamingTurnFollowRef.current) {
      return;
    }

    if (!isStreamingOutput) {
      hasPrimedMountedStreamingTurnFollowRef.current = true;
      return;
    }
    if (!latestTurnId) {
      return;
    }

    previousLatestTurnIdForFollowRef.current = latestTurnId;
    latestTurnAutoFollowStateRef.current = {
      turnId: null,
      sawPositiveFloor: false,
    };
    hasPrimedMountedStreamingTurnFollowRef.current = resumeFollowOutputForMountedStream();
  }, [
    activeSession?.sessionId,
    isStreamingOutput,
    latestTurnId,
    resumeFollowOutputForMountedStream,
    virtualItems.length,
  ]);

  useEffect(() => {
    const previousSessionId = previousSessionIdForFollowRef.current;
    if (previousSessionId !== activeSession?.sessionId) {
      previousSessionIdForFollowRef.current = activeSession?.sessionId;
      previousLatestTurnIdForFollowRef.current = latestTurnId;
      latestTurnAutoFollowStateRef.current = {
        turnId: latestTurnId,
        sawPositiveFloor: false,
      };

      const hasUnread = activeSession?.hasUnreadCompletion;
      const isFinished = !isStreamingOutput;
      if (hasUnread && isFinished && virtuosoRef.current && virtualItems.length > 0) {
        // Use scrollToIndex instead of scrollTo({ top: largeNumber }) because
        // Virtuoso's scrollHeight may not be stable immediately after a session
        // switch; scrolling by index lets Virtuoso resolve the correct position.
        const scrollToBottom = () => {
          virtuosoRef.current?.scrollToIndex({
            index: toVirtuosoIndex(virtualItems.length - 1),
            align: 'end',
            behavior: 'auto',
          });
        };
        // Allow two frames for virtual items to settle before scrolling.
        requestAnimationFrame(() => {
          requestAnimationFrame(scrollToBottom);
        });
      }

      // When switching to a streaming session, resume tail follow so the
      // viewport tracks new content as it arrives. Without this, the
      // Virtuoso stays at initialTopMostItemIndex (the user message),
      // and if that position is not yet rendered or measured the
      // viewport may show blank space.
      if (isStreamingOutput) {
        latestTurnAutoFollowStateRef.current = {
          turnId: null,
          sawPositiveFloor: false,
        };
        hasPrimedMountedStreamingTurnFollowRef.current = resumeFollowOutputForMountedStream();
      }

      return;
    }

    const previousLatestTurnId = previousLatestTurnIdForFollowRef.current;
    if (previousLatestTurnId === latestTurnId) {
      return;
    }

    previousLatestTurnIdForFollowRef.current = latestTurnId;
    latestTurnAutoFollowStateRef.current = {
      turnId: latestTurnId,
      sawPositiveFloor: false,
    };

    if (!latestTurnId) {
      cancelPendingAutoFollowArm();
      return;
    }

    armFollowOutputForNewTurn();
  }, [
    activeSession?.sessionId,
    activeSession?.hasUnreadCompletion,
    armFollowOutputForNewTurn,
    cancelPendingAutoFollowArm,
    isStreamingOutput,
    latestTurnId,
    resumeFollowOutputForMountedStream,
    toVirtuosoIndex,
    virtualItems.length,
  ]);

  const maybeHandoffPinnedTurnToTail = useCallback((reason: string) => {
    const trackingState = latestTurnAutoFollowStateRef.current;
    if (
      !latestTurnId ||
      trackingState.turnId !== latestTurnId ||
      isFollowingOutput
    ) {
      return false;
    }

    const hasPendingLatestStickyPin = (
      pendingTurnPin?.turnId === latestTurnId &&
      pendingTurnPin.pinMode === 'sticky-latest'
    );
    if (hasPendingLatestStickyPin) {
      return false;
    }

    const reservationState = bottomReservationStateRef.current;
    if (
      reservationState.pin.mode !== 'sticky-latest' ||
      reservationState.pin.targetTurnId !== latestTurnId
    ) {
      return false;
    }

    if (reservationState.pin.floorPx > COMPENSATION_EPSILON_PX) {
      trackingState.sawPositiveFloor = true;
    }

    const collapseIntent = pendingCollapseIntentRef.current;
    const hasPendingCollapseIntent = (
      collapseIntent.active
    );
    if (!canHandoffPinnedItemToTail({
      pinReservationPx: reservationState.pin.px,
      collapseReservationPx: reservationState.collapse.px,
      hasPendingCollapseIntent,
    })) {
      if (flowChatDiagnostics.isEnabled()) {
        flowChatDiagnostics.trace({
          hypothesis: 'A',
          location: 'VirtualMessageList.maybeHandoffPinnedTurnToTail',
          message: 'Pinned item to tail handoff remained blocked',
          data: () => ({
            reason,
            reservation: reservationState,
            hasPendingCollapseIntent,
            coordinatorMode: viewportCoordinatorRef.current.getMode(),
          }),
        });
      }
      return false;
    }

    if (activateArmedFollowOutput()) {
      if (flowChatDiagnostics.isEnabled()) {
        flowChatDiagnostics.trace({
          hypothesis: 'A',
          location: 'VirtualMessageList.maybeHandoffPinnedTurnToTail',
          message: 'Pinned item handed off to tail follow',
          data: () => ({ reason, reservation: reservationState }),
        });
      }
      latestTurnAutoFollowStateRef.current = {
        turnId: null,
        sawPositiveFloor: false,
      };
      return true;
    }
    return false;
  }, [
    activateArmedFollowOutput,
    isFollowingOutput,
    latestTurnId,
    pendingTurnPin?.pinMode,
    pendingTurnPin?.turnId,
  ]);
  maybeHandoffPinnedTurnToTailRef.current = maybeHandoffPinnedTurnToTail;

  useEffect(() => {
    maybeHandoffPinnedTurnToTail('reservation-state-change');
  }, [
    bottomReservationState.collapse.px,
    bottomReservationState.pin.floorPx,
    bottomReservationState.pin.mode,
    bottomReservationState.pin.px,
    bottomReservationState.pin.targetTurnId,
    isStreamingOutput,
    maybeHandoffPinnedTurnToTail,
  ]);

  followOutputControllerRef.current = {
    handleUserScrollIntent,
    handleScroll: handleFollowOutputScroll,
    scheduleFollowToLatest,
  };
  isFollowingOutputRef.current = isFollowingOutput;
  isStreamingOutputRef.current = isStreamingOutput;

  useEffect(() => {
    if (flowChatDiagnostics.isEnabled()) {
      flowChatDiagnostics.trace({
        hypothesis: 'D',
        location: 'VirtualMessageList.followOutputEffect',
        message: 'Follow output state synchronized with the viewport coordinator',
        data: () => ({
          isFollowingOutput,
          isStreamingOutput,
          coordinatorMode: viewportCoordinatorRef.current.getMode(),
          reservation: bottomReservationStateRef.current,
        }),
      });
    }
    if (isFollowingOutput) {
      viewportCoordinatorRef.current.followTail({ force: true });
      return;
    }
    if (viewportCoordinatorRef.current.getMode() === 'following-tail') {
      viewportCoordinatorRef.current.release('follow-ended');
    }
  }, [isFollowingOutput, isStreamingOutput]);

  // When entering follow-output during streaming, end any active protection
  // window but keep its reservation. Once the intent is inactive, real content
  // growth can consume that range without exposing a one-frame shrink.
  const previousIsFollowingOutputRef = useRef(false);
  useEffect(() => {
    if (!previousIsFollowingOutputRef.current && isFollowingOutput && isStreamingOutput) {
      const intent = pendingCollapseIntentRef.current;
      if (intent.active) {
        finalizeCollapseIntent('follow-output-entered', {
          expectedExpiresAtMs: intent.expiresAtMs,
          suppressHandoff: true,
        });
      }
    }
    previousIsFollowingOutputRef.current = isFollowingOutput;
  }, [
    finalizeCollapseIntent,
    isFollowingOutput,
    isStreamingOutput,
  ]);

  const scrollToTurn = useCallback((turnIndex: number) => {
    if (!virtuosoRef.current) return;
    if (turnIndex < 1 || turnIndex > userMessageItems.length) return;

    const targetItem = userMessageItems[turnIndex - 1];
    if (!targetItem) return;

    retainedCollapseAnchorRef.current = null;
    exitFollowOutput('scroll-to-turn');
    clearPinReservationForUserNavigation();

    if (targetItem.index === 0) {
      virtuosoRef.current.scrollTo({ top: 0, behavior: 'smooth' });
    } else {
      virtuosoRef.current.scrollToIndex({
        index: toVirtuosoIndex(targetItem.index),
        behavior: 'smooth',
        align: 'center',
      });
    }
  }, [clearPinReservationForUserNavigation, exitFollowOutput, toVirtuosoIndex, userMessageItems]);

  const scrollToIndex = useCallback((index: number) => {
    if (!virtuosoRef.current) return;
    if (index < 0 || index >= virtualItems.length) return;

    retainedCollapseAnchorRef.current = null;
    exitFollowOutput('scroll-to-index');
    clearPinReservationForUserNavigation();

    if (index === 0) {
      virtuosoRef.current.scrollTo({ top: 0, behavior: 'auto' });
    } else {
      virtuosoRef.current.scrollToIndex({ index: toVirtuosoIndex(index), align: 'center', behavior: 'auto' });
    }
  }, [clearPinReservationForUserNavigation, exitFollowOutput, toVirtuosoIndex, virtualItems.length]);

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
    const query = target.query.trim();
    if (
      !query ||
      target.virtualItemIndex < 0 ||
      target.virtualItemIndex >= virtualItems.length
    ) {
      clearSearchMatch();
      return;
    }

    const requestId = searchNavigationRequestIdRef.current + 1;
    searchNavigationRequestIdRef.current = requestId;
    setFlowChatSearchHighlight(null);
    retainedCollapseAnchorRef.current = null;
    exitFollowOutput('scroll-to-index');
    clearPinReservationForUserNavigation();

    const virtuoso = virtuosoRef.current;
    if (virtuoso) {
      if (target.virtualItemIndex === 0) {
        virtuoso.scrollTo({ top: 0, behavior: 'auto' });
      } else {
        virtuoso.scrollToIndex({
          index: toVirtuosoIndex(target.virtualItemIndex),
          align: 'center',
          behavior: 'auto',
        });
      }
    }

    let attempts = 0;
    let revealedCollapsedContent = false;
    // Scroll position the previous successful pass settled on. Expanding
    // collapsed content reflows everything below it, so one more pass is needed
    // after the expansion — but only until the answer stops moving. Re-arming
    // unconditionally kept re-running the full tree-walk, re-highlight and
    // ancestor `getComputedStyle` scan for all 24 attempts, roughly 22 of them
    // recomputing a position that had already converged.
    let lastResolvedScrollTop: number | null = null;
    const resolveExactTextPosition = () => {
      if (searchNavigationRequestIdRef.current !== requestId) {
        return;
      }

      attempts += 1;
      const scroller = scrollerElementRef.current;
      const wrapper = getRenderedVirtualItemElement(target.virtualItemIndex);
      if (!scroller || !wrapper) {
        const targetTurnId = virtualItems[target.virtualItemIndex]?.turnId;
        if (
          scroller &&
          !wrapper &&
          !virtuosoRef.current &&
          useStaticInitialHistoryListRef.current &&
          targetTurnId
        ) {
          setStaticAnchorWindowTurnId(currentTurnId => (
            currentTurnId === targetTurnId ? currentTurnId : targetTurnId
          ));
        }
        if (attempts < SEARCH_NAVIGATION_MAX_ATTEMPTS) {
          requestAnimationFrame(resolveExactTextPosition);
        }
        return;
      }

      for (const expandableId of target.expandableIds ?? []) {
        const expandable = findElementWithDataValue(
          wrapper,
          'data-tool-card-id',
          expandableId,
        );
        if (!expandable) {
          if (attempts < SEARCH_NAVIGATION_MAX_ATTEMPTS) {
            requestAnimationFrame(resolveExactTextPosition);
          }
          return;
        }

        if (expandable.dataset.expanded === 'false') {
          const toggle = expandable.querySelector<HTMLElement>(
            '[data-testid="chat-explore-group-toggle"], [data-testid="chat-thinking-toggle"]',
          );
          if (toggle) {
            revealedCollapsedContent = true;
            toggle.click();
          }
          if (attempts < SEARCH_NAVIGATION_MAX_ATTEMPTS) {
            requestAnimationFrame(resolveExactTextPosition);
          }
          return;
        }
      }

      if (target.flowItemId) {
        const flowItem = findElementWithDataValue(
          wrapper,
          'data-flow-item-id',
          target.flowItemId,
        );
        const thinkingItem = findElementWithDataValue(
          wrapper,
          'data-tool-card-id',
          target.flowItemId,
        );
        if (!flowItem && !thinkingItem) {
          if (attempts < SEARCH_NAVIGATION_MAX_ATTEMPTS) {
            requestAnimationFrame(resolveExactTextPosition);
          }
          return;
        }
      }

      const textRoot = getFlowChatSearchTextRoot(wrapper, target.flowItemId);
      const userMessage = textRoot.closest<HTMLElement>('.user-message-item');
      if (
        userMessage &&
        !userMessage.classList.contains('user-message-item--expanded') &&
        textRoot.scrollHeight > textRoot.clientHeight + 1
      ) {
        revealedCollapsedContent = true;
        textRoot.click();
        if (attempts < SEARCH_NAVIGATION_MAX_ATTEMPTS) {
          requestAnimationFrame(resolveExactTextPosition);
        }
        return;
      }

      const ranges = findFlowChatSearchTextRanges(textRoot, query);
      if (ranges.length === 0) {
        if (attempts < SEARCH_NAVIGATION_MAX_ATTEMPTS) {
          requestAnimationFrame(resolveExactTextPosition);
        }
        return;
      }

      // Markdown syntax can make the raw-content occurrence count exceed the
      // rendered one; clamping still lands navigation on a real occurrence.
      const rangeIndex = Math.min(Math.max(target.occurrenceIndex ?? 0, 0), ranges.length - 1);
      const range = ranges[rangeIndex];
      setFlowChatSearchHighlight(range, ranges.filter((_, index) => index !== rangeIndex));

      let ancestor = range.startContainer.parentElement;
      while (ancestor && ancestor !== scroller && wrapper.contains(ancestor)) {
        const styles = ancestor.ownerDocument.defaultView?.getComputedStyle(ancestor);
        const canScrollVertically = (
          ancestor.scrollHeight > ancestor.clientHeight + 1 &&
          (styles?.overflowY === 'auto' || styles?.overflowY === 'scroll')
        );
        if (canScrollVertically) {
          const rangeRect = range.getBoundingClientRect();
          const ancestorRect = ancestor.getBoundingClientRect();
          const nextScrollTop = ancestor.scrollTop +
            rangeRect.top -
            ancestorRect.top -
            Math.max(0, (ancestor.clientHeight - rangeRect.height) / 2);
          ancestor.scrollTop = Math.max(
            0,
            Math.min(nextScrollTop, ancestor.scrollHeight - ancestor.clientHeight),
          );
        }
        ancestor = ancestor.parentElement;
      }

      const rangeRect = range.getBoundingClientRect();
      const scrollerRect = scroller.getBoundingClientRect();
      const nextScrollTop = scroller.scrollTop +
        rangeRect.top -
        scrollerRect.top -
        Math.max(0, (scroller.clientHeight - rangeRect.height) / 2);
      scroller.scrollTop = Math.max(
        0,
        Math.min(nextScrollTop, scroller.scrollHeight - scroller.clientHeight),
      );
      previousScrollTopRef.current = scroller.scrollTop;
      recordScrollerGeometry(scroller);
      scheduleVisibleTurnMeasure(2);

      const settled =
        lastResolvedScrollTop !== null
        && Math.abs(scroller.scrollTop - lastResolvedScrollTop) <= 1;
      lastResolvedScrollTop = scroller.scrollTop;
      if (
        revealedCollapsedContent
        && !settled
        && attempts < SEARCH_NAVIGATION_MAX_ATTEMPTS
      ) {
        requestAnimationFrame(resolveExactTextPosition);
      }
    };

    requestAnimationFrame(resolveExactTextPosition);
  }, [
    clearPinReservationForUserNavigation,
    clearSearchMatch,
    exitFollowOutput,
    getRenderedVirtualItemElement,
    recordScrollerGeometry,
    scheduleVisibleTurnMeasure,
    toVirtuosoIndex,
    virtualItems,
  ]);

  useEffect(() => () => {
    searchNavigationRequestIdRef.current += 1;
    setFlowChatSearchHighlight(null);
  }, []);

  const pinTurnToTopWithStatus = useCallback((turnId: string, options?: { behavior?: ScrollBehavior; pinMode?: FlowChatPinTurnToTopMode }): FlowChatTurnPinRequestStatus => {
    const shouldExitFollowOutput = !(
      options?.pinMode === 'sticky-latest' &&
      turnId === latestTurnId
    );
    if (shouldExitFollowOutput) {
      exitFollowOutput('pin-turn-to-top');
      // Drop stale sticky tail padding before transient jumps so the previous
      // latest-turn reservation cannot leak into the new viewport.
      clearPinReservationForUserNavigation();
    }

    return requestTurnPinToTop(turnId, options);
  }, [clearPinReservationForUserNavigation, exitFollowOutput, latestTurnId, requestTurnPinToTop]);

  const pinTurnToTop = useCallback((turnId: string, options?: { behavior?: ScrollBehavior; pinMode?: FlowChatPinTurnToTopMode }) => {
    return pinTurnToTopWithStatus(turnId, options) !== 'rejected';
  }, [pinTurnToTopWithStatus]);

  const visibleTurnInfo = useModernFlowChatStore(state => state.visibleTurnInfo);

  const handleJumpToCurrentTurn = useCallback(() => {
    const currentTurnId = visibleTurnInfo?.turnId;
    if (!currentTurnId) return;
    pinTurnToTop(currentTurnId, { behavior: 'smooth', pinMode: 'transient' });
  }, [visibleTurnInfo?.turnId, pinTurnToTop]);

  const { shouldShowButton: shouldShowTurnHeaderButton, handleClick: handleTurnHeaderClick } = useScrollToTurnHeader({
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

  const scrollToPhysicalBottomAndClearPin = useCallback(() => {
    const scroller = scrollerElementRef.current;
    if (scroller) {
      clearAllBottomReservationsForUserNavigation();
      const nextScrollTop = Math.max(0, scroller.scrollHeight - scroller.clientHeight);
      scroller.scrollTo({
        top: nextScrollTop,
        behavior: 'auto',
      });
      staticInitialHistoryUserLeftBottomRef.current = false;
      previousScrollTopRef.current = nextScrollTop;
      previousMeasuredHeightRef.current = snapshotMeasuredContentHeight(scroller);
      recordScrollerGeometry(scroller);
      setIsAtBottom(true);
    }
  }, [clearAllBottomReservationsForUserNavigation, recordScrollerGeometry, snapshotMeasuredContentHeight]);

  const scrollToTurnEndAndClearPin = useCallback((turnId: string) => {
    const scroller = scrollerElementRef.current;
    const reject = (cause: string, extra: Record<string, unknown> = {}) => {
      startupTrace.markPhase('flowchat_latest_end_anchor_rejected', {
        cause,
        turnId,
        virtualItemCount: virtualItems.length,
        staticInitialHistoryList: useStaticInitialHistoryList,
        hasVirtuoso: Boolean(virtuosoRef.current),
        ...extra,
      });
      return false;
    };

    if (!scroller || virtualItems.length === 0) {
      return reject(!scroller ? 'missing_scroller' : 'empty_virtual_items');
    }

    let targetIndex = -1;
    for (let index = virtualItems.length - 1; index >= 0; index -= 1) {
      if (virtualItems[index]?.turnId === turnId) {
        targetIndex = index;
        break;
      }
    }

    if (targetIndex < 0) {
      return reject('missing_target_index');
    }

    exitFollowOutput('scroll-to-index');
    clearAllBottomReservationsForUserNavigation();

    if (!virtuosoRef.current) {
      const shouldSnapLatestToPhysicalBottom =
        useStaticInitialHistoryList &&
        (
          targetIndex >= virtualItems.length - 1 ||
          (Boolean(latestTurnId) && turnId === latestTurnId)
        );
      const targetElement = getRenderedVirtualItemElement(targetIndex);
      if (!targetElement) {
        return reject('missing_static_target_element', {
          targetIndex,
        });
      }

      const readStaticTargetEndState = () => {
        const rect = targetElement.getBoundingClientRect();
        const scrollerRect = scroller.getBoundingClientRect();
        const inputOverlayInsetPx = Math.max(
          0,
          inputStackFooterPxRef.current - FLOWCHAT_MESSAGE_TAIL_CLEARANCE_PX,
        );
        const visibleTop = scrollerRect.top + LATEST_END_ANCHOR_VISIBILITY_MARGIN_PX;
        const visibleBottom = Math.max(
          visibleTop + 1,
          scrollerRect.bottom - inputOverlayInsetPx - LATEST_END_ANCHOR_VISIBILITY_MARGIN_PX,
        );
        return {
          endDeltaPx: rect.bottom - visibleBottom,
          maxScrollTop: Math.max(0, scroller.scrollHeight - scroller.clientHeight),
          rect,
          visible: rect.bottom > visibleTop && rect.top < visibleBottom,
          visibleHeight: visibleBottom - visibleTop,
        };
      };
      const canUseStaticFastPath = (state: ReturnType<typeof readStaticTargetEndState>) => {
        const distanceFromBottom = Math.max(0, state.maxScrollTop - scroller.scrollTop);
        const bottomTolerancePx = shouldSnapLatestToPhysicalBottom
          ? LATEST_END_ANCHOR_STABLE_EPSILON_PX
          : LATEST_END_ANCHOR_STATIC_FAST_PATH_TOLERANCE_PX;
        return (
          state.visible &&
          state.rect.height <= state.visibleHeight + LATEST_END_ANCHOR_STATIC_FAST_PATH_TOLERANCE_PX &&
          Math.abs(state.endDeltaPx) <= LATEST_END_ANCHOR_STATIC_FAST_PATH_TOLERANCE_PX &&
          distanceFromBottom <= bottomTolerancePx
        );
      };
      let staticState = readStaticTargetEndState();
      if (!canUseStaticFastPath(staticState)) {
        const nextScrollTop = shouldSnapLatestToPhysicalBottom
          ? staticState.maxScrollTop
          : Math.max(
            0,
            Math.min(staticState.maxScrollTop, scroller.scrollTop + staticState.endDeltaPx),
          );
        if (Math.abs(nextScrollTop - scroller.scrollTop) > COMPENSATION_EPSILON_PX) {
          scroller.scrollTop = nextScrollTop;
          previousScrollTopRef.current = nextScrollTop;
          previousMeasuredHeightRef.current = snapshotMeasuredContentHeight(scroller);
          recordScrollerGeometry(scroller);
          recordStaticInitialHistoryBottomState(scroller);
          staticState = readStaticTargetEndState();
        }
      }
      if (canUseStaticFastPath(staticState)) {
        startupTrace.markPhase('flowchat_latest_end_anchor_request', {
          targetIndex,
          turnId,
          virtualItemCount: virtualItems.length,
          mode: 'static-initial-history-fast-path',
        });
        scheduleVisibleTurnMeasure(1);
        return true;
      }

      latestEndAnchorRequestRef.current = {
        turnId,
        targetIndex,
        attempts: 0,
        visibleFrames: 0,
        stableVisibleFrames: 0,
        lastScrollHeight: null,
        lastScrollTop: null,
        lastTargetTop: null,
        lastTargetBottom: null,
      };
      startupTrace.markPhase('flowchat_latest_end_anchor_request', {
        targetIndex,
        turnId,
        virtualItemCount: virtualItems.length,
        mode: 'static-initial-history',
      });
      resolveLatestEndAnchorStabilization('raf');
      return true;
    }

    latestEndAnchorRequestRef.current = {
      turnId,
      targetIndex,
      attempts: 0,
      visibleFrames: 0,
      stableVisibleFrames: 0,
      lastScrollHeight: null,
      lastScrollTop: null,
      lastTargetTop: null,
      lastTargetBottom: null,
    };
    startupTrace.markPhase('flowchat_latest_end_anchor_request', {
      targetIndex,
      turnId,
      virtualItemCount: virtualItems.length,
    });
    virtuosoRef.current.scrollToIndex({
      index: toVirtuosoIndex(targetIndex),
      align: 'end',
      behavior: 'auto',
    });
    if (latestEndAnchorStabilizationFrameRef.current === null) {
      latestEndAnchorStabilizationFrameRef.current = requestAnimationFrame(() => {
        latestEndAnchorStabilizationFrameRef.current = null;
        resolveLatestEndAnchorStabilization('raf');
      });
    }
    scheduleVisibleTurnMeasure(2);
    return true;
  }, [
    clearAllBottomReservationsForUserNavigation,
    exitFollowOutput,
    getRenderedVirtualItemElement,
    latestTurnId,
    recordScrollerGeometry,
    recordStaticInitialHistoryBottomState,
    resolveLatestEndAnchorStabilization,
    scheduleVisibleTurnMeasure,
    snapshotMeasuredContentHeight,
    toVirtuosoIndex,
    useStaticInitialHistoryList,
    virtualItems,
  ]);

  const scrollToLatestEndPosition = useCallback(() => {
    retainedCollapseAnchorRef.current = null;
    onUserScrollIntent?.();
    enterFollowOutput('jump-to-latest');
  }, [enterFollowOutput, onUserScrollIntent]);

  useImperativeHandle(ref, () => ({
    scrollToTurn,
    scrollToIndex,
    scrollToSearchMatch,
    clearSearchMatch,
    scrollToPhysicalBottomAndClearPin,
    scrollToTurnEndAndClearPin,
    isTurnRenderedInViewport,
    isTurnTextRenderedInViewport,
    scrollToLatestEndPosition,
    pinTurnToTop,
    pinTurnToTopWithStatus,
  }), [
    isTurnRenderedInViewport,
    isTurnTextRenderedInViewport,
    pinTurnToTop,
    pinTurnToTopWithStatus,
    clearSearchMatch,
    scrollToTurn,
    scrollToIndex,
    scrollToSearchMatch,
    scrollToPhysicalBottomAndClearPin,
    scrollToTurnEndAndClearPin,
    scrollToLatestEndPosition,
  ]);

  const handleAtBottomStateChange = useCallback((atBottom: boolean) => {
    setIsAtBottom(atBottom);
  }, []);

  const footerHeightPx = getFooterHeightPx(getTotalBottomCompensationPx());
  const initialHistoryTransitionState = React.useMemo<InitialHistoryTransitionState | null>(() => {
    if (!activeSessionId || !latestTurnId) {
      return null;
    }

    const contextRestoreState = activeSession?.contextRestoreState ?? 'unknown';
    const isPartial = activeSession?.isPartial === true;
    const usesInitialHistoryRenderBudget = useInitialHistoryRenderBudget;
    return {
      key: [
        activeSessionId,
        latestTurnId,
        virtualItems.length,
        isPartial ? 'partial' : 'full',
        contextRestoreState,
        usesInitialHistoryRenderBudget ? 'initial-budget' : 'normal',
      ].join(':'),
      sessionId: activeSessionId,
      isPartial,
      contextRestoreState,
      usesInitialHistoryRenderBudget,
    };
  }, [
    activeSession?.contextRestoreState,
    activeSession?.isPartial,
    activeSessionId,
    latestTurnId,
    useInitialHistoryRenderBudget,
    virtualItems.length,
  ]);
  useLayoutEffect(() => {
    const previousState = previousInitialHistoryTransitionStateRef.current;
    previousInitialHistoryTransitionStateRef.current = initialHistoryTransitionState;

    const shouldProtectTransition = Boolean(
      previousState &&
      initialHistoryTransitionState &&
      previousState.sessionId === initialHistoryTransitionState.sessionId &&
      previousState.key !== initialHistoryTransitionState.key &&
      (
        previousState.usesInitialHistoryRenderBudget ||
        initialHistoryTransitionState.usesInitialHistoryRenderBudget ||
        previousState.isPartial !== initialHistoryTransitionState.isPartial ||
        previousState.contextRestoreState !== initialHistoryTransitionState.contextRestoreState
      )
    );

    if (
      !shouldProtectTransition ||
      !initialHistoryTransitionState ||
      !activeSessionId ||
      historyProjectionHandoffRef.current?.sessionId === activeSessionId
    ) {
      return;
    }

    const snapshot = latestVisibleHistoryTailSnapshotRef.current;
    if (!snapshot || snapshot.sessionId !== activeSessionId || snapshot.items.length === 0) {
      return;
    }

    const handoff: HistoryProjectionHandoffSnapshot = {
      ...snapshot,
      reason: 'initial-history-content-transition',
      createdAtMs: performance.now(),
      targetTurnId: latestTurnId,
      footerHeightPx,
    };

    historyProjectionHandoffRef.current = handoff;
    setHistoryProjectionHandoff(handoff);
    startupTrace.markPhase('flowchat_history_projection_handoff_activated', {
      sessionId: handoff.sessionId,
      reason: handoff.reason,
      previousItemCount: handoff.items.length,
      nextItemCount: virtualItems.length,
      previousInitialHistoryBudget: previousState?.usesInitialHistoryRenderBudget ?? false,
      nextInitialHistoryBudget: initialHistoryTransitionState.usesInitialHistoryRenderBudget,
    });
    scheduleHistoryProjectionHandoffRelease(2);
  }, [
    activeSessionId,
    footerHeightPx,
    initialHistoryTransitionState,
    latestTurnId,
    scheduleHistoryProjectionHandoffRelease,
    virtualItems.length,
  ]);
  useLayoutEffect(() => {
    if (!activeSessionId || !latestTurnId || virtualItems.length === 0) {
      return;
    }

    if (!isTurnTextRenderedInViewportOutsideHandoff(latestTurnId)) {
      return;
    }

    const scroller = scrollerElementRef.current;
    if (!scroller) {
      return;
    }

    const tailStartIndex = Math.max(0, virtualItems.length - SESSION_OPEN_HANDOFF_ITEM_BUDGET);
    latestVisibleHistoryTailSnapshotRef.current = {
      sessionId: activeSessionId,
      reason: 'latest-visible-history-tail',
      createdAtMs: performance.now(),
      items: virtualItems.slice(tailStartIndex),
      mode: 'bottom-tail',
      targetTurnId: latestTurnId,
      footerHeightPx,
    };
  }, [
    activeSessionId,
    footerHeightPx,
    isTurnTextRenderedInViewportOutsideHandoff,
    latestTurnId,
    virtualItems,
  ]);
  const initialHistoryRenderWindow = React.useMemo(() => {
    if (!useStaticInitialHistoryList) {
      return {
        items: virtualItems,
        startIndex: 0,
        omittedEstimatedHeightPx: 0,
        trailingOmittedEstimatedHeightPx: 0,
        renderedEstimatedHeightPx: 0,
        totalEstimatedHeightPx: 0,
        isWindowed: false,
      };
    }

    if (!staticAnchorWindowTurnId) {
      return selectInitialHistoryRenderWindow(virtualItems);
    }

    const targetIndex = virtualItems.findIndex(item => (
      item.turnId === staticAnchorWindowTurnId && item.type === 'user-message'
    ));
    if (targetIndex < 0) {
      return selectInitialHistoryRenderWindow(virtualItems);
    }

    let startIndex = targetIndex;
    while (
      startIndex > 0 &&
      virtualItems[startIndex - 1]?.turnId === staticAnchorWindowTurnId
    ) {
      startIndex -= 1;
    }

    let renderedEstimatedHeightPx = 0;
    let endIndex = startIndex;
    const includedTurnIds = new Set<string>();
    for (; endIndex < virtualItems.length; endIndex += 1) {
      const item = virtualItems[endIndex];
      renderedEstimatedHeightPx += estimateVirtualMessageItemHeight(item);
      if (item.turnId) {
        includedTurnIds.add(item.turnId);
      }

      const nextItem = virtualItems[endIndex + 1];
      const stillInsideSameTurn = Boolean(item.turnId) && nextItem?.turnId === item.turnId;
      if (
        !stillInsideSameTurn &&
        includedTurnIds.size >= INITIAL_HISTORY_RENDER_MIN_TURN_COUNT &&
        renderedEstimatedHeightPx >= INITIAL_HISTORY_RENDER_MIN_ESTIMATED_HEIGHT_PX
      ) {
        endIndex += 1;
        break;
      }
    }

    const totalEstimatedHeightPx = virtualItems.reduce(
      (total, item) => total + estimateVirtualMessageItemHeight(item),
      0,
    );
    const omittedEstimatedHeightPx = virtualItems
      .slice(0, startIndex)
      .reduce((total, item) => total + estimateVirtualMessageItemHeight(item), 0);
    const trailingOmittedEstimatedHeightPx = virtualItems
      .slice(endIndex)
      .reduce((total, item) => total + estimateVirtualMessageItemHeight(item), 0);

    return {
      items: virtualItems.slice(startIndex, endIndex),
      startIndex,
      omittedEstimatedHeightPx,
      trailingOmittedEstimatedHeightPx,
      renderedEstimatedHeightPx,
      totalEstimatedHeightPx,
      isWindowed: startIndex > 0 || endIndex < virtualItems.length,
    };
  }, [staticAnchorWindowTurnId, useStaticInitialHistoryList, virtualItems]);
  const initialHistoryRenderKey = [
    activeSessionId ?? 'no-active-session',
    latestTurnId ?? 'no-latest-turn',
    virtualItems.length,
    initialHistoryRenderWindow.startIndex,
  ].join(':');
  const isInitialHistoryRenderWindowExpanded =
    !initialHistoryRenderWindow.isWindowed ||
    expandedInitialHistoryRenderKey === initialHistoryRenderKey;
  const renderedInitialHistoryItems = isInitialHistoryRenderWindowExpanded
    ? virtualItems
    : initialHistoryRenderWindow.items;
  const renderedInitialHistoryStartIndex = isInitialHistoryRenderWindowExpanded
    ? 0
    : initialHistoryRenderWindow.startIndex;
  const omittedInitialHistoryEstimatedHeightPx = isInitialHistoryRenderWindowExpanded
    ? 0
    : initialHistoryRenderWindow.omittedEstimatedHeightPx;
  const trailingOmittedInitialHistoryEstimatedHeightPx = isInitialHistoryRenderWindowExpanded
    ? 0
    : initialHistoryRenderWindow.trailingOmittedEstimatedHeightPx;
  const getStaticAnchorScrollTop = useCallback((turnId: string) => {
    const scroller = scrollerElementRef.current;
    const targetElement = getRenderedUserMessageElement(turnId);
    if (!scroller || !targetElement) return null;

    const targetRect = targetElement.getBoundingClientRect();
    const scrollerRect = scroller.getBoundingClientRect();
    const targetTop = scroller.scrollTop + targetRect.top - scrollerRect.top;
    const centerOffset = Math.max(0, (scroller.clientHeight - targetRect.height) / 2);
    return Math.max(0, targetTop - centerOffset);
  }, [getRenderedUserMessageElement]);
  const expandInitialHistoryRenderWindow = useCallback((reason: string) => {
    if (
      !useStaticInitialHistoryList ||
      !initialHistoryRenderWindow.isWindowed ||
      expandedInitialHistoryRenderKey === initialHistoryRenderKey
    ) {
      return;
    }

    const scroller = scrollerElementRef.current;
    if (scroller) {
      const maxScrollTop = Math.max(0, scroller.scrollHeight - scroller.clientHeight);
      pendingInitialHistoryExpansionRef.current = {
        scrollTop: scroller.scrollTop,
        scrollHeight: scroller.scrollHeight,
        omittedEstimatedHeightPx: initialHistoryRenderWindow.omittedEstimatedHeightPx,
        wasAtBottom: Math.abs(maxScrollTop - scroller.scrollTop) <= COMPENSATION_EPSILON_PX,
      };
    } else {
      pendingInitialHistoryExpansionRef.current = null;
    }

    startupTrace.markPhase('flowchat_initial_history_render_window_expanded', {
      sessionId: activeSessionId,
      reason,
      startIndex: initialHistoryRenderWindow.startIndex,
      renderedItemCount: initialHistoryRenderWindow.items.length,
      totalItemCount: virtualItems.length,
      omittedEstimatedHeightPx: Math.round(initialHistoryRenderWindow.omittedEstimatedHeightPx),
    });
    setExpandedInitialHistoryRenderKey(initialHistoryRenderKey);
  }, [
    activeSessionId,
    expandedInitialHistoryRenderKey,
    initialHistoryRenderKey,
    initialHistoryRenderWindow,
    useStaticInitialHistoryList,
    virtualItems.length,
  ]);
  useLayoutEffect(() => {
    const pendingAnchorTurnId = pendingStaticAnchorTurnIdRef.current;
    if (!pendingAnchorTurnId || pendingAnchorTurnId !== staticAnchorWindowTurnId) {
      return;
    }

    const scroller = scrollerElementRef.current;
    const anchorScrollTop = getStaticAnchorScrollTop(pendingAnchorTurnId);
    if (!scroller || anchorScrollTop === null) {
      return;
    }

    scroller.scrollTo({
      top: anchorScrollTop,
      behavior: 'smooth',
    });
    previousScrollTopRef.current = anchorScrollTop;
    previousMeasuredHeightRef.current = snapshotMeasuredContentHeight(scroller);
    recordScrollerGeometry(scroller);
    pendingStaticAnchorTurnIdRef.current = null;
  }, [
    getStaticAnchorScrollTop,
    initialHistoryRenderKey,
    recordScrollerGeometry,
    snapshotMeasuredContentHeight,
    staticAnchorWindowTurnId,
  ]);
  const expandInitialHistoryRenderWindowIfNeeded = useCallback((reason: string) => {
    if (
      !useStaticInitialHistoryList ||
      !initialHistoryRenderWindow.isWindowed ||
      isInitialHistoryRenderWindowExpanded ||
      omittedInitialHistoryEstimatedHeightPx <= 0
    ) {
      return;
    }

    const scroller = scrollerElementRef.current;
    if (!scroller || scroller.scrollTop > omittedInitialHistoryEstimatedHeightPx) {
      return;
    }

    expandInitialHistoryRenderWindow(reason);
  }, [
    expandInitialHistoryRenderWindow,
    initialHistoryRenderWindow.isWindowed,
    isInitialHistoryRenderWindowExpanded,
    omittedInitialHistoryEstimatedHeightPx,
    useStaticInitialHistoryList,
  ]);
  const scheduleInitialHistoryRenderWindowCheck = useCallback((reason: string) => {
    if (
      !useStaticInitialHistoryList ||
      !initialHistoryRenderWindow.isWindowed ||
      isInitialHistoryRenderWindowExpanded
    ) {
      return;
    }

    if (initialHistoryRenderWindowCheckFrameRef.current !== null) {
      cancelAnimationFrame(initialHistoryRenderWindowCheckFrameRef.current);
    }

    const scheduledSessionId = activeSessionId;
    initialHistoryRenderWindowCheckFrameRef.current = requestAnimationFrame(() => {
      initialHistoryRenderWindowCheckFrameRef.current = null;
      if (activeSessionIdRef.current !== scheduledSessionId) {
        return;
      }
      expandInitialHistoryRenderWindowIfNeeded(reason);
    });
  }, [
    activeSessionId,
    expandInitialHistoryRenderWindowIfNeeded,
    initialHistoryRenderWindow.isWindowed,
    isInitialHistoryRenderWindowExpanded,
    useStaticInitialHistoryList,
  ]);
  const handleInitialHistoryStaticScroll = useCallback((event: React.UIEvent<HTMLDivElement>) => {
    const scroller = event.currentTarget;
    const distanceFromBottom = Math.max(
      0,
      getEffectiveBottomScrollTop(scroller) - scroller.scrollTop,
    );
    const atBottom = distanceFromBottom <= 50;
    setIsAtBottom(atBottom);
    if (atBottom && staticAnchorWindowTurnId) {
      pendingStaticTurnPinRef.current = null;
      pendingStaticAnchorTurnIdRef.current = null;
      setStaticAnchorWindowTurnId(null);
    }
    expandInitialHistoryRenderWindowIfNeeded('scroll-near-omitted-history');
  }, [
    expandInitialHistoryRenderWindowIfNeeded,
    getEffectiveBottomScrollTop,
    staticAnchorWindowTurnId,
  ]);
  const handleInitialHistoryStaticWheelCapture = useCallback(() => {
    scheduleInitialHistoryRenderWindowCheck('wheel-near-omitted-history');
  }, [scheduleInitialHistoryRenderWindowCheck]);
  const handleInitialHistoryStaticKeyDownCapture = useCallback((event: React.KeyboardEvent<HTMLDivElement>) => {
    if (
      event.key === 'Home' ||
      event.key === 'PageUp' ||
      event.key === 'ArrowUp'
    ) {
      scheduleInitialHistoryRenderWindowCheck(`keyboard-${event.key}`);
    }
  }, [scheduleInitialHistoryRenderWindowCheck]);
  useEffect(() => {
    if (
      !useStaticInitialHistoryList ||
      !initialHistoryRenderWindow.isWindowed ||
      markedInitialHistoryRenderWindowKeyRef.current === initialHistoryRenderKey
    ) {
      return;
    }

    markedInitialHistoryRenderWindowKeyRef.current = initialHistoryRenderKey;
    startupTrace.markPhase('flowchat_initial_history_render_window', {
      sessionId: activeSessionId,
      startIndex: initialHistoryRenderWindow.startIndex,
      renderedItemCount: initialHistoryRenderWindow.items.length,
      totalItemCount: virtualItems.length,
      omittedEstimatedHeightPx: Math.round(initialHistoryRenderWindow.omittedEstimatedHeightPx),
      renderedEstimatedHeightPx: Math.round(initialHistoryRenderWindow.renderedEstimatedHeightPx),
    });
  }, [
    activeSessionId,
    initialHistoryRenderKey,
    initialHistoryRenderWindow,
    useStaticInitialHistoryList,
    virtualItems.length,
  ]);
  useLayoutEffect(() => {
    const pending = pendingInitialHistoryExpansionRef.current;
    if (!pending) {
      return;
    }

    if (expandedInitialHistoryRenderKey !== initialHistoryRenderKey) {
      pendingInitialHistoryExpansionRef.current = null;
      pendingStaticAnchorTurnIdRef.current = null;
      return;
    }

    pendingInitialHistoryExpansionRef.current = null;
    const scroller = scrollerElementRef.current;
    if (!scroller) {
      return;
    }

    const pendingAnchorTurnId = pendingStaticAnchorTurnIdRef.current;
    const anchorScrollTop = pendingAnchorTurnId
      ? getStaticAnchorScrollTop(pendingAnchorTurnId)
      : null;
    pendingStaticAnchorTurnIdRef.current = null;

    const nextScrollTop = anchorScrollTop ?? mapInitialHistoryExpansionScrollTop({
      previousScrollTop: pending.scrollTop,
      previousScrollHeight: pending.scrollHeight,
      nextScrollHeight: scroller.scrollHeight,
      omittedEstimatedHeightPx: pending.omittedEstimatedHeightPx,
      wasAtBottom: pending.wasAtBottom,
      clientHeight: scroller.clientHeight,
    });
    scroller.scrollTop = nextScrollTop;
    previousScrollTopRef.current = nextScrollTop;
    previousMeasuredHeightRef.current = snapshotMeasuredContentHeight(scroller);
    recordScrollerGeometry(scroller);
  }, [
    expandedInitialHistoryRenderKey,
    getStaticAnchorScrollTop,
    initialHistoryRenderKey,
    recordScrollerGeometry,
    snapshotMeasuredContentHeight,
  ]);
  useEffect(() => {
    return () => {
      if (initialHistoryRenderWindowCheckFrameRef.current !== null) {
        cancelAnimationFrame(initialHistoryRenderWindowCheckFrameRef.current);
        initialHistoryRenderWindowCheckFrameRef.current = null;
      }
    };
  }, []);
  const sessionOpenProjectionHandoff = React.useMemo<HistoryProjectionHandoffSnapshot | null>(() => {
    const previousActiveSessionId = previousActiveSessionIdForOpenHandoffRef.current;
    const isSessionSwitch = (
      previousActiveSessionId !== undefined &&
      previousActiveSessionId !== activeSessionId
    );

    if (
      !activeSessionId ||
      !isSessionSwitch ||
      activeSession?.historyState !== 'ready' ||
      activeSession?.isPartial === true ||
      useStaticInitialHistoryList ||
      !latestTurnId ||
      virtualItems.length < SESSION_OPEN_HANDOFF_ITEM_BUDGET ||
      sessionOpenHandoffSessionIdRef.current === activeSessionId ||
      historyProjectionHandoff?.sessionId === activeSessionId
    ) {
      return null;
    }

    const budgetStartIndex = Math.max(0, virtualItems.length - SESSION_OPEN_HANDOFF_ITEM_BUDGET);
    const latestStartIndex = Math.max(0, Math.min(latestUserMessageIndex, virtualItems.length - 1));
    const startIndex = Math.min(budgetStartIndex, latestStartIndex);
    const items = virtualItems.slice(startIndex);
    if (items.length === 0) {
      return null;
    }

    return {
      sessionId: activeSessionId,
      reason: 'session-open',
      createdAtMs: performance.now(),
      items,
      mode: 'bottom-tail',
      targetTurnId: latestTurnId,
      footerHeightPx,
    };
  }, [
    activeSession?.historyState,
    activeSession?.isPartial,
    activeSessionId,
    footerHeightPx,
    historyProjectionHandoff?.sessionId,
    latestTurnId,
    latestUserMessageIndex,
    useStaticInitialHistoryList,
    virtualItems,
  ]);
  useLayoutEffect(() => {
    if (
      !sessionOpenProjectionHandoff ||
      sessionOpenHandoffSessionIdRef.current === sessionOpenProjectionHandoff.sessionId
    ) {
      return;
    }

    historyProjectionHandoffRef.current = sessionOpenProjectionHandoff;
    sessionOpenHandoffSessionIdRef.current = sessionOpenProjectionHandoff.sessionId;
    setHistoryProjectionHandoff(sessionOpenProjectionHandoff);
    startupTrace.markPhase('flowchat_history_projection_handoff_activated', {
      sessionId: sessionOpenProjectionHandoff.sessionId,
      reason: sessionOpenProjectionHandoff.reason,
      previousItemCount: 0,
      nextItemCount: virtualItems.length,
    });
    // Use a longer initial delay (5 frames).  rangeChanged accelerations
    // have been removed so the handoff is only released once the Virtuoso
    // has had enough time to measure and position items, preventing a
    // blank viewport ("white screen") on session switches.
    scheduleHistoryProjectionHandoffRelease(5);
  }, [
    scheduleHistoryProjectionHandoffRelease,
    sessionOpenProjectionHandoff,
    virtualItems.length,
  ]);
  useLayoutEffect(() => {
    previousActiveSessionIdForOpenHandoffRef.current = activeSessionId;
  }, [activeSessionId]);
  const activeHistoryProjectionHandoff =
    activeSessionHistoryProjectionHandoff(historyProjectionHandoff, activeSessionId) ??
    activeSessionHistoryProjectionHandoff(sessionOpenProjectionHandoff, activeSessionId);
  const hasCompactHistoricalProjection = virtualItems.length >= 6 && virtualItems
    .slice(-16)
    .every(item =>
      item.type === 'user-message' ||
      item.type === 'user-steering-message' ||
      item.type === 'turn-completion-notice' ||
      item.type === 'turn-failure-notice' ||
      item.type === 'explore-group'
    );
  const hasInitialHistoryModelRoundProjection =
    useInitialHistoryRenderBudget &&
    virtualItems.slice(-16).some(item => item.type === 'model-round');
  const defaultItemHeight = getVirtualMessageDefaultItemHeight({
    isHistorical: activeSession?.isHistorical === true,
    hasCompactHistoricalProjection,
    hasInitialHistoryModelRoundProjection,
  });
  const initialHistoryHeightEstimates = React.useMemo(
    () => useInitialHistoryRenderBudget && !useStaticInitialHistoryList
      ? virtualItems.map(estimateVirtualMessageItemHeight)
      : undefined,
    [useInitialHistoryRenderBudget, useStaticInitialHistoryList, virtualItems],
  );
  useLayoutEffect(() => {
    if (!useStaticInitialHistoryList) {
      autoScrolledInitialHistoryRenderKeyRef.current = null;
      staticInitialHistoryUserLeftBottomRef.current = false;
      cancelStaticInitialHistoryBottomGuard();
      return;
    }

    const previousAutoScrollKey = autoScrolledInitialHistoryRenderKeyRef.current;
    if (previousAutoScrollKey === initialHistoryRenderKey) {
      return;
    }

    const scroller = scrollerElementRef.current;
    if (!scroller || staticAnchorWindowTurnId || pendingStaticAnchorTurnIdRef.current) {
      return;
    }

    if (previousAutoScrollKey !== null) {
      const distanceFromBottom = Math.max(
        0,
        getEffectiveBottomScrollTop(scroller) - scroller.scrollTop,
      );
      const wasAtEffectiveBottom = isGeometryAtEffectiveBottom(previousScrollerGeometryRef.current);
      if (
        staticInitialHistoryUserLeftBottomRef.current ||
        (!wasAtEffectiveBottom && distanceFromBottom > LATEST_END_ANCHOR_STABLE_EPSILON_PX)
      ) {
        staticInitialHistoryUserLeftBottomRef.current = true;
        recordScrollerGeometry(scroller);
        return;
      }
    }

    autoScrolledInitialHistoryRenderKeyRef.current = initialHistoryRenderKey;
    const nextScrollTop = getEffectiveBottomScrollTop(scroller);
    scroller.scrollTop = nextScrollTop;
    staticInitialHistoryUserLeftBottomRef.current = false;
    previousScrollTopRef.current = nextScrollTop;
    previousMeasuredHeightRef.current = snapshotMeasuredContentHeight(scroller);
    recordScrollerGeometry(scroller);
    startStaticInitialHistoryBottomGuard();
    scheduleVisibleTurnMeasure(1);
  }, [
    activeSessionId,
    cancelStaticInitialHistoryBottomGuard,
    getEffectiveBottomScrollTop,
    initialHistoryRenderKey,
    isGeometryAtEffectiveBottom,
    latestTurnId,
    recordScrollerGeometry,
    scheduleVisibleTurnMeasure,
    snapshotMeasuredContentHeight,
    startStaticInitialHistoryBottomGuard,
    staticAnchorWindowTurnId,
    useStaticInitialHistoryList,
  ]);
  useLayoutEffect(() => {
    if (
      !useStaticInitialHistoryList ||
      autoScrolledInitialHistoryRenderKeyRef.current !== initialHistoryRenderKey ||
      performance.now() <= userInitiatedUpwardScrollUntilMsRef.current
    ) {
      return;
    }

    const scroller = scrollerElementRef.current;
    if (!scroller) {
      return;
    }

    const effectiveBottomScrollTop = getEffectiveBottomScrollTop(scroller);
    if (Math.abs(effectiveBottomScrollTop - scroller.scrollTop) <= LATEST_END_ANCHOR_STABLE_EPSILON_PX) {
      staticInitialHistoryUserLeftBottomRef.current = false;
      recordScrollerGeometry(scroller);
      return;
    }

    if (staticInitialHistoryUserLeftBottomRef.current) {
      recordScrollerGeometry(scroller);
      return;
    }

    const previousWasAtBottom = isGeometryAtEffectiveBottom(previousScrollerGeometryRef.current);
    if (!previousWasAtBottom) {
      recordStaticInitialHistoryBottomState(scroller);
      recordScrollerGeometry(scroller);
      return;
    }

    scroller.scrollTop = effectiveBottomScrollTop;
    staticInitialHistoryUserLeftBottomRef.current = false;
    previousScrollTopRef.current = effectiveBottomScrollTop;
    previousMeasuredHeightRef.current = snapshotMeasuredContentHeight(scroller);
    recordScrollerGeometry(scroller);
  }, [
    footerHeightPx,
    getEffectiveBottomScrollTop,
    initialHistoryRenderKey,
    isGeometryAtEffectiveBottom,
    recordScrollerGeometry,
    recordStaticInitialHistoryBottomState,
    snapshotMeasuredContentHeight,
    useStaticInitialHistoryList,
  ]);
  const previousHistoryBoundaryStatusNode = React.useMemo(
    () => previousHistoryBoundaryStatus?.sessionId === activeSessionId ? (
      <div
        className="virtual-message-list__history-boundary-status"
        data-history-boundary-status={previousHistoryBoundaryStatus.state}
        role="status"
        aria-live="polite"
      >
        {previousHistoryBoundaryStatus.state === 'preparing'
          ? t('historyState.preparingOlderHistory')
          : t('historyState.olderHistoryNotReady')}
      </div>
    ) : null,
    [activeSessionId, previousHistoryBoundaryStatus, t],
  );
  const virtuosoContext = React.useMemo<FlowChatVirtuosoContext>(() => ({
    // Reservation pixels are applied imperatively to the stable Footer node.
    // Keeping them out of context prevents a measurement-sensitive Virtuoso
    // render for every compensation update.
    footerRef: handleFooterElementRef,
    previousHistoryBoundaryStatusNode,
    runtimeStatusSessionId: activeSessionId,
  }), [
    activeSessionId,
    handleFooterElementRef,
    previousHistoryBoundaryStatusNode,
  ]);
  const computeVirtuosoItemKey = useCallback((_: number, item: VirtualItem) => (
    `${activeSessionId ?? 'no-active-session'}:${getVirtualItemStableKey(item)}`
  ), [activeSessionId]);
  const renderVirtuosoItem = useCallback((index: number, item: VirtualItem) => (
    <VirtualItemRenderer
      item={item}
      index={index - virtuosoFirstItemIndex}
    />
  ), [virtuosoFirstItemIndex]);
  // ── Render ────────────────────────────────────────────────────────────
  if (virtualItems.length === 0) {
    return (
      <div
        className="virtual-message-list virtual-message-list--empty"
        data-testid="flowchat-message-list-empty"
      >
        <div className="empty-state">
          <p>No messages yet</p>
        </div>
      </div>
    );
  }

  return (
    <div
      className="virtual-message-list"
      data-testid="flowchat-message-list"
    >
      {useStaticInitialHistoryList ? (
        <div
          ref={handleScrollerRef}
          className="virtual-message-list__static-scroller"
          data-virtuoso-scroller="true"
          data-initial-history-render-windowed={initialHistoryRenderWindow.isWindowed ? 'true' : 'false'}
          onScroll={handleInitialHistoryStaticScroll}
          onWheelCapture={handleInitialHistoryStaticWheelCapture}
          onKeyDownCapture={handleInitialHistoryStaticKeyDownCapture}
        >
          <div className="message-list-header" />
          {previousHistoryBoundaryStatusNode}
          {omittedInitialHistoryEstimatedHeightPx > 0 ? (
            <div
              className="virtual-message-list__initial-history-spacer"
              data-history-initial-render-spacer="true"
              aria-hidden="true"
              style={{
                height: `${Math.round(omittedInitialHistoryEstimatedHeightPx)}px`,
              }}
            />
          ) : null}
          <div className="virtual-message-list__static-items">
            {renderedInitialHistoryItems.map((item, localIndex) => (
              <VirtualItemRenderer
                key={`${activeSessionId ?? 'no-active-session'}:${getVirtualItemStableKey(item)}`}
                item={item}
                index={renderedInitialHistoryStartIndex + localIndex}
              />
            ))}
          </div>
          {trailingOmittedInitialHistoryEstimatedHeightPx > 0 ? (
            <div
              className="virtual-message-list__initial-history-spacer"
              data-history-initial-render-tail-spacer="true"
              aria-hidden="true"
              style={{
                height: `${Math.round(trailingOmittedInitialHistoryEstimatedHeightPx)}px`,
              }}
            />
          ) : null}
          <div
            ref={handleFooterElementRef}
            className="message-list-footer"
            style={{
              height: `${footerHeightPx}px`,
              minHeight: `${footerHeightPx}px`,
            }}
          >
            <RuntimeStatusSlot sessionId={activeSessionId} placement="footer" />
          </div>
        </div>
      ) : (
        <Virtuoso
          key={activeSessionId ?? 'no-active-session'}
          ref={virtuosoRef}
          data={virtualItems}
          computeItemKey={computeVirtuosoItemKey}
          itemContent={renderVirtuosoItem}
          followOutput={false}

          alignToBottom={false}
          firstItemIndex={virtuosoFirstItemIndex}
          // New mounts start near the latest user turn to avoid flashing older
          // content before sticky pin logic can finish.
          initialTopMostItemIndex={initialTopMostItemIndex}
          overscan={FLOW_CHAT_VIRTUOSO_OVERSCAN}

          atBottomThreshold={50}
          atBottomStateChange={handleAtBottomStateChange}

          rangeChanged={handleRangeChanged}

          // Historical sessions often restore into compact user/explore rows.
          // Keep live sessions on the legacy estimate because active assistant
          // output can be much taller while streaming.
          defaultItemHeight={defaultItemHeight}
          heightEstimates={initialHistoryHeightEstimates}

          increaseViewportBy={FLOW_CHAT_VIRTUOSO_VIEWPORT_INCREASE}

          scrollerRef={handleScrollerRef}
          context={virtuosoContext}
          components={FLOW_CHAT_VIRTUOSO_COMPONENTS}
        />
      )}

      {activeHistoryProjectionHandoff ? (
        <div
          className="virtual-message-list__projection-handoff-overlay"
          data-history-projection-handoff="true"
          data-session-id={activeHistoryProjectionHandoff.sessionId}
          aria-hidden="true"
        >
          <div
            className="virtual-message-list__projection-handoff-content virtual-message-list__projection-handoff-content--bottom"
          >
            <div className="message-list-header" />
            <div className="virtual-message-list__static-items">
              {activeHistoryProjectionHandoff.items.map((item, index) => (
                <VirtualItemRenderer
                  key={`history-projection-handoff:${activeHistoryProjectionHandoff.sessionId}:${getVirtualItemStableKey(item)}`}
                  item={item}
                  index={index}
                />
              ))}
            </div>
            <div
              className="message-list-footer"
              style={{
                height: `${activeHistoryProjectionHandoff.footerHeightPx}px`,
                minHeight: `${activeHistoryProjectionHandoff.footerHeightPx}px`,
              }}
            />
          </div>
        </div>
      ) : null}

      <ScrollAnchor
        onAnchorNavigate={(turnId) => {
          if (!turnId) return;

          if (virtuosoRef.current) {
            pinTurnToTop(turnId, { behavior: 'smooth' });
            return;
          }

          const targetItem = userMessageItems.find(({ item }) => item.turnId === turnId);
          if (!targetItem) return;

          retainedCollapseAnchorRef.current = null;
          exitFollowOutput('scroll-to-turn');
          clearPinReservationForUserNavigation();

          pendingStaticAnchorTurnIdRef.current = turnId;
          const anchorScrollTop = getStaticAnchorScrollTop(turnId);
          if (anchorScrollTop === null) {
            setStaticAnchorWindowTurnId(turnId);
            return;
          }

          if (scrollerElementRef.current) {
            scrollerElementRef.current.scrollTo({
              top: anchorScrollTop,
              behavior: 'smooth',
            });
          }
          pendingStaticAnchorTurnIdRef.current = null;
        }}
        scrollerRef={scrollerElementRef}
      />

      <ScrollToTurnHeaderButton
        visible={shouldShowTurnHeaderButton}
        onClick={handleTurnHeaderClick}
        turnLabel={visibleTurnInfo ? `Turn ${visibleTurnInfo.turnIndex}` : undefined}
      />

      <StickyTaskIndicator
        visible={!!visibleTaskInfo}
        taskInfo={visibleTaskInfo}
        onClick={scrollToTask}
      />

      <ScrollToLatestBar
        visible={!isAtBottom && virtualItems.length > 0}
        onClick={scrollToLatestEndPosition}
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
  const sessionKey = activeSession?.sessionId ?? 'no-active-session';

  return <VirtualMessageListSession key={sessionKey} ref={ref} {...props} />;
});

VirtualMessageList.displayName = 'VirtualMessageList';
