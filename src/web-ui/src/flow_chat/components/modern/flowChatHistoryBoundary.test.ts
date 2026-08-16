import { describe, expect, it } from 'vitest';
import {
  HISTORY_BOUNDARY_LEAD_SCREENS,
  historyBoundariesForVisibleRange,
  historyBoundariesReached,
} from './flowChatHistoryBoundary';

const ITEM_COUNT = 40;
const VIEWPORT_PX = 600;
const LEAD_PX = VIEWPORT_PX * HISTORY_BOUNDARY_LEAD_SCREENS;

/** Far enough from both ends that only the argument under test can decide. */
function proximity(overrides: { aboveViewportPx?: number; belowViewportPx?: number }) {
  return {
    aboveViewportPx: LEAD_PX * 10,
    belowViewportPx: LEAD_PX * 10,
    viewportHeightPx: VIEWPORT_PX,
    ...overrides,
  };
}

describe('historyBoundariesForVisibleRange', () => {
  it('asks for nothing from the middle of the window', () => {
    expect(historyBoundariesForVisibleRange(
      { startIndex: 12, endIndex: 20 },
      ITEM_COUNT,
      'history-window',
    )).toEqual([]);
  });

  it('reaches for older Turns before the leading item is alone on screen', () => {
    // One item of slack: the leading item is a user message a couple of dozen
    // pixels tall, so waiting for index 0 would start the fetch after the blank.
    expect(historyBoundariesForVisibleRange({ startIndex: 1, endIndex: 9 }, ITEM_COUNT, 'tail'))
      .toEqual(['before']);
    expect(historyBoundariesForVisibleRange({ startIndex: 2, endIndex: 9 }, ITEM_COUNT, 'tail'))
      .toEqual([]);
  });

  it('reaches for newer Turns near the end of a history window', () => {
    expect(historyBoundariesForVisibleRange(
      { startIndex: 30, endIndex: ITEM_COUNT - 2 },
      ITEM_COUNT,
      'history-window',
    )).toEqual(['after']);
    expect(historyBoundariesForVisibleRange(
      { startIndex: 30, endIndex: ITEM_COUNT - 3 },
      ITEM_COUNT,
      'history-window',
    )).toEqual([]);
  });

  it('never reaches past the end of a tail presentation', () => {
    // A tail is already anchored to the newest Turn: there is nothing after it.
    expect(historyBoundariesForVisibleRange(
      { startIndex: 30, endIndex: ITEM_COUNT + 4 },
      ITEM_COUNT,
      'tail',
    )).toEqual([]);
  });

  it('asks in both directions when the whole window fits on screen', () => {
    expect(historyBoundariesForVisibleRange(
      { startIndex: 0, endIndex: 5 },
      6,
      'history-window',
    )).toEqual(['before', 'after']);
  });

  it('asks for older Turns first, so a reader scrolling up is served first', () => {
    const [first] = historyBoundariesForVisibleRange(
      { startIndex: 0, endIndex: 5 },
      6,
      'history-window',
    );
    expect(first).toBe('before');
  });

  describe('the lead time', () => {
    it('reaches back a screenful before the head is on screen at all', () => {
      // The whole point: the junction happens where the reader is not looking.
      // The item range says the head is nowhere near, and it still asks.
      expect(historyBoundariesForVisibleRange(
        { startIndex: 12, endIndex: 20 },
        ITEM_COUNT,
        'history-window',
        proximity({ aboveViewportPx: LEAD_PX - 1 }),
      )).toEqual(['before']);
    });

    it('stays quiet a screenful and one pixel out', () => {
      expect(historyBoundariesForVisibleRange(
        { startIndex: 12, endIndex: 20 },
        ITEM_COUNT,
        'history-window',
        proximity({ aboveViewportPx: LEAD_PX + 1 }),
      )).toEqual([]);
    });

    it('reaches forward a screenful before the end of a history window', () => {
      expect(historyBoundariesForVisibleRange(
        { startIndex: 12, endIndex: 20 },
        ITEM_COUNT,
        'history-window',
        proximity({ belowViewportPx: LEAD_PX - 1 }),
      )).toEqual(['after']);
    });

    it('does not reach past the end of a tail, however close it is', () => {
      expect(historyBoundariesForVisibleRange(
        { startIndex: 12, endIndex: 20 },
        ITEM_COUNT,
        'tail',
        proximity({ belowViewportPx: 0 }),
      )).toEqual([]);
    });

    it('still answers from the item range when there is no geometry to read', () => {
      // A boundary genuinely on screen is never missed for want of a
      // measurement; only the lead is lost.
      expect(historyBoundariesForVisibleRange(
        { startIndex: 0, endIndex: 9 },
        ITEM_COUNT,
        'tail',
        null,
      )).toEqual(['before']);
    });

    it('asks once, not once per rule, when both agree', () => {
      expect(historyBoundariesForVisibleRange(
        { startIndex: 0, endIndex: 9 },
        ITEM_COUNT,
        'tail',
        proximity({ aboveViewportPx: 0 }),
      )).toEqual(['before']);
    });
  });
});

describe('historyBoundariesReached', () => {
  /*
   * The arming latch's question, and the reason it is a second function. The
   * latch disarms a direction on dispatch and re-arms it when the reader is no
   * longer at the boundary — so if "at the boundary" reached a screen out, the
   * latch had no way back. Measured: one page fetched, then `not-rearmed`
   * refusals for the next six minutes while the reader scrolled into a wall.
   */
  it('says nothing is reached from a screenful away, however keen the ask is', () => {
    const near = proximity({ aboveViewportPx: 0, belowViewportPx: 0 });
    const range = { startIndex: 12, endIndex: 20 };

    expect(historyBoundariesForVisibleRange(range, ITEM_COUNT, 'history-window', near))
      .toEqual(['before', 'after']);
    expect(historyBoundariesReached(range, ITEM_COUNT, 'history-window')).toEqual([]);
  });

  it('says a boundary on screen is reached', () => {
    expect(historyBoundariesReached({ startIndex: 1, endIndex: 9 }, ITEM_COUNT, 'tail'))
      .toEqual(['before']);
    expect(historyBoundariesReached(
      { startIndex: 30, endIndex: ITEM_COUNT - 2 },
      ITEM_COUNT,
      'history-window',
    )).toEqual(['after']);
  });

  it('never reports the end of a tail as reached', () => {
    expect(historyBoundariesReached(
      { startIndex: 30, endIndex: ITEM_COUNT + 4 },
      ITEM_COUNT,
      'tail',
    )).toEqual([]);
  });
});
