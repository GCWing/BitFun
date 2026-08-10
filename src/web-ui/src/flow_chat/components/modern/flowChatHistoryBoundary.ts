/**
 * When looking at the transcript should pull more of it in.
 *
 * This is the policy half of history paging: it decides *that* a boundary is
 * worth asking about, from nothing but which items are on screen. Whether the
 * ask is honoured — budgets, in-flight requests, exhausted directions — belongs
 * to the container, and how the range was arrived at belongs to whatever is
 * placing the items.
 */

import type { SessionHistoryWindowDirection } from '../../store/FlowChatStore';

export interface VisibleItemRange {
  startIndex: number;
  endIndex: number;
}

/**
 * How close to the first item counts as reaching for older Turns.
 *
 * One item of slack, not zero: the leading item is a user message a couple of
 * dozen pixels tall, so a reader who has arrived at it is already looking at
 * the top of the window, and waiting for it to be the *only* thing on screen
 * would start the fetch after the blank appears rather than before.
 */
const LEADING_ITEM_SLACK = 1;

/**
 * How close to the last item counts as reaching for newer Turns.
 *
 * Two items, because the trailing edge of a Turn is a model round followed by
 * its user message, and reaching the round means the window ends within a
 * screen or so.
 */
const TRAILING_ITEM_SLACK = 2;

/**
 * Directions worth asking about for a given visible range.
 *
 * `'after'` only applies to a history window. A tail presentation is already
 * anchored to the newest Turn, so there is nothing past its end to fetch.
 */
export function historyBoundariesForVisibleRange(
  range: VisibleItemRange,
  itemCount: number,
  presentationMode: 'tail' | 'history-window',
): SessionHistoryWindowDirection[] {
  const directions: SessionHistoryWindowDirection[] = [];
  if (range.startIndex <= LEADING_ITEM_SLACK) {
    directions.push('before');
  }
  if (presentationMode === 'history-window' && range.endIndex >= itemCount - TRAILING_ITEM_SLACK) {
    directions.push('after');
  }
  return directions;
}
