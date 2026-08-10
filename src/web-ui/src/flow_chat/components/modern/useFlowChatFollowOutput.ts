import { useCallback, useEffect, useRef, useState, type RefObject } from 'react';
import {
  contentEndScrollTop,
  FLOWCHAT_AT_CONTENT_END_THRESHOLD_PX,
  memorylessFollowState,
  nextTailFollowState,
  tailHoldMaxGapPx,
  tailSnapBackScrollTop,
  type TailFollowState,
} from './flowChatTailFollow';

export type FollowOutputEnterReason =
  | 'jump-to-latest'
  | 'new-turn'
  | 'session-open'
  | 'streaming-resumed'
  | 'tail-snap-back';
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

export interface ViewportResizeInput {
  /** Change in the scroller's client height; `0` on a reflow-only callback. */
  viewportHeightDeltaPx: number;
  /**
   * Whether the viewport was resting at the end of the transcript *before* the
   * resize. Afterwards that is unknowable — a viewport on the content end and
   * one parked deliberately above it are the same geometry.
   */
  wasAtTail: boolean;
}

interface UseFlowChatFollowOutputResult {
  isFollowingOutput: boolean;
  enterFollowOutput: (reason: FollowOutputEnterReason) => void;
  exitFollowOutput: (reason: FollowOutputExitReason) => void;
  scheduleFollowToLatest: () => void;
  handleUserScrollIntent: () => void;
  handleScroll: () => void;
  /** A scroll gesture has come to rest; snap out of the reserved blank if in it. */
  handleScrollSettled: () => void;
  /**
   * The viewport was resized; keep whatever was on the bottom edge there.
   */
  handleViewportResize: (input: ViewportResizeInput) => void;
  /** Offset the follow rule owns, or `null` when it does not own the viewport. */
  getFollowTargetScrollTop: () => number | null;
}

const BOTTOM_EPSILON_PX = 2;

/**
 * Frames a pinned Turn may stay unmeasurable before the pin is abandoned.
 * Virtuoso renders the tail immediately, so this only guards against a Turn
 * that never mounts at all.
 */
const PIN_RESOLVE_MAX_ATTEMPTS = 30;

/**
 * Frames a programmatic smooth scroll owns the viewport before the follow loop
 * resumes writing.
 *
 * The loop assigns `scrollTop` outright, which cancels an in-flight smooth
 * scroll on the very next frame — every `'smooth'` request in this hook was in
 * practice a jump. This is a frame budget rather than a flag so that a missing
 * completion signal costs a few idle frames instead of stalling follow.
 */
const SMOOTH_SCROLL_YIELD_FRAMES = 45;

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
  const smoothScrollFramesRef = useRef(0);
  const pendingSnapBackTargetRef = useRef<number | null>(null);

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

  /**
   * Forget which Turn is pinned.
   *
   * This is the pin's *identity*, not its activity. A user takeover only
   * suspends the pin — it must survive so that a snap back out of the reserved
   * blank returns a short new Turn to the viewport top instead of yanking it
   * into the middle. Only three things retire a pin: the crossover to
   * `hold-tail`, a newer Turn replacing it, and the session changing. The
   * crossover is one-way by construction, since nothing re-pins a Turn whose
   * identity has been dropped; a card collapse that pulls content back under one
   * viewport must not resurrect the pin.
   */
  const retirePin = useCallback(() => {
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
        retirePin();
      }
    }
    return pinScrollTopRef.current;
  }, [retirePin, resolveTurnTopScrollTop]);

  /**
   * Offset the follow rule would own for the current geometry, ignoring any
   * offset it was holding. Used to decide, and to aim, a snap back — both of
   * which happen while the viewport belongs to nobody.
   *
   * Retires a pin that has crossed over on the way past: with no frame loop
   * running, this is the only place that crossover can be noticed.
   */
  const resolveFollowTargetScrollTop = useCallback((scroller: HTMLElement) => {
    const desired = readContentEndScrollTop(scroller);
    const next = memorylessFollowState(
      pinTurnIdRef.current ? 'pin-turn-top' : 'hold-tail',
      {
        desiredScrollTop: desired,
        pinScrollTop: readPinScrollTop(),
        maxGapPx: tailHoldMaxGapPx(scroller.clientHeight),
      },
    );
    if (next.mode === 'hold-tail') {
      retirePin();
    }
    return next.target;
  }, [readContentEndScrollTop, readPinScrollTop, retirePin]);

  /**
   * Issue the one-shot content-end scroll, yielding the frame loop to it when
   * it is animated. Without the yield the next frame overwrites `scrollTop` and
   * the animation never plays.
   */
  const runContentEndScroll = useCallback((behavior: ScrollBehavior) => {
    smoothScrollFramesRef.current = behavior === 'smooth' ? SMOOTH_SCROLL_YIELD_FRAMES : 0;
    scrollToContentEnd(behavior);
  }, [scrollToContentEnd]);

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
      retirePin();
    }
    // Content is still moving, so keep the settle window open.
    if (Math.abs(next.target - previous.target) > BOTTOM_EPSILON_PX) {
      settleFramesRef.current = SETTLE_FRAMES;
    }

    const onTarget = Math.abs(next.target - scroller.scrollTop) <= BOTTOM_EPSILON_PX;
    if (smoothScrollFramesRef.current > 0) {
      // An animated scroll of ours is in flight and heading for this same
      // target. Track the state, but leave the writing to it.
      smoothScrollFramesRef.current = onTarget ? 0 : smoothScrollFramesRef.current - 1;
      return;
    }

    if (!onTarget) {
      scroller.scrollTop = next.target;
    }
  }, [
    isOpeningViewport,
    readContentEndScrollTop,
    readPinScrollTop,
    retirePin,
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

    // A snap back has already placed the viewport on the target it chose, and
    // that target was resolved under whichever mode still applies. Resume
    // ownership without a second move.
    if (reason === 'tail-snap-back') {
      followStateRef.current = {
        mode: pinTurnIdRef.current ? 'pin-turn-top' : 'hold-tail',
        target: scroller?.scrollTop ?? contentEnd,
      };
      startFollowFrame();
      return;
    }

    /*
     * A pin on the newest Turn already satisfies "show me the latest output".
     * The mode only holds while that Turn's answer is shorter than one
     * viewport, so everything it has produced is on screen; re-aiming at the
     * content end would scroll *up* and shove the message the user just sent
     * into the middle of the viewport.
     *
     * This fires for real: restoring the tail presentation asks for a jump to
     * latest one frame after the Turn that caused it got pinned, which
     * overwrote the pin every time.
     */
    if (
      reason === 'jump-to-latest' &&
      pinTurnIdRef.current !== null &&
      pinTurnIdRef.current === latestTurnIdRef.current
    ) {
      const pinTarget = readPinScrollTop() ?? scroller?.scrollTop ?? contentEnd;
      followStateRef.current = { mode: 'pin-turn-top', target: pinTarget };
      // Animated like every other jump to latest. The frame loop would cancel
      // the animation on its next tick, so hand it the same yield budget
      // `runContentEndScroll` uses.
      if (scroller && Math.abs(scroller.scrollTop - pinTarget) > BOTTOM_EPSILON_PX) {
        smoothScrollFramesRef.current = SMOOTH_SCROLL_YIELD_FRAMES;
        scroller.scrollTo({ top: pinTarget, behavior: 'smooth' });
      }
      startFollowFrame();
      return;
    }

    // A newly submitted Turn opens at the viewport top; every other entry
    // reason resumes at the end of real content.
    const pinTurnId = reason === 'new-turn' ? latestTurnIdRef.current : null;
    if (pinTurnId && scrollTurnToTop(pinTurnId)) {
      pinTurnIdRef.current = pinTurnId;
      pinScrollTopRef.current = null;
      pinAttemptsRef.current = 0;
      smoothScrollFramesRef.current = 0;
      followStateRef.current = { mode: 'pin-turn-top', target: scroller?.scrollTop ?? contentEnd };
    } else {
      retirePin();
      followStateRef.current = { mode: 'hold-tail', target: contentEnd };
      runContentEndScroll(reason === 'jump-to-latest' ? 'smooth' : 'auto');
    }

    startFollowFrame();
  }, [
    readContentEndScrollTop,
    readPinScrollTop,
    retirePin,
    runContentEndScroll,
    scrollTurnToTop,
    scrollerRef,
    startFollowFrame,
  ]);

  /**
   * Release the viewport without forgetting the pin. The user owns it from
   * here; the pin stays on record so a snap back can restore the mode rather
   * than fall through to the tail.
   */
  const exitFollowOutput = useCallback((_reason: FollowOutputExitReason) => {
    isFollowingOutputRef.current = false;
    setIsFollowingOutput(false);
    smoothScrollFramesRef.current = 0;
    pendingSnapBackTargetRef.current = null;
    stopFollowFrame();
  }, [stopFollowFrame]);

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
    pendingSnapBackTargetRef.current = null;
    exitFollowOutput('user-scroll');
  }, [exitFollowOutput]);

  const handleScroll = useCallback(() => {
    // Scroll events describe the resulting viewport position, but do not prove user intent.
    // Layout growth and virtualizer remeasurement can emit them while output follow still owns
    // the viewport. Explicit wheel, touch, and keyboard handlers release that ownership instead.
  }, []);

  /**
   * A scroll gesture has come to rest.
   *
   * The reserved tail spacer is a full viewport of blank that the user can park
   * in, and during slow streaming it can take a long time for output to push it
   * away — with nothing on screen and, until the viewport is back in the tail
   * band, no jump-to-latest affordance either. So resting below the follow
   * target snaps back to it and hands the viewport to follow, whether or not
   * follow owned it before.
   *
   * Acting on rest rather than on every scroll event is what keeps this from
   * fighting momentum: the correction runs after the gesture is over, never
   * during it.
   */
  const handleScrollSettled = useCallback(() => {
    const scroller = scrollerRef.current;
    if (!scroller || !isViewportActiveRef.current) {
      return;
    }

    const pendingTarget = pendingSnapBackTargetRef.current;
    if (pendingTarget !== null) {
      pendingSnapBackTargetRef.current = null;
      // Take the viewport back only if our own snap is what landed it here. A
      // gesture that overrode the animation mid-flight belongs to the user.
      if (Math.abs(scroller.scrollTop - pendingTarget) <= FLOWCHAT_AT_CONTENT_END_THRESHOLD_PX) {
        enterFollowOutput('tail-snap-back');
        return;
      }
    }

    if (isFollowingOutputRef.current || isOpeningViewport()) {
      return;
    }

    const snapTo = tailSnapBackScrollTop({
      scrollTop: scroller.scrollTop,
      followTargetScrollTop: resolveFollowTargetScrollTop(scroller),
      thresholdPx: FLOWCHAT_AT_CONTENT_END_THRESHOLD_PX,
    });
    if (snapTo === null) {
      return;
    }

    pendingSnapBackTargetRef.current = snapTo;
    scroller.scrollTo({ top: snapTo, behavior: 'smooth' });
  }, [
    enterFollowOutput,
    isOpeningViewport,
    resolveFollowTargetScrollTop,
    scrollerRef,
  ]);

  const getFollowTargetScrollTop = useCallback(() => (
    isFollowingOutputRef.current ? followStateRef.current.target : null
  ), []);

  /**
   * The viewport was resized. Keep whatever was on the bottom edge on the
   * bottom edge.
   *
   * A plain scroller preserves `scrollTop`, which anchors the *top* edge: the
   * bottom is where content gets revealed or swallowed. For a transcript that
   * is backwards — the interesting end is the bottom — so this anchors there
   * instead. Follow output already behaves this way for a viewport it owns;
   * this is the same rule for one it does not.
   *
   * The two halves are not equally capable, and the difference is worth
   * knowing:
   *
   * - **A height change moves no content.** Preserving `scrollTop +
   *   clientHeight` is therefore exact and needs no judgement about what the
   *   user was doing. It also preserves the distance to the content end, so a
   *   viewport that was at the end stays at the end for free. Growing the
   *   viewport is additionally a restoration: the browser used to clamp a
   *   bottom-anchored viewport at `scrollHeight - clientHeight`, and the
   *   resident spacer removed that clamp.
   * - **A width change reflows the transcript.** Where the line that was on the
   *   bottom edge went is a DOM question, and by the time a resize is observed
   *   the reflow has already happened — answering it would mean sampling an
   *   element anchor on the scroll path. The end of the transcript is the one
   *   position that can be recomputed from geometry, so that case is handled
   *   and the general one is not.
   *
   * Never animated. A height change moves the viewport by exactly the height
   * that was added or removed, so nothing appears to move at all; the rest is a
   * correction the user is already watching happen under their cursor.
   * Ownership does not change either: a gesture ending in the blank expresses
   * an intent to be at the end, a layout change expresses nothing.
   */
  const handleViewportResize = useCallback((input: ViewportResizeInput) => {
    const scroller = scrollerRef.current;
    if (!scroller || isFollowingOutputRef.current) {
      // Follow re-asserts its own target through `scheduleFollowToLatest`.
      return;
    }

    if (input.viewportHeightDeltaPx !== 0) {
      scroller.scrollTop = Math.max(0, scroller.scrollTop - input.viewportHeightDeltaPx);
    }

    const followTarget = resolveFollowTargetScrollTop(scroller);
    if (
      input.wasAtTail &&
      followTarget - scroller.scrollTop > FLOWCHAT_AT_CONTENT_END_THRESHOLD_PX
    ) {
      scroller.scrollTop = followTarget;
      return;
    }

    // Whatever left the viewport below the target — a reflow, or a gesture that
    // never settled — it must not stay there.
    const snapTo = tailSnapBackScrollTop({
      scrollTop: scroller.scrollTop,
      followTargetScrollTop: followTarget,
      thresholdPx: FLOWCHAT_AT_CONTENT_END_THRESHOLD_PX,
    });
    if (snapTo !== null) {
      scroller.scrollTop = snapTo;
    }
  }, [resolveFollowTargetScrollTop, scrollerRef]);

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
      retirePin();
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
    retirePin,
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
      runContentEndScroll('smooth');
    }
  }, [isStreaming, readContentEndScrollTop, runContentEndScroll, scrollerRef]);

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
    handleScrollSettled,
    handleViewportResize,
    getFollowTargetScrollTop,
  };
}
