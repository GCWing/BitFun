import { describe, expect, it } from 'vitest';
import type { Session } from '../types/flow-chat';
import {
  isSessionWorktreeBindingLocked,
  sessionWorktreeBindingSubscriptionKey,
} from './sessionWorktree';

function session(overrides: Partial<Session> = {}): Session {
  return {
    sessionId: 'session-1',
    dialogTurns: [],
    status: 'active',
    config: {
      executionTarget: {
        kind: 'local',
        rootPath: '/repo',
      },
    },
    createdAt: 0,
    lastActiveAt: 0,
    error: null,
    sessionKind: 'normal',
    ...overrides,
  };
}

describe('session worktree control', () => {
  it('does not treat the selected session status as runtime processing', () => {
    expect(isSessionWorktreeBindingLocked(session({ status: 'active' }), false)).toBe(false);
    expect(isSessionWorktreeBindingLocked(session(), true)).toBe(true);
  });

  it('locks metadata-only history before its dialog turns are hydrated', () => {
    expect(isSessionWorktreeBindingLocked(session({ totalTurnCount: 1 }), false)).toBe(true);
  });

  it('invalidates the composer subscription after hydrate and rebind', () => {
    const initial = sessionWorktreeBindingSubscriptionKey(session());
    const hydrated = sessionWorktreeBindingSubscriptionKey(session({ totalTurnCount: 1 }));
    const rebound = sessionWorktreeBindingSubscriptionKey(session({
      workspacePath: '/worktrees/wt-1',
      projectWorkspacePath: '/repo',
      config: {
        projectWorkspacePath: '/repo',
        executionTarget: {
          kind: 'managedWorktree',
          worktreeId: 'wt-1',
          rootPath: '/worktrees/wt-1',
        },
      },
    }));

    expect(hydrated).not.toBe(initial);
    expect(rebound).not.toBe(initial);
  });
});
