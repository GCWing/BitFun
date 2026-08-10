import { describe, expect, it } from 'vitest';
import {
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
