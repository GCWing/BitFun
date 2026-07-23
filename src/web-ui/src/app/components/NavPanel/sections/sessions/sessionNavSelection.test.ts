import { describe, expect, it } from 'vitest';
import { isSessionNavRowActive } from './sessionNavSelection';

describe('isSessionNavRowActive', () => {
  it('falls back to the main active session when an aux child belongs to another parent', () => {
    expect(
      isSessionNavRowActive({
        rowSessionId: 'child-1',
        activeTabId: 'session',
        activeSessionId: 'session-2',
        activeChildSessionId: 'child-1',
        activeChildParentSessionId: 'session-1',
      }),
    ).toBe(false);

    expect(
      isSessionNavRowActive({
        rowSessionId: 'session-2',
        activeTabId: 'session',
        activeSessionId: 'session-2',
        activeChildSessionId: 'child-1',
        activeChildParentSessionId: 'session-1',
      }),
    ).toBe(true);
  });

  it('keeps the active child highlighted while its parent is the main active session', () => {
    expect(
      isSessionNavRowActive({
        rowSessionId: 'child-1',
        activeTabId: 'session',
        activeSessionId: 'session-1',
        activeChildSessionId: 'child-1',
        activeChildParentSessionId: 'session-1',
      }),
    ).toBe(true);

    expect(
      isSessionNavRowActive({
        rowSessionId: 'session-1',
        activeTabId: 'session',
        activeSessionId: 'session-1',
        activeChildSessionId: 'child-1',
        activeChildParentSessionId: 'session-1',
      }),
    ).toBe(false);
  });
});
