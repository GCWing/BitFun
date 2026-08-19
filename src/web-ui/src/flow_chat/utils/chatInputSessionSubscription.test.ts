import { describe, expect, it } from 'vitest';

import type { Session } from '../types/flow-chat';
import { chatInputSessionSubscriptionKey } from './chatInputSessionSubscription';

function session(overrides: Partial<Session> = {}): Session {
  return {
    sessionId: 'session-1',
    dialogTurns: [],
    status: 'idle',
    config: {},
    createdAt: 1,
    lastActiveAt: 1,
    error: null,
    ...overrides,
  } as Session;
}

describe('chatInputSessionSubscriptionKey', () => {
  it('invalidates the first-turn lock when projected history counts change', () => {
    expect(chatInputSessionSubscriptionKey(session({ totalTurnCount: 1 }))).not.toBe(
      chatInputSessionSubscriptionKey(session({ totalTurnCount: 0 })),
    );
  });
});
