/**
 * The reading position, kept where the reader left it.
 *
 * A virtualizer places items in the scroll range before it knows how tall they
 * are, so every late measurement rewrites the offset of everything below it.
 * react-virtuoso's own correction for that is a scroll by the change in *total*
 * list height, which assumes the change happened above the viewport, and it is
 * gated on scroll direction and on no programmatic scroll being in flight.
 * Measured on a 27-Turn session: one item measuring 38px -> 1003px produced a
 * 965px scroll across a whole Turn, and a 1680px growth that really was above
 * the viewport produced no correction at all — `scrollTop` held at 1133 while
 * `scrollHeight` went 8393 -> 10073, sliding the transcript down by the full
 * amount.
 *
 * So the correction cannot be delegated to whatever is doing the placing. This
 * restores a relationship instead of a delta, which makes it idempotent: when
 * the placement was right the correction is zero, and when it was wrong or
 * never came, this is what makes it right.
 *
 * It is also why capture and restore are separate: the scheme this replaced
 * captured once per prepend, in the commit that changed the item list, which is
 * a frame before the heights it was meant to compensate reach the DOM.
 * Measured corrections over three reproductions: +1, 0, 0.
 *
 * This hook talks to a scroller element and the DOM inside it, and to nothing
 * else. It is deliberately independent of the virtualizer.
 */

import { useCallback, useEffect, useMemo, useRef, type RefObject } from 'react';
import {
  roundViewportPx,
  traceViewport,
  traceViewportRepeating,
} from '@/infrastructure/diagnostics/flowChatViewportDiagnostics';
import {
  ANCHOR_CORRECTION_EPSILON_PX,
  ANCHOR_MISSING_TURN_ATTEMPTS,
  ANCHOR_SETTLE_FRAMES,
  findRenderedTurnAnchorElement,
  readTurnAnchorOffsetPx,
  readViewportAnchorCandidates,
  selectViewportAnchor,
  shouldCaptureViewportAnchorOnScroll,
  viewportAnchorCorrectionPx,
  type ViewportAnchor,
} from './flowChatViewportAnchor';

export interface UseFlowChatViewportAnchorOptions {
  scrollerRef: RefObject<HTMLElement | null>;
  /**
   * Someone is aiming the viewport at a target of their own, so the anchor must
   * stay quiet.
   *
   * The anchor judges by geometry alone, which cannot tell a movement made on
   * purpose from a displacement to undo — so it is told. The caller answers
   * from the viewport register; this hook only needs the answer, which is what
   * keeps it independent of everything that moves the viewport.
   *
   * Deliberately *not* true of the reader scrolling. A gesture chooses a
   * position in the transcript; it does not stop the transcript from moving
   * underneath them, and undoing that is all this hook does. The reader is also
   * re-anchored on every scroll of theirs, so a correction during a gesture is
   * zero unless something else really moved.
   */
  isViewportOwnedElsewhere: () => boolean;
  /**
   * Move the viewport *by* the correction. Supplied rather than performed here
   * so that the repair is registered like every other one, without this hook
   * having to know what else writes.
   */
  shiftViewport: (byPx: number) => void;
}

/**
 * What an attempt to put the reading position back actually did.
 *
 * The settle window refreshes on evidence that the settle is still running, and
 * only a frame that got as far as looking at the DOM has any. A boolean cannot
 * carry that: `false` is a stand-down, a Turn not rendered yet, and no anchor at
 * all, and the loop used to tell those apart by reading a counter that two of
 * the three never touch. Measured: 27 seconds of `anchor.stoodDown`, one per
 * frame, with the viewport parked and follow-output resting on it.
 */
type AnchorRestoreOutcome =
  | 'corrected'
  | 'in-place'
  | 'awaiting-turn'
  | 'stood-down'
  | 'no-anchor';

export interface FlowChatViewportAnchorApi {
  /** Anchor to wherever the viewport is now, unconditionally. */
  captureAnchor: () => void;
  /**
   * Anchor to wherever the viewport is now, if this scroll was the user's.
   *
   * A scroll event alone cannot say whether the user moved or the transcript
   * moved under them, so the qualifier is a recent intent event — the same
   * distinction follow-output draws.
   *
   * With one exception, and it is the case this hook exists for: while the
   * anchored Turn is missing from the rendered window a correction is owed and
   * cannot yet be measured, so the anchor is carried through the scroll rather
   * than replaced by it. Only the reader's own travel is credited.
   */
  captureAnchorForScroll: () => void;
  /** The user did something that scrolls, by their own hand. */
  markUserScrollIntent: () => void;
  /**
   * Put the anchored Turn back where it was. False when there was no anchor to
   * work from, which is the caller's signal to fall back on something coarser.
   *
   * Call where a correction can still beat the paint: a ResizeObserver
   * callback, which the browser delivers after layout and before painting, or
   * a layout effect.
   */
  restoreAnchor: () => boolean;
  /**
   * Correct now, and hold the anchor across the settle that follows.
   *
   * One callback per change is not enough, and no callback covers all of it, so
   * every signal that the transcript moved opens a window and the anchor is
   * re-asserted on each frame of it. A frame that had to correct refreshes the
   * window, so a settle that runs long is followed to its end rather than
   * abandoned partway.
   *
   * The first correction is synchronous because the caller is where the paint
   * can still be beaten. It is not always *able* to correct there — at a
   * history junction the anchored Turn is one frame away from being rendered —
   * but where it is, the displacement and its repair are one paint instead of
   * two.
   */
  openSettleWindow: () => void;
}

export function useFlowChatViewportAnchor({
  scrollerRef,
  isViewportOwnedElsewhere,
  shiftViewport,
}: UseFlowChatViewportAnchorOptions): FlowChatViewportAnchorApi {
  const shiftViewportRef = useRef(shiftViewport);
  shiftViewportRef.current = shiftViewport;
  const anchorRef = useRef<ViewportAnchor | null>(null);
  /** Consecutive attempts the anchored Turn has been missing from the window. */
  const missingTurnAttemptsRef = useRef(0);
  /**
   * The viewport position the anchor's offset was last agreed against.
   *
   * Only read while the anchored Turn is missing. It is what lets the reader go
   * on scrolling through a repair they are owed: a scroll they made is a change
   * to `scrollTop`, and crediting it to the anchor keeps the rest — the part
   * the transcript moved — outstanding.
   */
  const anchorScrollTopRef = useRef(0);
  /*
   * How long the anchored Turn has been out of the rendered window this time.
   *
   * The wait is the thing that decides whether a junction is seen. The
   * synchronous correction in the commit is the one that costs nothing — it
   * shares the paint with the displacement — and at a history junction it has
   * never once been able to run, because the virtualizer windows from a scroll
   * offset it only learns from scroll events and so renders the position the
   * reader has just been moved off. Every correction therefore falls to the
   * settle loop, and whether the reader sees it is a race: measured over five
   * junctions, four corrections shared the prepend's frame and one landed 27ms
   * into the next, visible as a 93px jump.
   *
   * `attempts` alone cannot answer how long the wait is. It counts restore
   * calls, its trace is coalesced so only the first is ever emitted, and it
   * says nothing about frames or milliseconds. These say what the wait cost, at
   * the one moment it is known: when it ends.
   */
  const missingSinceMsRef = useRef<number | null>(null);
  /** Settle frames that have run while the anchored Turn was missing. */
  const missingFramesRef = useRef(0);
  /** Reader travel credited to the anchor while it waited. */
  const carriedDuringWaitPxRef = useRef(0);
  /** The rAF timestamp of the frame a settle correction is running in, if any. */
  const correctionFrameStartMsRef = useRef<number | null>(null);
  /** When the user last did something that scrolls, by their own hand. */
  const lastUserScrollIntentAtRef = useRef(0);
  /** Frames left in which the anchor is still being re-asserted. */
  const settleFramesRef = useRef(0);
  const settleFrameRef = useRef<number | null>(null);
  /*
   * The settle loop is installed once and outlives every render, so it cannot
   * close over anything that changes identity between them.
   */
  const isViewportOwnedElsewhereRef = useRef(isViewportOwnedElsewhere);
  isViewportOwnedElsewhereRef.current = isViewportOwnedElsewhere;

  /**
   * Forget a wait, without reporting it. For the cases where the wait stopped
   * being about anything rather than ending in an answer.
   */
  const endMissingTurnWait = useCallback(() => {
    missingTurnAttemptsRef.current = 0;
    missingSinceMsRef.current = null;
    missingFramesRef.current = 0;
    carriedDuringWaitPxRef.current = 0;
  }, []);

  /**
   * What a wait for the anchored Turn cost, reported where it is known.
   *
   * Once per wait, at its end, because that is the only moment the whole of it
   * exists. `outcome` separates the Turn coming back from the anchor being
   * given up on: both end the wait, and only one of them can still repair
   * anything.
   */
  const reportMissingTurnWait = useCallback((
    scroller: HTMLElement,
    turnId: string,
    outcome: 'returned' | 'given-up',
  ) => {
    const startedAtMs = missingSinceMsRef.current;
    if (startedAtMs === null) return;
    /*
     * Read out here rather than inside the thunk. The wait ends on the next
     * line, and a `data` callback is evaluated by the trace, not by this — so
     * a thunk over the refs would report the state after the reset every time.
     */
    const waited = {
      turnId,
      outcome,
      waitedForMs: Math.round(performance.now() - startedAtMs),
      waitedFrames: missingFramesRef.current,
      waitedAttempts: missingTurnAttemptsRef.current,
      carriedPx: roundViewportPx(carriedDuringWaitPxRef.current),
      scrollTopPx: roundViewportPx(scroller.scrollTop),
      scrollRangePx: roundViewportPx(scroller.scrollHeight),
    };
    endMissingTurnWait();
    traceViewport({
      location: outcome === 'returned' ? 'anchor.turnReturned' : 'anchor.waitAbandoned',
      message: outcome === 'returned'
        ? 'the anchored Turn came back into the rendered window'
        : 'the anchored Turn never came back',
      data: () => waited,
    });
  }, [endMissingTurnWait]);

  /**
   * Where the reading position is taken from, and when.
   *
   * Traced because capture is the half of this hook that decides what every
   * correction afterwards will restore *to*, and it is otherwise invisible: a
   * correction that returns the reader to a position the transcript held for
   * one frame mid-settle looks exactly like a correction that worked. The
   * source matters as much as the value — a scroll event carries no proof of
   * whose scroll it was, and this hook's own writes produce them too.
   */
  const captureAnchorAt = useCallback((source: 'explicit' | 'scroll') => {
    const scroller = scrollerRef.current;
    if (!scroller) return;
    const previous = anchorRef.current;
    const next = selectViewportAnchor(readViewportAnchorCandidates(scroller));
    anchorRef.current = next;
    anchorScrollTopRef.current = scroller.scrollTop;
    // A new reading position is not the old one arriving, so whatever the old
    // one was waiting for stops being a fact about anything.
    endMissingTurnWait();
    traceViewportRepeating(`anchor|captured|${source}|${next?.turnId ?? 'none'}`, {
      location: 'anchor.captured',
      message: 'reading position taken',
      data: () => ({
        source,
        turnId: next?.turnId ?? null,
        offsetFromScrollerTopPx: next === null ? null : roundViewportPx(next.offsetFromScrollerTop),
        replacedTurnId: previous?.turnId ?? null,
        replacedOffsetPx: previous === null
          ? null
          : roundViewportPx(previous.offsetFromScrollerTop),
        scrollTopPx: roundViewportPx(scroller.scrollTop),
      }),
    });
  }, [endMissingTurnWait, scrollerRef]);

  const captureAnchor = useCallback(() => {
    captureAnchorAt('explicit');
  }, [captureAnchorAt]);

  const markUserScrollIntent = useCallback(() => {
    lastUserScrollIntentAtRef.current = performance.now();
  }, []);

  /**
   * Carry the anchor through a scroll instead of replacing it.
   *
   * Used only while the anchored Turn is missing from the rendered window,
   * which is to say while a correction is owed and cannot yet be measured. A
   * scroll then means two things at once — the reader moved, and the transcript
   * moved under them — and re-reading the anchor from the DOM accepts both. So
   * only the reader's half is credited: their travel is a change to
   * `scrollTop`, and it is subtracted from where the Turn is expected to be.
   * Whatever remains when the Turn renders is the displacement, unchanged by
   * however far they scrolled while waiting for it.
   *
   * Measured at three junctions in one session: the anchor was owed 104px, then
   * 140px, then an amount never established, and at each of them a scroll
   * 77ms later replaced it with a Turn at its displaced position. Two of the
   * three were never corrected at all.
   */
  const carryAnchorThroughScroll = useCallback((scroller: HTMLElement) => {
    const anchor = anchorRef.current;
    if (!anchor) return;
    const travelledPx = scroller.scrollTop - anchorScrollTopRef.current;
    anchorScrollTopRef.current = scroller.scrollTop;
    if (travelledPx === 0) return;
    carriedDuringWaitPxRef.current += travelledPx;
    anchorRef.current = {
      ...anchor,
      offsetFromScrollerTop: anchor.offsetFromScrollerTop - travelledPx,
    };
    traceViewportRepeating(`anchor|carried|${anchor.turnId}`, {
      location: 'anchor.carried',
      message: 'the reader scrolled while a correction was still owed',
      travelPx: travelledPx,
      data: () => ({
        turnId: anchor.turnId,
        travelledPx: roundViewportPx(travelledPx),
        expectedOffsetPx: roundViewportPx(anchor.offsetFromScrollerTop - travelledPx),
        missingAttempts: missingTurnAttemptsRef.current,
        scrollTopPx: roundViewportPx(scroller.scrollTop),
      }),
    });
  }, []);

  const captureAnchorForScroll = useCallback(() => {
    const scroller = scrollerRef.current;
    if (!scroller) return;
    if (missingTurnAttemptsRef.current > 0) {
      carryAnchorThroughScroll(scroller);
      return;
    }
    const captures = shouldCaptureViewportAnchorOnScroll(
      anchorRef.current !== null,
      performance.now() - lastUserScrollIntentAtRef.current,
    );
    if (captures) captureAnchorAt('scroll');
  }, [captureAnchorAt, carryAnchorThroughScroll, scrollerRef]);

  const attemptRestore = useCallback((): AnchorRestoreOutcome => {
    const scroller = scrollerRef.current;
    const anchor = anchorRef.current;
    if (!scroller || !anchor) return 'no-anchor';
    /*
     * Every way this declines is silent and looks like the transcript simply
     * not moving, which is why each of them is traced. Standing down for
     * another owner is the intended case; the other two are how a reading
     * position gets lost, and they have both happened.
     */
    if (isViewportOwnedElsewhereRef.current()) {
      traceViewportRepeating('anchor|owned-elsewhere', {
        location: 'anchor.stoodDown',
        message: 'anchor stood down for another owner',
        data: () => ({ turnId: anchor.turnId, scrollTopPx: roundViewportPx(scroller.scrollTop) }),
      });
      return 'stood-down';
    }
    const element = findRenderedTurnAnchorElement(scroller, anchor.turnId);
    /*
     * The anchored Turn can be absent from the rendered window, and one frame
     * of absence is not the same fact as the reader having left it behind.
     *
     * The single frame is the common case: the virtualizer chooses its window
     * from a scroll offset it only learns from scroll events, so the commit
     * that prepends history is still windowing the position the reader has just
     * been moved off, and their Turn arrives a frame later. Dropping there threw
     * away the only record of where they were reading, one frame before it
     * could have been used — measured at four junctions in a row, every one a
     * drop and not one correction.
     */
    if (!element) {
      if (missingSinceMsRef.current === null) missingSinceMsRef.current = performance.now();
      missingTurnAttemptsRef.current += 1;
      const givingUp = missingTurnAttemptsRef.current >= ANCHOR_MISSING_TURN_ATTEMPTS;
      traceViewportRepeating(`anchor|turn-not-rendered|${givingUp}`, {
        location: givingUp ? 'anchor.dropped' : 'anchor.turnNotRendered',
        message: givingUp
          ? 'anchored Turn stayed out of the rendered window, so the anchor was given up'
          : 'anchored Turn is not in the rendered window yet',
        data: () => ({
          turnId: anchor.turnId,
          attempts: missingTurnAttemptsRef.current,
          scrollTopPx: roundViewportPx(scroller.scrollTop),
          scrollRangePx: roundViewportPx(scroller.scrollHeight),
        }),
      });
      if (givingUp) {
        reportMissingTurnWait(scroller, anchor.turnId, 'given-up');
        anchorRef.current = null;
        return 'no-anchor';
      }
      return 'awaiting-turn';
    }
    // The wait is reported before the correction it made possible, so the two
    // read as cause and effect in the order they are written down.
    reportMissingTurnWait(scroller, anchor.turnId, 'returned');
    const correction = viewportAnchorCorrectionPx(
      anchor,
      readTurnAnchorOffsetPx(scroller, element),
    );
    // Already where it belongs, which still counts as answered.
    if (Math.abs(correction) < ANCHOR_CORRECTION_EPSILON_PX) {
      anchorScrollTopRef.current = scroller.scrollTop;
      return 'in-place';
    }
    traceViewportRepeating('anchor|correcting', {
      location: 'anchor.correct',
      message: 'anchor put the reading position back',
      travelPx: correction,
      data: () => ({
        turnId: anchor.turnId,
        correctionPx: roundViewportPx(correction),
        frameStartMs: correctionFrameStartMsRef.current === null
          ? null
          : Math.round(correctionFrameStartMsRef.current),
        scrollTopPx: roundViewportPx(scroller.scrollTop),
        /*
         * The scroll range, so a correction can be told apart from the reason
         * for it. A large correction means either that whoever moved the
         * viewport last over-shot, or that the transcript above the reader
         * measured smaller than it had reserved — and those differ by whether
         * the range changed since the change that opened this settle. Without
         * it, a 412px correction and a 412px mistake read the same.
         */
        scrollRangePx: roundViewportPx(scroller.scrollHeight),
      }),
    });
    shiftViewportRef.current(correction);
    anchorScrollTopRef.current = scroller.scrollTop;
    return 'corrected';
  }, [reportMissingTurnWait, scrollerRef]);

  const attemptRestoreRef = useRef(attemptRestore);
  attemptRestoreRef.current = attemptRestore;

  /*
   * The public answer is "did this leave the reading position where it belongs",
   * which the outcome refines rather than replaces. Callers outside the settle
   * loop only ever needed the two.
   */
  const restoreAnchor = useCallback((): boolean => {
    const outcome = attemptRestoreRef.current();
    return outcome === 'corrected' || outcome === 'in-place';
  }, []);

  const openSettleWindow = useCallback(() => {
    settleFramesRef.current = ANCHOR_SETTLE_FRAMES;
    /*
     * The viewport as of the change being settled, before anything the reader
     * does about it. Whoever moved the viewport on account of this change — the
     * prepend compensation, from its own layout effect just before this one —
     * has already written, and that write is not the reader scrolling.
     */
    const scroller = scrollerRef.current;
    if (scroller) anchorScrollTopRef.current = scroller.scrollTop;
    // In the caller's own frame, so the displacement and its correction are one
    // paint rather than two. Callers are a layout effect or a ResizeObserver
    // callback; both still run before the browser paints.
    attemptRestoreRef.current();
    if (settleFrameRef.current !== null) return;
    const step = (frameStartMs: number) => {
      settleFrameRef.current = null;
      settleFramesRef.current -= 1;
      /*
       * The frame this correction belongs to, which is the only way to tell a
       * repair the reader never saw from one they watched happen. A correction
       * whose frame started before the displacement's own frame was painted is
       * invisible; one a frame or more later is the flicker.
       */
      correctionFrameStartMsRef.current = frameStartMs;
      /*
       * A frame that answered refreshes the window — and so does one still
       * waiting for the anchored Turn to be rendered. "Not there yet" is
       * neither a repair nor a failure, and spending a frame on it makes the
       * settle outlast the wait only for as long as `ANCHOR_SETTLE_FRAMES` and
       * `ANCHOR_MISSING_TURN_ATTEMPTS` happen to be the same number. They are
       * independent constants describing different things; this says what was
       * meant instead of relying on them coinciding.
       */
      const outcome = attemptRestoreRef.current();
      // Counted here rather than beside `attempts`, because it is frames the
      // wait is measured in: a Turn that arrives in the next frame costs the
      // reader nothing, and one that takes five is five painted frames of
      // being in the wrong place. A frame that stood down is none of those —
      // it never looked, and counting it reported a 28-second wait for a
      // reading position that was correct the whole time.
      if (outcome === 'awaiting-turn') missingFramesRef.current += 1;
      /*
       * Only a frame that looked can say the settle is still running. A
       * stand-down looked at nothing — the owner it deferred to is placing the
       * viewport, and while that owner rests on it, as follow-output does at
       * the tail, nothing here will ever change. Refreshing on it kept the loop
       * alive off a missing-Turn count left by the last frame that *did* look,
       * and that count can only advance on a frame that does not stand down:
       * the one condition jammed the loop and made its only exit unreachable.
       */
      if (outcome === 'corrected' || outcome === 'in-place' || outcome === 'awaiting-turn') {
        settleFramesRef.current = ANCHOR_SETTLE_FRAMES;
      }
      correctionFrameStartMsRef.current = null;
      if (settleFramesRef.current > 0) {
        settleFrameRef.current = requestAnimationFrame(step);
      }
    };
    settleFrameRef.current = requestAnimationFrame(step);
  }, [scrollerRef]);

  useEffect(() => () => {
    if (settleFrameRef.current !== null) {
      cancelAnimationFrame(settleFrameRef.current);
      settleFrameRef.current = null;
    }
  }, []);

  return useMemo(() => ({
    captureAnchor,
    captureAnchorForScroll,
    markUserScrollIntent,
    restoreAnchor,
    openSettleWindow,
  }), [
    captureAnchor,
    captureAnchorForScroll,
    markUserScrollIntent,
    openSettleWindow,
    restoreAnchor,
  ]);
}
