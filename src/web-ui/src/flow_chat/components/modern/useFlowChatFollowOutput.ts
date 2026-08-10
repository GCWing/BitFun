import { useCallback, useEffect, useRef, useState, type RefObject } from 'react';
import {
  contentEndScrollTop,
  nextTailFollowState,
  tailHoldMaxGapPx,
  type TailFollowState,
} from './flowChatTailFollow';

export type FollowOutputEnterReason =
  | 'jump-to-latest'
  | 'new-turn'
  | 'session-open'
  | 'streaming-resumed';
export type FollowOutputExitReason =
  | 'session-changed'
  | 'user-scroll'
  | 'scroll-to-turn'
  | 'scroll-to-index';

interface UseFlowChatFollowOutputOptions {
  activeSessionId?: string;
  latestTurnId: string | null;
  virtualItemCount: number;
  isStreaming: boolean;
  isViewportActive: boolean;
  scrollerRef: RefObject<HTMLElement | null>;
  /** Height of the resident tail spacer currently rendered below the content. */
  getTailSpacerPx: () => number;
  /** One-shot scroll placing the end of real content at the viewport bottom. */
  scrollToContentEnd: (behavior: ScrollBehavior) => void;
  /** One-shot scroll placing a Turn's user message at the viewport top. */
  scrollTurnToTop: (turnId: string) => boolean;
  /** Offset that would place a Turn's user message at the viewport top, if rendered. */
  resolveTurnTopScrollTop: (turnId: string) => number | null;
  /** True while the transcript is still hidden for the opening reveal. */
  isOpeningViewport: () => boolean;
}

interface UseFlowChatFollowOutputResult {
  isFollowingOutput: boolean;
  enterFollowOutput: (reason: FollowOutputEnterReason) => void;
  exitFollowOutput: (reason: FollowOutputExitReason) => void;
  scheduleFollowToLatest: () => void;
  handleUserScrollIntent: () => void;
  handleScroll: () => void;
}

const BOTTOM_EPSILON_PX = 2;

/**
 * Frames a pinned Turn may stay unmeasurable before the pin is abandoned.
 * Virtuoso renders the tail immediately, so this only guards against a Turn
 * that never mounts at all.
 */
const PIN_RESOLVE_MAX_ATTEMPTS = 30;

/**
 * Frames the follow target keeps being re-asserted after a non-streaming entry,
 * refreshed whenever it actually moves.
 *
 * A session opens against an unsettled transcript: item heights are still
 * estimates and `isPartial` sessions page older Turns in, so the end of content
 * can travel thousands of pixels after the first alignment. The browser used to
 * absorb that for free — a bottom-aligned scroll was clamped at `scrollHeight -
 * clientHeight`, so any target at or past the end snapped onto it. The resident
 * tail spacer removes that clamp, so the settle has to be explicit.
 */
const SETTLE_FRAMES = 90;

export function useFlowChatFollowOutput({
  activeSessionId,
  latestTurnId,
  virtualItemCount,
  isStreaming,
  isViewportActive,
  scrollerRef,
  getTailSpacerPx,
  scrollToContentEnd,
  scrollTurnToTop,
  resolveTurnTopScrollTop,
  isOpeningViewport,
}: UseFlowChatFollowOutputOptions): UseFlowChatFollowOutputResult {
  const [isFollowingOutput, setIsFollowingOutput] = useState(false);
  const isFollowingOutputRef = useRef(false);
  const isStreamingRef = useRef(isStreaming);
  const isViewportActiveRef = useRef(isViewportActive);
  const latestTurnIdRef = useRef(latestTurnId);
  const followFrameRef = useRef<number | null>(null);
  const previousSessionIdRef = useRef(activeSessionId);
  const previousLatestTurnIdRef = useRef<string | null>(latestTurnId);
  const hasMountedRef = useRef(false);
  const wasStreamingRef = useRef(isStreaming);

  const followStateRef = useRef<TailFollowState>({ mode: 'hold-tail', target: 0 });
  const pinTurnIdRef = useRef<string | null>(null);
  const pinScrollTopRef = useRef<number | null>(null);
  const pinAttemptsRef = useRef(0);
  const settleFramesRef = useRef(0);

  isFollowingOutputRef.current = isFollowingOutput;
  isStreamingRef.current = isStreaming;
  isViewportActiveRef.current = isViewportActive;
  latestTurnIdRef.current = latestTurnId;

  const stopFollowFrame = useCallback(() => {
    if (followFrameRef.current !== null) {
      cancelAnimationFrame(followFrameRef.current);
      followFrameRef.current = null;
    }
  }, []);

  const readContentEndScrollTop = useCallback((scroller: HTMLElement) => (
    contentEndScrollTop({
      scrollHeight: scroller.scrollHeight,
      clientHeight: scroller.clientHeight,
      tailSpacerPx: getTailSpacerPx(),
    })
  ), [getTailSpacerPx]);

  const clearPin = useCallback(() => {
    pinTurnIdRef.current = null;
    pinScrollTopRef.current = null;
    pinAttemptsRef.current = 0;
  }, []);

  /**
   * Resolve the pinned Turn's offset from live layout every frame rather than
   * caching it once. Virtuoso re-estimates the height of unrendered items while
   * it scrolls, which shifts absolute offsets; a cached pin would drift.
   */
  const readPinScrollTop = useCallback((): number | null => {
    const pinTurnId = pinTurnIdRef.current;
    if (!pinTurnId) {
      return null;
    }

    const resolved = resolveTurnTopScrollTop(pinTurnId);
    if (resolved !== null) {
      pinScrollTopRef.current = resolved;
      pinAttemptsRef.current = 0;
      return resolved;
    }

    if (pinScrollTopRef.current === null) {
      pinAttemptsRef.current += 1;
      if (pinAttemptsRef.current >= PIN_RESOLVE_MAX_ATTEMPTS) {
        clearPin();
      }
    }
    return pinScrollTopRef.current;
  }, [clearPin, resolveTurnTopScrollTop]);

  /** Move the viewport to whatever the follow state currently owns. */
  const applyFollowTarget = useCallback(() => {
    const scroller = scrollerRef.current;
    if (!scroller) {
      return;
    }

    const remembered = followStateRef.current;
    const desired = readContentEndScrollTop(scroller);
    /*
     * While the transcript is opening it is still hidden, so nothing is gained
     * by remembering an earlier offset: drop the memory and track the content
     * end exactly. Virtuoso writes `scrollTop` too during this window — it
     * compensates a history prepend from the item index before the prepended
     * heights reach the DOM — and any accommodation of that is both invisible
     * and, once paging stops, permanent. The gap tolerance is a *streaming*
     * allowance: blank below the live output is acceptable only because more
     * output is about to fill it.
     */
    const previous: TailFollowState = isOpeningViewport()
      ? { mode: remembered.mode, target: desired }
      : remembered;
    const pin = readPinScrollTop();
    const next = nextTailFollowState(previous, {
      desiredScrollTop: desired,
      pinScrollTop: pin,
      maxGapPx: tailHoldMaxGapPx(scroller.clientHeight),
    });
    followStateRef.current = next;
    if (next.mode === 'hold-tail') {
      clearPin();
    }
    // Content is still moving, so keep the settle window open.
    if (Math.abs(next.target - previous.target) > BOTTOM_EPSILON_PX) {
      settleFramesRef.current = SETTLE_FRAMES;
    }

    if (Math.abs(next.target - scroller.scrollTop) > BOTTOM_EPSILON_PX) {
      scroller.scrollTop = next.target;
    }
  }, [
    clearPin,
    isOpeningViewport,
    readContentEndScrollTop,
    readPinScrollTop,
    scrollerRef,
  ]);

  const runFollowFrame = useCallback(() => {
    followFrameRef.current = null;
    if (
      !isFollowingOutputRef.current ||
      !isViewportActiveRef.current ||
      document.hidden
    ) {
      return;
    }
    // Streaming holds the loop open indefinitely; anything else runs only
    // until the transcript stops moving.
    if (!isStreamingRef.current && settleFramesRef.current <= 0) {
      return;
    }
    if (!isStreamingRef.current) {
      settleFramesRef.current -= 1;
    }

    applyFollowTarget();
    followFrameRef.current = requestAnimationFrame(runFollowFrame);
  }, [applyFollowTarget]);

  const startFollowFrame = useCallback(() => {
    if (
      followFrameRef.current === null &&
      isFollowingOutputRef.current &&
      (isStreamingRef.current || settleFramesRef.current > 0)
    ) {
      followFrameRef.current = requestAnimationFrame(runFollowFrame);
    }
  }, [runFollowFrame]);

  const enterFollowOutput = useCallback((reason: FollowOutputEnterReason) => {
    if (!isViewportActiveRef.current) {
      return;
    }
    isFollowingOutputRef.current = true;
    setIsFollowingOutput(true);
    settleFramesRef.current = SETTLE_FRAMES;

    const scroller = scrollerRef.current;
    const contentEnd = scroller ? readContentEndScrollTop(scroller) : 0;
    const pinTurnId = reason === 'new-turn' ? latestTurnIdRef.current : null;


    // A newly submitted Turn opens at the viewport top; every other entry
    // reason resumes at the end of real content.
    if (pinTurnId && scrollTurnToTop(pinTurnId)) {
      pinTurnIdRef.current = pinTurnId;
      pinScrollTopRef.current = null;
      pinAttemptsRef.current = 0;
      followStateRef.current = { mode: 'pin-turn-top', target: scroller?.scrollTop ?? contentEnd };
    } else {
      clearPin();
      followStateRef.current = { mode: 'hold-tail', target: contentEnd };
      scrollToContentEnd(reason === 'jump-to-latest' ? 'smooth' : 'auto');
    }

    startFollowFrame();
  }, [
    clearPin,
    readContentEndScrollTop,
    scrollToContentEnd,
    scrollTurnToTop,
    scrollerRef,
    startFollowFrame,
  ]);

  const exitFollowOutput = useCallback((_reason: FollowOutputExitReason) => {
    isFollowingOutputRef.current = false;
    setIsFollowingOutput(false);
    clearPin();
    stopFollowFrame();
  }, [clearPin, stopFollowFrame]);

  /**
   * Re-assert ownership after a layout change. This deliberately does not force
   * the viewport to the content end: a tool-card collapse resizes the content
   * too, and the hold rule is what keeps that from dragging earlier content
   * down.
   */
  const scheduleFollowToLatest = useCallback(() => {
    if (!isFollowingOutputRef.current || !isViewportActiveRef.current) {
      return;
    }
    settleFramesRef.current = SETTLE_FRAMES;
    applyFollowTarget();
    startFollowFrame();
  }, [applyFollowTarget, startFollowFrame]);

  const handleUserScrollIntent = useCallback(() => {
    exitFollowOutput('user-scroll');
  }, [exitFollowOutput]);

  const handleScroll = useCallback(() => {
    // Scroll events describe the resulting viewport position, but do not prove user intent.
    // Layout growth and virtualizer remeasurement can emit them while output follow still owns
    // the viewport. Explicit wheel, touch, and keyboard handlers release that ownership instead.
  }, []);

  useEffect(() => {
    if (!hasMountedRef.current) {
      hasMountedRef.current = true;
      if (virtualItemCount > 0) {
        enterFollowOutput(isStreaming ? 'streaming-resumed' : 'session-open');
      }
      return;
    }

    if (previousSessionIdRef.current !== activeSessionId) {
      previousSessionIdRef.current = activeSessionId;
      previousLatestTurnIdRef.current = latestTurnId;
      exitFollowOutput('session-changed');
      if (virtualItemCount > 0) {
        enterFollowOutput(isStreaming ? 'streaming-resumed' : 'session-open');
      }
      return;
    }

    const isNewTurn = Boolean(latestTurnId && latestTurnId !== previousLatestTurnIdRef.current);
    previousLatestTurnIdRef.current = latestTurnId;
    if (isNewTurn && virtualItemCount > 0) {
      enterFollowOutput('new-turn');
    }
  }, [
    activeSessionId,
    enterFollowOutput,
    exitFollowOutput,
    isStreaming,
    latestTurnId,
    virtualItemCount,
  ]);

  useEffect(() => {
    if (!isViewportActive) {
      stopFollowFrame();
      return;
    }
    if (isFollowingOutput && isStreaming) {
      scheduleFollowToLatest();
    }
  }, [isFollowingOutput, isStreaming, isViewportActive, scheduleFollowToLatest, stopFollowFrame]);

  // Settle any blank the hold rule accumulated once output stops arriving.
  // A pinned Turn keeps its blank: that space is the mode, not a leftover.
  useEffect(() => {
    const wasStreaming = wasStreamingRef.current;
    wasStreamingRef.current = isStreaming;
    if (wasStreaming === isStreaming || isStreaming) {
      return;
    }
    if (!isFollowingOutputRef.current || followStateRef.current.mode !== 'hold-tail') {
      return;
    }

    const scroller = scrollerRef.current;
    if (!scroller) {
      return;
    }
    const contentEnd = readContentEndScrollTop(scroller);
    if (followStateRef.current.target - contentEnd > BOTTOM_EPSILON_PX) {
      followStateRef.current = { mode: 'hold-tail', target: contentEnd };
      scrollToContentEnd('smooth');
    }
  }, [isStreaming, readContentEndScrollTop, scrollToContentEnd, scrollerRef]);

  useEffect(() => {
    const handleVisibilityChange = () => {
      if (!document.hidden) {
        scheduleFollowToLatest();
      }
    };
    document.addEventListener('visibilitychange', handleVisibilityChange);
    return () => document.removeEventListener('visibilitychange', handleVisibilityChange);
  }, [scheduleFollowToLatest]);

  useEffect(() => stopFollowFrame, [stopFollowFrame]);

  return {
    isFollowingOutput,
    enterFollowOutput,
    exitFollowOutput,
    scheduleFollowToLatest,
    handleUserScrollIntent,
    handleScroll,
  };
}
