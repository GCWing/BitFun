/**
 * FlowChat tail-follow geometry.
 *
 * The transcript keeps a resident tail spacer of roughly one viewport below the
 * real content. The spacer is static: its height follows the viewport and
 * nothing else. It never reacts to a measured content change, so it is a
 * reservation rather than a compensation.
 *
 * The spacer alone does not stabilise the viewport. It only removes the
 * browser's forced `scrollTop` clamp when content shrinks; the follow target
 * below is what then chooses to stay where it is instead of dragging earlier
 * content down. Both halves are required.
 */

/** Blank tail tolerated below the live output, as a share of the viewport. */
export const FLOWCHAT_TAIL_HOLD_GAP_RATIO = 0.6;

export type TailFollowMode = 'pin-turn-top' | 'hold-tail';

export interface TailFollowState {
  mode: TailFollowMode;
  /** Scroll offset the viewport currently owns. */
  target: number;
}

export interface TailFollowGeometry {
  scrollHeight: number;
  clientHeight: number;
  tailSpacerPx: number;
}

export interface TailFollowInput {
  /** Offset placing the end of real content at the viewport bottom. */
  desiredScrollTop: number;
  /** Offset placing the pinned Turn's user message at the viewport top. */
  pinScrollTop: number | null;
  /** Largest blank tail the hold rule accepts before it gives ground. */
  maxGapPx: number;
}

/** Height of the resident tail spacer for a given viewport. */
export function tailSpacerPxForViewport(clientHeight: number): number {
  return Math.max(0, Math.round(clientHeight));
}

/**
 * Scroll offset that places the end of real content at the viewport bottom.
 * The tail spacer sits below that point and is deliberately excluded.
 */
export function contentEndScrollTop(geometry: TailFollowGeometry): number {
  return Math.max(
    0,
    geometry.scrollHeight - geometry.tailSpacerPx - geometry.clientHeight,
  );
}

export function tailHoldMaxGapPx(clientHeight: number): number {
  return Math.max(0, Math.round(clientHeight * FLOWCHAT_TAIL_HOLD_GAP_RATIO));
}

/**
 * Resolve the next follow target.
 *
 * `pin-turn-top` holds a freshly submitted Turn at the viewport top while its
 * answer is still shorter than one viewport, then hands off to `hold-tail` at
 * the crossover. The pinned phase ignores `maxGapPx`: the blank below a new
 * Turn is the point of the mode, not a failure of it.
 *
 * `hold-tail` never moves backwards for free. It keeps its previous offset when
 * content shrinks, which is what leaves earlier content visually still, and
 * only gives ground once the blank below the live output exceeds `maxGapPx`.
 */
export function nextTailFollowState(
  previous: TailFollowState,
  input: TailFollowInput,
): TailFollowState {
  if (previous.mode === 'pin-turn-top') {
    if (input.pinScrollTop === null) {
      // The Turn is not measurable yet; behave like a plain tail follow so the
      // viewport is never stranded, and pin once the element resolves.
      return { mode: 'pin-turn-top', target: input.desiredScrollTop };
    }
    if (input.desiredScrollTop < input.pinScrollTop) {
      return { mode: 'pin-turn-top', target: input.pinScrollTop };
    }
    return { mode: 'hold-tail', target: input.desiredScrollTop };
  }

  return {
    mode: 'hold-tail',
    target: Math.max(
      input.desiredScrollTop,
      Math.min(previous.target, input.desiredScrollTop + input.maxGapPx),
    ),
  };
}
