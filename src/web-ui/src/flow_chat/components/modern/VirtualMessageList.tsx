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
import { Virtuoso, VirtuosoHandle } from 'react-virtuoso';
import { useActiveSessionState } from '../../hooks/useActiveSessionState';
import { VirtualItemRenderer } from './VirtualItemRenderer';
import { ScrollToLatestBar } from '../ScrollToLatestBar';
import { ScrollToTurnHeaderButton } from '../ScrollToTurnHeaderButton';
import { useScrollToTurnHeader } from '../../hooks/useScrollToTurnHeader';
import { useVisibleTaskInfo } from '../../hooks/useVisibleTaskInfo';
import { StickyTaskIndicator } from '../StickyTaskIndicator';
import { ProcessingIndicator } from './ProcessingIndicator';
import {
  shouldReserveProcessingIndicatorSpace,
  shouldShowProcessingIndicator,
} from './processingIndicatorVisibility';
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
import {
  estimateVirtualMessageItemHeight,
  getVirtualMessageDefaultItemHeight,
  mapInitialHistoryExpansionScrollTop,
  selectInitialHistoryRenderWindow,
} from './virtualMessageListLayout';
import './VirtualMessageList.scss';

const COMPENSATION_EPSILON_PX = 0.5;
const ANCHOR_LOCK_MIN_DEVIATION_PX = 0.5;
const ANCHOR_LOCK_DURATION_MS = 450;
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

type LatestEndAnchorResolveReason =
  | 'raf'
  | 'range-changed'
  | 'resize-observer'
  | 'transition-finish';

// Read `FLOWCHAT_SCROLL_STABILITY.md` before changing collapse compensation logic.

/**
 * Methods exposed by VirtualMessageList.
 */
export interface VirtualMessageListRef {
  scrollToTurn: (turnIndex: number) => void;
  scrollToIndex: (index: number) => void;
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
}

interface ScrollAnchorLockState {
  active: boolean;
  targetScrollTop: number;
  reason: 'transition-shrink' | 'instant-shrink' | null;
  lockUntilMs: number;
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

interface HistoryProjectionHandoffSnapshot {
  sessionId: string;
  reason: string;
  createdAtMs: number;
  items: VirtualItem[];
  mode: 'scroll-position' | 'bottom-tail';
  targetTurnId: string | null;
  anchorKey: string | null;
  anchorOffsetTopPx: number;
  scrollTop: number;
  scrollHeight: number;
  clientHeight: number;
  footerHeightPx: number;
}

type BottomReservationKind = 'collapse' | 'pin';

interface BottomReservationBase {
  kind: BottomReservationKind;
  px: number;
  floorPx: number;
}

interface CollapseBottomReservation extends BottomReservationBase {
  kind: 'collapse';
}

interface PinBottomReservation extends BottomReservationBase {
  kind: 'pin';
  mode: FlowChatPinTurnToTopMode;
  targetTurnId: string | null;
}

interface BottomReservationState {
  collapse: CollapseBottomReservation;
  pin: PinBottomReservation;
}

interface ScrollerGeometrySnapshot {
  scrollTop: number;
  scrollHeight: number;
  clientHeight: number;
}

interface PendingTurnPinState {
  turnId: string;
  behavior: ScrollBehavior;
  pinMode: FlowChatPinTurnToTopMode;
  expiresAtMs: number;
  attempts: number;
}

function createInitialBottomReservationState(): BottomReservationState {
  return {
    collapse: {
      kind: 'collapse',
      px: 0,
      floorPx: 0,
    },
    pin: {
      kind: 'pin',
      px: 0,
      floorPx: 0,
      mode: 'transient',
      targetTurnId: null,
    },
  };
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

function sanitizeBottomReservationState(state: BottomReservationState): BottomReservationState {
  const collapsePx = sanitizeReservationPx(state.collapse.px);
  const collapseFloorPx = Math.min(collapsePx, sanitizeReservationPx(state.collapse.floorPx));
  const pinPx = sanitizeReservationPx(state.pin.px);
  const pinFloorPx = Math.min(pinPx, sanitizeReservationPx(state.pin.floorPx));

  return {
    collapse: {
      kind: 'collapse',
      px: collapsePx,
      floorPx: collapseFloorPx,
    },
    pin: {
      kind: 'pin',
      px: pinPx,
      floorPx: pinFloorPx,
      mode: state.pin.mode ?? 'transient',
      targetTurnId: state.pin.targetTurnId ?? null,
    },
  };
}

function areBottomReservationStatesEqual(left: BottomReservationState, right: BottomReservationState): boolean {
  return (
    Math.abs(left.collapse.px - right.collapse.px) <= COMPENSATION_EPSILON_PX &&
    Math.abs(left.collapse.floorPx - right.collapse.floorPx) <= COMPENSATION_EPSILON_PX &&
    Math.abs(left.pin.px - right.pin.px) <= COMPENSATION_EPSILON_PX &&
    Math.abs(left.pin.floorPx - right.pin.floorPx) <= COMPENSATION_EPSILON_PX &&
    left.pin.mode === right.pin.mode &&
    left.pin.targetTurnId === right.pin.targetTurnId
  );
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

function getReservationTotalPx(reservation: BottomReservationBase): number {
  return Math.max(0, reservation.px);
}

function getReservationConsumablePx(reservation: BottomReservationBase): number {
  return Math.max(0, reservation.px - reservation.floorPx);
}

const VirtualMessageListSession = forwardRef<VirtualMessageListRef>((_, ref) => {
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

  const scrollerElementRef = useRef<HTMLElement | null>(null);
  const footerElementRef = useRef<HTMLDivElement | null>(null);
  const activeSessionIdRef = useRef<string | null>(null);
  const pendingHistoryProjectionHandoffRef = useRef<HistoryProjectionHandoffSnapshot | null>(null);
  const historyProjectionHandoffRef = useRef<HistoryProjectionHandoffSnapshot | null>(null);
  const historyProjectionHandoffReleaseFrameRef = useRef<number | null>(null);
  const fullHistoryProjectionIntentFrameRef = useRef<number | null>(null);
  const pendingFullHistoryProjectionReasonRef = useRef<string | null>(null);
  const sessionOpenHandoffSessionIdRef = useRef<string | null>(null);
  const previousActiveSessionIdForOpenHandoffRef = useRef<string | null | undefined>(undefined);
  const bottomReservationStateRef = useRef<BottomReservationState>(createInitialBottomReservationState());
  const previousMeasuredHeightRef = useRef<number | null>(null);
  const previousScrollTopRef = useRef(0);
  const previousScrollerGeometryRef = useRef<ScrollerGeometrySnapshot | null>(null);
  const markedInitialHistoryRenderWindowKeyRef = useRef<string | null>(null);
  const autoScrolledInitialHistoryRenderKeyRef = useRef<string | null>(null);
  const pendingInitialHistoryExpansionRef = useRef<{
    scrollTop: number;
    scrollHeight: number;
    omittedEstimatedHeightPx: number;
    wasAtBottom: boolean;
  } | null>(null);
  const initialHistoryRenderWindowCheckFrameRef = useRef<number | null>(null);
  const measureFrameRef = useRef<number | null>(null);
  const visibleTurnMeasureFrameRef = useRef<number | null>(null);
  const pinReservationReconcileFrameRef = useRef<number | null>(null);
  const turnPinStabilizationFrameRef = useRef<number | null>(null);
  const latestEndAnchorStabilizationFrameRef = useRef<number | null>(null);
  const staticInitialHistoryBottomGuardFrameRef = useRef<number | null>(null);
  const staticInitialHistoryBottomGuardUntilMsRef = useRef(0);
  const latestEndAnchorRequestRef = useRef<LatestEndAnchorRequestState | null>(null);
  const resolveLatestEndAnchorStabilizationRef = useRef<((reason: LatestEndAnchorResolveReason) => boolean) | null>(null);
  const resizeObserverRef = useRef<ResizeObserver | null>(null);
  const mutationObserverRef = useRef<MutationObserver | null>(null);
  const layoutTransitionCountRef = useRef(0);
  const touchScrollIntentStartYRef = useRef<number | null>(null);
  const scrollbarPointerInteractionActiveRef = useRef(false);
  // Timestamp until which we treat any upward scroll as user-initiated. Set by
  // wheel/touch/keyboard/scrollbar handlers BEFORE the browser actually moves
  // the scroller. Used by the handleScroll "shrink-clamp restore" intercept
  // (below) to distinguish a genuine user upward scroll from a browser auto
  // clamp caused by content shrinking near the bottom in follow mode.
  const userInitiatedUpwardScrollUntilMsRef = useRef(0);
  const anchorLockRef = useRef<ScrollAnchorLockState>({
    active: false,
    targetScrollTop: 0,
    reason: null,
    lockUntilMs: 0,
  });
  const pendingCollapseIntentRef = useRef<PendingCollapseIntentState>({
    active: false,
    anchorScrollTop: 0,
    toolId: null,
    toolName: null,
    expiresAtMs: 0,
    distanceFromBottomBeforeCollapse: 0,
    baseTotalCompensationPx: 0,
    cumulativeShrinkPx: 0,
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
  // pre-compensation and anchor lock so the continuous follow loop can keep
  // tracking the bottom without fighting the layout-stability machinery.
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
  const processingPhase = activeSessionState.processingPhase;

  const getFooterHeightPx = useCallback((compensationPx: number) => {
    return inputStackFooterPxRef.current + compensationPx;
  }, []);

  const getTotalBottomCompensationPx = useCallback((state: BottomReservationState = bottomReservationStateRef.current) => {
    return getReservationTotalPx(state.collapse) + getReservationTotalPx(state.pin);
  }, []);

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

  const syncPhysicalBottomAfterViewportResize = useCallback((scroller: HTMLElement): boolean => {
    const previousGeometry = previousScrollerGeometryRef.current;
    if (!previousGeometry) {
      return false;
    }

    const viewportGeometryChanged =
      Math.abs(scroller.scrollHeight - previousGeometry.scrollHeight) > COMPENSATION_EPSILON_PX ||
      Math.abs(scroller.clientHeight - previousGeometry.clientHeight) > COMPENSATION_EPSILON_PX;
    if (!viewportGeometryChanged || pendingCollapseIntentRef.current.active) {
      return false;
    }

    const previousMaxScrollTop = Math.max(0, previousGeometry.scrollHeight - previousGeometry.clientHeight);
    const wasAtPhysicalBottom =
      Math.abs(previousMaxScrollTop - previousGeometry.scrollTop) <= LATEST_END_ANCHOR_STABLE_EPSILON_PX;
    if (!wasAtPhysicalBottom) {
      return false;
    }

    const maxScrollTop = Math.max(0, scroller.scrollHeight - scroller.clientHeight);
    if (Math.abs(maxScrollTop - scroller.scrollTop) > COMPENSATION_EPSILON_PX) {
      scroller.scrollTop = maxScrollTop;
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
    if (!scroller?.classList.contains('virtual-message-list__static-scroller')) {
      return;
    }

    const maxScrollTop = Math.max(0, scroller.scrollHeight - scroller.clientHeight);
    if (Math.abs(maxScrollTop - scroller.scrollTop) > LATEST_END_ANCHOR_STABLE_EPSILON_PX) {
      return;
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
        !currentScroller?.classList.contains('virtual-message-list__static-scroller') ||
        now <= userInitiatedUpwardScrollUntilMsRef.current
      ) {
        staticInitialHistoryBottomGuardUntilMsRef.current = 0;
        return;
      }

      syncPhysicalBottomAfterViewportResize(currentScroller);
      const distanceFromBottom = Math.max(
        0,
        currentScroller.scrollHeight - currentScroller.clientHeight - currentScroller.scrollTop,
      );
      if (distanceFromBottom <= LATEST_END_ANCHOR_STABLE_EPSILON_PX) {
        recordScrollerGeometry(currentScroller);
      }

      staticInitialHistoryBottomGuardFrameRef.current = requestAnimationFrame(tick);
    };

    staticInitialHistoryBottomGuardFrameRef.current = requestAnimationFrame(tick);
  }, [recordScrollerGeometry, syncPhysicalBottomAfterViewportResize]);

  const updateBottomReservationState = useCallback((
    updater: BottomReservationState | ((prev: BottomReservationState) => BottomReservationState),
  ) => {
    setBottomReservationState(prev => {
      const rawNext = typeof updater === 'function' ? updater(prev) : updater;
      const next = sanitizeBottomReservationState(rawNext);
      bottomReservationStateRef.current = next;
      return areBottomReservationStatesEqual(next, prev) ? prev : next;
    });
  }, []);

  const clearTurnPinRequest = useCallback(() => {
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

  const consumeBottomCompensation = useCallback((amountPx: number) => {
    if (amountPx <= COMPENSATION_EPSILON_PX) {
      return bottomReservationStateRef.current;
    }

    let resolvedNextState = bottomReservationStateRef.current;
    updateBottomReservationState(prev => {
      let remaining = Math.max(0, amountPx);

      const collapseConsumablePx = getReservationConsumablePx(prev.collapse);
      const collapseConsumed = Math.min(collapseConsumablePx, remaining);
      remaining -= collapseConsumed;

      const pinConsumablePx = getReservationConsumablePx(prev.pin);
      const pinConsumed = Math.min(pinConsumablePx, remaining);

      const nextState: BottomReservationState = {
        collapse: {
          ...prev.collapse,
          px: Math.max(prev.collapse.floorPx, prev.collapse.px - collapseConsumed),
        },
        pin: {
          ...prev.pin,
          px: Math.max(prev.pin.floorPx, prev.pin.px - pinConsumed),
        },
      };
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
    const footerHeightPx = getFooterHeightPx(compensationPx);
    footer.style.height = `${footerHeightPx}px`;
    footer.style.minHeight = `${footerHeightPx}px`;
    void footer.offsetHeight;
    void scroller.scrollHeight;
  }, [getFooterHeightPx, getTotalBottomCompensationPx]);

  // When the input collapses (e.g. user sends a message), the ResizeObserver
  // fires synchronously, `inputHeight` drops, and `inputStackFooterPx` shrinks.
  // React commits the smaller footer height, the browser clamps scrollTop
  // upward, and the viewport shows blank space ("white screen").  The existing
  // scroll-clamp protection in handleScroll is gated on isFollowingOutput
  // which is false at this point (follow has been armed, not yet activated).
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
      return;
    }

    const scroller = scrollerElementRef.current;
    // Use the pre-commit scrollTop captured during render (before React
    // applied the smaller footer height and the browser clamped).
    // This is the only reliable source of the pre-clamp value because
    // by the time useLayoutEffect runs, scroller.scrollTop has already
    // been reduced by the browser's clamp.
    const preClampScrollTop = footerShrinkPreScrollTopRef.current;

    const baseState = bottomReservationStateRef.current;
    const currentCollapsePx = getReservationTotalPx(baseState.collapse);
    const nextReservationState: BottomReservationState = {
      ...baseState,
      collapse: {
        ...baseState.collapse,
        px: currentCollapsePx + shrinkAmount,
        floorPx: 0,
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
  }, [inputStackFooterPx, updateBottomReservationState, applyFooterCompensationNow]);

  const releaseAnchorLock = useCallback((_reason: string) => {
    if (!anchorLockRef.current.active) return;
    anchorLockRef.current = {
      active: false,
      targetScrollTop: 0,
      reason: null,
      lockUntilMs: 0,
    };
  }, []);

  const activateAnchorLock = useCallback((targetScrollTop: number, reason: 'transition-shrink' | 'instant-shrink') => {
    const nextTarget = Math.max(anchorLockRef.current.targetScrollTop, targetScrollTop);
    anchorLockRef.current = {
      active: true,
      targetScrollTop: nextTarget,
      reason,
      lockUntilMs: performance.now() + ANCHOR_LOCK_DURATION_MS,
    };
  }, []);

  const restoreAnchorLockNow = useCallback((reason: string) => {
    const scroller = scrollerElementRef.current;
    const lockState = anchorLockRef.current;
    if (!scroller || !lockState.active) return false;

    const now = performance.now();
    if (now > lockState.lockUntilMs && layoutTransitionCountRef.current === 0) {
      releaseAnchorLock(`expired-before-${reason}`);
      return false;
    }

    const maxScrollTop = Math.max(0, scroller.scrollHeight - scroller.clientHeight);
    const targetScrollTop = Math.min(lockState.targetScrollTop, maxScrollTop);
    const currentScrollTop = scroller.scrollTop;
    const restoreDelta = targetScrollTop - currentScrollTop;

    if (Math.abs(restoreDelta) <= ANCHOR_LOCK_MIN_DEVIATION_PX) {
      return false;
    }

    scroller.scrollTop = targetScrollTop;
    previousScrollTopRef.current = targetScrollTop;
    recordScrollerGeometry(scroller);
    return true;
  }, [recordScrollerGeometry, releaseAnchorLock]);

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

    if (viewportGeometryChanged && !pendingCollapseIntentRef.current.active) {
      if (wasAtPhysicalBottom) {
        const maxScrollTop = Math.max(0, scroller.scrollHeight - scroller.clientHeight);
        if (Math.abs(maxScrollTop - scroller.scrollTop) > COMPENSATION_EPSILON_PX) {
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
      if (currentTotalCompensation > COMPENSATION_EPSILON_PX && layoutTransitionCountRef.current > 0) {
        previousScrollTopRef.current = currentScrollTop;
        recordScrollerGeometry(scroller);
        return;
      }

      const nextReservationState = consumeBottomCompensation(heightDelta);
      applyFooterCompensationNow(nextReservationState);
      previousScrollTopRef.current = currentScrollTop;
      recordScrollerGeometry(scroller);
      return;
    }

    // Content shrank: preserve the current visual anchor by extending the footer
    // when the user does not already have enough distance from the bottom.
    const shrinkAmount = -heightDelta;
    // Note: previously this branch returned early in follow-output mode to let
    // the continuous follow loop chase the bottom every frame. That caused the
    // visible "sink-down" jitter when tool-card auto-collapse shrank content
    // above the viewport. We now run the full compensation path regardless of
    // follow state — the bottom-reservation footer keeps `scrollHeight` stable
    // and the anchor lock preserves the upper visual anchor during the
    // animation. The continuous follow loop is gated by
    // `shouldSuspendAutoFollow` while a collapse intent / layout transition is
    // in flight, so it does not fight the anchor lock; once the transition
    // ends, the deferred follow path resumes bottom-tracking smoothly.
    const collapseIntent = pendingCollapseIntentRef.current;
    const now = performance.now();
    const hasValidCollapseIntent = collapseIntent.active && collapseIntent.expiresAtMs >= now;
    // For unsignaled shrinks, the visible gap to the bottom is what matters.
    // Existing synthetic footer compensation may be stale from an earlier
    // protected collapse, and subtracting it here makes the list think the
    // viewport is still pinned near the bottom when the user has already moved
    // away. That misclassification re-arms anchor restore and causes jitter.
    const currentCollapseCompensation = getReservationTotalPx(bottomReservationStateRef.current.collapse);
    const fallbackRequiredCollapseCompensation = Math.max(0, shrinkAmount - distanceFromBottom);
    const cumulativeShrinkPx = hasValidCollapseIntent
      ? collapseIntent.cumulativeShrinkPx + shrinkAmount
      : 0;
    const resolvedIntentCompensation = hasValidCollapseIntent
      ? collapseIntent.baseTotalCompensationPx + Math.max(0, cumulativeShrinkPx - collapseIntent.distanceFromBottomBeforeCollapse)
      : 0;
    const nextTotalCompensation = hasValidCollapseIntent
      ? (
        layoutTransitionCountRef.current > 0
          ? Math.max(currentTotalCompensation, resolvedIntentCompensation)
          : resolvedIntentCompensation
      )
      : getReservationTotalPx(bottomReservationStateRef.current.pin) + Math.max(
        currentCollapseCompensation,
        fallbackRequiredCollapseCompensation,
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

    const nextReservationState: BottomReservationState = {
      ...bottomReservationStateRef.current,
      collapse: {
        ...bottomReservationStateRef.current.collapse,
        px: Math.max(0, nextTotalCompensation - getReservationTotalPx(bottomReservationStateRef.current.pin)),
        floorPx: 0,
      },
    };
    updateBottomReservationState(nextReservationState);
    if (nextTotalCompensation > COMPENSATION_EPSILON_PX) {
      const anchorTarget =
        hasValidCollapseIntent
          ? collapseIntent.anchorScrollTop
          : previousScrollTop;

      activateAnchorLock(
        anchorTarget,
        layoutTransitionCountRef.current > 0 ? 'transition-shrink' : 'instant-shrink'
      );
      applyFooterCompensationNow(nextReservationState);
      restoreAnchorLockNow('measure-shrink');
      if (layoutTransitionCountRef.current === 0) {
        pendingCollapseIntentRef.current = {
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
    }

    previousScrollTopRef.current = currentScrollTop;
    recordScrollerGeometry(scroller);
  }, [
    activateAnchorLock,
    applyFooterCompensationNow,
    consumeBottomCompensation,
    getTotalBottomCompensationPx,
    recordScrollerGeometry,
    restoreAnchorLockNow,
    snapshotMeasuredContentHeight,
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
  const hasPendingHistoryCompletion = activeSession?.sessionId
    ? flowChatStore.hasPendingSessionHistoryCompletion(activeSession.sessionId)
    : false;
  const hasPartialHistoryInitialViewport =
    activeSession?.historyState === 'ready' &&
    activeSession.contextRestoreState === 'pending' &&
    (activeSession.dialogTurns.length ?? 0) <= PARTIAL_HISTORY_INITIAL_TAIL_TURN_BUDGET;
  const useInitialHistoryRenderBudget = hasPendingHistoryCompletion || hasPartialHistoryInitialViewport;
  const useStaticInitialHistoryList = useInitialHistoryRenderBudget;
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

  const findVirtualItemIndexByStableKey = useCallback((stableKey: string | null) => {
    if (!stableKey) {
      return -1;
    }

    return virtualItems.findIndex(item => getVirtualItemStableKey(item) === stableKey);
  }, [virtualItems]);

  const captureVisibleVirtualItemAnchor = useCallback(() => {
    const scroller = scrollerElementRef.current;
    if (!scroller) {
      return { anchorKey: null, anchorOffsetTopPx: 0 };
    }

    const scrollerRect = scroller.getBoundingClientRect();
    const nodes = Array.from(
      scroller.querySelectorAll<HTMLElement>('.virtual-item-wrapper[data-virtual-index]'),
    );
    const anchorNode = nodes.find(node => {
      const rect = node.getBoundingClientRect();
      return rect.bottom > scrollerRect.top && rect.top < scrollerRect.bottom;
    });
    const anchorIndex = Number(anchorNode?.dataset.virtualIndex);
    const anchorItem = Number.isInteger(anchorIndex) ? virtualItems[anchorIndex] : undefined;
    if (!anchorNode || !anchorItem) {
      return { anchorKey: null, anchorOffsetTopPx: 0 };
    }

    return {
      anchorKey: getVirtualItemStableKey(anchorItem),
      anchorOffsetTopPx: anchorNode.getBoundingClientRect().top - scrollerRect.top,
    };
  }, [virtualItems]);

  const isHistoryProjectionHandoffAnchorReady = useCallback((snapshot: HistoryProjectionHandoffSnapshot) => {
    const scroller = scrollerElementRef.current;
    const anchorIndex = findVirtualItemIndexByStableKey(snapshot.anchorKey);
    if (!scroller || anchorIndex < 0) {
      return false;
    }

    const anchorElement = getRenderedVirtualItemElement(anchorIndex);
    if (!anchorElement) {
      return false;
    }

    const scrollerRect = scroller.getBoundingClientRect();
    const anchorRect = anchorElement.getBoundingClientRect();
    const visible = anchorRect.bottom > scrollerRect.top && anchorRect.top < scrollerRect.bottom;
    const offsetDelta = Math.abs((anchorRect.top - scrollerRect.top) - snapshot.anchorOffsetTopPx);
    return visible && offsetDelta <= 8;
  }, [findVirtualItemIndexByStableKey, getRenderedVirtualItemElement]);

  const isHistoryProjectionHandoffTargetReady = useCallback((snapshot: HistoryProjectionHandoffSnapshot) => {
    if (snapshot.mode === 'scroll-position' && snapshot.anchorKey) {
      return isHistoryProjectionHandoffAnchorReady(snapshot);
    }

    if (snapshot.targetTurnId) {
      return isTurnTextRenderedInViewportOutsideHandoff(snapshot.targetTurnId);
    }

    return hasVisibleRenderedVirtualItem();
  }, [
    hasVisibleRenderedVirtualItem,
    isHistoryProjectionHandoffAnchorReady,
    isTurnTextRenderedInViewportOutsideHandoff,
  ]);

  const clearHistoryProjectionHandoff = useCallback((reason: string) => {
    if (historyProjectionHandoffReleaseFrameRef.current !== null) {
      cancelAnimationFrame(historyProjectionHandoffReleaseFrameRef.current);
      historyProjectionHandoffReleaseFrameRef.current = null;
    }

    const snapshot = historyProjectionHandoffRef.current;
    pendingHistoryProjectionHandoffRef.current = null;
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

  const captureHistoryProjectionHandoffSnapshot = useCallback((reason: string) => {
    const sessionId = activeSession?.sessionId;
    const scroller = scrollerElementRef.current;
    if (
      !sessionId ||
      activeSession?.isPartial !== true ||
      !scroller ||
      virtualItems.length === 0
    ) {
      return;
    }

    const { anchorKey, anchorOffsetTopPx } = captureVisibleVirtualItemAnchor();
    const snapshot: HistoryProjectionHandoffSnapshot = {
      sessionId,
      reason,
      createdAtMs: performance.now(),
      items: virtualItems,
      mode: 'scroll-position',
      targetTurnId: latestTurnId,
      anchorKey,
      anchorOffsetTopPx,
      scrollTop: scroller.scrollTop,
      scrollHeight: scroller.scrollHeight,
      clientHeight: scroller.clientHeight,
      footerHeightPx: getFooterHeightPx(getTotalBottomCompensationPx()),
    };

    pendingHistoryProjectionHandoffRef.current = snapshot;
    historyProjectionHandoffRef.current = snapshot;
    setHistoryProjectionHandoff(snapshot);
    startupTrace.markPhase('flowchat_history_projection_handoff_captured', {
      sessionId,
      reason,
      anchorKey,
      anchorOffsetTopPx: Math.round(anchorOffsetTopPx),
      itemCount: snapshot.items.length,
      scrollTop: Math.round(snapshot.scrollTop),
      scrollHeight: Math.round(snapshot.scrollHeight),
      clientHeight: Math.round(snapshot.clientHeight),
    });
    scheduleHistoryProjectionHandoffRelease(2);
  }, [
    activeSession?.isPartial,
    activeSession?.sessionId,
    captureVisibleVirtualItemAnchor,
    getFooterHeightPx,
    getTotalBottomCompensationPx,
    latestTurnId,
    scheduleHistoryProjectionHandoffRelease,
    virtualItems,
  ]);

  const buildPinReservation = useCallback((
    turnId: string,
    pinMode: FlowChatPinTurnToTopMode,
    requiredTailSpacePx: number,
    currentPinReservation: PinBottomReservation = bottomReservationStateRef.current.pin,
  ): PinBottomReservation => {
    const resolvedRequiredTailSpacePx = sanitizeReservationPx(requiredTailSpacePx);
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
    const preservedPx = shouldPreserveCurrentPx ? currentPinReservation.px : 0;
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
    const missingTailSpace = Math.max(0, desiredScrollTop - rawMaxScrollTop);

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
    const hasActiveCollapseTransition = (
      layoutTransitionCountRef.current > 0 &&
      collapseIntent.active &&
      collapseIntent.expiresAtMs >= performance.now()
    );
    // During a collapse animation, let collapse compensation own the footer space.
    // Recomputing sticky pin floor from intermediate DOM heights causes the two
    // reservations to fight each other and reintroduces visible vertical jitter.
    if (hasActiveCollapseTransition) {
      return false;
    }

    const resolvedMetrics = resolveTurnPinMetrics(
      pinReservation.targetTurnId,
      pinReservation.px,
    );
    if (!resolvedMetrics) {
      return false;
    }

    const requiredFloorPx = sanitizeReservationPx(resolvedMetrics.missingTailSpace);
    const hadOnlyFloor = pinReservation.px <= pinReservation.floorPx + COMPENSATION_EPSILON_PX;
    const nextPinPx = hadOnlyFloor
      ? requiredFloorPx
      : Math.max(requiredFloorPx, pinReservation.px);
    const nextPinReservation: PinBottomReservation = {
      ...pinReservation,
      px: nextPinPx,
      floorPx: requiredFloorPx,
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

  const tryResolvePendingTurnPin = useCallback((request: PendingTurnPinState) => {
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
      const fallbackBehavior: ScrollBehavior = request.pinMode === 'sticky-latest'
        ? 'auto'
        : targetItem.index === 0
          ? 'auto'
        : request.attempts === 0 && request.behavior === 'smooth'
          ? 'smooth'
          : 'auto';
      const maxScrollTop = Math.max(0, scroller.scrollHeight - scroller.clientHeight);
      const provisionalPinPx = request.pinMode === 'sticky-latest'
        ? Math.max(maxScrollTop, currentPinReservation.px)
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

  const handleScrollerRef = useCallback((el: HTMLElement | Window | null) => {
    if (el && el instanceof HTMLElement) {
      scrollerElementRef.current = el;
      setScrollerElement(el);
      return;
    }

    scrollerElementRef.current = null;
    setScrollerElement(null);
  }, []);

  const requestFullHistoryProjectionForUserIntent = useCallback((reason: string) => {
    const sessionId = activeSession?.sessionId;
    if (
      !sessionId ||
      activeSession.historyState !== 'ready' ||
      activeSession.isPartial !== true
    ) {
      return;
    }

    const hasFullHistoryWork =
      flowChatStore.hasPendingSessionHistoryCompletion(sessionId) ||
      flowChatStore.hasDeferredSessionHistoryProjection(sessionId);
    if (!hasFullHistoryWork) {
      return;
    }

    captureHistoryProjectionHandoffSnapshot(reason);
    flowChatStore.requestSessionFullHistoryProjection(sessionId, reason);
  }, [
    activeSession?.historyState,
    activeSession?.isPartial,
    activeSession?.sessionId,
    captureHistoryProjectionHandoffSnapshot,
  ]);

  const shouldRequestFullHistoryProjectionForUserIntent = useCallback((options?: { force?: boolean }) => {
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

  const scheduleFullHistoryProjectionForUserIntent = useCallback((reason: string, options?: { force?: boolean }) => {
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

        if (!shouldRequestFullHistoryProjectionForUserIntent(options)) {
          return;
        }

        requestFullHistoryProjectionForUserIntent(pendingReason);
      });
    });
  }, [
    activeSession?.sessionId,
    requestFullHistoryProjectionForUserIntent,
    shouldRequestFullHistoryProjectionForUserIntent,
  ]);

  useLayoutEffect(() => {
    const snapshot = pendingHistoryProjectionHandoffRef.current;
    if (
      !snapshot ||
      activeSession?.sessionId !== snapshot.sessionId ||
      activeSession?.isPartial === true ||
      virtualItems.length <= snapshot.items.length
    ) {
      return;
    }

    pendingHistoryProjectionHandoffRef.current = null;
    historyProjectionHandoffRef.current = snapshot;
    setHistoryProjectionHandoff(snapshot);
    startupTrace.markPhase('flowchat_history_projection_handoff_activated', {
      sessionId: snapshot.sessionId,
      reason: snapshot.reason,
      previousItemCount: snapshot.items.length,
      nextItemCount: virtualItems.length,
    });
    scheduleHistoryProjectionHandoffRelease(2);
  }, [
    activeSession?.isPartial,
    activeSession?.sessionId,
    scheduleHistoryProjectionHandoffRelease,
    virtualItems.length,
  ]);

  const shouldSuspendAutoFollow = useCallback(() => {
    const collapseIntent = pendingCollapseIntentRef.current;
    return (
      layoutTransitionCountRef.current > 0 ||
      (collapseIntent.active && collapseIntent.expiresAtMs >= performance.now())
    );
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
    clearTurnPinRequest();
    cancelLatestEndAnchorStabilization();
    cancelStaticInitialHistoryBottomGuard();
    anchorLockRef.current = {
      active: false,
      targetScrollTop: 0,
      reason: null,
      lockUntilMs: 0,
    };
    pendingCollapseIntentRef.current = {
      active: false,
      anchorScrollTop: 0,
      toolId: null,
      toolName: null,
      expiresAtMs: 0,
      distanceFromBottomBeforeCollapse: 0,
      baseTotalCompensationPx: 0,
      cumulativeShrinkPx: 0,
    };
    previousScrollerGeometryRef.current = null;
    const currentSessionId = activeSession?.sessionId ?? null;
    const activeHandoff = historyProjectionHandoffRef.current;
    if (!activeHandoff || activeHandoff.sessionId !== currentSessionId) {
      clearHistoryProjectionHandoff('session-reset');
    }
    const pendingHandoff = pendingHistoryProjectionHandoffRef.current;
    if (pendingHandoff && pendingHandoff.sessionId !== currentSessionId) {
      pendingHistoryProjectionHandoffRef.current = null;
    }
    resetBottomReservations();
  }, [
    activeSession?.sessionId,
    cancelLatestEndAnchorStabilization,
    cancelStaticInitialHistoryBottomGuard,
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
      resetBottomReservations();
    }
  }, [virtualItems.length, cancelLatestEndAnchorStabilization, clearTurnPinRequest, resetBottomReservations]);

  useEffect(() => {
    return () => {
      cancelLatestEndAnchorStabilization();
      cancelStaticInitialHistoryBottomGuard();
      if (historyProjectionHandoffReleaseFrameRef.current !== null) {
        cancelAnimationFrame(historyProjectionHandoffReleaseFrameRef.current);
        historyProjectionHandoffReleaseFrameRef.current = null;
      }
    };
  }, [cancelLatestEndAnchorStabilization, cancelStaticInitialHistoryBottomGuard]);

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

    const isLayoutTransitionProperty = (propertyName: string) => (
      propertyName === 'grid-template-rows' ||
      propertyName === 'height' ||
      propertyName === 'max-height'
    );

    const handleTransitionRun = (event: TransitionEvent) => {
      if (!isLayoutTransitionProperty(event.propertyName)) return;
      layoutTransitionCountRef.current += 1;
    };

    const handleTransitionFinish = (event: TransitionEvent) => {
      if (!isLayoutTransitionProperty(event.propertyName)) return;
      layoutTransitionCountRef.current = Math.max(0, layoutTransitionCountRef.current - 1);
      resolveLatestEndAnchorStabilizationRef.current?.('transition-finish');
      scheduleHeightMeasure(2);
      scheduleVisibleTurnMeasure(2);
      schedulePinReservationReconcile(2);
      scheduleTransientTurnPinStabilization(2);
      scheduleHistoryProjectionHandoffRelease(1);
      if (layoutTransitionCountRef.current === 0 && pendingCollapseIntentRef.current.active) {
        pendingCollapseIntentRef.current = {
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
      if (layoutTransitionCountRef.current === 0 && deferredFollowReasonRef.current && !shouldSuspendAutoFollow()) {
        const deferredReason = deferredFollowReasonRef.current;
        deferredFollowReasonRef.current = null;
        followOutputControllerRef.current.scheduleFollowToLatest(`${deferredReason}-after-transition`);
      }
    };
    scrollerElement.addEventListener('transitionrun', handleTransitionRun, true);
    scrollerElement.addEventListener('transitionend', handleTransitionFinish, true);
    scrollerElement.addEventListener('transitioncancel', handleTransitionFinish, true);

    const handleScroll = () => {
      const now = performance.now();
      if (anchorLockRef.current.active && now > anchorLockRef.current.lockUntilMs && layoutTransitionCountRef.current === 0) {
        releaseAnchorLock('expired-before-scroll');
      }

      // Reactive shrink-clamp restore: in follow + streaming mode, an upward
      // jump in scrollTop that we did NOT request from JS and that is NOT
      // attributable to a user gesture is the browser auto-clamping scrollTop
      // because `scrollHeight` shrunk below `scrollTop + clientHeight`
      // (typical cause: an unsignaled item shrink from Virtuoso re-measure
      // or a tool result finalizing). With `overflow-anchor: none` we cannot
      // ask the browser to keep the visual anchor for us, so we extend the
      // bottom collapse reservation by the clamp amount and restore
      // `scrollTop` to its pre-clamp value. The widened footer prevents the
      // browser from re-clamping immediately; subsequent streaming-token
      // growth drains the reservation via the grow branch of
      // `measureHeightChange`. This is the only place that protects against
      // unsignaled shrinks that do not arrive with a `collapse-intent` event.
      const intentCheckScrollTop = scrollerElement.scrollTop;
      const intentCheckPreviousScrollTop = previousScrollTopRef.current;
      const intentCheckScrollDelta = intentCheckScrollTop - intentCheckPreviousScrollTop;
      const hasRecentUserUpwardIntent = now <= userInitiatedUpwardScrollUntilMsRef.current;
      if (
        intentCheckScrollDelta < -COMPENSATION_EPSILON_PX &&
        isFollowingOutputRef.current &&
        isStreamingOutputRef.current &&
        !hasRecentUserUpwardIntent &&
        !anchorLockRef.current.active &&
        layoutTransitionCountRef.current === 0
      ) {
        // Cap the clamp amount to what the footer actually needs.  Without
        // this, repeated scroll-clamp events during CSS transitions can
        // ratchet `collapse.px` upward without bound because the
        // consumption path is blocked while transitions are active.
        const rawClampAmount = -intentCheckScrollDelta;
        const maxClampAmount = Math.max(0,
          scrollerElement.scrollHeight - scrollerElement.clientHeight - scrollerElement.scrollTop,
        );
        const clampAmount = Math.min(rawClampAmount, maxClampAmount);
        const baseState = bottomReservationStateRef.current;
        const nextReservationState: BottomReservationState = {
          ...baseState,
          collapse: {
            ...baseState.collapse,
            px: baseState.collapse.px + clampAmount,
            floorPx: baseState.collapse.floorPx,
          },
        };
        updateBottomReservationState(nextReservationState);
        applyFooterCompensationNow(nextReservationState);
        scrollerElement.scrollTop = intentCheckPreviousScrollTop;
        previousScrollTopRef.current = intentCheckPreviousScrollTop;
        previousMeasuredHeightRef.current = snapshotMeasuredContentHeight(
          scrollerElement,
          nextReservationState,
        );
        recordScrollerGeometry(scrollerElement);
        return;
      }

      const currentTotalCompensation = getTotalBottomCompensationPx();
      if (
        currentTotalCompensation > COMPENSATION_EPSILON_PX &&
        !anchorLockRef.current.active &&
        layoutTransitionCountRef.current === 0
      ) {
        const nextScrollTop = scrollerElement.scrollTop;
        const scrollDelta = nextScrollTop - previousScrollTopRef.current;
        if (scrollDelta > COMPENSATION_EPSILON_PX) {
          const nextCompensationState = consumeBottomCompensation(scrollDelta);
          applyFooterCompensationNow(nextCompensationState);
          previousMeasuredHeightRef.current = snapshotMeasuredContentHeight(
            scrollerElement,
            nextCompensationState,
          );
        }
      }

      if (getTotalBottomCompensationPx() > COMPENSATION_EPSILON_PX) {
        const nextScrollTop = scrollerElement.scrollTop;
        const maxScrollTop = Math.max(0, scrollerElement.scrollHeight - scrollerElement.clientHeight);
        if (anchorLockRef.current.active && performance.now() <= anchorLockRef.current.lockUntilMs) {
          const targetScrollTop = Math.min(anchorLockRef.current.targetScrollTop, maxScrollTop);
          const restoreDelta = targetScrollTop - nextScrollTop;
          if (Math.abs(restoreDelta) > ANCHOR_LOCK_MIN_DEVIATION_PX) {
            scrollerElement.scrollTop = targetScrollTop;
            previousScrollTopRef.current = targetScrollTop;
            recordScrollerGeometry(scrollerElement);
            return;
          }
        }
      }
      previousScrollTopRef.current = scrollerElement.scrollTop;
      recordScrollerGeometryIfLayoutStable(scrollerElement);
      scheduleVisibleTurnMeasure();
      followOutputControllerRef.current.handleScroll();
      scheduleFullHistoryProjectionForUserIntent('scroll-near-partial-history-boundary');

      if (anchorLockRef.current.active && performance.now() > anchorLockRef.current.lockUntilMs && layoutTransitionCountRef.current === 0) {
        releaseAnchorLock('expired-after-scroll');
      }
    };
    scrollerElement.addEventListener('scroll', handleScroll, { passive: true });

    const handleWheel = (event: WheelEvent) => {
      if (event.deltaY !== 0) {
        clearTurnPinRequest();
        cancelLatestEndAnchorStabilization();
      }

      if (event.deltaY < 0) {
        userInitiatedUpwardScrollUntilMsRef.current =
          performance.now() + USER_UPWARD_SCROLL_INTENT_WINDOW_MS;
        scheduleFullHistoryProjectionForUserIntent('wheel-up');
        followOutputControllerRef.current.handleUserScrollIntent();
        releaseAnchorLock('wheel-up');
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
        clearTurnPinRequest();
        cancelLatestEndAnchorStabilization();
      }

      if (currentY - startY > TOUCH_SCROLL_INTENT_EXIT_THRESHOLD_PX) {
        touchScrollIntentStartYRef.current = currentY;
        userInitiatedUpwardScrollUntilMsRef.current =
          performance.now() + USER_UPWARD_SCROLL_INTENT_WINDOW_MS;
        scheduleFullHistoryProjectionForUserIntent('touch-scroll-up');
        followOutputControllerRef.current.handleUserScrollIntent();
        releaseAnchorLock('touch-scroll-up');
      }
    };

    const resetTouchScrollIntent = () => {
      touchScrollIntentStartYRef.current = null;
    };

    const handleKeyDown = (event: KeyboardEvent) => {
      if (!isUpwardScrollIntentKey(event) || isEditableElement(event.target)) {
        return;
      }

      clearTurnPinRequest();
      cancelLatestEndAnchorStabilization();
      userInitiatedUpwardScrollUntilMsRef.current =
        performance.now() + USER_UPWARD_SCROLL_INTENT_WINDOW_MS;
      scheduleFullHistoryProjectionForUserIntent('keyboard-scroll-up', { force: event.key === 'Home' });
      followOutputControllerRef.current.handleUserScrollIntent();
      releaseAnchorLock('keyboard-scroll-up');
    };

    const handlePointerDown = (event: PointerEvent) => {
      if (event.pointerType === 'touch' || event.button !== 0) {
        return;
      }

      if (!isPointerOnScrollbarGutter(scrollerElement, event.clientX, event.clientY)) {
        return;
      }

      scrollbarPointerInteractionActiveRef.current = true;
      clearTurnPinRequest();
      cancelLatestEndAnchorStabilization();
      userInitiatedUpwardScrollUntilMsRef.current =
        performance.now() + USER_UPWARD_SCROLL_INTENT_WINDOW_MS;
      scheduleFullHistoryProjectionForUserIntent('scrollbar-pointer-down');
      followOutputControllerRef.current.handleUserScrollIntent();
      releaseAnchorLock('scrollbar-pointer-down');
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
      cancelLatestEndAnchorStabilization();
      userInitiatedUpwardScrollUntilMsRef.current =
        performance.now() + USER_UPWARD_SCROLL_INTENT_WINDOW_MS;
      scheduleFullHistoryProjectionForUserIntent('scrollbar-pointer-move');
      followOutputControllerRef.current.handleUserScrollIntent();
      releaseAnchorLock('scrollbar-pointer-move');
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
        filePath?: string | null;
        reason?: string | null;
      }>).detail;
      // In follow-output mode, the user wants the viewport pinned to the
      // latest streaming token. Reserving footer space + locking an upper
      // anchor would freeze the viewport on older content during the
      // collapse animation, producing the "stutter then jump" effect. Skip
      // the protection path entirely and let the continuous follow loop
      // absorb the shrink frame-by-frame.
      // Note: in follow-output mode we still run the full collapse pre-compensation
      // path. Pinning the upper visual anchor during the collapse animation keeps
      // the conversation visually stable; the continuous follow loop is gated by
      // `shouldSuspendAutoFollow` while the layout transition is in progress, and
      // resumes bottom-tracking via the deferred-follow path after the transition
      // ends and the collapse reservation is consumed.
      const baseTotalCompensationPx = getTotalBottomCompensationPx();
      const distanceFromBottom = Math.max(
        0,
        scrollerElement.scrollHeight - scrollerElement.clientHeight - scrollerElement.scrollTop
      );
      const effectiveDistanceFromBottom = Math.max(0, distanceFromBottom - baseTotalCompensationPx);
      const estimatedShrink = Math.max(0, detail?.cardHeight ?? 0);
      const provisionalTotalCompensationPx = Math.max(
        0,
        baseTotalCompensationPx + Math.max(0, estimatedShrink - effectiveDistanceFromBottom)
      );
      pendingCollapseIntentRef.current = {
        active: true,
        anchorScrollTop: scrollerElement.scrollTop,
        toolId: detail?.toolId ?? null,
        toolName: detail?.toolName ?? null,
        expiresAtMs: performance.now() + 1000,
        distanceFromBottomBeforeCollapse: effectiveDistanceFromBottom,
        baseTotalCompensationPx,
        cumulativeShrinkPx: 0,
      };
      if (provisionalTotalCompensationPx - baseTotalCompensationPx > COMPENSATION_EPSILON_PX) {
        const nextReservationState: BottomReservationState = {
          ...bottomReservationStateRef.current,
          collapse: {
            ...bottomReservationStateRef.current.collapse,
            px: Math.max(0, provisionalTotalCompensationPx - getReservationTotalPx(bottomReservationStateRef.current.pin)),
            floorPx: 0,
          },
        };
        updateBottomReservationState(nextReservationState);
        applyFooterCompensationNow(nextReservationState);
        activateAnchorLock(scrollerElement.scrollTop, 'instant-shrink');
      }

      scheduleVisibleTurnMeasure(2);
      schedulePinReservationReconcile(2);
    };

    window.addEventListener('tool-card-toggle', handleToolCardToggle);
    window.addEventListener('flowchat:tool-card-collapse-intent', handleToolCardCollapseIntent as EventListener);
    scheduleVisibleTurnMeasure(2);

    return () => {
      scrollerElement.removeEventListener('transitionrun', handleTransitionRun, true);
      scrollerElement.removeEventListener('transitionend', handleTransitionFinish, true);
      scrollerElement.removeEventListener('transitioncancel', handleTransitionFinish, true);
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
    activateAnchorLock,
    applyFooterCompensationNow,
    cancelLatestEndAnchorStabilization,
    cancelStaticInitialHistoryBottomGuard,
    consumeBottomCompensation,
    clearTurnPinRequest,
    getTotalBottomCompensationPx,
    latestTurnId,
    pendingTurnPin?.pinMode,
    pendingTurnPin?.turnId,
    recordScrollerGeometry,
    recordScrollerGeometryIfLayoutStable,
    releaseAnchorLock,
    scheduleHeightMeasure,
    scheduleFollowToLatestWithViewportState,
    scheduleFullHistoryProjectionForUserIntent,
    scheduleHistoryProjectionHandoffRelease,
    schedulePinReservationReconcile,
    scheduleTransientTurnPinStabilization,
    scheduleVisibleTurnMeasure,
    scrollerElement,
    shouldSuspendAutoFollow,
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

    if (performance.now() > pendingTurnPin.expiresAtMs) {
      clearTurnPinRequest();
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
        if (!prev || prev.turnId !== pendingTurnPin.turnId) {
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
    clearTurnPinRequest,
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

    releaseAnchorLock('user-navigation');
    clearTurnPinRequest();
    pendingCollapseIntentRef.current = {
      active: false,
      anchorScrollTop: 0,
      toolId: null,
      toolName: null,
      expiresAtMs: 0,
      distanceFromBottomBeforeCollapse: 0,
      baseTotalCompensationPx: 0,
      cumulativeShrinkPx: 0,
    };

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
    recordScrollerGeometry,
    releaseAnchorLock,
    snapshotMeasuredContentHeight,
    updateBottomReservationState,
  ]);

  const clearPinReservationForUserNavigation = useCallback(() => {
    const currentState = bottomReservationStateRef.current;
    const scroller = scrollerElementRef.current;
    const hasActivePin = (
      currentState.pin.px > COMPENSATION_EPSILON_PX ||
      currentState.pin.floorPx > COMPENSATION_EPSILON_PX ||
      currentState.pin.targetTurnId !== null ||
      currentState.pin.mode !== 'transient'
    );

    releaseAnchorLock('user-navigation');
    clearTurnPinRequest();

    if (!hasActivePin) {
      return;
    }

    const nextReservationState: BottomReservationState = {
      ...currentState,
      pin: {
        kind: 'pin',
        px: 0,
        floorPx: 0,
        mode: 'transient',
        targetTurnId: null,
      },
    };
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
    recordScrollerGeometry,
    releaseAnchorLock,
    snapshotMeasuredContentHeight,
    updateBottomReservationState,
  ]);

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
    if (
      historyProjectionHandoff?.mode === 'scroll-position' &&
      historyProjectionHandoff.sessionId === activeSessionId
    ) {
      const anchorIndex = findVirtualItemIndexByStableKey(historyProjectionHandoff.anchorKey);
      if (anchorIndex >= 0) {
        return {
          index: toVirtuosoIndex(anchorIndex),
          align: 'start' as const,
        };
      }
    }

    if (isStreamingOutput) {
      return toVirtuosoIndex(latestUserMessageIndex);
    }

    return {
      index: toVirtuosoIndex(Math.max(0, virtualItems.length - 1)),
      align: 'end' as const,
    };
  }, [
    activeSessionId,
    findVirtualItemIndexByStableKey,
    historyProjectionHandoff?.anchorKey,
    historyProjectionHandoff?.mode,
    historyProjectionHandoff?.sessionId,
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

    const pinReservation = bottomReservationStateRef.current.pin;
    if (
      pinReservation.mode !== 'sticky-latest' ||
      !pinReservation.targetTurnId ||
      getReservationTotalPx(pinReservation) <= COMPENSATION_EPSILON_PX
    ) {
      return;
    }

    clearPinReservationForUserNavigation();
    requestAnimationFrame(() => {
      const scroller = scrollerElementRef.current;
      if (!scroller) {
        return;
      }
      scroller.scrollTo({
        top: Math.max(0, scroller.scrollHeight - scroller.clientHeight),
        behavior: 'auto',
      });
    });
  }, [clearPinReservationForUserNavigation, isStreamingOutput]);

  const scrollToLatestEndPositionInternal = useCallback((behavior: 'auto' | 'smooth') => {
    const scroller = scrollerElementRef.current;
    if (!scroller) return;

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
      }
      return;
    }

    clearAllBottomReservationsForUserNavigation();
    scroller.scrollTo({
      top: Math.max(0, scroller.scrollHeight - scroller.clientHeight),
      behavior,
    });
  }, [clearAllBottomReservationsForUserNavigation, clearPinReservationForUserNavigation, getTotalBottomCompensationPx]);

  const requestTurnPinToTop = useCallback((turnId: string, options?: { behavior?: ScrollBehavior; pinMode?: FlowChatPinTurnToTopMode }) => {
    const requestedPinMode = options?.pinMode ?? 'transient';
    const requestedBehavior = options?.behavior ?? 'auto';
    const targetTurn = findDialogTurn(activeSession?.dialogTurns, turnId);
    if (requestedPinMode === 'sticky-latest' && !shouldUseStickyLatestPin(targetTurn)) {
      return false;
    }
    const targetItem = userMessageItems.find(({ item }) => item.turnId === turnId);
    if (!targetItem || !virtuosoRef.current) {
      return false;
    }

    if (targetItem.index === 0 && requestedPinMode === 'transient') {
      // The first turn has a deterministic destination, so bypass the deferred
      // pin pipeline and snap to the true top immediately.
      clearTurnPinRequest();
      virtuosoRef.current.scrollTo({ top: 0, behavior: 'auto' });

      return true;
    }

    const request: PendingTurnPinState = {
      turnId,
      behavior: requestedBehavior,
      pinMode: requestedPinMode,
      expiresAtMs: performance.now() + 1500,
      attempts: 0,
    };

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
      return true;
    }

    setPendingTurnPin(request);
    return true;
  }, [
    activeSession?.dialogTurns,
    activateTransientTurnPinStabilization,
    clearTurnPinRequest,
    scheduleTransientTurnPinStabilization,
    scheduleVisibleTurnMeasure,
    tryResolvePendingTurnPin,
    userMessageItems,
  ]);

  const performAutoFollowSync = useCallback(() => {
    scrollToLatestEndPositionInternal('auto');
  }, [scrollToLatestEndPositionInternal]);

  const {
    isFollowingOutput,
    enterFollowOutput,
    exitFollowOutput,
    armFollowOutputForNewTurn,
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

    hasPrimedMountedStreamingTurnFollowRef.current = true;
    if (!latestTurnId || !isStreamingOutput) {
      return;
    }

    latestTurnAutoFollowStateRef.current = {
      turnId: latestTurnId,
      sawPositiveFloor: false,
    };
    armFollowOutputForNewTurn();
  }, [
    activeSession?.sessionId,
    armFollowOutputForNewTurn,
    isStreamingOutput,
    latestTurnId,
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

      // When switching to a streaming session, arm follow output so the
      // viewport tracks new content as it arrives.  Without this, the
      // Virtuoso stays at initialTopMostItemIndex (the user message),
      // and if that position is not yet rendered or measured the
      // viewport may show blank space.
      if (isStreamingOutput) {
        // Reset the one-shot streaming prime flag so arm-follow can
        // fire again for the new session's latest turn.
        hasPrimedMountedStreamingTurnFollowRef.current = false;
        armFollowOutputForNewTurn();
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
    toVirtuosoIndex,
    virtualItems.length,
  ]);

  useEffect(() => {
    const trackingState = latestTurnAutoFollowStateRef.current;
    if (
      !latestTurnId ||
      trackingState.turnId !== latestTurnId ||
      isFollowingOutput ||
      !isStreamingOutput
    ) {
      return;
    }

    const hasPendingLatestStickyPin = (
      pendingTurnPin?.turnId === latestTurnId &&
      pendingTurnPin.pinMode === 'sticky-latest'
    );
    if (hasPendingLatestStickyPin) {
      return;
    }

    if (
      bottomReservationState.pin.mode !== 'sticky-latest' ||
      bottomReservationState.pin.targetTurnId !== latestTurnId
    ) {
      return;
    }

    if (bottomReservationState.pin.floorPx > COMPENSATION_EPSILON_PX) {
      trackingState.sawPositiveFloor = true;
      return;
    }

    if (activateArmedFollowOutput()) {
      latestTurnAutoFollowStateRef.current = {
        turnId: null,
        sawPositiveFloor: false,
      };
    }
  }, [
    activateArmedFollowOutput,
    bottomReservationState.pin.floorPx,
    bottomReservationState.pin.mode,
    bottomReservationState.pin.targetTurnId,
    isFollowingOutput,
    isStreamingOutput,
    latestTurnId,
    pendingTurnPin?.pinMode,
    pendingTurnPin?.turnId,
  ]);

  followOutputControllerRef.current = {
    handleUserScrollIntent,
    handleScroll: handleFollowOutputScroll,
    scheduleFollowToLatest,
  };
  isFollowingOutputRef.current = isFollowingOutput;
  isStreamingOutputRef.current = isStreamingOutput;

  const scrollToTurn = useCallback((turnIndex: number) => {
    if (!virtuosoRef.current) return;
    if (turnIndex < 1 || turnIndex > userMessageItems.length) return;

    const targetItem = userMessageItems[turnIndex - 1];
    if (!targetItem) return;

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

    exitFollowOutput('scroll-to-index');
    clearPinReservationForUserNavigation();

    if (index === 0) {
      virtuosoRef.current.scrollTo({ top: 0, behavior: 'auto' });
    } else {
      virtuosoRef.current.scrollToIndex({ index: toVirtuosoIndex(index), align: 'center', behavior: 'auto' });
    }
  }, [clearPinReservationForUserNavigation, exitFollowOutput, toVirtuosoIndex, virtualItems.length]);

  const pinTurnToTop = useCallback((turnId: string, options?: { behavior?: ScrollBehavior; pinMode?: FlowChatPinTurnToTopMode }) => {
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
      previousScrollTopRef.current = nextScrollTop;
      previousMeasuredHeightRef.current = snapshotMeasuredContentHeight(scroller);
      recordScrollerGeometry(scroller);
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
    resolveLatestEndAnchorStabilization,
    scheduleVisibleTurnMeasure,
    snapshotMeasuredContentHeight,
    toVirtuosoIndex,
    useStaticInitialHistoryList,
    virtualItems,
  ]);

  const scrollToLatestEndPosition = useCallback(() => {
    enterFollowOutput('jump-to-latest');
  }, [enterFollowOutput]);

  useImperativeHandle(ref, () => ({
    scrollToTurn,
    scrollToIndex,
    scrollToPhysicalBottomAndClearPin,
    scrollToTurnEndAndClearPin,
    isTurnRenderedInViewport,
    isTurnTextRenderedInViewport,
    scrollToLatestEndPosition,
    pinTurnToTop,
  }), [
    isTurnRenderedInViewport,
    isTurnTextRenderedInViewport,
    pinTurnToTop,
    scrollToTurn,
    scrollToIndex,
    scrollToPhysicalBottomAndClearPin,
    scrollToTurnEndAndClearPin,
    scrollToLatestEndPosition,
  ]);

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
    const isTurnProcessing =
      lastDialogTurn?.status === 'processing' ||
      lastDialogTurn?.status === 'finishing' ||
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
    return shouldShowProcessingIndicator({
      isTurnProcessing: lastItemInfo.isTurnProcessing,
      isSessionProcessing: isProcessing,
      processingPhase,
      lastItem: lastItemInfo.lastItem,
      isContentGrowing,
    });
  }, [isProcessing, processingPhase, lastItemInfo, isContentGrowing]);

  const reserveSpaceForIndicator = React.useMemo(() => {
    return shouldReserveProcessingIndicatorSpace({
      isTurnProcessing: lastItemInfo.isTurnProcessing,
      isSessionProcessing: isProcessing,
      processingPhase,
      lastItem: lastItemInfo.lastItem,
      isContentGrowing,
    });
  }, [lastItemInfo.isTurnProcessing, lastItemInfo.lastItem, isProcessing, processingPhase, isContentGrowing]);

  const footerHeightPx = getFooterHeightPx(getTotalBottomCompensationPx(bottomReservationState));
  const initialHistoryRenderWindow = React.useMemo(
    () => useStaticInitialHistoryList
      ? selectInitialHistoryRenderWindow(virtualItems)
      : {
        items: virtualItems,
        startIndex: 0,
        omittedEstimatedHeightPx: 0,
        renderedEstimatedHeightPx: 0,
        totalEstimatedHeightPx: 0,
        isWindowed: false,
      },
    [useStaticInitialHistoryList, virtualItems],
  );
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
  const handleInitialHistoryStaticScroll = useCallback(() => {
    expandInitialHistoryRenderWindowIfNeeded('scroll-near-omitted-history');
  }, [expandInitialHistoryRenderWindowIfNeeded]);
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
      return;
    }

    pendingInitialHistoryExpansionRef.current = null;
    const scroller = scrollerElementRef.current;
    if (!scroller) {
      return;
    }

    const nextScrollTop = mapInitialHistoryExpansionScrollTop({
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

    const scroller = scrollerElementRef.current;
    const budgetStartIndex = Math.max(0, virtualItems.length - SESSION_OPEN_HANDOFF_ITEM_BUDGET);
    const latestStartIndex = Math.max(0, Math.min(latestUserMessageIndex, virtualItems.length - 1));
    const startIndex = Math.min(budgetStartIndex, latestStartIndex);
    const items = virtualItems.slice(startIndex);
    if (items.length === 0) {
      return null;
    }

    const clientHeight = scroller?.clientHeight ?? 0;
    return {
      sessionId: activeSessionId,
      reason: 'session-open',
      createdAtMs: performance.now(),
      items,
      mode: 'bottom-tail',
      targetTurnId: latestTurnId,
      anchorKey: null,
      anchorOffsetTopPx: 0,
      scrollTop: 0,
      scrollHeight: clientHeight,
      clientHeight,
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

    pendingHistoryProjectionHandoffRef.current = null;
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
  const activeHistoryProjectionHandoff = historyProjectionHandoff ?? sessionOpenProjectionHandoff;
  const hasCompactHistoricalProjection = virtualItems.length >= 6 && virtualItems
    .slice(-16)
    .every(item =>
      item.type === 'user-message' ||
      item.type === 'user-steering-message' ||
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
  const virtuosoOverscan = { main: 600, reverse: 600 };
  const virtuosoViewportIncrease = { top: 600, bottom: 600 };
  useLayoutEffect(() => {
    if (!useStaticInitialHistoryList) {
      autoScrolledInitialHistoryRenderKeyRef.current = null;
      cancelStaticInitialHistoryBottomGuard();
      return;
    }

    if (autoScrolledInitialHistoryRenderKeyRef.current === initialHistoryRenderKey) {
      return;
    }

    const scroller = scrollerElementRef.current;
    if (!scroller) {
      return;
    }

    autoScrolledInitialHistoryRenderKeyRef.current = initialHistoryRenderKey;
    const nextScrollTop = Math.max(0, scroller.scrollHeight - scroller.clientHeight);
    scroller.scrollTop = nextScrollTop;
    previousScrollTopRef.current = nextScrollTop;
    previousMeasuredHeightRef.current = snapshotMeasuredContentHeight(scroller);
    recordScrollerGeometry(scroller);
    startStaticInitialHistoryBottomGuard();
    scheduleVisibleTurnMeasure(1);
  }, [
    activeSessionId,
    cancelStaticInitialHistoryBottomGuard,
    initialHistoryRenderKey,
    latestTurnId,
    recordScrollerGeometry,
    scheduleVisibleTurnMeasure,
    snapshotMeasuredContentHeight,
    startStaticInitialHistoryBottomGuard,
    useStaticInitialHistoryList,
  ]);
  // ── Render ────────────────────────────────────────────────────────────
  useLayoutEffect(() => {
    const snapshot = historyProjectionHandoffRef.current;
    if (
      !snapshot ||
      snapshot.mode !== 'scroll-position' ||
      snapshot.sessionId !== activeSessionId ||
      !snapshot.anchorKey
    ) {
      return;
    }

    const scroller = scrollerElementRef.current;
    const anchorIndex = findVirtualItemIndexByStableKey(snapshot.anchorKey);
    if (!scroller || anchorIndex < 0) {
      return;
    }

    const anchorElement = getRenderedVirtualItemElement(anchorIndex);
    if (!anchorElement) {
      return;
    }

    const scrollerRect = scroller.getBoundingClientRect();
    const anchorRect = anchorElement.getBoundingClientRect();
    const offsetDelta = (anchorRect.top - scrollerRect.top) - snapshot.anchorOffsetTopPx;
    if (Math.abs(offsetDelta) <= COMPENSATION_EPSILON_PX) {
      return;
    }

    const nextScrollTop = Math.max(0, scroller.scrollTop + offsetDelta);
    scroller.scrollTop = nextScrollTop;
    previousScrollTopRef.current = nextScrollTop;
    previousMeasuredHeightRef.current = snapshotMeasuredContentHeight(scroller);
    recordScrollerGeometry(scroller);
    scheduleHistoryProjectionHandoffRelease(2);
  }, [
    activeSessionId,
    findVirtualItemIndexByStableKey,
    getRenderedVirtualItemElement,
    historyProjectionHandoff?.anchorKey,
    historyProjectionHandoff?.anchorOffsetTopPx,
    historyProjectionHandoff?.mode,
    historyProjectionHandoff?.sessionId,
    recordScrollerGeometry,
    scheduleHistoryProjectionHandoffRelease,
    snapshotMeasuredContentHeight,
    virtualItems,
  ]);

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
    <div
      className="virtual-message-list"
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
          <ProcessingIndicator visible={showBreathingIndicator} reserveSpace={reserveSpaceForIndicator} />
          <div
            ref={footerElementRef}
            className="message-list-footer"
            style={{
              height: `${footerHeightPx}px`,
              minHeight: `${footerHeightPx}px`,
            }}
          />
        </div>
      ) : (
        <Virtuoso
          key={activeSessionId ?? 'no-active-session'}
          ref={virtuosoRef}
          data={virtualItems}
          computeItemKey={(_, item) => `${activeSessionId ?? 'no-active-session'}:${getVirtualItemStableKey(item)}`}
          itemContent={(index, item) => (
            <VirtualItemRenderer
              item={item}
              index={index - virtuosoFirstItemIndex}
            />
          )}
          followOutput={false}

          alignToBottom={false}
          firstItemIndex={virtuosoFirstItemIndex}
          // New mounts start near the latest user turn to avoid flashing older
          // content before sticky pin logic can finish.
          initialTopMostItemIndex={initialTopMostItemIndex}
          overscan={virtuosoOverscan}

          atBottomThreshold={50}
          atBottomStateChange={handleAtBottomStateChange}

          rangeChanged={handleRangeChanged}

          // Historical sessions often restore into compact user/explore rows.
          // Keep live sessions on the legacy estimate because active assistant
          // output can be much taller while streaming.
          defaultItemHeight={defaultItemHeight}
          heightEstimates={initialHistoryHeightEstimates}

          increaseViewportBy={virtuosoViewportIncrease}

          scrollerRef={handleScrollerRef}

          components={{
            Header: () => <div className="message-list-header" />,
            Footer: () => (
              <>
                <ProcessingIndicator visible={showBreathingIndicator} reserveSpace={reserveSpaceForIndicator} />
                <div
                  ref={footerElementRef}
                  className="message-list-footer"
                  style={{
                    height: `${footerHeightPx}px`,
                    minHeight: `${footerHeightPx}px`,
                  }}
                />
              </>
            ),
          }}
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
            className={[
              'virtual-message-list__projection-handoff-content',
              activeHistoryProjectionHandoff.mode === 'bottom-tail'
                ? 'virtual-message-list__projection-handoff-content--bottom'
                : null,
            ].filter(Boolean).join(' ')}
            style={activeHistoryProjectionHandoff.mode === 'bottom-tail'
              ? undefined
              : {
                minHeight: `${Math.max(activeHistoryProjectionHandoff.scrollHeight, activeHistoryProjectionHandoff.clientHeight)}px`,
                transform: `translate3d(0, -${Math.max(0, activeHistoryProjectionHandoff.scrollTop)}px, 0)`,
              }}
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
          pinTurnToTop(turnId, { behavior: 'smooth' });
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

export const VirtualMessageList = forwardRef<VirtualMessageListRef>((_, ref) => {
  const activeSession = useActiveSession();
  const sessionKey = activeSession?.sessionId ?? 'no-active-session';

  return <VirtualMessageListSession key={sessionKey} ref={ref} />;
});

VirtualMessageList.displayName = 'VirtualMessageList';
