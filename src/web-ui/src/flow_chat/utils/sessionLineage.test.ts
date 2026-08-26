import { describe, expect, it } from 'vitest';
import type {
  SessionLineageEntry,
  SessionLineageSnapshot,
} from '@/infrastructure/api/service-api/SessionAPI';
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
): SessionLineageEntry {
  return {
    sessionId,
    sessionName: sessionId,
    agentType: parentSessionId ? 'Explore' : 'agentic',
    createdAtMs: createdAt,
    status: parentSessionId ? 'completed' : 'active',
    parentSessionId,
    parentToolCallId: parentSessionId ? `tool-${sessionId}` : undefined,
    subagentType: parentSessionId ? 'Explore' : undefined,
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
  it('preserves the authoritative Runtime order in the recursive tree', () => {
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

    expect(tree?.children.map(node => node.sessionId)).toEqual(['child-b', 'child-a']);
    expect(tree?.children[1].children[0].sessionId).toBe('grandchild');
    expect(countSessionLineageDescendants(tree)).toBe(3);
  });

  it('projects an active persisted child as running without a live shell', () => {
    const child = metadata('child', 'root', 2);
    child.activeTurnId = 'turn-child';
    const snapshot: SessionLineageSnapshot = {
      rootSessionId: 'root',
      sessions: [metadata('root'), child],
    };

    const tree = buildSessionLineageTree('root', snapshot, new Map());

    expect(tree?.children[0]).toMatchObject({
      sessionId: 'child',
      lifecycle: 'running',
    });
    expect([...collectExpandedRunningBranches(tree)]).toEqual(['root']);
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

  it('keeps the persisted title and agent ID when a live child shell overlays it', () => {
    const snapshot: SessionLineageSnapshot = {
      rootSessionId: 'root',
      sessions: [
        metadata('root'),
        {
          ...metadata('child', 'root', 2),
          sessionName: 'Review authentication boundary',
          agentId: 'security-review',
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
      agentId: 'security-review',
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
