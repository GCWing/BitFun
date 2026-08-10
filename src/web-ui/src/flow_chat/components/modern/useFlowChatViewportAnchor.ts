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
   * Follow-output writes its own target every frame, and an opening reveal is
   * still walking the viewport down to the tail. Neither wants a second opinion
   * about where the viewport belongs.
   */
  isViewportOwnedElsewhere: () => boolean;
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
}: UseFlowChatViewportAnchorOptions): FlowChatViewportAnchorApi {
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
    if (!scroller || !anchor || isViewportOwnedElsewhereRef.current()) return false;
    const element = findRenderedTurnAnchorElement(scroller, anchor.turnId);
    // The anchored Turn can leave the rendered window entirely. Nothing can be
    // restored from it then, so drop it and let the next scroll pick a Turn
    // that is actually on screen.
    if (!element) {
      anchorRef.current = null;
      return false;
    }
    const correction = viewportAnchorCorrectionPx(
      anchor,
      readTurnAnchorOffsetPx(scroller, element),
    );
    // Already where it belongs, which still counts as answered.
    if (Math.abs(correction) < ANCHOR_CORRECTION_EPSILON_PX) return true;
    scroller.scrollTop += correction;
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
