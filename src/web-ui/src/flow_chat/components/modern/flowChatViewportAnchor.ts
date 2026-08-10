/**
 * Geometry for the element-anchored viewport keeper.
 *
 * The keeper restores a *relationship* — a Turn at an offset from the viewport
 * top — rather than replaying a scroll delta. That is what makes it idempotent:
 * applying it when nothing moved is a no-op, and applying it twice is the same
 * as applying it once. Every offset here is measured from the scroller's top
 * edge for the same reason. No absolute `scrollTop` survives a re-measure, but
 * the offset of a rendered Turn from the viewport top does.
 *
 * Nothing here knows which virtualizer placed the items, or that there is one.
 */

/**
 * DOM contract for the anchor element.
 *
 * A Turn's user message is the anchor: it is the one item every Turn has, it is
 * short enough that its own height rarely changes under a re-measure, and it is
 * what the reader is looking for when they scroll back through history.
 */
const TURN_ANCHOR_SELECTOR = '.virtual-item-wrapper[data-item-type="user-message"]';

/**
 * Where the user is reading, expressed as a Turn rather than an offset.
 *
 * `scrollTop` is not a stable description of a reading position in a list whose
 * item heights are discovered lazily: every late measurement rewrites the offset
 * of everything below it, so the same number means a different place.
 */
export interface ViewportAnchor {
  turnId: string;
  offsetFromScrollerTop: number;
}

export interface ViewportAnchorCandidate {
  turnId: string | null;
  offsetFromScrollerTop: number;
  bottomOffsetFromScrollerTop: number;
}

/**
 * How long after a wheel, touch, key, or scrollbar press the scrolling it
 * causes still counts as the user's.
 *
 * The anchor has to distinguish "the user went somewhere" from "the transcript
 * moved underneath them", and a scroll event says nothing about which. Intent
 * events do — but they fire *before* the scroll they cause, so the position
 * worth recording is the one that arrives shortly after, not the one at the
 * moment of the gesture. Recording at the gesture instead is what drags a
 * scrolling viewport backwards.
 *
 * Long enough to cover a wheel notch's smooth scroll and the gap between
 * notches of a continuous gesture; short enough that a mutation arriving in a
 * pause is not mistaken for the user.
 */
export const USER_DRIVEN_SCROLL_WINDOW_MS = 200;

/**
 * Frames the viewport anchor keeps being re-asserted after the transcript moves.
 *
 * A virtualizer settles a prepend over several frames — a margin holds the
 * position, then the real item heights land in padding, then the margin is
 * released — and a margin change fires no ResizeObserver at all. There is
 * therefore no single callback that can catch every step, so the anchor is
 * re-asserted for a window instead. Measured: four consecutive painted frames
 * displaced by 896px before the correction arrived, which is the flicker.
 *
 * Refreshed whenever a correction is actually needed, so a long settle keeps
 * the window open rather than running out halfway through it.
 */
export const ANCHOR_SETTLE_FRAMES = 20;

/** Below this a correction is rounding, not movement. */
export const ANCHOR_CORRECTION_EPSILON_PX = 0.5;

/**
 * The topmost Turn the viewport still shows any part of.
 *
 * A candidate that has not reached the top edge yet is the one the reader is
 * looking at, so the first one whose bottom is inside the viewport wins. If
 * that candidate carries no Turn there is no anchor at all — falling through to
 * the next one would anchor to a Turn further down the screen than the one the
 * reading position belongs to.
 */
export function selectViewportAnchor(
  candidates: readonly ViewportAnchorCandidate[],
): ViewportAnchor | null {
  const candidate = candidates.find(entry => entry.bottomOffsetFromScrollerTop > 0);
  if (!candidate?.turnId) return null;
  return {
    turnId: candidate.turnId,
    offsetFromScrollerTop: candidate.offsetFromScrollerTop,
  };
}

/**
 * How far the anchored Turn has drifted from where it was.
 *
 * Positive means it moved down the screen, which is also how much `scrollTop`
 * has to grow to put it back — the sign is the correction, not its opposite.
 */
export function viewportAnchorCorrectionPx(
  anchor: ViewportAnchor,
  currentOffsetFromScrollerTop: number,
): number {
  return currentOffsetFromScrollerTop - anchor.offsetFromScrollerTop;
}

/**
 * Whether a scroll event should move the anchor to where the viewport now is.
 *
 * Keying off "did the content height change" instead does not work: lazy
 * measurement changes it on almost every frame, so the anchor froze seconds in
 * the past and the keeper spent its corrections dragging a scrolling viewport
 * backwards. Measured: 1075 blocked captures against 8 accepted ones, and a
 * 1037px correction against the user's own gesture.
 *
 * Having no anchor overrides the window, which is also what bootstraps the very
 * first one.
 */
export function shouldCaptureViewportAnchorOnScroll(
  hasAnchor: boolean,
  msSinceUserScrollIntent: number,
): boolean {
  return !hasAnchor || msSinceUserScrollIntent <= USER_DRIVEN_SCROLL_WINDOW_MS;
}

export function readViewportAnchorCandidates(scroller: HTMLElement): ViewportAnchorCandidate[] {
  const scrollerTop = scroller.getBoundingClientRect().top;
  return Array.from(
    scroller.querySelectorAll<HTMLElement>(TURN_ANCHOR_SELECTOR),
  ).map(element => {
    const rect = element.getBoundingClientRect();
    return {
      turnId: element.dataset.turnId ?? null,
      offsetFromScrollerTop: rect.top - scrollerTop,
      bottomOffsetFromScrollerTop: rect.bottom - scrollerTop,
    };
  });
}

export function findRenderedTurnAnchorElement(
  scroller: HTMLElement | null,
  turnId: string,
): HTMLElement | null {
  return Array.from(
    scroller?.querySelectorAll<HTMLElement>(TURN_ANCHOR_SELECTOR) ?? [],
  ).find(element => element.dataset.turnId === turnId) ?? null;
}

export function readTurnAnchorOffsetPx(scroller: HTMLElement, element: HTMLElement): number {
  return element.getBoundingClientRect().top - scroller.getBoundingClientRect().top;
}
