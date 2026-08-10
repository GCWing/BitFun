import { useCallback, useEffect, useRef, useState, type RefObject } from 'react';
import {
  contentEndScrollTop,
  nextTailFollowState,
  tailHoldMaxGapPx,
  type TailFollowState,
} from './flowChatTailFollow';

export type FollowOutputEnterReason = 'jump-to-latest' | 'new-turn' | 'streaming-resumed';
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

    const next = nextTailFollowState(followStateRef.current, {
      desiredScrollTop: readContentEndScrollTop(scroller),
      pinScrollTop: readPinScrollTop(),
      maxGapPx: tailHoldMaxGapPx(scroller.clientHeight),
    });
    followStateRef.current = next;
    if (next.mode === 'hold-tail') {
      clearPin();
    }

    if (Math.abs(next.target - scroller.scrollTop) > BOTTOM_EPSILON_PX) {
      scroller.scrollTop = next.target;
    }
  }, [clearPin, readContentEndScrollTop, readPinScrollTop, scrollerRef]);

  const runFollowFrame = useCallback(() => {
    followFrameRef.current = null;
    if (
      !isFollowingOutputRef.current ||
      !isStreamingRef.current ||
      !isViewportActiveRef.current ||
      document.hidden
    ) {
      return;
    }

    applyFollowTarget();
    followFrameRef.current = requestAnimationFrame(runFollowFrame);
  }, [applyFollowTarget]);

  const startFollowFrame = useCallback(() => {
    if (followFrameRef.current === null && isFollowingOutputRef.current && isStreamingRef.current) {
      followFrameRef.current = requestAnimationFrame(runFollowFrame);
    }
  }, [runFollowFrame]);

  const enterFollowOutput = useCallback((reason: FollowOutputEnterReason) => {
    if (!isViewportActiveRef.current) {
      return;
    }
    isFollowingOutputRef.current = true;
    setIsFollowingOutput(true);

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
      if (isStreaming && virtualItemCount > 0) {
        enterFollowOutput('streaming-resumed');
      }
      return;
    }

    if (previousSessionIdRef.current !== activeSessionId) {
      previousSessionIdRef.current = activeSessionId;
      previousLatestTurnIdRef.current = latestTurnId;
      exitFollowOutput('session-changed');
      if (isStreaming && virtualItemCount > 0) {
        enterFollowOutput('streaming-resumed');
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
