import { describe, expect, it } from 'vitest';
import {
  deriveSessionReviewActivity,
  isReviewActivityBlocking,
} from './sessionReviewActivity';
import { SessionExecutionState } from '../state-machine/types';
import type { FlowChatState, Session } from '../types/flow-chat';

function createMockSession(overrides: Partial<Session> = {}): Session {
  return {
    sessionId: 'child-1',
    parentSessionId: 'parent-1',
    sessionKind: 'deep_review',
    status: 'active',
    createdAt: 1000,
    updatedAt: 2000,
    lastActiveAt: 2000,
    dialogTurns: [],
    ...overrides,
  } as Session;
}

function createMockState(sessions: Session[]): FlowChatState {
  return {
    sessions: new Map(sessions.map(s => [s.sessionId, s])),
  } as FlowChatState;
}

describe('deriveLifecycleFromTurn', () => {
  it('maps pending to running', () => {
    const session = createMockSession({
      dialogTurns: [{ status: 'pending' } as any],
    });
    const activity = deriveSessionReviewActivity(
      createMockState([session]),
      'parent-1',
    );
    expect(activity?.lifecycle).toBe('running');
  });

  it('maps processing to running', () => {
    const session = createMockSession({
      dialogTurns: [{ status: 'processing' } as any],
    });
    const activity = deriveSessionReviewActivity(
      createMockState([session]),
      'parent-1',
    );
    expect(activity?.lifecycle).toBe('running');
  });

  it('maps finishing to finishing', () => {
    const session = createMockSession({
      dialogTurns: [{ status: 'finishing' } as any],
    });
    const activity = deriveSessionReviewActivity(
      createMockState([session]),
      'parent-1',
    );
    expect(activity?.lifecycle).toBe('finishing');
  });

  it('maps completed to completed', () => {
    const session = createMockSession({
      dialogTurns: [{ status: 'completed' } as any],
    });
    const activity = deriveSessionReviewActivity(
      createMockState([session]),
      'parent-1',
    );
    expect(activity?.lifecycle).toBe('completed');
  });

  it('maps cancelled to cancelled', () => {
    const session = createMockSession({
      dialogTurns: [{ status: 'cancelled' } as any],
    });
    const activity = deriveSessionReviewActivity(
      createMockState([session]),
      'parent-1',
    );
    expect(activity?.lifecycle).toBe('cancelled');
  });

  it('maps error to error', () => {
    const session = createMockSession({
      dialogTurns: [{ status: 'error' } as any],
    });
    const activity = deriveSessionReviewActivity(
      createMockState([session]),
      'parent-1',
    );
    expect(activity?.lifecycle).toBe('error');
  });

  it('session error takes precedence over turn status', () => {
    const session = createMockSession({
      error: 'Some error',
      dialogTurns: [{ status: 'processing' } as any],
    });
    const activity = deriveSessionReviewActivity(
      createMockState([session]),
      'parent-1',
    );
    expect(activity?.lifecycle).toBe('error');
  });

  it('defaults to idle for unknown status', () => {
    const session = createMockSession({
      dialogTurns: [{ status: 'unknown' } as any],
    });
    const activity = deriveSessionReviewActivity(
      createMockState([session]),
      'parent-1',
    );
    expect(activity?.lifecycle).toBe('idle');
  });
});

describe('deriveLifecycle with executionState', () => {
  it('PROCESSING maps to running', () => {
    const session = createMockSession();
    const activity = deriveSessionReviewActivity(
      createMockState([session]),
      'parent-1',
      () => SessionExecutionState.PROCESSING,
    );
    expect(activity?.lifecycle).toBe('running');
  });

  it('FINISHING maps to finishing', () => {
    const session = createMockSession();
    const activity = deriveSessionReviewActivity(
      createMockState([session]),
      'parent-1',
      () => SessionExecutionState.FINISHING,
    );
    expect(activity?.lifecycle).toBe('finishing');
  });

  it('ERROR maps to error', () => {
    const session = createMockSession();
    const activity = deriveSessionReviewActivity(
      createMockState([session]),
      'parent-1',
      () => SessionExecutionState.ERROR,
    );
    expect(activity?.lifecycle).toBe('error');
  });

  it('IDLE falls back to turn-based derivation', () => {
    const session = createMockSession({
      dialogTurns: [{ status: 'completed' } as any],
    });
    const activity = deriveSessionReviewActivity(
      createMockState([session]),
      'parent-1',
      () => SessionExecutionState.IDLE,
    );
    expect(activity?.lifecycle).toBe('completed');
  });
});

describe('toReviewActivity filtering', () => {
  it('returns null for non-review sessions', () => {
    const session = createMockSession({ sessionKind: 'normal' });
    const activity = deriveSessionReviewActivity(
      createMockState([session]),
      'parent-1',
    );
    expect(activity).toBeNull();
  });

  it('returns null when parentSessionId does not match', () => {
    const session = createMockSession({ parentSessionId: 'different-parent' });
    const activity = deriveSessionReviewActivity(
      createMockState([session]),
      'parent-1',
    );
    expect(activity).toBeNull();
  });

  it('returns activity for review session', () => {
    const session = createMockSession({ sessionKind: 'review' });
    const activity = deriveSessionReviewActivity(
      createMockState([session]),
      'parent-1',
    );
    expect(activity).not.toBeNull();
    expect(activity?.kind).toBe('review');
  });

  it('returns activity for deep_review session', () => {
    const session = createMockSession({ sessionKind: 'deep_review' });
    const activity = deriveSessionReviewActivity(
      createMockState([session]),
      'parent-1',
    );
    expect(activity).not.toBeNull();
    expect(activity?.kind).toBe('deep_review');
  });
});

describe('deriveSessionReviewActivity selection', () => {
  it('returns null when no parentSessionId provided', () => {
    const state = createMockState([createMockSession()]);
    expect(deriveSessionReviewActivity(state, null)).toBeNull();
    expect(deriveSessionReviewActivity(state, undefined)).toBeNull();
  });

  it('selects most recently updated activity among blocking ones', () => {
    const sessions = [
      createMockSession({
        sessionId: 'child-1',
        dialogTurns: [{ status: 'processing' } as any],
        updatedAt: 1000,
        lastActiveAt: 1000,
      }),
      createMockSession({
        sessionId: 'child-2',
        dialogTurns: [{ status: 'processing' } as any],
        updatedAt: 2000,
        lastActiveAt: 2000,
      }),
    ];
    const activity = deriveSessionReviewActivity(
      createMockState(sessions),
      'parent-1',
    );
    expect(activity?.childSessionId).toBe('child-2');
  });

  it('falls back to non-blocking activities when no blocking ones exist', () => {
    const sessions = [
      createMockSession({
        sessionId: 'child-1',
        dialogTurns: [{ status: 'completed' } as any],
        updatedAt: 1000,
        lastActiveAt: 1000,
      }),
      createMockSession({
        sessionId: 'child-2',
        dialogTurns: [{ status: 'completed' } as any],
        updatedAt: 2000,
        lastActiveAt: 2000,
      }),
    ];
    const activity = deriveSessionReviewActivity(
      createMockState(sessions),
      'parent-1',
    );
    expect(activity?.childSessionId).toBe('child-2');
  });
});

describe('isReviewActivityBlocking', () => {
  it('returns true for running activity', () => {
    expect(isReviewActivityBlocking({ lifecycle: 'running', isBlocking: true } as any)).toBe(true);
  });

  it('returns true for finishing activity', () => {
    expect(isReviewActivityBlocking({ lifecycle: 'finishing', isBlocking: true } as any)).toBe(true);
  });

  it('returns false for completed activity', () => {
    expect(isReviewActivityBlocking({ lifecycle: 'completed', isBlocking: false } as any)).toBe(false);
  });

  it('returns false for null/undefined', () => {
    expect(isReviewActivityBlocking(null)).toBe(false);
    expect(isReviewActivityBlocking(undefined)).toBe(false);
  });
});
