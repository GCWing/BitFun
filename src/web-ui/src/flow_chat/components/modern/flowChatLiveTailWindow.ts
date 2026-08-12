/**
 * Where a rendered history window stands relative to the live tail.
 *
 * Both questions here used to be answered by the viewport intent, which was a
 * faithful proxy only while turn navigation was the sole thing that activated a
 * window. Automatic tail paging activates one with nobody navigating, and the
 * proxy then reports "the user is reading history" for a viewport sitting on
 * the newest output. These are the ordinal facts the callers actually need —
 * ledger numbers, not measurements.
 */

export interface RenderedWindowPosition {
  /** End of the rendered history window, or `null` when none is rendered. */
  windowEndOrdinalExclusive: number | null;
  /** Turns the session is known to hold, loaded or not. */
  knownTurnCount: number;
}

/**
 * Whether the transcript on screen still reaches the newest Turn.
 *
 * Rendering no window means rendering the canonical transcript, which contains
 * the live tail by definition.
 */
export function transcriptReachesLatestTurn(input: RenderedWindowPosition): boolean {
  return input.windowEndOrdinalExclusive === null
    || input.windowEndOrdinalExclusive >= input.knownTurnCount;
}

export type TailWindowGrowthAction =
  /** No window is rendered; forget any remembered tail anchor. */
  | 'release'
  /** The window reaches the newest Turn; remember its end as the tail anchor. */
  | 'anchor'
  /** The session grew out from under the tail-anchored window; grow it too. */
  | 'extend'
  /** A window that was never anchored to the tail. Leave it where it is. */
  | 'none';

export interface TailWindowGrowthInput extends RenderedWindowPosition {
  /** End of the window last seen reaching the newest Turn, if any. */
  tailAnchoredWindowEnd: number | null;
}

/**
 * Decide what a rendered history window owes the live tail.
 *
 * This is deliberately a function of the *current* state rather than of a
 * transition. An edge — "it reached the tail last render and does not now" — is
 * consumed whether or not the caller managed to act on it, which strands the
 * window permanently on a single failure. Here the answer stays `'extend'`
 * until the window is actually repaired, so every render retries it.
 *
 * `tailAnchoredWindowEnd` is what separates a window paged in from the tail
 * from one the user navigated to: only a window whose end is exactly where a
 * tail-reaching window's end was may grow. A window sitting in the middle of
 * the session has a different end and is left alone.
 */
export function resolveTailWindowGrowth(
  input: TailWindowGrowthInput,
): TailWindowGrowthAction {
  if (input.windowEndOrdinalExclusive === null) {
    return 'release';
  }
  if (transcriptReachesLatestTurn(input)) {
    return 'anchor';
  }
  return input.tailAnchoredWindowEnd === input.windowEndOrdinalExclusive
    ? 'extend'
    : 'none';
}

export interface RenderedTranscriptRange {
  startOrdinal: number;
  endOrdinalExclusive: number;
}

export type HistoryBoundaryTargetResolution =
  /** The ordinal a page in this direction has to load. */
  | { status: 'ask'; targetOrdinal: number }
  /** There is nothing in this direction, and which nothing it is. */
  | {
      status: 'exhausted';
      reason: 'reached-start' | 'reached-latest' | 'beyond-known-total';
    };

/**
 * What paging past an edge of the transcript on screen would have to load.
 *
 * **The range is the one the reader is looking at, never the window the store
 * cut.** Those are the same thing only until the continuous projection splices
 * a window that starts at ordinal 0 with the live tail: the rendered transcript
 * then runs to the newest Turn while the store's window still ends where it was
 * paged in.
 *
 * Deriving the ask from the store's window there asks to load a Turn that is
 * already on screen, and it fails in the worst available way. Measured: a
 * window ending at ordinal 6 against a session of 10, projected continuous, so
 * the reader was at the newest output; reaching the bottom asked for ordinal 6,
 * which the *catalog* — six entries, and not extended as the session streamed —
 * could not resolve. 266 asks in one session, every one answered `not-found`,
 * every one leaving a boundary status the reader saw as "preparing history"
 * over a transcript that was already complete.
 *
 * `reached-latest` is separated from `beyond-known-total` because only the
 * second is a symptom. Asking past the newest Turn is what the bottom edge of a
 * live transcript does every time the reader reaches it; asking past the known
 * total in the *other* direction means Turns have gone missing.
 */
export function resolveHistoryBoundaryTarget(input: {
  direction: 'before' | 'after';
  renderedRange: RenderedTranscriptRange;
  knownTurnCount: number;
}): HistoryBoundaryTargetResolution {
  const targetOrdinal = input.direction === 'before'
    ? input.renderedRange.startOrdinal - 1
    : input.renderedRange.endOrdinalExclusive;
  if (targetOrdinal < 0) {
    return { status: 'exhausted', reason: 'reached-start' };
  }
  if (targetOrdinal >= input.knownTurnCount) {
    return {
      status: 'exhausted',
      reason: input.direction === 'after' ? 'reached-latest' : 'beyond-known-total',
    };
  }
  return { status: 'ask', targetOrdinal };
}
