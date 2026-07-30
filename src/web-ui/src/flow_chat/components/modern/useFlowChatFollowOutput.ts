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
const CONTINUOUS_FOLLOW_TIME_CONSTANT_MS = 55;
const CONTINUOUS_FOLLOW_MIN_STEP_PX = 0.75;
const CONTINUOUS_FOLLOW_MAX_STEP_PX = 32;
const CONTINUOUS_FOLLOW_SNAP_THRESHOLD_PX = 0.5;
const CONTINUOUS_FOLLOW_MAX_FRAME_DELTA_MS = 34;

/**
 * Move toward a newly-grown tail without snapping by a whole text line.
 *
 * Streaming prose only changes layout when a line wraps, so the physical
 * bottom advances in line-height-sized steps even though characters arrive
 * smoothly. This exponential step is retargetable on every animation frame:
 * a later wrap extends the same motion instead of restarting a browser-native
 * smooth scroll.
 */
export function computeContinuousFollowStep(
  distancePx: number,
  frameDeltaMs: number,
): number {
  const distance = Number.isFinite(distancePx) ? Math.max(0, distancePx) : 0;
  if (distance <= CONTINUOUS_FOLLOW_SNAP_THRESHOLD_PX) {
    return distance;
  }

  const deltaMs = Number.isFinite(frameDeltaMs)
    ? Math.min(CONTINUOUS_FOLLOW_MAX_FRAME_DELTA_MS, Math.max(0, frameDeltaMs))
    : 0;
  const easedStep = distance * (
    1 - Math.exp(-deltaMs / CONTINUOUS_FOLLOW_TIME_CONSTANT_MS)
  );
  const step = Math.min(
    distance,
    CONTINUOUS_FOLLOW_MAX_STEP_PX,
    Math.max(CONTINUOUS_FOLLOW_MIN_STEP_PX, easedStep),
  );

  return distance - step <= CONTINUOUS_FOLLOW_SNAP_THRESHOLD_PX
    ? distance
    : step;
}

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
   * reasons (collapse animation, layout transition, pending collapse intent).
   * Both the event-driven `scheduleFollowToLatest` and the continuous follow
   * loop honour this signal: while a known collapse animation is in flight we
   * must not fight the anchor-lock + bottom-reservation machinery, otherwise
   * the conversation visibly "sinks down" each time content above shrinks.
   * The continuous loop keeps requesting frames while suspended and resumes
   * bottom-tracking on the next frame after the suspension clears.
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
  resumeFollowOutputForMountedStream: () => boolean;
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
  const programmaticScrollUntilMsRef = useRef(0);
  const explicitUserScrollIntentUntilMsRef = useRef(0);
  const lastObservedScrollTopRef = useRef(0);
  const previousSessionIdRef = useRef<string | undefined>(activeSessionId);
  const armedAutoFollowTurnIdRef = useRef<string | null>(null);
  const continuousFollowFrameRef = useRef<number | null>(null);
  const lastContinuousFollowFrameMsRef = useRef<number | null>(null);
  const prefersReducedMotionRef = useRef(false);
  const isStreamingRef = useRef(isStreaming);
  const performAutoFollowScrollRef = useRef(performAutoFollowScroll);
  const onContinuousFollowFrameRef = useRef(onContinuousFollowFrame);
  const getAutoFollowDistanceFromBottomRef = useRef(getAutoFollowDistanceFromBottom);
  const shouldSuspendAutoFollowRef = useRef(shouldSuspendAutoFollow);

  isStreamingRef.current = isStreaming;
  performAutoFollowScrollRef.current = performAutoFollowScroll;
  onContinuousFollowFrameRef.current = onContinuousFollowFrame;
  getAutoFollowDistanceFromBottomRef.current = getAutoFollowDistanceFromBottom;
  shouldSuspendAutoFollowRef.current = shouldSuspendAutoFollow;

  const setFollowingOutput = useCallback((nextValue: boolean) => {
    isFollowingOutputRef.current = nextValue;
    setIsFollowingOutput(prev => (prev === nextValue ? prev : nextValue));
    if (!nextValue && continuousFollowFrameRef.current !== null) {
      cancelAnimationFrame(continuousFollowFrameRef.current);
      continuousFollowFrameRef.current = null;
    }
    if (!nextValue) {
      lastContinuousFollowFrameMsRef.current = null;
    }
  }, []);

  const stopContinuousFollowLoop = useCallback(() => {
    if (continuousFollowFrameRef.current !== null) {
      cancelAnimationFrame(continuousFollowFrameRef.current);
      continuousFollowFrameRef.current = null;
    }
    lastContinuousFollowFrameMsRef.current = null;
  }, []);

  /**
   * Continuous RAF-driven follow loop.
   *
   * Why this exists:
   *  - Streaming text + auto-collapsing tool cards generate dense bursts of
   *    DOM mutations and CSS transitions.
   *  - Text layout still grows one whole line at a time. Snapping to the new
   *    bottom on each wrap makes otherwise-smooth typewriter output shake.
   *  - This loop runs every animation frame while follow + streaming is
   *    active and eases scrollTop toward the latest tail. New growth retargets
   *    the in-flight motion without restarting it.
   *
   * Safety:
   *  - Programmatic scrolls inside this loop bump
   *    `programmaticScrollUntilMsRef` so the user-intent detector does not
   *    misclassify them as upward scrolls.
   *  - The loop bails out as soon as follow is exited, streaming ends, the
   *    scroller disappears, or the viewport is already pinned to the bottom.
   */
  const runContinuousFollowFrame = useCallback((nowMs: number) => {
    continuousFollowFrameRef.current = null;

    if (!isFollowingOutputRef.current || !isStreamingRef.current) {
      lastContinuousFollowFrameMsRef.current = null;
      return;
    }

    const scroller = scrollerRef.current;
    if (!scroller) {
      lastContinuousFollowFrameMsRef.current = null;
      return;
    }

    onContinuousFollowFrameRef.current?.();

    // While a known collapse animation / layout transition is in flight, the
    // VirtualMessageList anchor-lock + bottom-reservation footer is preserving
    // the upper visual anchor. Issuing a programmatic scroll-to-bottom from
    // this loop would fight that machinery and re-introduce the "sink-down"
    // jitter the user reported. We simply re-arm the next frame and resume on
    // the first frame after the suspension clears.
    const isSuspended = shouldSuspendAutoFollowRef.current?.() === true;
    const measuredDistance = getAutoFollowDistanceFromBottomRef.current?.(scroller)
      ?? getDistanceFromBottom(scroller);
    const previousFrameMs = lastContinuousFollowFrameMsRef.current;
    const frameDeltaMs = previousFrameMs === null
      ? 1000 / 60
      : nowMs - previousFrameMs;
    lastContinuousFollowFrameMsRef.current = nowMs;

    if (!isSuspended && measuredDistance > CONTINUOUS_FOLLOW_SNAP_THRESHOLD_PX) {
      programmaticScrollUntilMsRef.current = nowMs + PROGRAMMATIC_SCROLL_GUARD_MS;
      explicitUserScrollIntentUntilMsRef.current = 0;

      if (prefersReducedMotionRef.current) {
        performAutoFollowScrollRef.current();
      } else {
        const step = computeContinuousFollowStep(measuredDistance, frameDeltaMs);
        scroller.scrollTop += step;
      }
      lastObservedScrollTopRef.current = scroller.scrollTop;
    }

    if (!isFollowingOutputRef.current || !isStreamingRef.current) {
      return;
    }

    // Stop the loop when the page is hidden to avoid unnecessary work
    if (document.hidden) {
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
    explicitUserScrollIntentUntilMsRef.current = 0;
    setFollowingOutput(true);
    const followAction = reason === 'jump-to-latest'
      ? performUserFollowScroll
      : performAutoFollowScroll;
    runProgrammaticScroll(followAction);
  }, [
    cancelPendingAutoFollowArm,
    performAutoFollowScroll,
    performUserFollowScroll,
    runProgrammaticScroll,
    setFollowingOutput,
  ]);

  const exitFollowOutput = useCallback((_reason: FollowOutputExitReason) => {
    cancelPendingAutoFollowArm();
    explicitUserScrollIntentUntilMsRef.current = 0;
    setFollowingOutput(false);
    const scroller = scrollerRef.current;
    if (scroller) {
      lastObservedScrollTopRef.current = scroller.scrollTop;
    }
  }, [cancelPendingAutoFollowArm, scrollerRef, setFollowingOutput]);

  const armFollowOutputForNewTurn = useCallback(() => {
    if (!latestTurnId) {
      cancelPendingAutoFollowArm();
      return;
    }

    armedAutoFollowTurnIdRef.current = latestTurnId;
    setFollowingOutput(false);
    runProgrammaticScroll(performLatestTurnStickyPin);
  }, [
    cancelPendingAutoFollowArm,
    latestTurnId,
    performLatestTurnStickyPin,
    runProgrammaticScroll,
    setFollowingOutput,
  ]);

  const resumeFollowOutputForMountedStream = useCallback(() => {
    if (!latestTurnId || !isStreaming || virtualItemCount === 0) {
      return false;
    }

    enterFollowOutput('auto-follow');
    return true;
  }, [enterFollowOutput, isStreaming, latestTurnId, virtualItemCount]);

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
    setFollowingOutput(true);
    runProgrammaticScroll(performAutoFollowScroll);
    return true;
  }, [
    cancelPendingAutoFollowArm,
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

      if (
        !alreadyAwayFromBottom &&
        !isFollowingOutputRef.current &&
        armedAutoFollowTurnIdRef.current === null
      ) {
        return;
      }

      programmaticScrollUntilMsRef.current = Math.min(
        programmaticScrollUntilMsRef.current,
        now + USER_SCROLL_INTENT_PROGRAMMATIC_GRACE_MS,
      );
    }
    explicitUserScrollIntentUntilMsRef.current = now + USER_SCROLL_INTENT_WINDOW_MS;

    if (isFollowingOutputRef.current) {
      // Input handlers see the upward intent before scrollTop necessarily moves.
      exitFollowOutput('user-scroll-up');
      return;
    }

    cancelPendingAutoFollowArm();
  }, [cancelPendingAutoFollowArm, exitFollowOutput, scrollerRef]);

  const scheduleFollowToLatest = useCallback((_reason: string) => {
    if (
      !isFollowingOutputRef.current ||
      !isStreamingRef.current ||
      virtualItemCount === 0
    ) {
      return;
    }

    // Observer notifications only ensure that the continuous, retargetable
    // loop is awake. They must not start a separate native smooth scroll or
    // snap directly to the bottom; either would recreate the per-line jump.
    startContinuousFollowLoop();
  }, [startContinuousFollowLoop, virtualItemCount]);

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

  // Restart follow loop when the page becomes visible again
  useEffect(() => {
    const handleVisibility = () => {
      if (!document.hidden && isFollowingOutputRef.current && isStreamingRef.current) {
        startContinuousFollowLoop();
      }
    };
    document.addEventListener('visibilitychange', handleVisibility);
    return () => document.removeEventListener('visibilitychange', handleVisibility);
  }, [startContinuousFollowLoop]);

  useEffect(() => {
    if (typeof window === 'undefined' || typeof window.matchMedia !== 'function') {
      return;
    }
    const media = window.matchMedia('(prefers-reduced-motion: reduce)');
    const syncPreference = () => {
      prefersReducedMotionRef.current = media.matches;
    };
    syncPreference();
    media.addEventListener?.('change', syncPreference);
    return () => media.removeEventListener?.('change', syncPreference);
  }, []);

  useEffect(() => {
    return () => {
      stopContinuousFollowLoop();
    };
  }, [stopContinuousFollowLoop]);

  return {
    isFollowingOutput,
    enterFollowOutput,
    exitFollowOutput,
    armFollowOutputForNewTurn,
    resumeFollowOutputForMountedStream,
    activateArmedFollowOutput,
    cancelPendingAutoFollowArm,
    scheduleFollowToLatest,
    handleUserScrollIntent,
    handleScroll,
  };
}
