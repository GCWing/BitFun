import { describe, expect, it } from 'vitest';
import type { Session } from '../types/flow-chat';
import {
  compareSessionMetadataForDisplay,
  compareSessionsForDisplay,
  compareSessionsForNavStable,
  getSessionMetadataSortTimestamp,
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
    lastActiveAt: undefined,
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
  it('uses lastFinishedAt when available', () => {
    const session = createSession({ createdAt: 1234, lastActiveAt: 5678, lastFinishedAt: 9999 });
    expect(getSessionSortTimestamp(session)).toBe(9999);
  });

  it('uses createdAt as fallback', () => {
    const session = createSession({ createdAt: 1234, lastActiveAt: 5678 });
    expect(getSessionSortTimestamp(session)).toBe(1234);
  });

  it('does not move a switched or streaming session above newer display timestamps', () => {
    const sessions = [
      createSession({ sessionId: 'older-new', createdAt: 1000 }),
      createSession({ sessionId: 'completed', createdAt: 500, lastFinishedAt: 3000 }),
      createSession({ sessionId: 'switched-or-streaming', createdAt: 200, lastActiveAt: 5000 }),
      createSession({ sessionId: 'newest-new', createdAt: 2000 }),
    ];

    const orderedIds = [...sessions].sort(compareSessionsForDisplay).map(session => session.sessionId);
    expect(orderedIds).toEqual(['completed', 'newest-new', 'older-new', 'switched-or-streaming']);
  });

  it('falls back to stable ordering when timestamps are equal', () => {
    const sessions = [
      createSession({ sessionId: 'b', createdAt: 1000 }),
      createSession({ sessionId: 'a', createdAt: 1000 }),
    ];

    const orderedIds = [...sessions].sort(compareSessionsForDisplay).map(session => session.sessionId);
    expect(orderedIds).toEqual(['a', 'b']);
  });

  it('nav stable sort ignores lastActiveAt so order does not change on session switch', () => {
    const sessions = [
      createSession({ sessionId: 'first', createdAt: 3000, lastActiveAt: 100 }),
      createSession({ sessionId: 'second', createdAt: 2000, lastActiveAt: 99999 }),
    ];
    const orderedIds = [...sessions].sort(compareSessionsForNavStable).map(s => s.sessionId);
    expect(orderedIds).toEqual(['first', 'second']);
  });

  it('sorts persisted metadata by lastFinishedAt before createdAt without using lastActiveAt', () => {
    const metadata = [
      { sessionId: 'older-new', createdAt: 1000, lastActiveAt: 9000 },
      { sessionId: 'completed', createdAt: 500, lastActiveAt: 600, customMetadata: { lastFinishedAt: 3000 } },
      { sessionId: 'newest-new', createdAt: 2000, lastActiveAt: 2500 },
    ];

    expect(getSessionMetadataSortTimestamp(metadata[0])).toBe(1000);
    const orderedIds = [...metadata].sort(compareSessionMetadataForDisplay).map(session => session.sessionId);
    expect(orderedIds).toEqual(['completed', 'newest-new', 'older-new']);
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

  it('remote SSH: parses stable connection ids without ports when host metadata is absent', () => {
    const session = {
      workspacePath: '/home/u/project-a',
      remoteConnectionId: 'ssh-user@myserver.example.com:22',
      remoteSshHost: undefined,
    };

    expect(
      sessionBelongsToWorkspaceNavRow(
        session,
        '/home/u/project-a',
        'ssh-user@myserver.example.com',
        undefined
      )
    ).toBe(true);
  });
});
