import { useCallback, useEffect, useRef, useState, type RefObject } from 'react';
import {
  roundViewportPx,
  traceViewportRepeating,
} from '@/infrastructure/diagnostics/flowChatViewportDiagnostics';
import {
  isTailFollowDiagnosticsEnabled,
  noteTailFollowStep,
} from '@/infrastructure/diagnostics/flowChatTailFollowDiagnostics';
import type { FlowChatViewportOwnerApi } from './useFlowChatViewportOwner';
import { SNAP_BACK_HOLD_MS } from './flowChatViewportOwnership';
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
  | 'tail-snap-back'
  | 'turns-rolled-back';
export type FollowOutputExitReason =
  | 'session-changed'
  | 'user-scroll'
  | 'scroll-to-turn'
  | 'scroll-to-index';

interface UseFlowChatFollowOutputOptions {
  activeSessionId?: string;
  latestTurnId: string | null;
  /**
   * Turns in the session ledger, so that an arrival can be told from a
   * truncation. `latestTurnId` alone cannot: a rollback moves it backwards to a
   * Turn that has been there all along, which is a change and not an arrival.
   */
  dialogTurnCount: number;
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
  /**
   * Who is moving the viewport. Every write below goes through it, so that
   * nothing else has to carry a private opinion about when this hook is busy.
   */
  viewportOwner: FlowChatViewportOwnerApi;
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
  /** Whether follow-output owns the viewport now, not one render ago. */
  isFollowingOutputNow: () => boolean;
  handleUserScrollIntent: () => void;
  /** Turns were rolled back out of the session; end on the new tail. */
  handleTurnsRolledBack: () => void;
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
 * The virtualizer renders the tail immediately, so this only guards against a Turn
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
  dialogTurnCount,
  virtualItemCount,
  isStreaming,
  isViewportActive,
  scrollerRef,
  getTailSpacerPx,
  scrollToContentEnd,
  scrollTurnToTop,
  resolveTurnTopScrollTop,
  isOpeningViewport,
  viewportOwner,
}: UseFlowChatFollowOutputOptions): UseFlowChatFollowOutputResult {
  const [isFollowingOutput, setIsFollowingOutput] = useState(false);
  const isFollowingOutputRef = useRef(false);
  const isStreamingRef = useRef(isStreaming);
  const isViewportActiveRef = useRef(isViewportActive);
  const latestTurnIdRef = useRef(latestTurnId);
  const followFrameRef = useRef<number | null>(null);
  const previousSessionIdRef = useRef(activeSessionId);
  const previousLatestTurnIdRef = useRef<string | null>(latestTurnId);
  const previousDialogTurnCountRef = useRef(dialogTurnCount);
  const hasMountedRef = useRef(false);
  const wasStreamingRef = useRef(isStreaming);

  const followStateRef = useRef<TailFollowState>({ mode: 'hold-tail', target: 0 });
  const pinTurnIdRef = useRef<string | null>(null);
  const pinScrollTopRef = useRef<number | null>(null);
  const pinAttemptsRef = useRef(0);
  /**
   * A Turn that arrived while it was not in the transcript on screen.
   *
   * Submitting from inside a history window is the case: the session gains the
   * Turn a beat before the presentation is restored to the live tail, so the
   * one moment the arrival is *detectable* is not a moment it can be answered.
   * The answer is deferred rather than dropped — kept until the Turn can
   * actually be aligned, which is what the reader is waiting to see.
   */
  const pendingNewTurnIdRef = useRef<string | null>(null);
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
   * caching it once. Unrendered items are estimates until measured, and every
   * measurement shifts the absolute offsets below it; a cached pin would drift.
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
     * end exactly. The virtualizer writes `scrollTop` too during this window — it
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
      const fromPx = scroller.scrollTop;
      viewportOwner.write({ owner: 'follow-output', topPx: next.target });
      if (isTailFollowDiagnosticsEnabled()) {
        noteTailFollowStep('list', {
          /*
           * Read back rather than taken from the target. The register can
           * refuse this write outright, and a refused follow is a step of zero
           * — believing the target there would report a follow that is being
           * outranked as the smoothest one in the session.
           */
          stepPx: scroller.scrollTop - fromPx,
          lagPx: next.target - fromPx,
          innerScroll: true,
          snapped: true,
        });
      }
    }
  }, [
    isOpeningViewport,
    readContentEndScrollTop,
    readPinScrollTop,
    retirePin,
    scrollerRef,
    viewportOwner,
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

    /*
     * A newly submitted Turn opens at the viewport top, and that is the whole
     * of the answer to one arriving. Until it is in the transcript on screen
     * there is nothing to align, and the fallback below — the end of real
     * content — is not a stand-in for it: it would leave the Turn unpinned
     * where the reader was, or pull them out of a history window entirely.
     *
     * So the answer waits for the Turn instead. Resolved before ownership
     * changes hands, because waiting has to leave the viewport exactly as it
     * was found.
     */
    const pinTurnId = reason === 'new-turn' ? latestTurnIdRef.current : null;
    const pinnedTurnToTop = pinTurnId !== null && scrollTurnToTop(pinTurnId);
    if (reason === 'new-turn') {
      pendingNewTurnIdRef.current = pinnedTurnToTop ? null : pinTurnId;
      if (!pinnedTurnToTop) {
        /*
         * The viewport is deliberately left exactly as it was, so the only
         * evidence that a submission was answered at all is this line. A
         * deferral with no `followOutput.enter` after it is the reader's
         * "nothing happened when I sent a message".
         */
        traceViewportRepeating('follow|deferred-new-turn', {
          location: 'followOutput.deferNewTurn',
          message: 'new Turn is not in the transcript on screen yet, so the answer waits',
          data: () => ({ turnId: pinTurnId }),
        });
        return;
      }
    }

    isFollowingOutputRef.current = true;
    setIsFollowingOutput(true);
    settleFramesRef.current = SETTLE_FRAMES;
    traceViewportRepeating(`follow|enter|${reason}`, {
      location: 'followOutput.enter',
      message: 'follow-output took the viewport',
      data: () => ({
        reason,
        pinnedTurnId: pinnedTurnToTop ? pinTurnId : pinTurnIdRef.current,
        isStreaming: isStreamingRef.current,
        scrollTopPx: roundViewportPx(scrollerRef.current?.scrollTop ?? 0),
      }),
    });
    /*
     * Ownership is taken here rather than at each write, because following is
     * continuous: between two frames of the loop the viewport is still ours,
     * and a correction slipping into that gap is the thing this prevents. A
     * refused claim is not a reason to stop following — the loop keeps its
     * state and simply writes nothing until whoever outranks it is done.
     */
    viewportOwner.claim('follow-output');

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
        viewportOwner.write({
          owner: 'follow-output',
          topPx: pinTarget,
          behavior: 'smooth',
        });
      }
      startFollowFrame();
      return;
    }

    // Every other entry reason resumes at the end of real content.
    if (pinnedTurnToTop) {
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
    viewportOwner,
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
  const exitFollowOutput = useCallback((reason: FollowOutputExitReason) => {
    if (isFollowingOutputRef.current) {
      traceViewportRepeating(`follow|exit|${reason}`, {
        location: 'followOutput.exit',
        message: 'follow-output gave the viewport up',
        data: () => ({
          reason,
          pinnedTurnId: pinTurnIdRef.current,
          scrollTopPx: roundViewportPx(scrollerRef.current?.scrollTop ?? 0),
        }),
      });
    }
    isFollowingOutputRef.current = false;
    setIsFollowingOutput(false);
    smoothScrollFramesRef.current = 0;
    pendingSnapBackTargetRef.current = null;
    viewportOwner.release('follow-output');
    viewportOwner.release('snap-back');
    stopFollowFrame();
  }, [scrollerRef, stopFollowFrame, viewportOwner]);

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

  /**
   * Turns were rolled back out of the session.
   *
   * The ledger cannot say this on its own — a shorter `dialogTurns` is also
   * what a window re-cut and a hydration merge look like — so the rollback
   * announces it, the same way a submission does. What it asks for is the
   * *absence* of the pin: the Turn that was pinned is one of the ones that just
   * stopped existing, and the transcript now ends somewhere else.
   *
   * This takes the viewport whether or not follow owned it, which is the same
   * licence the snap back has and rests on the same asymmetry. A rollback at
   * Turn N removes N and everything after it, and the reader had N on screen —
   * they clicked its own button. So the new tail is always within a Turn of
   * where they already are, and there is no history below them to be pulled out
   * of. Gating this on ownership instead made it dead code in the case it was
   * written for: reaching a Turn far enough up to want it gone means scrolling,
   * and scrolling is exactly what hands the viewport back to the reader.
   *
   * Without it the viewport anchor answers instead, and answers the wrong
   * question — it holds the reader's Turn at its offset from the viewport top,
   * so an 8-Turn session rolled back at Turn 7 came to rest showing Turns 2..6
   * with the new last Turn's answer below the fold.
   */
  const handleTurnsRolledBack = useCallback(() => {
    pendingNewTurnIdRef.current = null;
    enterFollowOutput('turns-rolled-back');
  }, [enterFollowOutput]);

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
      // The animation is over either way, so the hold ends here rather than
      // waiting out its backstop.
      viewportOwner.release('snap-back');
      // Take the viewport back only if our own snap is what landed it here. A
      // gesture that overrode the animation mid-flight belongs to the user.
      const arrived = Math.abs(scroller.scrollTop - pendingTarget)
        <= FLOWCHAT_AT_CONTENT_END_THRESHOLD_PX;
      traceViewportRepeating(`snapBack|settled|${arrived}`, {
        location: 'snapBack.settled',
        message: arrived
          ? 'snap back arrived and handed the viewport to follow'
          : 'snap back was overridden before it arrived',
        data: () => ({
          targetPx: roundViewportPx(pendingTarget),
          scrollTopPx: roundViewportPx(scroller.scrollTop),
        }),
      });
      if (arrived) {
        enterFollowOutput('tail-snap-back');
        return;
      }
    }

    /*
     * Ownership is not correction, and the difference is deliberate: the frame
     * loop stops once the settle budget runs out, but ownership survives so
     * that streaming can resume following. Reading ownership as "something is
     * correcting this" hands the viewport to a loop that is not running —
     * measured, a drag came to rest 813px into the reserved blank with
     * `isFollowingOutput` true, the loop asleep, and nothing to bring it back.
     *
     * A live loop still gets the viewport to itself. It reaches its target in a
     * single frame, so a snap back would only be racing it.
     */
    const isFollowCorrectingViewport = isFollowingOutputRef.current
      && followFrameRef.current !== null;
    if (isFollowCorrectingViewport || isOpeningViewport()) {
      // "The wheel went down and nothing brought me back" is this line or the
      // one below it, and they are not the same fault.
      traceViewportRepeating(
        `snapBack|declined|${isFollowCorrectingViewport ? 'follow-correcting' : 'opening'}`,
        {
          location: 'snapBack.declined',
          message: 'gesture came to rest, but the snap back is not this settle\'s business',
          data: () => ({
            reason: isFollowCorrectingViewport ? 'follow-correcting' : 'opening-reveal',
            scrollTopPx: roundViewportPx(scroller.scrollTop),
          }),
        },
      );
      return;
    }

    const followTarget = resolveFollowTargetScrollTop(scroller);
    const snapTo = tailSnapBackScrollTop({
      scrollTop: scroller.scrollTop,
      followTargetScrollTop: followTarget,
      thresholdPx: FLOWCHAT_AT_CONTENT_END_THRESHOLD_PX,
    });
    if (snapTo === null) {
      traceViewportRepeating('snapBack|not-in-blank', {
        location: 'snapBack.notNeeded',
        message: 'gesture came to rest above the follow target, so nothing to snap back from',
        data: () => ({
          scrollTopPx: roundViewportPx(scroller.scrollTop),
          followTargetPx: roundViewportPx(followTarget),
        }),
      });
      return;
    }

    /*
     * Held for the animation, not just the write. Ownership passing to
     * follow-output only on landing is what left the snap belonging to nobody
     * while it travelled, and the anchor undid its first 0.7px 958 times.
     */
    const issued = viewportOwner.write({
      owner: 'snap-back',
      topPx: snapTo,
      behavior: 'smooth',
      holdForMs: SNAP_BACK_HOLD_MS,
    });
    traceViewportRepeating(`snapBack|issued|${issued}`, {
      location: 'snapBack.issued',
      message: issued ? 'snap back is on its way' : 'snap back was refused the viewport',
      data: () => ({
        fromPx: roundViewportPx(scroller.scrollTop),
        targetPx: roundViewportPx(snapTo),
        followTargetPx: roundViewportPx(followTarget),
      }),
    });
    if (issued) {
      pendingSnapBackTargetRef.current = snapTo;
    }
  }, [
    viewportOwner,
    enterFollowOutput,
    isOpeningViewport,
    resolveFollowTargetScrollTop,
    scrollerRef,
  ]);

  const getFollowTargetScrollTop = useCallback(() => (
    isFollowingOutputRef.current ? followStateRef.current.target : null
  ), []);

  /**
   * Whether follow-output owns the viewport *now*.
   *
   * The `isFollowingOutput` state answers the same question one render later,
   * which is a different answer inside an event handler that just released it.
   * A reader's gesture releases ownership and then asks whether the boundary
   * they are on is worth paging: mirroring the state into a ref at render time
   * made that ask see the ownership it had itself just ended, and refuse.
   */
  const isFollowingOutputNow = useCallback(() => isFollowingOutputRef.current, []);

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

    traceViewportRepeating(`resize|${input.wasAtTail}|${input.viewportHeightDeltaPx !== 0}`, {
      location: 'followOutput.viewportResize',
      message: 'the scroller box changed under a viewport nobody was following',
      travelPx: input.viewportHeightDeltaPx,
      data: () => ({
        viewportHeightDeltaPx: roundViewportPx(input.viewportHeightDeltaPx),
        wasAtTail: input.wasAtTail,
        scrollTopPx: roundViewportPx(scroller.scrollTop),
        clientHeightPx: scroller.clientHeight,
      }),
    });

    if (input.viewportHeightDeltaPx !== 0) {
      viewportOwner.write({
        owner: 'layout-correction',
        topPx: Math.max(0, scroller.scrollTop - input.viewportHeightDeltaPx),
      });
    }

    const followTarget = resolveFollowTargetScrollTop(scroller);
    if (
      input.wasAtTail &&
      followTarget - scroller.scrollTop > FLOWCHAT_AT_CONTENT_END_THRESHOLD_PX
    ) {
      viewportOwner.write({ owner: 'layout-correction', topPx: followTarget });
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
      viewportOwner.write({ owner: 'layout-correction', topPx: snapTo });
    }
  }, [resolveFollowTargetScrollTop, scrollerRef, viewportOwner]);

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
      previousDialogTurnCountRef.current = dialogTurnCount;
      exitFollowOutput('session-changed');
      retirePin();
      // A Turn waiting to be shown belongs to the session that gained it.
      pendingNewTurnIdRef.current = null;
      if (virtualItemCount > 0) {
        enterFollowOutput(isStreaming ? 'streaming-resumed' : 'session-open');
      }
      return;
    }

    /*
     * An arrival, not a change. `latestTurnId` is the ledger's last Turn and it
     * is the right identity — but a rollback truncates the ledger, which moves
     * that identity *backwards* onto a Turn that has been there all along. Read
     * as an arrival it pinned the survivor to the viewport top, which is the
     * reader's "I undid my message and it jumped to the one before it".
     *
     * The ledger growing is what separates the two. Nothing else that rewrites
     * `dialogTurns` — a history page merging in above, a window re-cut, a
     * hydration — moves the last Turn, so requiring growth costs nothing and
     * excludes every truncation.
     */
    const previousDialogTurnCount = previousDialogTurnCountRef.current;
    previousDialogTurnCountRef.current = dialogTurnCount;
    const isNewTurn = Boolean(
      latestTurnId
      && latestTurnId !== previousLatestTurnIdRef.current
      && dialogTurnCount > previousDialogTurnCount,
    );
    previousLatestTurnIdRef.current = latestTurnId;
    if (virtualItemCount === 0) {
      return;
    }
    if (isNewTurn) {
      enterFollowOutput('new-turn');
      return;
    }
    /*
     * The transcript changed without a new Turn, which is the moment a deferred
     * one can become alignable — the presentation being restored to the live
     * tail is exactly that. A retry that still cannot align it leaves the
     * viewport alone and stays pending.
     */
    if (pendingNewTurnIdRef.current === latestTurnId && latestTurnId !== null) {
      enterFollowOutput('new-turn');
    }
  }, [
    activeSessionId,
    dialogTurnCount,
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
    isFollowingOutputNow,
    handleUserScrollIntent,
    handleTurnsRolledBack,
    handleScroll,
    handleScrollSettled,
    handleViewportResize,
    getFollowTargetScrollTop,
  };
}
