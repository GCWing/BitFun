/**
 * Follow-output controller for the modern virtualized FlowChat list.
 *
 * Keeps follow state local to the viewport layer while separating the
 * "when should we follow" policy from the low-level list scroll mechanics.
 */

import { useCallback, useEffect, useRef, useState, type RefObject } from 'react';

const PROGRAMMATIC_SCROLL_GUARD_MS = 160;
const AUTO_FOLLOW_BOTTOM_THRESHOLD_PX = 24;
const USER_SCROLL_DIRECTION_EPSILON_PX = 0.5;
const USER_SCROLL_INTENT_WINDOW_MS = 450;
const USER_SCROLL_INTENT_PROGRAMMATIC_GRACE_MS = 80;

export type FollowOutputEnterReason = 'jump-to-latest' | 'auto-follow';
export type FollowOutputExitReason =
  | 'session-changed'
  | 'user-scroll-up'
  | 'scroll-to-turn'
  | 'scroll-to-index'
  | 'pin-turn-to-top';

interface UseFlowChatFollowOutputOptions {
  activeSessionId?: string;
  latestTurnId: string | null;
  virtualItemCount: number;
  isStreaming: boolean;
  scrollerRef: RefObject<HTMLElement | null>;
  performUserFollowScroll: () => void;
  performAutoFollowScroll: () => void;
  performLatestTurnStickyPin: () => void;
  /**
   * Returns true when auto-follow should be suspended for layout-protection
   * reasons (collapse animation, etc.). The continuous follow loop ignores
   * this signal because follow mode actively wants to track the bottom even
   * while intermediate cards collapse; only the event-driven `scheduleFollowToLatest`
   * still respects it for backward compatibility with anchor restore paths.
   */
  shouldSuspendAutoFollow?: () => boolean;
  getAutoFollowDistanceFromBottom?: (scroller: HTMLElement) => number;
  /**
   * Optional per-frame hook invoked from inside the continuous follow loop.
   * Used to reconcile sticky-latest pin floor in lockstep with the scroll
   * adjustment so the pin reservation never lags behind a shrinking layout.
   */
  onContinuousFollowFrame?: () => void;
}

interface UseFlowChatFollowOutputResult {
  isFollowingOutput: boolean;
  enterFollowOutput: (reason: FollowOutputEnterReason) => void;
  exitFollowOutput: (reason: FollowOutputExitReason) => void;
  armFollowOutputForNewTurn: () => void;
  activateArmedFollowOutput: () => boolean;
  cancelPendingAutoFollowArm: () => void;
  scheduleFollowToLatest: (reason: string) => void;
  handleUserScrollIntent: () => void;
  handleScroll: () => void;
}

function getDistanceFromBottom(scroller: HTMLElement): number {
  return Math.max(0, scroller.scrollHeight - scroller.clientHeight - scroller.scrollTop);
}

export function useFlowChatFollowOutput({
  activeSessionId,
  latestTurnId,
  virtualItemCount,
  isStreaming,
  scrollerRef,
  performUserFollowScroll,
  performAutoFollowScroll,
  performLatestTurnStickyPin,
  shouldSuspendAutoFollow,
  getAutoFollowDistanceFromBottom,
  onContinuousFollowFrame,
}: UseFlowChatFollowOutputOptions): UseFlowChatFollowOutputResult {
  const [isFollowingOutput, setIsFollowingOutput] = useState(false);

  const isFollowingOutputRef = useRef(isFollowingOutput);
  const followFrameRef = useRef<number | null>(null);
  const programmaticScrollUntilMsRef = useRef(0);
  const explicitUserScrollIntentUntilMsRef = useRef(0);
  const lastObservedScrollTopRef = useRef(0);
  const previousSessionIdRef = useRef<string | undefined>(activeSessionId);
  const armedAutoFollowTurnIdRef = useRef<string | null>(null);
  const continuousFollowFrameRef = useRef<number | null>(null);
  const isStreamingRef = useRef(isStreaming);
  const performAutoFollowScrollRef = useRef(performAutoFollowScroll);
  const onContinuousFollowFrameRef = useRef(onContinuousFollowFrame);
  const getAutoFollowDistanceFromBottomRef = useRef(getAutoFollowDistanceFromBottom);

  isStreamingRef.current = isStreaming;
  performAutoFollowScrollRef.current = performAutoFollowScroll;
  onContinuousFollowFrameRef.current = onContinuousFollowFrame;
  getAutoFollowDistanceFromBottomRef.current = getAutoFollowDistanceFromBottom;

  const setFollowingOutput = useCallback((nextValue: boolean) => {
    isFollowingOutputRef.current = nextValue;
    setIsFollowingOutput(prev => (prev === nextValue ? prev : nextValue));
    if (!nextValue && continuousFollowFrameRef.current !== null) {
      cancelAnimationFrame(continuousFollowFrameRef.current);
      continuousFollowFrameRef.current = null;
    }
  }, []);

  const cancelScheduledFollow = useCallback(() => {
    if (followFrameRef.current !== null) {
      cancelAnimationFrame(followFrameRef.current);
      followFrameRef.current = null;
    }
  }, []);

  const stopContinuousFollowLoop = useCallback(() => {
    if (continuousFollowFrameRef.current !== null) {
      cancelAnimationFrame(continuousFollowFrameRef.current);
      continuousFollowFrameRef.current = null;
    }
  }, []);

  /**
   * Continuous RAF-driven follow loop.
   *
   * Why this exists:
   *  - Streaming text + auto-collapsing tool cards generate dense bursts of
   *    DOM mutations and CSS transitions. Event-driven follow (via observers)
   *    is gated by `shouldSuspendAutoFollow` during transitions, which makes
   *    the viewport visibly stall and then jump after the transition ends.
   *  - This loop runs every animation frame while follow + streaming is
   *    active, pushing scrollTop toward the latest token regardless of any
   *    intermediate layout shrink. The result is a smooth, continuous tail.
   *
   * Safety:
   *  - Programmatic scrolls inside this loop bump
   *    `programmaticScrollUntilMsRef` so the user-intent detector does not
   *    misclassify them as upward scrolls.
   *  - The loop bails out as soon as follow is exited, streaming ends, the
   *    scroller disappears, or the viewport is already pinned to the bottom.
   */
  const runContinuousFollowFrame = useCallback(() => {
    continuousFollowFrameRef.current = null;

    if (!isFollowingOutputRef.current || !isStreamingRef.current) {
      return;
    }

    const scroller = scrollerRef.current;
    if (!scroller) {
      return;
    }

    onContinuousFollowFrameRef.current?.();

    const rawDistance = getDistanceFromBottom(scroller);
    const measuredDistance = getAutoFollowDistanceFromBottomRef.current?.(scroller) ?? rawDistance;
    if (measuredDistance > AUTO_FOLLOW_BOTTOM_THRESHOLD_PX) {
      programmaticScrollUntilMsRef.current = performance.now() + PROGRAMMATIC_SCROLL_GUARD_MS;
      explicitUserScrollIntentUntilMsRef.current = 0;
      performAutoFollowScrollRef.current();
      lastObservedScrollTopRef.current = scroller.scrollTop;
    }

    if (!isFollowingOutputRef.current || !isStreamingRef.current) {
      return;
    }

    continuousFollowFrameRef.current = requestAnimationFrame(runContinuousFollowFrame);
  }, [scrollerRef]);

  const startContinuousFollowLoop = useCallback(() => {
    if (continuousFollowFrameRef.current !== null) {
      return;
    }
    if (!isFollowingOutputRef.current || !isStreamingRef.current) {
      return;
    }
    continuousFollowFrameRef.current = requestAnimationFrame(runContinuousFollowFrame);
  }, [runContinuousFollowFrame]);

  const cancelPendingAutoFollowArm = useCallback(() => {
    armedAutoFollowTurnIdRef.current = null;
  }, []);

  const runProgrammaticScroll = useCallback((scrollAction: () => void) => {
    programmaticScrollUntilMsRef.current = performance.now() + PROGRAMMATIC_SCROLL_GUARD_MS;
    explicitUserScrollIntentUntilMsRef.current = 0;
    scrollAction();
    const scroller = scrollerRef.current;
    if (scroller) {
      lastObservedScrollTopRef.current = scroller.scrollTop;
    }
  }, [scrollerRef]);

  const enterFollowOutput = useCallback((reason: FollowOutputEnterReason) => {
    cancelPendingAutoFollowArm();
    cancelScheduledFollow();
    explicitUserScrollIntentUntilMsRef.current = 0;
    setFollowingOutput(true);
    const followAction = reason === 'jump-to-latest'
      ? performUserFollowScroll
      : performAutoFollowScroll;
    runProgrammaticScroll(followAction);
  }, [
    cancelPendingAutoFollowArm,
    cancelScheduledFollow,
    performAutoFollowScroll,
    performUserFollowScroll,
    runProgrammaticScroll,
    setFollowingOutput,
  ]);

  const exitFollowOutput = useCallback((_reason: FollowOutputExitReason) => {
    cancelPendingAutoFollowArm();
    cancelScheduledFollow();
    explicitUserScrollIntentUntilMsRef.current = 0;
    setFollowingOutput(false);
    const scroller = scrollerRef.current;
    if (scroller) {
      lastObservedScrollTopRef.current = scroller.scrollTop;
    }
  }, [cancelPendingAutoFollowArm, cancelScheduledFollow, scrollerRef, setFollowingOutput]);

  const armFollowOutputForNewTurn = useCallback(() => {
    if (!latestTurnId) {
      cancelPendingAutoFollowArm();
      return;
    }

    armedAutoFollowTurnIdRef.current = latestTurnId;
    cancelScheduledFollow();
    setFollowingOutput(false);
    runProgrammaticScroll(performLatestTurnStickyPin);
  }, [
    cancelPendingAutoFollowArm,
    cancelScheduledFollow,
    latestTurnId,
    performLatestTurnStickyPin,
    runProgrammaticScroll,
    setFollowingOutput,
  ]);

  const activateArmedFollowOutput = useCallback(() => {
    const armedTurnId = armedAutoFollowTurnIdRef.current;
    const isAlreadyFollowing = isFollowingOutputRef.current;
    const isArmedForLatestTurn = Boolean(latestTurnId && armedTurnId === latestTurnId);
    const isAutoFollowSuspended = shouldSuspendAutoFollow?.() === true;

    if (!latestTurnId || !isArmedForLatestTurn || isAlreadyFollowing) {
      return false;
    }

    if (isAutoFollowSuspended) {
      return false;
    }

    cancelPendingAutoFollowArm();
    cancelScheduledFollow();
    setFollowingOutput(true);
    runProgrammaticScroll(performAutoFollowScroll);
    return true;
  }, [
    cancelPendingAutoFollowArm,
    cancelScheduledFollow,
    latestTurnId,
    performAutoFollowScroll,
    runProgrammaticScroll,
    setFollowingOutput,
    shouldSuspendAutoFollow,
  ]);

  const handleUserScrollIntent = useCallback(() => {
    if (!isFollowingOutputRef.current && armedAutoFollowTurnIdRef.current === null) {
      return;
    }

    const now = performance.now();
    if (now <= programmaticScrollUntilMsRef.current) {
      const scroller = scrollerRef.current;
      const alreadyAwayFromBottom = scroller
        ? getDistanceFromBottom(scroller) > AUTO_FOLLOW_BOTTOM_THRESHOLD_PX
        : false;

      if (!alreadyAwayFromBottom) {
        return;
      }

      programmaticScrollUntilMsRef.current = Math.min(
        programmaticScrollUntilMsRef.current,
        now + USER_SCROLL_INTENT_PROGRAMMATIC_GRACE_MS,
      );
    }
    explicitUserScrollIntentUntilMsRef.current = now + USER_SCROLL_INTENT_WINDOW_MS;
  }, [scrollerRef]);

  const scheduleFollowToLatest = useCallback((_reason: string) => {
    if (
      !isFollowingOutputRef.current ||
      !isStreaming ||
      virtualItemCount === 0 ||
      shouldSuspendAutoFollow?.() === true
    ) {
      return;
    }

    if (followFrameRef.current !== null) {
      return;
    }

    followFrameRef.current = requestAnimationFrame(() => {
      followFrameRef.current = null;

      if (!isFollowingOutputRef.current || !isStreaming || virtualItemCount === 0) {
        return;
      }

      if (shouldSuspendAutoFollow?.() === true) {
        return;
      }

      const scroller = scrollerRef.current;
      if (!scroller) {
        return;
      }

      const rawDistanceFromBottom = getDistanceFromBottom(scroller);
      const distanceFromBottom = getAutoFollowDistanceFromBottom?.(scroller) ?? rawDistanceFromBottom;
      if (distanceFromBottom <= AUTO_FOLLOW_BOTTOM_THRESHOLD_PX) {
        return;
      }

      runProgrammaticScroll(performAutoFollowScroll);
    });
  }, [getAutoFollowDistanceFromBottom, isStreaming, performAutoFollowScroll, runProgrammaticScroll, scrollerRef, shouldSuspendAutoFollow, virtualItemCount]);

  const handleScroll = useCallback(() => {
    const scroller = scrollerRef.current;
    if (!scroller) {
      return;
    }

    const currentScrollTop = scroller.scrollTop;
    const previousScrollTop = lastObservedScrollTopRef.current;
    lastObservedScrollTopRef.current = currentScrollTop;

    if (!isFollowingOutputRef.current && armedAutoFollowTurnIdRef.current === null) {
      return;
    }

    if (performance.now() <= programmaticScrollUntilMsRef.current) {
      return;
    }

    const upwardDelta = previousScrollTop - currentScrollTop;
    if (upwardDelta > USER_SCROLL_DIRECTION_EPSILON_PX) {
      const now = performance.now();
      const hasRecentExplicitUserIntent = now <= explicitUserScrollIntentUntilMsRef.current;
      const distanceFromBottom = getDistanceFromBottom(scroller);
      if (!hasRecentExplicitUserIntent) {
        if (
          isFollowingOutputRef.current &&
          distanceFromBottom <= AUTO_FOLLOW_BOTTOM_THRESHOLD_PX
        ) {
          return;
        }
        return;
      }

      if (shouldSuspendAutoFollow?.() === true) {
        if (isFollowingOutputRef.current && hasRecentExplicitUserIntent) {
          exitFollowOutput('user-scroll-up');
        }
        explicitUserScrollIntentUntilMsRef.current = 0;
        return;
      }

      explicitUserScrollIntentUntilMsRef.current = 0;

      if (!isFollowingOutputRef.current) {
        cancelPendingAutoFollowArm();
        return;
      }

      exitFollowOutput('user-scroll-up');
    }
  }, [cancelPendingAutoFollowArm, exitFollowOutput, scrollerRef, shouldSuspendAutoFollow]);

  useEffect(() => {
    const scroller = scrollerRef.current;
    if (scroller) {
      lastObservedScrollTopRef.current = scroller.scrollTop;
    }
  }, [scrollerRef]);

  useEffect(() => {
    const previousSessionId = previousSessionIdRef.current;
    if (previousSessionId === activeSessionId) {
      return;
    }

    previousSessionIdRef.current = activeSessionId;
    cancelPendingAutoFollowArm();
    cancelScheduledFollow();
    explicitUserScrollIntentUntilMsRef.current = 0;
    const nextFollowState = Boolean(activeSessionId && virtualItemCount === 0);

    if (nextFollowState) {
      setFollowingOutput(true);
      return;
    }

    setFollowingOutput(false);
  }, [
    activeSessionId,
    cancelPendingAutoFollowArm,
    cancelScheduledFollow,
    latestTurnId,
    setFollowingOutput,
    virtualItemCount,
  ]);

  useEffect(() => {
    if (!isFollowingOutput || !isStreaming) {
      stopContinuousFollowLoop();
      return;
    }

    scheduleFollowToLatest('streaming-started');
    startContinuousFollowLoop();
  }, [isFollowingOutput, isStreaming, scheduleFollowToLatest, startContinuousFollowLoop, stopContinuousFollowLoop]);

  useEffect(() => {
    return () => {
      cancelScheduledFollow();
      stopContinuousFollowLoop();
    };
  }, [cancelScheduledFollow, stopContinuousFollowLoop]);

  return {
    isFollowingOutput,
    enterFollowOutput,
    exitFollowOutput,
    armFollowOutputForNewTurn,
    activateArmedFollowOutput,
    cancelPendingAutoFollowArm,
    scheduleFollowToLatest,
    handleUserScrollIntent,
    handleScroll,
  };
}
