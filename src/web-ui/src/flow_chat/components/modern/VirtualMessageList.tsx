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

import React, { useRef, useState, useCallback, useEffect, forwardRef, useImperativeHandle } from 'react';
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
import { useVirtualItems, useActiveSession, useModernFlowChatStore, type VisibleTurnInfo } from '../../store/modernFlowChatStore';
import { useChatInputState } from '../../store/chatInputStateStore';
import { computeFlowChatInputStackFooterPx } from '../../utils/flowChatScrollLayout';
import './VirtualMessageList.scss';

const COMPENSATION_EPSILON_PX = 0.5;
const ANCHOR_LOCK_MIN_DEVIATION_PX = 0.5;
const ANCHOR_LOCK_DURATION_MS = 450;
const PINNED_TURN_VIEWPORT_OFFSET_PX = 57; // Keep in sync with `.message-list-header`.
const TOUCH_SCROLL_INTENT_EXIT_THRESHOLD_PX = 6;

// Read `FLOWCHAT_SCROLL_STABILITY.md` before changing collapse compensation logic.

/**
 * Methods exposed by VirtualMessageList.
 */
export interface VirtualMessageListRef {
  scrollToTurn: (turnIndex: number) => void;
  scrollToIndex: (index: number) => void;
  // Clears pin reservation first, then scrolls to the physical bottom.
  scrollToPhysicalBottomAndClearPin: () => void;
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

function getReservationTotalPx(reservation: BottomReservationBase): number {
  return Math.max(0, reservation.px);
}

function getReservationConsumablePx(reservation: BottomReservationBase): number {
  return Math.max(0, reservation.px - reservation.floorPx);
}

export const VirtualMessageList = forwardRef<VirtualMessageListRef>((_, ref) => {
  const virtuosoRef = useRef<VirtuosoHandle>(null);
  const virtualItems = useVirtualItems();
  const activeSession = useActiveSession();

  const [isAtBottom, setIsAtBottom] = useState(true);
  const [scrollerElement, setScrollerElement] = useState<HTMLElement | null>(null);
  const [bottomReservationState, setBottomReservationState] = useState<BottomReservationState>(
    () => createInitialBottomReservationState()
  );
  const [pendingTurnPin, setPendingTurnPin] = useState<PendingTurnPinState | null>(null);

  const scrollerElementRef = useRef<HTMLElement | null>(null);
  const footerElementRef = useRef<HTMLDivElement | null>(null);
  const bottomReservationStateRef = useRef<BottomReservationState>(createInitialBottomReservationState());
  const previousMeasuredHeightRef = useRef<number | null>(null);
  const previousScrollTopRef = useRef(0);
  const measureFrameRef = useRef<number | null>(null);
  const visibleTurnMeasureFrameRef = useRef<number | null>(null);
  const pinReservationReconcileFrameRef = useRef<number | null>(null);
  const resizeObserverRef = useRef<ResizeObserver | null>(null);
  const mutationObserverRef = useRef<MutationObserver | null>(null);
  const layoutTransitionCountRef = useRef(0);
  const touchScrollIntentStartYRef = useRef<number | null>(null);
  const scrollbarPointerInteractionActiveRef = useRef(false);
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

  const isInputActive = useChatInputState(state => state.isActive);
  const isInputExpanded = useChatInputState(state => state.isExpanded);
  const inputHeight = useChatInputState(state => state.inputHeight);

  const inputStackFooterPxRef = useRef(0);
  const inputStackFooterPx = computeFlowChatInputStackFooterPx(inputHeight, isInputActive);
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
    return true;
  }, [releaseAnchorLock]);

  const measureHeightChange = useCallback(() => {
    const scroller = scrollerElementRef.current;
    if (!scroller) return;

    const currentScrollTop = scroller.scrollTop;
    const previousScrollTop = previousScrollTopRef.current;
    const currentTotalCompensation = getTotalBottomCompensationPx();
    const effectiveScrollHeight = Math.max(
      0,
      scroller.scrollHeight - currentTotalCompensation - inputStackFooterPxRef.current,
    );
    const previousMeasuredHeight = previousMeasuredHeightRef.current;
    previousMeasuredHeightRef.current = effectiveScrollHeight;

    if (previousMeasuredHeight === null) {
      previousScrollTopRef.current = currentScrollTop;
      return;
    }

    const heightDelta = effectiveScrollHeight - previousMeasuredHeight;
    if (Math.abs(heightDelta) <= COMPENSATION_EPSILON_PX) {
      previousScrollTopRef.current = currentScrollTop;
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
        return;
      }

      const nextReservationState = consumeBottomCompensation(heightDelta);
      applyFooterCompensationNow(nextReservationState);
      previousScrollTopRef.current = currentScrollTop;
      return;
    }

    // Content shrank: preserve the current visual anchor by extending the footer
    // when the user does not already have enough distance from the bottom.
    const shrinkAmount = -heightDelta;
    // Follow-output mode wants to chase the bottom; absorbing the shrink with
    // synthetic footer would visually freeze the viewport mid-animation. The
    // continuous follow loop will scroll back to the tail on the next frame.
    if (isFollowingOutputRef.current && isStreamingOutputRef.current) {
      previousScrollTopRef.current = currentScrollTop;
      return;
    }
    const collapseIntent = pendingCollapseIntentRef.current;
    const now = performance.now();
    const hasValidCollapseIntent = collapseIntent.active && collapseIntent.expiresAtMs >= now;
    // For unsignaled shrinks, the visible gap to the bottom is what matters.
    // Existing synthetic footer compensation may be stale from an earlier
    // protected collapse, and subtracting it here makes the list think the
    // viewport is still pinned near the bottom when the user has already moved
    // away. That misclassification re-arms anchor restore and causes jitter.
    const fallbackAdditionalCompensation = Math.max(0, shrinkAmount - distanceFromBottom);
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
      : currentTotalCompensation + fallbackAdditionalCompensation;
    if (hasValidCollapseIntent) {
      pendingCollapseIntentRef.current = {
        ...collapseIntent,
        cumulativeShrinkPx,
      };
    }

    if (!hasValidCollapseIntent && fallbackAdditionalCompensation <= COMPENSATION_EPSILON_PX) {
      // If the user is already far enough from the bottom, this shrink does not
      // need protection. Reusing stale bottom compensation here makes the
      // scroll listener restore an older anchor during upward scroll and causes
      // the visible "wall hit" jitter.
      previousScrollTopRef.current = currentScrollTop;
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
  }, [
    activateAnchorLock,
    applyFooterCompensationNow,
    consumeBottomCompensation,
    getTotalBottomCompensationPx,
    restoreAnchorLockNow,
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

  const getRenderedUserMessageElement = useCallback((turnId: string) => {
    const scroller = scrollerElementRef.current;
    if (!scroller) return null;

    return scroller.querySelector<HTMLElement>(
      `.virtual-item-wrapper[data-item-type="user-message"][data-turn-id="${turnId}"]`,
    );
  }, []);

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
    const additiveRetryPx = (
      shouldPreserveCurrentPx &&
      pinMode === 'transient' &&
      resolvedRequiredTailSpacePx > COMPENSATION_EPSILON_PX
    )
      ? currentPinReservation.px + resolvedRequiredTailSpacePx
      : 0;
    const shouldRetainTarget = (
      pinMode === 'sticky-latest' ||
      resolvedRequiredTailSpacePx > COMPENSATION_EPSILON_PX ||
      shouldPreserveCurrentPx
    );

    return {
      kind: 'pin',
      px: Math.max(nextFloorPx, resolvedRequiredTailSpacePx, preservedPx, additiveRetryPx),
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

    if (!scroller || !virtuoso) return false;

    const targetItem = userMessageItems.find(({ item }) => item.turnId === request.turnId);
    if (!targetItem) return false;

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
        index: targetItem.index,
        align: 'start',
        behavior: fallbackBehavior,
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

    const alignedRect = resolvedMetrics.targetElement.getBoundingClientRect();
    const alignedWithinTolerance = Math.abs(alignedRect.top - resolvedMetrics.viewportTop) <= 1.5;

    return alignedWithinTolerance;
  }, [
    buildPinReservation,
    applyFooterCompensationNow,
    getRenderedUserMessageElement,
    resolveTurnPinMetrics,
    schedulePinReservationReconcile,
    scheduleVisibleTurnMeasure,
    snapshotMeasuredContentHeight,
    updateBottomReservationState,
    userMessageItems,
  ]);

  const handleScrollerRef = useCallback((el: HTMLElement | Window | null) => {
    if (el && el instanceof HTMLElement) {
      scrollerElementRef.current = el;
      setScrollerElement(el);
      return;
    }

    scrollerElementRef.current = null;
    setScrollerElement(null);
  }, []);

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

  useEffect(() => {
    previousMeasuredHeightRef.current = null;
    previousScrollTopRef.current = 0;
    setPendingTurnPin(null);
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
    resetBottomReservations();
  }, [activeSession?.sessionId, resetBottomReservations]);

  useEffect(() => {
    if (virtualItems.length === 0) {
      previousMeasuredHeightRef.current = null;
      setPendingTurnPin(null);
      resetBottomReservations();
    }
  }, [virtualItems.length, resetBottomReservations]);

  useEffect(() => {
    if (!scrollerElement) {
      previousMeasuredHeightRef.current = null;
      return;
    }

    const resizeTarget =
      scrollerElement.firstElementChild instanceof HTMLElement
        ? scrollerElement.firstElementChild
        : scrollerElement;

    previousMeasuredHeightRef.current = snapshotMeasuredContentHeight(scrollerElement);
    previousScrollTopRef.current = scrollerElement.scrollTop;

    resizeObserverRef.current?.disconnect();
    resizeObserverRef.current = new ResizeObserver(() => {
      scheduleHeightMeasure();
      scheduleVisibleTurnMeasure(2);
      schedulePinReservationReconcile(2);
      scheduleFollowToLatestWithViewportState('resize-observer');
    });
    resizeObserverRef.current.observe(resizeTarget);

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
        scheduleHeightMeasure(2);
        scheduleVisibleTurnMeasure(2);
        schedulePinReservationReconcile(2);
        scheduleFollowToLatestWithViewportState('mutation-observer');
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
      scheduleHeightMeasure(2);
      scheduleVisibleTurnMeasure(2);
      schedulePinReservationReconcile(2);
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
            return;
          }
        }
      }
      previousScrollTopRef.current = scrollerElement.scrollTop;
      scheduleVisibleTurnMeasure();
      followOutputControllerRef.current.handleScroll();

      if (anchorLockRef.current.active && performance.now() > anchorLockRef.current.lockUntilMs && layoutTransitionCountRef.current === 0) {
        releaseAnchorLock('expired-after-scroll');
      }
    };
    scrollerElement.addEventListener('scroll', handleScroll, { passive: true });

    const handleWheel = (event: WheelEvent) => {
      if (event.deltaY < 0) {
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

      if (currentY - startY > TOUCH_SCROLL_INTENT_EXIT_THRESHOLD_PX) {
        touchScrollIntentStartYRef.current = currentY;
        followOutputControllerRef.current.handleUserScrollIntent();
      }
    };

    const resetTouchScrollIntent = () => {
      touchScrollIntentStartYRef.current = null;
    };

    const handleKeyDown = (event: KeyboardEvent) => {
      if (!isUpwardScrollIntentKey(event) || isEditableElement(event.target)) {
        return;
      }

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
      if (isFollowingOutputRef.current && isStreamingOutputRef.current) {
        scheduleVisibleTurnMeasure(2);
        schedulePinReservationReconcile(2);
        return;
      }
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
    };
  }, [
    activateAnchorLock,
    applyFooterCompensationNow,
    consumeBottomCompensation,
    getTotalBottomCompensationPx,
    latestTurnId,
    pendingTurnPin?.pinMode,
    pendingTurnPin?.turnId,
    releaseAnchorLock,
    scheduleHeightMeasure,
    scheduleFollowToLatestWithViewportState,
    schedulePinReservationReconcile,
    scheduleVisibleTurnMeasure,
    scrollerElement,
    shouldSuspendAutoFollow,
    snapshotMeasuredContentHeight,
    isProcessing,
    updateBottomReservationState,
  ]);

  // `rangeChanged` is affected by overscan/increaseViewportBy, so treat it as a
  // "rendered DOM changed" signal and derive the pinned turn from real DOM visibility.
  const handleRangeChanged = useCallback(() => {
    scheduleVisibleTurnMeasure(2);
    schedulePinReservationReconcile(2);
    scheduleFollowToLatestWithViewportState('range-changed');
  }, [scheduleFollowToLatestWithViewportState, schedulePinReservationReconcile, scheduleVisibleTurnMeasure]);

  useEffect(() => {
    if (userMessageItems.length === 0) {
      const setVisibleTurnInfo = useModernFlowChatStore.getState().setVisibleTurnInfo;
      setVisibleTurnInfo(null);
      return;
    }

    scheduleVisibleTurnMeasure(2);
    schedulePinReservationReconcile(2);
  }, [activeSession?.sessionId, schedulePinReservationReconcile, scheduleVisibleTurnMeasure, scrollerElement, userMessageItems, virtualItems.length]);

  useEffect(() => {
    if (!pendingTurnPin) return;

    if (performance.now() > pendingTurnPin.expiresAtMs) {
      setPendingTurnPin(null);
      return;
    }

    const frameId = requestAnimationFrame(() => {
      const resolved = tryResolvePendingTurnPin(pendingTurnPin);
      if (resolved) {
        setPendingTurnPin(null);
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
  }, [pendingTurnPin, scheduleVisibleTurnMeasure, tryResolvePendingTurnPin]);

  // ── Navigation helpers ────────────────────────────────────────────────
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
    setPendingTurnPin(null);

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
    }
  }, [
    applyFooterCompensationNow,
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

  const scrollToLatestEndPositionInternal = useCallback((behavior: ScrollBehavior) => {
    if (virtuosoRef.current && virtualItems.length > 0) {
      releaseAnchorLock('scroll-to-latest');
      setPendingTurnPin(null);
      virtuosoRef.current.scrollTo({ top: 999999999, behavior });
    }
  }, [releaseAnchorLock, virtualItems.length]);

  const requestTurnPinToTop = useCallback((turnId: string, options?: { behavior?: ScrollBehavior; pinMode?: FlowChatPinTurnToTopMode }) => {
    const requestedPinMode = options?.pinMode ?? 'transient';
    const requestedBehavior = options?.behavior ?? 'auto';
    const targetItem = userMessageItems.find(({ item }) => item.turnId === turnId);
    if (!targetItem || !virtuosoRef.current) {
      return false;
    }

    if (targetItem.index === 0 && requestedPinMode === 'transient') {
      // The first turn has a deterministic destination, so bypass the deferred
      // pin pipeline and snap to the true top immediately.
      setPendingTurnPin(null);
      virtuosoRef.current.scrollTo({ top: 0, behavior: 'auto' });

      return true;
    }

    setPendingTurnPin({
      turnId,
      behavior: requestedBehavior,
      pinMode: requestedPinMode,
      expiresAtMs: performance.now() + 1500,
      attempts: 0,
    });
    return true;
  }, [userMessageItems]);

  const performAutoFollowSync = useCallback(() => {
    if (!latestTurnId) {
      return;
    }

    const currentPinReservation = bottomReservationStateRef.current.pin;
    const totalBottomCompensationPx = getTotalBottomCompensationPx();
    const hasPendingLatestStickyPin = (
      pendingTurnPin?.turnId === latestTurnId &&
      pendingTurnPin.pinMode === 'sticky-latest'
    );
    const hasAppliedLatestStickyPin = (
      currentPinReservation.mode === 'sticky-latest' &&
      currentPinReservation.targetTurnId === latestTurnId
    );
    const shouldKeepStickyLatest = (
      hasAppliedLatestStickyPin &&
      currentPinReservation.floorPx > COMPENSATION_EPSILON_PX
    );
    const shouldPreserveSyntheticTail = (
      hasAppliedLatestStickyPin &&
      totalBottomCompensationPx > COMPENSATION_EPSILON_PX
    );

    if (hasPendingLatestStickyPin) {
      return;
    }

    if (!hasAppliedLatestStickyPin) {
      requestTurnPinToTop(latestTurnId, {
        behavior: 'auto',
        pinMode: 'sticky-latest',
      });
      return;
    }

    if (shouldKeepStickyLatest) {
      return;
    }

    if (shouldPreserveSyntheticTail) {
      return;
    }

    scrollToLatestEndPositionInternal('auto');
  }, [
    getTotalBottomCompensationPx,
    latestTurnId,
    pendingTurnPin?.pinMode,
    pendingTurnPin?.turnId,
    requestTurnPinToTop,
    scrollToLatestEndPositionInternal,
  ]);

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
      if (latestTurnId) {
        requestTurnPinToTop(latestTurnId, {
          behavior: 'auto',
          pinMode: 'sticky-latest',
        });
      }
    },
    shouldSuspendAutoFollow,
    getAutoFollowDistanceFromBottom: (scroller) => (
      Math.max(0, scroller.scrollHeight - scroller.clientHeight - scroller.scrollTop - getTotalBottomCompensationPx())
    ),
    onContinuousFollowFrame: () => {
      // Keep sticky-latest pin floor aligned with the live DOM as collapses
      // shrink the layout. Without this the pin reservation would lag for one
      // RAF tick and the viewport would briefly land below the latest user
      // message.
      reconcileStickyPinReservation();
    },
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
            index: virtualItems.length - 1,
            align: 'end',
            behavior: 'auto',
          });
        };
        // Allow two frames for virtual items to settle before scrolling.
        requestAnimationFrame(() => {
          requestAnimationFrame(scrollToBottom);
        });
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
        index: targetItem.index,
        behavior: 'smooth',
        align: 'center',
      });
    }
  }, [clearPinReservationForUserNavigation, exitFollowOutput, userMessageItems]);

  const scrollToIndex = useCallback((index: number) => {
    if (!virtuosoRef.current) return;
    if (index < 0 || index >= virtualItems.length) return;

    exitFollowOutput('scroll-to-index');
    clearPinReservationForUserNavigation();

    if (index === 0) {
      virtuosoRef.current.scrollTo({ top: 0, behavior: 'auto' });
    } else {
      virtuosoRef.current.scrollToIndex({ index, align: 'center', behavior: 'auto' });
    }
  }, [clearPinReservationForUserNavigation, exitFollowOutput, virtualItems.length]);

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
    if (virtuosoRef.current && virtualItems.length > 0) {
      clearPinReservationForUserNavigation();
      virtuosoRef.current.scrollTo({ top: 999999999, behavior: 'smooth' });
    }
  }, [clearPinReservationForUserNavigation, virtualItems.length]);

  const scrollToLatestEndPosition = useCallback(() => {
    enterFollowOutput('jump-to-latest');
  }, [enterFollowOutput]);

  useImperativeHandle(ref, () => ({
    scrollToTurn,
    scrollToIndex,
    scrollToPhysicalBottomAndClearPin,
    scrollToLatestEndPosition,
    pinTurnToTop,
  }), [pinTurnToTop, scrollToTurn, scrollToIndex, scrollToPhysicalBottomAndClearPin, scrollToLatestEndPosition]);

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
        followOutput={false}

        alignToBottom={false}
        // New mounts start near the latest user turn to avoid flashing older
        // content before sticky pin logic can finish.
        initialTopMostItemIndex={latestUserMessageIndex}

        overscan={{ main: 600, reverse: 600 }}

        atBottomThreshold={50}
        atBottomStateChange={handleAtBottomStateChange}

        rangeChanged={handleRangeChanged}

        defaultItemHeight={200}

        increaseViewportBy={{ top: 600, bottom: 600 }}

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

VirtualMessageList.displayName = 'VirtualMessageList';
