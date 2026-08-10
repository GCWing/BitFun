import { describe, expect, it } from 'vitest';
import { historyBoundariesForVisibleRange } from './flowChatHistoryBoundary';

const ITEM_COUNT = 40;

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
});
