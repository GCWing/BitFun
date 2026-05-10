import { describe, expect, it } from 'vitest';
import {
  DEEP_REVIEW_SESSION_CONCURRENCY_WARNING_THRESHOLD,
  deriveDeepReviewSessionConcurrencyGuard,
} from './deepReviewCapacityGuard';
import type { FlowChatState, FlowToolItem, Session } from '../types/flow-chat';

function createTaskItem(id: string, status: FlowToolItem['status']): FlowToolItem {
  return {
    id,
    type: 'tool',
    toolName: 'Task',
    timestamp: 1000,
    status,
    toolCall: {
      id,
      input: { subagent_type: 'ReviewSecurity' },
    },
  };
}

function createSession(items: FlowToolItem[]): Session {
  return {
    sessionId: 'parent-session',
    sessionKind: 'normal',
    status: 'active',
    createdAt: 1000,
    updatedAt: 2000,
    lastActiveAt: 2000,
    dialogTurns: [
      {
        id: 'turn-1',
        status: 'processing',
        modelRounds: [
          {
            id: 'round-1',
            items,
          },
        ],
      } as any,
    ],
  } as Session;
}

function createState(session: Session): FlowChatState {
  return {
    sessions: new Map([[session.sessionId, session]]),
    activeSessionId: session.sessionId,
  } as FlowChatState;
}

describe('deriveDeepReviewSessionConcurrencyGuard', () => {
  it('warns when the target session already has multiple active Task subagents', () => {
    const state = createState(createSession([
      createTaskItem('task-1', 'running'),
      createTaskItem('task-2', 'streaming'),
    ]));

    const guard = deriveDeepReviewSessionConcurrencyGuard(state, 'parent-session');

    expect(guard.activeSubagentCount).toBe(DEEP_REVIEW_SESSION_CONCURRENCY_WARNING_THRESHOLD);
    expect(guard.highActivity).toBe(true);
  });

  it('ignores completed Task subagents and unrelated sessions', () => {
    const targetSession = createSession([
      createTaskItem('task-1', 'completed'),
    ]);
    const unrelatedSession = {
      ...createSession([createTaskItem('task-2', 'running')]),
      sessionId: 'unrelated-session',
    } as Session;
    const state = {
      sessions: new Map([
        [targetSession.sessionId, targetSession],
        [unrelatedSession.sessionId, unrelatedSession],
      ]),
      activeSessionId: targetSession.sessionId,
    } as FlowChatState;

    const guard = deriveDeepReviewSessionConcurrencyGuard(state, 'parent-session');

    expect(guard.activeSubagentCount).toBe(0);
    expect(guard.highActivity).toBe(false);
  });
});
