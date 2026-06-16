import { afterEach, describe, expect, it } from 'vitest';
import {
  useBackgroundSubagentActivityStore,
  visibleBackgroundSubagentActivitiesForSession,
} from './backgroundSubagentActivityStore';
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

function createParentSession(subagentSessionId = 'subagent-1'): Session {
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
                subagentSessionId,
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
  } as FlowChatState;
}

describe('backgroundSubagentActivityStore', () => {
  afterEach(() => {
    useBackgroundSubagentActivityStore.getState().clear();
  });

  it('upserts one running background subagent from flow state', () => {
    const subagent = createSession({
      sessionId: 'subagent-1',
      title: '',
      parentSessionId: 'parent-1',
      sessionKind: 'subagent',
      createdAt: 2000,
      lastActiveAt: 3000,
      dialogTurns: [{ status: 'processing', modelRounds: [] }] as any,
    });

    useBackgroundSubagentActivityStore
      .getState()
      .reconcileSession(createState([createParentSession(), subagent]), 'subagent-1');

    expect(useBackgroundSubagentActivityStore.getState().activities['subagent-1']).toMatchObject({
      sessionId: 'subagent-1',
      parentSessionId: 'parent-1',
      title: 'Review auth changes',
      agentType: 'ReviewSecurity',
      status: 'processing',
    });
  });

  it('removes an activity when the subagent is no longer active', () => {
    const runningSubagent = createSession({
      sessionId: 'subagent-1',
      parentSessionId: 'parent-1',
      sessionKind: 'subagent',
      dialogTurns: [{ status: 'processing', modelRounds: [] }] as any,
    });
    const completedSubagent = {
      ...runningSubagent,
      dialogTurns: [{ status: 'completed', modelRounds: [] }],
    } as Session;

    const store = useBackgroundSubagentActivityStore.getState();
    store.reconcileSession(createState([createParentSession(), runningSubagent]), 'subagent-1');
    store.reconcileSession(createState([createParentSession(), completedSubagent]), 'subagent-1');

    expect(useBackgroundSubagentActivityStore.getState().activities['subagent-1']).toBeUndefined();
  });

  it('reconciles one parent session and preserves unrelated parent activities', () => {
    const parentTwo = createSession({
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
                    input: { run_in_background: true, description: 'Docs pass' },
                  },
                },
              ],
            },
          ],
        },
      ],
    } as Partial<Session>);
    const subagentOne = createSession({
      sessionId: 'subagent-1',
      parentSessionId: 'parent-1',
      sessionKind: 'subagent',
      dialogTurns: [{ status: 'completed', modelRounds: [] }] as any,
    });
    const subagentTwo = createSession({
      sessionId: 'subagent-2',
      parentSessionId: 'parent-2',
      sessionKind: 'subagent',
      dialogTurns: [{ status: 'processing', modelRounds: [] }] as any,
    });

    const store = useBackgroundSubagentActivityStore.getState();
    store.reconcileSession(createState([parentTwo, subagentTwo]), 'subagent-2');
    store.reconcileParent(createState([createParentSession(), subagentOne, parentTwo, subagentTwo]), 'parent-1');

    expect(visibleBackgroundSubagentActivitiesForSession(
      useBackgroundSubagentActivityStore.getState().activities,
      'parent-1',
    )).toEqual([]);
    expect(visibleBackgroundSubagentActivitiesForSession(
      useBackgroundSubagentActivityStore.getState().activities,
      'parent-2',
    )).toHaveLength(1);
  });
});
