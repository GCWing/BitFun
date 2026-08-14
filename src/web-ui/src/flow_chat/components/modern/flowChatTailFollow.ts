/**
 * FlowChat tail-follow geometry.
 *
 * The transcript keeps a resident tail spacer below the real content, sized
 * from the viewport and the current input-stack inset. The spacer is static: it
 * never reacts to a measured content change, so it is a reservation rather than
 * a compensation.
 *
 * The spacer alone does not stabilise the viewport. It only removes the
 * browser's forced `scrollTop` clamp when content shrinks; the follow target
 * below is what then chooses to stay where it is instead of dragging earlier
 * content down. Both halves are required.
 */

/** Blank tail tolerated below the live output, as a share of the viewport. */
export const FLOWCHAT_TAIL_HOLD_GAP_RATIO = 0.6;

/**
 * Distance from an owned offset still treated as being on it.
 *
 * One constant deliberately serves both "the viewport is at the end" and "the
 * viewport has come to rest too far below the end". They are the two sides of
 * the same band; separate tolerances would leave a seam where the jump-to-latest
 * affordance is shown but nothing snaps back, or the reverse.
 */
export const FLOWCHAT_AT_CONTENT_END_THRESHOLD_PX = 50;

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

/**
 * Breathing gap held above a Turn whose user message is aligned to the viewport
 * top.
 *
 * The first Turn gets this for free: `.message-list-header` sits above it in
 * the scroll content, so a transcript resting at `scrollTop: 0` already shows
 * the gap. Every other Turn is reached by a scroll that lands its message flat
 * on the top edge, which read as a different alignment rather than the same one
 * — so top-aligning aims at `top - this` instead, and the header renders at
 * exactly this height so the two cannot drift apart.
 */
export const FLOWCHAT_TURN_TOP_GAP_PX = 8;

/**
 * Lower bound on the rendered height of a user-message item.
 *
 * This sizes the pin reserve below, so it must be an *under*estimate. Too low
 * only leaves the pin a few spare pixels; too high makes the pinned offset
 * exceed the scroll range, and the browser clamps the Turn back down from the
 * viewport top with the follow loop rewriting the clamped offset every frame.
 * A single line with no timestamp row is comfortably above this at any
 * supported font size, and the item is free to be taller.
 */
const PINNED_TURN_MIN_ITEM_HEIGHT_PX = 40;

/**
 * Height of the resident tail spacer.
 *
 * The spacer exists to keep two offsets inside the scroll range, and it is
 * sized to the larger of what they need — one viewport was simply the cheapest
 * bound that covered both, and it costs a full screen of blank at the end of
 * the scroll range.
 *
 * - **A pinned Turn.** Worst case its user message is the newest item and
 *   nothing has answered it yet, so everything below the message top is the
 *   message, the input-stack inset, and this spacer. Reserving
 *   `clientHeight - bottomInsetPx - PINNED_TURN_MIN_ITEM_HEIGHT_PX` is exactly
 *   enough to put that message on the top edge.
 * - **A held collapse gap.** `hold-tail` keeps an offset up to
 *   `tailHoldMaxGapPx` past the content end, and an offset the browser clamps
 *   is an offset the hold rule does not actually get to hold.
 *
 * `bottomInsetPx` is the input-stack footer, which grows as the composer does.
 * That is a layout input, not a content measurement — and while the pin reserve
 * is the binding bound the two sum to a constant, so growing the composer moves
 * the content end without moving the end of the scroll range.
 */
export function tailSpacerPxForViewport(
  clientHeight: number,
  bottomInsetPx: number,
): number {
  const pinnedTurnReservePx = clientHeight - bottomInsetPx - PINNED_TURN_MIN_ITEM_HEIGHT_PX;
  return Math.max(0, Math.round(Math.max(pinnedTurnReservePx, tailHoldMaxGapPx(clientHeight))));
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

export interface TurnTopAlignmentInput {
  /** Offset that would place the Turn's user message at the viewport top. */
  turnTopScrollTop: number;
  contentEndScrollTop: number;
}

/**
 * Whether top-aligning a Turn would park the viewport in the reserved blank.
 *
 * The blank belongs to follow-output. `pin-turn-top` holds it for output that
 * is on its way, and nothing is on its way under a Turn the user navigated to
 * — so a navigation that lands there shows a screen of nothing, and the snap
 * back then reclaims it as a second, visible movement.
 *
 * A clamp at the content end is the whole rule. There is no "is this the last
 * Turn" test and no measurement of what lies below it: a Turn with a viewport
 * of content under it has its top above the content end already, so this
 * returns false and the alignment stands. Before the resident spacer the
 * browser did exactly this for free, by clamping at the end of the scroll
 * range; reserving the blank is what removed the clamp.
 */
export function turnTopAlignmentEntersReservedBlank(
  input: TurnTopAlignmentInput,
): boolean {
  return input.turnTopScrollTop > input.contentEndScrollTop;
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

/**
 * The state the follow rule would hold right now, judged from live geometry
 * alone.
 *
 * `hold-tail` normally refuses to move backwards, and that refusal is what
 * keeps a collapse from dragging earlier content down. The memory it refuses
 * with belongs to a viewport the follow rule has been holding continuously;
 * once the user has taken over and come to rest somewhere else there is nothing
 * left to preserve, and carrying the stale offset forward would land them on a
 * position neither side chose.
 *
 * The returned mode still matters: a pinned Turn whose answer has outgrown the
 * viewport reports `hold-tail`, which is how a caller learns the pin has
 * crossed over even though no follow loop was running to notice.
 */
export function memorylessFollowState(
  mode: TailFollowMode,
  input: TailFollowInput,
): TailFollowState {
  return nextTailFollowState({ mode, target: input.desiredScrollTop }, input);
}

export interface TailSnapBackInput {
  scrollTop: number;
  /** Offset the follow rule owns, from `memorylessFollowTarget`. */
  followTargetScrollTop: number;
  thresholdPx: number;
}

/**
 * Offset to snap back to once a gesture has come to rest below the follow
 * target, or `null` when it has not.
 *
 * Only the region *below* the target qualifies, and that region is the reserved
 * tail spacer. It carries no content, so a gesture ending there can only mean
 * "take me to the end" — the one direction in which reading intent from
 * geometry is unambiguous. Scrolling up to read history can never satisfy this,
 * and neither can a pinned Turn or a held collapse gap, because both are the
 * target rather than a departure from it.
 */
export function tailSnapBackScrollTop(input: TailSnapBackInput): number | null {
  return input.scrollTop - input.followTargetScrollTop > input.thresholdPx
    ? input.followTargetScrollTop
    : null;
}

export interface ViewportAtTailInput {
  scrollTop: number;
  contentEndScrollTop: number;
  /** Upper bound of the band; the content end when nothing owns the viewport. */
  followTargetScrollTop: number;
  thresholdPx: number;
}

/**
 * Whether the viewport counts as being at the end of the transcript.
 *
 * The band runs from the content end down to whatever the follow rule owns, so
 * a pinned Turn and a held collapse gap both sit inside it — neither is a
 * reason to offer a jump to the latest output. Past the lower bound is reserved
 * blank, which is.
 */
export function isViewportAtTail(input: ViewportAtTailInput): boolean {
  return input.scrollTop >= input.contentEndScrollTop - input.thresholdPx
    && input.scrollTop <= input.followTargetScrollTop + input.thresholdPx;
}

/**
 * Viewports a jump to the latest output may animate across.
 *
 * Counted in viewports rather than pixels because the question is whether the
 * reader can follow the movement, and what they can follow is a share of what
 * they can see: the same 2000px is two and a half screens on a laptop and most
 * of one on a tall display.
 *
 * Three is about the largest number the animation reliably finishes. The frame
 * loop yields to a smooth scroll for a bounded time and takes the viewport back
 * afterwards wherever it has got to — measured, a jump issued for 8717px
 * animated 5480 of them and was finished by the loop in a single 3290px write.
 * That is 38% of the distance delivered as a hard jump at the end of an
 * animation, which is worse than either half on its own. The same measurement
 * puts the sustained rate near 4570px/s, so three 800px viewports travel in
 * roughly half the budget.
 *
 * `followOutput.animatedScrollEnded` is where this number is checked: a
 * `backstop` reason means an animation ran out its yield without arriving, and
 * this is then too high.
 */
export const FLOWCHAT_ANIMATED_JUMP_MAX_VIEWPORTS = 3;

export interface AnimatedJumpInput {
  /** Where the viewport is now. */
  fromPx: number;
  /** Where the jump would leave it. */
  targetPx: number;
  clientHeight: number;
}

/**
 * How a jump to the latest output should travel.
 *
 * An animation earns its cost only while the reader can read it. Its whole job
 * is spatial continuity — showing which way and how far the viewport went — and
 * past a few screens the transcript in between goes by faster than anyone can
 * track, leaving a wait where the answer was. So a far jump lands instantly,
 * the way every other navigation in the transcript already does, and a near one
 * animates.
 *
 * Distance also costs more than it looks. Animating across N screens of a
 * virtualized transcript renders and measures every item passed while the
 * animation runs, and heights are estimates until they are measured — so the
 * content end moves under an animation aimed at where it used to be, and the
 * follow loop corrects that afterwards as a second, visible movement. A jump
 * inside a few viewports passes items that are already rendered and measured.
 */
export function resolveAnimatedJumpBehavior(input: AnimatedJumpInput): 'smooth' | 'auto' {
  // An unmeasured scroller is not a short distance, it is no distance at all:
  // there is nothing to scale the budget by and nothing on screen to follow.
  if (input.clientHeight <= 0) return 'auto';
  return Math.abs(input.targetPx - input.fromPx)
    <= input.clientHeight * FLOWCHAT_ANIMATED_JUMP_MAX_VIEWPORTS
    ? 'smooth'
    : 'auto';
}
