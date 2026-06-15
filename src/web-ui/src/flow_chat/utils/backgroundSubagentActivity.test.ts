import { describe, expect, it } from 'vitest';
import {
  buildBackgroundSubagentActivityIndex,
  deriveBackgroundSubagentActivity,
} from './backgroundSubagentActivity';
import { SessionExecutionState } from '../state-machine/types';
import type { FlowChatState, Session } from '../types/flow-chat';

function createSession(overrides: Partial<Session>): Session {
  return {
    sessionId: 'session-1',
    title: 'Session',
    dialogTurns: [],
    status: 'idle',
    config: {},
    createdAt: 1000,
    lastActiveAt: 1000,
    error: null,
    sessionKind: 'normal',
    ...overrides,
  } as Session;
}

function createParentSession(): Session {
  return createSession({
    sessionId: 'parent-1',
    title: 'Parent',
    dialogTurns: [
      {
        id: 'turn-1',
        status: 'completed',
        modelRounds: [
          {
            id: 'round-1',
            items: [
              {
                id: 'tool-1',
                type: 'tool',
                toolName: 'Task',
                subagentSessionId: 'subagent-1',
                toolCall: {
                  id: 'call-1',
                  input: {
                    run_in_background: true,
                    description: 'Review auth changes',
                    subagent_type: 'ReviewSecurity',
                  },
                },
              },
            ],
          },
        ],
      },
    ],
  } as Partial<Session>);
}

function createState(sessions: Session[]): FlowChatState {
  return {
    activeSessionId: 'parent-1',
    sessions: new Map(sessions.map(session => [session.sessionId, session])),
  };
}

describe('backgroundSubagentActivity', () => {
  it('indexes running background subagents under an idle parent session', () => {
    const parent = createParentSession();
    const subagent = createSession({
      sessionId: 'subagent-1',
      title: '',
      parentSessionId: 'parent-1',
      sessionKind: 'subagent',
      createdAt: 2000,
      lastActiveAt: 3000,
      dialogTurns: [{ status: 'completed', modelRounds: [] }] as any,
    });

    const index = buildBackgroundSubagentActivityIndex(
      createState([parent, subagent]).sessions,
      sessionId => sessionId === 'subagent-1' ? SessionExecutionState.PROCESSING : SessionExecutionState.IDLE,
    );

    expect(index.get('parent-1')).toMatchObject({
      runningCount: 1,
      finishingCount: 0,
      totalCount: 1,
      items: [
        {
          sessionId: 'subagent-1',
          parentSessionId: 'parent-1',
          title: 'Review auth changes',
          agentType: 'ReviewSecurity',
          status: 'processing',
          parentToolCallId: 'call-1',
        },
      ],
    });
  });

  it('ignores foreground subagents and completed background subagents', () => {
    const foregroundParent = createSession({
      sessionId: 'parent-2',
      dialogTurns: [
        {
          id: 'turn-2',
          status: 'completed',
          modelRounds: [
            {
              id: 'round-2',
              items: [
                {
                  id: 'tool-2',
                  type: 'tool',
                  toolName: 'Task',
                  subagentSessionId: 'subagent-2',
                  toolCall: {
                    id: 'call-2',
                    input: { run_in_background: false, description: 'Foreground task' },
                  },
                },
              ],
            },
          ],
        },
      ],
    } as Partial<Session>);
    const foregroundSubagent = createSession({
      sessionId: 'subagent-2',
      parentSessionId: 'parent-2',
      sessionKind: 'subagent',
      dialogTurns: [{ status: 'processing', modelRounds: [] }] as any,
    });
    const completedSubagent = createSession({
      sessionId: 'subagent-1',
      parentSessionId: 'parent-1',
      sessionKind: 'subagent',
      dialogTurns: [{ status: 'completed', modelRounds: [] }] as any,
    });

    const activity = deriveBackgroundSubagentActivity(
      createState([createParentSession(), completedSubagent, foregroundParent, foregroundSubagent]),
      'parent-1',
    );

    expect(activity.totalCount).toBe(0);
  });
});
