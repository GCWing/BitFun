import { describe, expect, it } from 'vitest';
import type { SessionLineageSnapshot } from '@/infrastructure/api/service-api/SessionAPI';
import type { SessionMetadata } from '@/shared/types/session-history';
import type { Session } from '../types/flow-chat';
import {
  buildSessionLineageTree,
  collectExpandedRunningBranches,
  countSessionLineageDescendants,
  hasActiveSessionLineageDescendants,
} from './sessionLineage';

function metadata(
  sessionId: string,
  parentSessionId?: string,
  createdAt = 1,
): SessionMetadata {
  return {
    sessionId,
    sessionName: sessionId,
    agentType: parentSessionId ? 'Explore' : 'agentic',
    modelName: 'model',
    createdAt,
    lastActiveAt: createdAt,
    turnCount: 0,
    messageCount: 0,
    toolCallCount: 0,
    status: parentSessionId ? 'completed' : 'active',
    tags: [],
    relationship: parentSessionId
      ? { kind: 'subagent', parentSessionId, parentToolCallId: `tool-${sessionId}` }
      : undefined,
  };
}

function liveSession(sessionId: string, parentSessionId: string): Session {
  return {
    sessionId,
    title: sessionId,
    dialogTurns: [{
      id: `turn-${sessionId}`,
      turnIndex: 0,
      userMessage: { id: 'user', content: 'work', timestamp: 1 },
      modelRounds: [],
      status: 'processing',
      startTime: 1,
    }],
    status: 'idle',
    config: {},
    createdAt: 3,
    lastActiveAt: 3,
    error: null,
    parentSessionId,
    sessionKind: 'subagent',
  };
}

describe('sessionLineage', () => {
  it('builds a stable recursive tree from persisted metadata', () => {
    const snapshot: SessionLineageSnapshot = {
      rootSessionId: 'root',
      sessions: [
        metadata('root'),
        metadata('child-b', 'root', 3),
        metadata('child-a', 'root', 2),
        metadata('grandchild', 'child-a', 4),
      ],
    };

    const tree = buildSessionLineageTree('root', snapshot, new Map());

    expect(tree?.children.map(node => node.sessionId)).toEqual(['child-a', 'child-b']);
    expect(tree?.children[0].children[0].sessionId).toBe('grandchild');
    expect(countSessionLineageDescendants(tree)).toBe(3);
  });

  it('overlays live descendants and expands their ancestor branches', () => {
    const snapshot: SessionLineageSnapshot = {
      rootSessionId: 'root',
      sessions: [metadata('root'), metadata('child', 'root', 2)],
    };
    const live = liveSession('grandchild', 'child');

    const tree = buildSessionLineageTree(
      'root',
      snapshot,
      new Map([[live.sessionId, live]]),
    );

    expect(tree?.children[0].children[0]).toMatchObject({
      sessionId: 'grandchild',
      lifecycle: 'running',
    });
    expect([...collectExpandedRunningBranches(tree)]).toEqual(
      expect.arrayContaining(['root', 'child']),
    );
  });

  it('keeps the persisted title when a live child session has a placeholder title', () => {
    const snapshot: SessionLineageSnapshot = {
      rootSessionId: 'root',
      sessions: [
        metadata('root'),
        {
          ...metadata('child', 'root', 2),
          sessionName: 'Review authentication boundary',
        },
      ],
    };
    const live = liveSession('child', 'root');
    live.title = 'Child session';

    const tree = buildSessionLineageTree(
      'root',
      snapshot,
      new Map([[live.sessionId, live]]),
    );

    expect(tree?.children[0]).toMatchObject({
      sessionId: 'child',
      title: 'Review authentication boundary',
      lifecycle: 'running',
    });
  });

  it('detects active foreground descendants through the live session hierarchy', () => {
    const completedChild = liveSession('child', 'root');
    completedChild.dialogTurns[0].status = 'completed';
    const runningGrandchild = liveSession('grandchild', 'child');

    expect(hasActiveSessionLineageDescendants(
      'root',
      new Map([
        [completedChild.sessionId, completedChild],
        [runningGrandchild.sessionId, runningGrandchild],
      ]),
    )).toBe(true);

    runningGrandchild.dialogTurns[0].status = 'completed';
    expect(hasActiveSessionLineageDescendants(
      'root',
      new Map([
        [completedChild.sessionId, completedChild],
        [runningGrandchild.sessionId, runningGrandchild],
      ]),
    )).toBe(false);
  });
});
