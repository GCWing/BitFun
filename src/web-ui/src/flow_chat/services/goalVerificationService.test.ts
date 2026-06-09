import { describe, expect, it, vi, beforeEach } from 'vitest';
import { flowChatStore } from '../store/FlowChatStore';
import type { Session } from '../types/flow-chat';
import {
  handleGoalVerificationFinished,
  handleGoalVerificationStarted,
} from './goalVerificationService';

vi.mock('@/shared/notification-system', () => ({
  notificationService: {
    success: vi.fn(),
    error: vi.fn(),
  },
}));

function createSession(overrides: Partial<Session> = {}): Session {
  return {
    sessionId: 'session-1',
    dialogTurns: [],
    status: 'idle',
    config: { modelName: 'auto' },
    createdAt: 1,
    lastActiveAt: 1,
    error: null,
    ...overrides,
  };
}

describe('goalVerificationService', () => {
  beforeEach(() => {
    flowChatStore.setState(() => ({
      sessions: new Map(),
      activeSessionId: null,
    }));
  });

  it('inserts and removes a local goal verifying turn', () => {
    const session = createSession();
    flowChatStore.setState(() => ({
      sessions: new Map([[session.sessionId, session]]),
      activeSessionId: session.sessionId,
    }));

    handleGoalVerificationStarted(
      { sessionId: session.sessionId, sourceTurnId: 'turn-1' },
      'Checking if the session goal is met...',
    );

    expect(flowChatStore.getState().sessions.get(session.sessionId)?.dialogTurns).toHaveLength(1);

    handleGoalVerificationFinished(
      { sessionId: session.sessionId, sourceTurnId: 'turn-1', outcome: 'continuing' },
      {
        achievedTitle: 'Session goal achieved',
        failedMessage: 'Goal verification failed',
      },
    );

    expect(flowChatStore.getState().sessions.get(session.sessionId)?.dialogTurns).toHaveLength(0);
  });

  it('clears goal mode active flag when the session goal is achieved', () => {
    const session = createSession({ goalModeActive: true });
    flowChatStore.setState(() => ({
      sessions: new Map([[session.sessionId, session]]),
      activeSessionId: session.sessionId,
    }));

    handleGoalVerificationFinished(
      { sessionId: session.sessionId, sourceTurnId: 'turn-1', outcome: 'achieved' },
      {
        achievedTitle: 'Session goal achieved',
        failedMessage: 'Goal verification failed',
      },
    );

    expect(flowChatStore.getState().sessions.get(session.sessionId)?.goalModeActive).toBe(false);
  });
});
