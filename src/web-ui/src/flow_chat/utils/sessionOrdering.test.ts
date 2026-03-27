import { describe, expect, it } from 'vitest';
import type { Session } from '../types/flow-chat';
import {
  compareSessionsForDisplay,
  getSessionSortTimestamp,
  sessionBelongsToWorkspaceNavRow,
} from './sessionOrdering';

function createSession(overrides: Partial<Session> = {}): Session {
  return {
    sessionId: 'session-1',
    title: 'Session Title',
    titleStatus: 'generated',
    dialogTurns: [],
    status: 'idle',
    config: {
      modelName: 'gpt-test',
      agentType: 'agentic',
    },
    createdAt: 1000,
    lastActiveAt: 1000,
    lastFinishedAt: undefined,
    error: null,
    todos: [],
    maxContextTokens: 128128,
    mode: 'agentic',
    workspacePath: '/workspace',
    parentSessionId: undefined,
    sessionKind: 'normal',
    btwThreads: [],
    btwOrigin: undefined,
    ...overrides,
  };
}

describe('sessionOrdering', () => {
  it('uses createdAt for sessions without completed tasks', () => {
    const session = createSession({ createdAt: 1234 });
    expect(getSessionSortTimestamp(session)).toBe(1234);
  });

  it('sorts sessions by lastFinishedAt before createdAt', () => {
    const sessions = [
      createSession({ sessionId: 'older-new', createdAt: 1000 }),
      createSession({ sessionId: 'completed', createdAt: 500, lastFinishedAt: 3000 }),
      createSession({ sessionId: 'newest-new', createdAt: 2000 }),
    ];

    const orderedIds = [...sessions].sort(compareSessionsForDisplay).map(session => session.sessionId);
    expect(orderedIds).toEqual(['completed', 'newest-new', 'older-new']);
  });

  it('falls back to stable ordering when timestamps are equal', () => {
    const sessions = [
      createSession({ sessionId: 'b', createdAt: 1000 }),
      createSession({ sessionId: 'a', createdAt: 1000 }),
    ];

    const orderedIds = [...sessions].sort(compareSessionsForDisplay).map(session => session.sessionId);
    expect(orderedIds).toEqual(['a', 'b']);
  });

  it('remote SSH: same host but different remote root does not share nav row', () => {
    const conn = 'ssh-user@myserver.example.com:22';
    const host = 'myserver.example.com';
    const rowPath = '/home/u/project-a';
    const otherPath = '/home/u/project-b';

    const sessionA = {
      workspacePath: rowPath,
      remoteConnectionId: conn,
      remoteSshHost: host,
    };
    const sessionB = {
      workspacePath: otherPath,
      remoteConnectionId: conn,
      remoteSshHost: host,
    };

    expect(
      sessionBelongsToWorkspaceNavRow(sessionA, rowPath, conn, host)
    ).toBe(true);
    expect(
      sessionBelongsToWorkspaceNavRow(sessionB, rowPath, conn, host)
    ).toBe(false);
  });
});
