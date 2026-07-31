import { describe, expect, it } from 'vitest';

import { computeSessionRowRemovalTransition } from './sessionRowShift';

const offsets = (entries: Array<[string, number]>): Map<string, number> => new Map(entries);

describe('computeSessionRowRemovalTransition', () => {
  it('slides the rows below a deleted session up from their old slot', () => {
    const transition = computeSessionRowRemovalTransition(
      offsets([['a', 0], ['b', 26], ['c', 52], ['d', 78]]),
      offsets([['a', 0], ['c', 26], ['d', 52]]),
    );

    expect(transition).toEqual({
      hasRemovedRow: true,
      shifts: [
        { sessionId: 'c', fromOffsetY: 26 },
        { sessionId: 'd', fromOffsetY: 26 },
      ],
      enteringSessionIds: [],
    });
  });

  it('leaves rows above the deleted session alone', () => {
    const transition = computeSessionRowRemovalTransition(
      offsets([['a', 0], ['b', 26], ['c', 52]]),
      offsets([['a', 0], ['b', 26]]),
    );

    expect(transition.shifts).toEqual([]);
  });

  it('fades in the buffered row that backfills the freed slot', () => {
    // `e` renders in the same commit the delete lands in.
    const transition = computeSessionRowRemovalTransition(
      offsets([['a', 0], ['b', 26], ['c', 52]]),
      offsets([['a', 0], ['c', 26], ['e', 52]]),
    );

    expect(transition).toEqual({
      hasRemovedRow: true,
      shifts: [{ sessionId: 'c', fromOffsetY: 26 }],
      enteringSessionIds: ['e'],
    });
  });

  it('ignores renders that only add rows', () => {
    expect(computeSessionRowRemovalTransition(
      offsets([['a', 0]]),
      offsets([['a', 26], ['b', 0]]),
    )).toEqual({ hasRemovedRow: false, shifts: [], enteringSessionIds: [] });
    expect(computeSessionRowRemovalTransition(
      offsets([]),
      offsets([['a', 0]]),
    )).toEqual({ hasRemovedRow: false, shifts: [], enteringSessionIds: [] });
  });

  it('ignores sub-pixel movement', () => {
    const transition = computeSessionRowRemovalTransition(
      offsets([['a', 0], ['b', 26], ['c', 52.2]]),
      offsets([['a', 0], ['c', 52]]),
    );

    expect(transition.shifts).toEqual([]);
  });
});
