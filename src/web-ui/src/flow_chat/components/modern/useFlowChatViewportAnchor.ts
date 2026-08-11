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
  traceViewportRepeating,
} from '@/infrastructure/diagnostics/flowChatViewportDiagnostics';
import {
  ANCHOR_CORRECTION_EPSILON_PX,
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
   * Another writer owns the viewport right now, so the anchor must stay quiet.
   *
   * The anchor judges by geometry alone, which cannot tell a movement made on
   * purpose from a displacement to undo — so it is told. The caller answers
   * from the viewport register; this hook only needs the answer, which is what
   * keeps it independent of everything that moves the viewport.
   */
  isViewportOwnedElsewhere: () => boolean;
  /**
   * Move the viewport. Supplied rather than performed here so that the
   * correction is registered like every other deliberate write, without this
   * hook having to know what else writes.
   */
  writeViewport: (topPx: number) => void;
}

export interface FlowChatViewportAnchorApi {
  /** Anchor to wherever the viewport is now, unconditionally. */
  captureAnchor: () => void;
  /**
   * Anchor to wherever the viewport is now, if this scroll was the user's.
   *
   * A scroll event alone cannot say whether the user moved or the transcript
   * moved under them, so the qualifier is a recent intent event — the same
   * distinction follow-output draws.
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
   * Hold the anchor across a settle.
   *
   * One callback per change is not enough, and no callback covers all of it, so
   * every signal that the transcript moved opens a window and the anchor is
   * re-asserted on each frame of it. A frame that had to correct refreshes the
   * window, so a settle that runs long is followed to its end rather than
   * abandoned partway.
   */
  openSettleWindow: () => void;
}

export function useFlowChatViewportAnchor({
  scrollerRef,
  isViewportOwnedElsewhere,
  writeViewport,
}: UseFlowChatViewportAnchorOptions): FlowChatViewportAnchorApi {
  const writeViewportRef = useRef(writeViewport);
  writeViewportRef.current = writeViewport;
  const anchorRef = useRef<ViewportAnchor | null>(null);
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

  const captureAnchor = useCallback(() => {
    const scroller = scrollerRef.current;
    if (!scroller) return;
    anchorRef.current = selectViewportAnchor(readViewportAnchorCandidates(scroller));
  }, [scrollerRef]);

  const markUserScrollIntent = useCallback(() => {
    lastUserScrollIntentAtRef.current = performance.now();
  }, []);

  const captureAnchorForScroll = useCallback(() => {
    const captures = shouldCaptureViewportAnchorOnScroll(
      anchorRef.current !== null,
      performance.now() - lastUserScrollIntentAtRef.current,
    );
    if (captures) captureAnchor();
  }, [captureAnchor]);

  const restoreAnchor = useCallback((): boolean => {
    const scroller = scrollerRef.current;
    const anchor = anchorRef.current;
    if (!scroller || !anchor) return false;
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
      return false;
    }
    const element = findRenderedTurnAnchorElement(scroller, anchor.turnId);
    // The anchored Turn can leave the rendered window entirely. Nothing can be
    // restored from it then, so drop it and let the next scroll pick a Turn
    // that is actually on screen.
    if (!element) {
      anchorRef.current = null;
      traceViewportRepeating('anchor|turn-gone', {
        location: 'anchor.dropped',
        message: 'anchored Turn left the rendered window, so nothing was restored',
        data: () => ({ turnId: anchor.turnId, scrollTopPx: roundViewportPx(scroller.scrollTop) }),
      });
      return false;
    }
    const correction = viewportAnchorCorrectionPx(
      anchor,
      readTurnAnchorOffsetPx(scroller, element),
    );
    // Already where it belongs, which still counts as answered.
    if (Math.abs(correction) < ANCHOR_CORRECTION_EPSILON_PX) return true;
    traceViewportRepeating('anchor|correcting', {
      location: 'anchor.correct',
      message: 'anchor put the reading position back',
      travelPx: correction,
      data: () => ({
        turnId: anchor.turnId,
        correctionPx: roundViewportPx(correction),
        scrollTopPx: roundViewportPx(scroller.scrollTop),
      }),
    });
    writeViewportRef.current(scroller.scrollTop + correction);
    return true;
  }, [scrollerRef]);

  const restoreAnchorRef = useRef(restoreAnchor);
  restoreAnchorRef.current = restoreAnchor;

  const openSettleWindow = useCallback(() => {
    settleFramesRef.current = ANCHOR_SETTLE_FRAMES;
    if (settleFrameRef.current !== null) return;
    const step = () => {
      settleFrameRef.current = null;
      settleFramesRef.current -= 1;
      if (restoreAnchorRef.current()) {
        settleFramesRef.current = ANCHOR_SETTLE_FRAMES;
      }
      if (settleFramesRef.current > 0) {
        settleFrameRef.current = requestAnimationFrame(step);
      }
    };
    settleFrameRef.current = requestAnimationFrame(step);
  }, []);

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
