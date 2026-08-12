import { describe, expect, it } from 'vitest';
import {
  resolveHistoryBoundaryTarget,
  resolveTailWindowGrowth,
  transcriptReachesLatestTurn,
} from './flowChatLiveTailWindow';

describe('transcriptReachesLatestTurn', () => {
  it('counts the canonical transcript, which always holds the live tail', () => {
    expect(transcriptReachesLatestTurn({
      windowEndOrdinalExclusive: null,
      knownTurnCount: 28,
    })).toBe(true);
  });

  it('counts a window paged in from the tail', () => {
    // The window the automatic paging produces on open: it ends exactly where
    // the session does, so the viewport on it is on the newest output.
    expect(transcriptReachesLatestTurn({
      windowEndOrdinalExclusive: 27,
      knownTurnCount: 27,
    })).toBe(true);
  });

  it('excludes a window the session has since grown past', () => {
    expect(transcriptReachesLatestTurn({
      windowEndOrdinalExclusive: 27,
      knownTurnCount: 28,
    })).toBe(false);
  });

  it('excludes a window the user navigated into the middle of', () => {
    expect(transcriptReachesLatestTurn({
      windowEndOrdinalExclusive: 10,
      knownTurnCount: 28,
    })).toBe(false);
  });
});

describe('resolveTailWindowGrowth', () => {
  it('remembers the end of a window that reaches the newest Turn', () => {
    expect(resolveTailWindowGrowth({
      windowEndOrdinalExclusive: 27,
      knownTurnCount: 27,
      tailAnchoredWindowEnd: null,
    })).toBe('anchor');
  });

  it('grows the tail-anchored window when a Turn is appended', () => {
    // Without this the newest Turn is not rendered at all: the message the user
    // just sent is missing, and since the follow rule reads the latest Turn off
    // the rendered items it never learns the Turn exists.
    expect(resolveTailWindowGrowth({
      windowEndOrdinalExclusive: 27,
      knownTurnCount: 28,
      tailAnchoredWindowEnd: 27,
    })).toBe('extend');
  });

  it('keeps asking until the window is actually repaired', () => {
    // The whole reason this is not edge-triggered. A failed extension leaves
    // the state untouched, and the next render asks again.
    const stillBroken = {
      windowEndOrdinalExclusive: 27,
      knownTurnCount: 28,
      tailAnchoredWindowEnd: 27,
    };
    expect(resolveTailWindowGrowth(stillBroken)).toBe('extend');
    expect(resolveTailWindowGrowth(stillBroken)).toBe('extend');
  });

  it('re-anchors once the window has caught up', () => {
    expect(resolveTailWindowGrowth({
      windowEndOrdinalExclusive: 28,
      knownTurnCount: 28,
      tailAnchoredWindowEnd: 27,
    })).toBe('anchor');
  });

  it('grows again when a partial extension still falls short', () => {
    // Two Turns arrived and the loaded range only covered one of them.
    expect(resolveTailWindowGrowth({
      windowEndOrdinalExclusive: 28,
      knownTurnCount: 29,
      tailAnchoredWindowEnd: 28,
    })).toBe('extend');
  });

  it('leaves a window the user navigated to alone', () => {
    // Its end is not where a tail-reaching window's end was, so the session
    // growing says nothing about it.
    expect(resolveTailWindowGrowth({
      windowEndOrdinalExclusive: 10,
      knownTurnCount: 28,
      tailAnchoredWindowEnd: 27,
    })).toBe('none');
  });

  it('leaves a mid-session window alone even with no anchor on record', () => {
    expect(resolveTailWindowGrowth({
      windowEndOrdinalExclusive: 10,
      knownTurnCount: 28,
      tailAnchoredWindowEnd: null,
    })).toBe('none');
  });

  it('releases the anchor when no window is rendered', () => {
    expect(resolveTailWindowGrowth({
      windowEndOrdinalExclusive: null,
      knownTurnCount: 28,
      tailAnchoredWindowEnd: 27,
    })).toBe('release');
  });
});

describe('resolveHistoryBoundaryTarget', () => {
  it('asks for the Turn before the transcript on screen', () => {
    expect(resolveHistoryBoundaryTarget({
      direction: 'before',
      renderedRange: { startOrdinal: 12, endOrdinalExclusive: 24 },
      knownTurnCount: 40,
    })).toEqual({ status: 'ask', targetOrdinal: 11 });
  });

  it('asks for the Turn after the transcript on screen', () => {
    expect(resolveHistoryBoundaryTarget({
      direction: 'after',
      renderedRange: { startOrdinal: 12, endOrdinalExclusive: 24 },
      knownTurnCount: 40,
    })).toEqual({ status: 'ask', targetOrdinal: 24 });
  });

  it('is exhausted at the head of the session', () => {
    expect(resolveHistoryBoundaryTarget({
      direction: 'before',
      renderedRange: { startOrdinal: 0, endOrdinalExclusive: 24 },
      knownTurnCount: 40,
    })).toEqual({ status: 'exhausted', reason: 'reached-start' });
  });

  it('is exhausted at the newest Turn, and says which nothing that is', () => {
    /*
     * The reported case, in ordinals. A window paged in from the tail ended at
     * 6 while the session had grown to 10, the continuous projection spliced
     * the two so the transcript on screen ran to the newest Turn, and reaching
     * its bottom asked to load ordinal 6 — a Turn already on screen, which the
     * catalog could not resolve. 266 asks, every one answered `not-found`, and
     * a boundary status the reader read as history being prepared forever.
     *
     * Judged against the rendered range there is nothing to ask for, which is
     * the honest answer and the one that latches the direction quiet.
     */
    expect(resolveHistoryBoundaryTarget({
      direction: 'after',
      renderedRange: { startOrdinal: 0, endOrdinalExclusive: 10 },
      knownTurnCount: 10,
    })).toEqual({ status: 'exhausted', reason: 'reached-latest' });
  });

  it('would still ask against the window the store cut, which is the bug', () => {
    // The same session, judged against the store's window instead of the
    // transcript: an ordinal that is both already rendered and unresolvable.
    expect(resolveHistoryBoundaryTarget({
      direction: 'after',
      renderedRange: { startOrdinal: 0, endOrdinalExclusive: 6 },
      knownTurnCount: 10,
    })).toEqual({ status: 'ask', targetOrdinal: 6 });
  });

  it('separates asking past the head from asking past the newest Turn', () => {
    // Only the first is a symptom: it means Turns the session claims to have
    // are not reachable, which is how history goes missing rather than late.
    expect(resolveHistoryBoundaryTarget({
      direction: 'before',
      renderedRange: { startOrdinal: 41, endOrdinalExclusive: 48 },
      knownTurnCount: 40,
    })).toEqual({ status: 'exhausted', reason: 'beyond-known-total' });
  });
});
