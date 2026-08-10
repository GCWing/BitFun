/**
 * Everything that exists only because react-virtuoso is the virtualizer.
 *
 * Two things leak out of a virtualizer and neither is portable: the index space
 * it counts in, and the corrections it applies on its own behalf. Both are
 * confined here, so the rest of FlowChat can talk about virtual items, Turns,
 * and scroll offsets without knowing which library is placing anything.
 *
 * A headless virtualizer deletes this file rather than reimplementing it.
 */

import { getLeadingVirtualItemIndexDelta } from './virtualMessageListLayout';

/**
 * Where a session's index space starts.
 *
 * Virtuoso identifies an item by its index in an imaginary infinite list, and a
 * prepend is expressed by lowering the index its first item claims. Starting
 * well above zero leaves room to page backwards for the life of a session; the
 * cursor clamps at zero, so an unusually long history degrades into re-anchored
 * indices rather than negative ones.
 */
export const VIRTUOSO_FIRST_ITEM_INDEX_BASE = 1_000_000;

export interface VirtuosoIndexCursor<T> {
  sessionId: string | null;
  /** The Virtuoso index the current first item claims. */
  firstItemIndex: number;
  /** The item list this cursor was computed against, held by identity. */
  virtualItems: readonly T[];
}

export interface VirtualizerItemRange {
  startIndex: number;
  endIndex: number;
}

export function createVirtuosoIndexCursor<T>(
  sessionId: string | null,
  virtualItems: readonly T[],
): VirtuosoIndexCursor<T> {
  return { sessionId, firstItemIndex: VIRTUOSO_FIRST_ITEM_INDEX_BASE, virtualItems };
}

/**
 * Follow a change to the item list without moving what is already placed.
 *
 * Items prepended ahead of the previous first item lower the cursor by exactly
 * their count, which is what tells Virtuoso that the items it has already
 * measured are the same ones. A new session starts over: nothing placed under
 * the old one means anything under the new one.
 *
 * Returns the same cursor when nothing changed, so a render that did not touch
 * the item list costs nothing.
 */
export function advanceVirtuosoIndexCursor<T>(
  cursor: VirtuosoIndexCursor<T>,
  sessionId: string | null,
  virtualItems: readonly T[],
  getStableKey: (item: T) => string,
): VirtuosoIndexCursor<T> {
  if (cursor.sessionId !== sessionId) return createVirtuosoIndexCursor(sessionId, virtualItems);
  if (cursor.virtualItems === virtualItems) return cursor;
  const leadingDelta = getLeadingVirtualItemIndexDelta(
    cursor.virtualItems,
    virtualItems,
    getStableKey,
  );
  return {
    sessionId,
    firstItemIndex: Math.max(0, cursor.firstItemIndex + leadingDelta),
    virtualItems,
  };
}

/**
 * Translate a range Virtuoso reports into offsets in the current item list.
 *
 * Deliberately not clamped against the item count: a range that runs past the
 * end is what a caller needs to see to know it is at the end.
 */
export function toLocalItemRange(
  range: VirtualizerItemRange,
  firstItemIndex: number,
): VirtualizerItemRange {
  const startIndex = Math.max(0, range.startIndex - firstItemIndex);
  return {
    startIndex,
    endIndex: Math.max(startIndex, range.endIndex - firstItemIndex),
  };
}

/** Virtuoso accepts only these two, and `instant` is spelled `auto` here. */
export function normalizeVirtuosoBehavior(behavior: ScrollBehavior): 'auto' | 'smooth' {
  return behavior === 'smooth' ? 'smooth' : 'auto';
}

/**
 * Correction for a Virtuoso `scrollToIndex({ align: 'end' })`.
 *
 * react-virtuoso adds the *entire* footer height when the target is the last
 * index, so that scrolling to the end of the last item reveals the footer
 * (`dist/index.mjs`: `xt === "end" ? (… , ft === wt && (St += O))`, where `O`
 * is `footerHeight`). That is the behaviour that makes a new session open with
 * the input-stack clearance visible, so it must be kept — but the resident tail
 * spacer lives in the same footer, and revealing *that* means opening on a
 * screen of blank.
 *
 * Cancel exactly the spacer's share. Virtuoso samples `footerHeight` and this
 * offset in the same reaction, so they cannot disagree.
 */
export function endAlignedTailOffsetPx(
  targetIndex: number,
  itemCount: number,
  tailSpacerPx: number,
): number {
  return targetIndex >= itemCount - 1 && tailSpacerPx > 0 ? -tailSpacerPx : 0;
}

/**
 * Where a session opens: the end of real content, not the end of the range.
 *
 * Virtuoso reveals the whole footer when it end-aligns the last item, and the
 * resident tail spacer lives in that footer, so the spacer's share is cancelled
 * out. Virtuoso samples this alongside `footerHeight` in the same reaction, so
 * the two cannot disagree.
 */
export function initialTopMostItemForSessionOpen(
  itemCount: number,
  tailSpacerPx: number,
): { index: number; align: 'end'; offset: number } {
  const index = Math.max(0, itemCount - 1);
  return {
    index,
    align: 'end',
    offset: endAlignedTailOffsetPx(index, itemCount, tailSpacerPx),
  };
}
