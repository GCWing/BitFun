import { describe, expect, it } from 'vitest';
import {
  VIRTUOSO_FIRST_ITEM_INDEX_BASE,
  advanceVirtuosoIndexCursor,
  createVirtuosoIndexCursor,
  endAlignedTailOffsetPx,
  initialTopMostItemForSessionOpen,
  normalizeVirtuosoBehavior,
  toLocalItemRange,
} from './flowChatVirtuosoBridge';

const SPACER = 632;

/** Stand-ins for virtual items: the cursor only ever reads their key. */
const key = (item: string) => item;
const BASE = VIRTUOSO_FIRST_ITEM_INDEX_BASE;

describe('the Virtuoso index cursor', () => {
  it('starts a session well above zero, with room to page backwards', () => {
    const cursor = createVirtuosoIndexCursor('session-1', ['a', 'b']);
    expect(cursor.firstItemIndex).toBe(BASE);
    expect(BASE).toBeGreaterThan(100_000);
  });

  it('lowers the first index by exactly what was prepended', () => {
    const cursor = createVirtuosoIndexCursor('session-1', ['c', 'd']);
    const next = advanceVirtuosoIndexCursor(cursor, 'session-1', ['a', 'b', 'c', 'd'], key);
    // 'c' has to keep the index it already claims, or every measured item moves.
    expect(next.firstItemIndex).toBe(BASE - 2);
  });

  it('accumulates across successive prepends', () => {
    let cursor = createVirtuosoIndexCursor('session-1', ['e']);
    cursor = advanceVirtuosoIndexCursor(cursor, 'session-1', ['c', 'd', 'e'], key);
    cursor = advanceVirtuosoIndexCursor(cursor, 'session-1', ['a', 'b', 'c', 'd', 'e'], key);
    expect(cursor.firstItemIndex).toBe(BASE - 4);
  });

  it('raises the first index when leading items are dropped', () => {
    const cursor = createVirtuosoIndexCursor('session-1', ['a', 'b', 'c']);
    const next = advanceVirtuosoIndexCursor(cursor, 'session-1', ['c'], key);
    expect(next.firstItemIndex).toBe(BASE + 2);
  });

  it('leaves the index alone when items are only appended', () => {
    const cursor = createVirtuosoIndexCursor('session-1', ['a', 'b']);
    const next = advanceVirtuosoIndexCursor(cursor, 'session-1', ['a', 'b', 'c'], key);
    expect(next.firstItemIndex).toBe(BASE);
  });

  it('starts over for a new session', () => {
    const cursor = advanceVirtuosoIndexCursor(
      createVirtuosoIndexCursor('session-1', ['c']),
      'session-1',
      ['a', 'b', 'c'],
      key,
    );
    expect(cursor.firstItemIndex).toBe(BASE - 2);

    const switched = advanceVirtuosoIndexCursor(cursor, 'session-2', ['x', 'y'], key);
    expect(switched.firstItemIndex).toBe(BASE);
    expect(switched.virtualItems).toEqual(['x', 'y']);
  });

  it('never goes negative, however far back a session is paged', () => {
    const items = ['tail'];
    let cursor = { sessionId: 'session-1', firstItemIndex: 1, virtualItems: items };
    cursor = advanceVirtuosoIndexCursor(cursor, 'session-1', ['a', 'b', 'c', 'tail'], key);
    expect(cursor.firstItemIndex).toBe(0);
  });

  it('costs nothing on a render that did not touch the item list', () => {
    const items = ['a', 'b'];
    const cursor = createVirtuosoIndexCursor('session-1', items);
    expect(advanceVirtuosoIndexCursor(cursor, 'session-1', items, key)).toBe(cursor);
  });
});

describe('toLocalItemRange', () => {
  it('translates out of the virtualizer index space', () => {
    expect(toLocalItemRange({ startIndex: BASE + 4, endIndex: BASE + 9 }, BASE))
      .toEqual({ startIndex: 4, endIndex: 9 });
  });

  it('reads a range that starts before the first item as the first item', () => {
    // A prepend that has landed in the cursor but not yet in the reported range.
    expect(toLocalItemRange({ startIndex: BASE - 3, endIndex: BASE + 2 }, BASE))
      .toEqual({ startIndex: 0, endIndex: 2 });
  });

  it('never reports an end before its start', () => {
    expect(toLocalItemRange({ startIndex: BASE - 5, endIndex: BASE - 4 }, BASE))
      .toEqual({ startIndex: 0, endIndex: 0 });
  });

  it('leaves a range that runs past the end alone', () => {
    // The caller needs to see the overshoot to know it is at the end.
    expect(toLocalItemRange({ startIndex: BASE + 40, endIndex: BASE + 60 }, BASE))
      .toEqual({ startIndex: 40, endIndex: 60 });
  });
});

describe('endAlignedTailOffsetPx', () => {
  it('cancels the spacer when Virtuoso end-aligns the last item', () => {
    // Virtuoso reveals the whole footer for `align: 'end'` on the last index,
    // which would otherwise scroll the resident spacer into view as blank.
    expect(endAlignedTailOffsetPx(9, 10, SPACER)).toBe(-SPACER);
  });

  it('leaves earlier items alone', () => {
    // Virtuoso only adds footerHeight for the last index.
    expect(endAlignedTailOffsetPx(8, 10, SPACER)).toBe(0);
    expect(endAlignedTailOffsetPx(0, 10, SPACER)).toBe(0);
  });

  it('is inert before the viewport is measured', () => {
    expect(endAlignedTailOffsetPx(9, 10, 0)).toBe(0);
    expect(Object.is(endAlignedTailOffsetPx(9, 10, 0), -0)).toBe(false);
  });
});

describe('initialTopMostItemForSessionOpen', () => {
  it('opens on the end of real content rather than the end of the range', () => {
    expect(initialTopMostItemForSessionOpen(10, SPACER))
      .toEqual({ index: 9, align: 'end', offset: -SPACER });
  });

  it('treats a single-item transcript as its own last item', () => {
    expect(initialTopMostItemForSessionOpen(1, SPACER))
      .toEqual({ index: 0, align: 'end', offset: -SPACER });
  });
});

describe('normalizeVirtuosoBehavior', () => {
  it('passes a smooth scroll through', () => {
    expect(normalizeVirtuosoBehavior('smooth')).toBe('smooth');
  });

  it('collapses everything else onto the instant scroll Virtuoso understands', () => {
    expect(normalizeVirtuosoBehavior('auto')).toBe('auto');
    expect(normalizeVirtuosoBehavior('instant')).toBe('auto');
  });
});
