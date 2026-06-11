import { afterEach, describe, expect, it, vi } from 'vitest';

import {
  clearHistorySessionOpenTransition,
  clearRecentHistorySessionOpenIntent,
  consumeRecentHistorySessionOpenIntent,
  dispatchHistorySessionOpenIntent,
  getHistorySessionOpenTransitionSnapshot,
  subscribeHistorySessionOpenTransition,
} from './sessionOpenIntent';

describe('sessionOpenIntent', () => {
  afterEach(() => {
    clearRecentHistorySessionOpenIntent();
    clearHistorySessionOpenTransition();
    vi.useRealTimers();
    vi.unstubAllGlobals();
  });

  it('tracks and clears the active history session open transition', () => {
    const listener = vi.fn();
    const unsubscribe = subscribeHistorySessionOpenTransition(listener);

    dispatchHistorySessionOpenIntent('history-1', 'History 1');

    expect(getHistorySessionOpenTransitionSnapshot()).toMatchObject({
      sessionId: 'history-1',
    });
    expect(listener).toHaveBeenCalledTimes(1);

    clearHistorySessionOpenTransition('history-1');

    expect(getHistorySessionOpenTransitionSnapshot()).toBeNull();
    expect(listener).toHaveBeenCalledTimes(2);

    unsubscribe();
  });

  it('keeps transition active when before-hydrate intent is consumed', () => {
    dispatchHistorySessionOpenIntent('history-1');

    expect(consumeRecentHistorySessionOpenIntent('history-1')).toBe(true);
    expect(getHistorySessionOpenTransitionSnapshot()).toMatchObject({
      sessionId: 'history-1',
    });
  });

  it('notifies subscribers when a transition expires without owner cleanup', () => {
    vi.useFakeTimers();
    const listener = vi.fn();
    const unsubscribe = subscribeHistorySessionOpenTransition(listener);

    dispatchHistorySessionOpenIntent('history-1');

    expect(getHistorySessionOpenTransitionSnapshot()).toMatchObject({
      sessionId: 'history-1',
    });
    expect(listener).toHaveBeenCalledTimes(1);

    vi.advanceTimersByTime(3_999);
    expect(getHistorySessionOpenTransitionSnapshot()).toMatchObject({
      sessionId: 'history-1',
    });
    expect(listener).toHaveBeenCalledTimes(1);

    vi.advanceTimersByTime(1);

    expect(getHistorySessionOpenTransitionSnapshot()).toBeNull();
    expect(listener).toHaveBeenCalledTimes(2);

    unsubscribe();
  });

  it('expires stale recent intents and stale transition snapshots', () => {
    const now = vi.fn(() => 100);
    vi.stubGlobal('performance', { now });

    dispatchHistorySessionOpenIntent('history-1');

    now.mockReturnValue(900);
    expect(consumeRecentHistorySessionOpenIntent('history-1')).toBe(false);

    now.mockReturnValue(4_200);
    expect(getHistorySessionOpenTransitionSnapshot()).toBeNull();
  });
});
