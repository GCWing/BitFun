import { describe, expect, it } from 'vitest';

import { getSubagentProjectionState } from './subagentProjection';
import type { FlowChatState, Session } from '../types/flow-chat';

function createState(session: Session): FlowChatState {
  return {
    sessions: new Map([[session.sessionId, session]]),
    activeSessionId: null,
  };
}

describe('getSubagentProjectionState', () => {
  it('projects only the last round when requested, including a streaming round', () => {
    const session = {
      sessionId: 'subagent-1',
      sessionKind: 'subagent',
      parentSessionId: 'parent-1',
      parentToolCallId: 'task-call-1',
      dialogTurns: [
        {
          id: 'turn-1',
          sessionId: 'subagent-1',
          userMessage: {
            id: 'user-1',
            content: 'do work',
            timestamp: 1,
          },
          modelRounds: [
            {
              id: 'round-1',
              index: 0,
              items: [
                {
                  id: 'text-1',
                  type: 'text',
                  timestamp: 2,
                  status: 'completed',
                  content: 'first round',
                  isStreaming: false,
                },
              ],
              isStreaming: false,
              isComplete: true,
              status: 'completed',
              startTime: 2,
            },
            {
              id: 'round-2',
              index: 1,
              items: [
                {
                  id: 'text-2',
                  type: 'text',
                  timestamp: 3,
                  status: 'streaming',
                  content: 'second round streaming',
                  isStreaming: true,
                },
              ],
              isStreaming: true,
              isComplete: false,
              status: 'streaming',
              startTime: 3,
            },
          ],
          status: 'processing',
          startTime: 1,
        },
      ],
      status: 'idle',
      config: { agentType: 'agentic', modelName: 'auto' },
      createdAt: 1,
      lastActiveAt: 3,
      error: null,
    } as unknown as Session;

    const projection = getSubagentProjectionState(
      createState(session),
      {
        parentSessionId: 'parent-1',
        parentToolIds: new Set(['task-call-1']),
      },
      { itemsMode: 'last-round' },
    );

    expect(projection.session?.sessionId).toBe('subagent-1');
    expect(projection.turn?.id).toBe('turn-1');
    expect(projection.round?.id).toBe('round-2');
    expect(projection.isRunning).toBe(true);
    expect(projection.items).toHaveLength(1);
    expect(projection.items[0]?.id).toBe('text-2');
  });

  it('keeps full-turn projection mode for callers that still need it', () => {
    const session = {
      sessionId: 'subagent-1',
      sessionKind: 'subagent',
      parentSessionId: 'parent-1',
      parentToolCallId: 'task-call-1',
      dialogTurns: [
        {
          id: 'turn-1',
          sessionId: 'subagent-1',
          userMessage: {
            id: 'user-1',
            content: 'do work',
            timestamp: 1,
          },
          modelRounds: [
            {
              id: 'round-1',
              index: 0,
              items: [
                {
                  id: 'text-1',
                  type: 'text',
                  timestamp: 2,
                  status: 'completed',
                  content: 'first round',
                  isStreaming: false,
                },
              ],
              isStreaming: false,
              isComplete: true,
              status: 'completed',
              startTime: 2,
            },
            {
              id: 'round-2',
              index: 1,
              items: [
                {
                  id: 'text-2',
                  type: 'text',
                  timestamp: 3,
                  status: 'completed',
                  content: 'second round',
                  isStreaming: false,
                },
              ],
              isStreaming: false,
              isComplete: true,
              status: 'completed',
              startTime: 3,
            },
          ],
          status: 'completed',
          startTime: 1,
          endTime: 4,
        },
      ],
      status: 'idle',
      config: { agentType: 'agentic', modelName: 'auto' },
      createdAt: 1,
      lastActiveAt: 4,
      error: null,
    } as unknown as Session;

    const projection = getSubagentProjectionState(
      createState(session),
      {
        parentSessionId: 'parent-1',
        parentToolIds: new Set(['task-call-1']),
      },
    );

    expect(projection.round?.id).toBe('round-2');
    expect(projection.items.map(item => item.id)).toEqual(['text-1', 'text-2']);
  });

  it('uses the direct subagent turn id instead of the latest reused-session turn', () => {
    const session = {
      sessionId: 'subagent-1',
      sessionKind: 'subagent',
      parentSessionId: 'parent-1',
      parentToolCallId: 'task-call-2',
      dialogTurns: [
        {
          id: 'turn-1',
          sessionId: 'subagent-1',
          userMessage: {
            id: 'user-1',
            content: 'first task call',
            timestamp: 1,
          },
          modelRounds: [
            {
              id: 'round-1',
              index: 0,
              items: [
                {
                  id: 'text-1',
                  type: 'text',
                  timestamp: 2,
                  status: 'completed',
                  content: 'first task answer',
                  isStreaming: false,
                },
              ],
              isStreaming: false,
              isComplete: true,
              status: 'completed',
              startTime: 2,
            },
          ],
          status: 'completed',
          startTime: 1,
          endTime: 3,
        },
        {
          id: 'turn-2',
          sessionId: 'subagent-1',
          userMessage: {
            id: 'user-2',
            content: 'send input',
            timestamp: 4,
          },
          modelRounds: [
            {
              id: 'round-2',
              index: 0,
              items: [
                {
                  id: 'text-2',
                  type: 'text',
                  timestamp: 5,
                  status: 'streaming',
                  content: 'second task answer',
                  isStreaming: true,
                },
              ],
              isStreaming: true,
              isComplete: false,
              status: 'streaming',
              startTime: 5,
            },
          ],
          status: 'processing',
          startTime: 4,
        },
      ],
      status: 'idle',
      config: { agentType: 'agentic', modelName: 'auto' },
      createdAt: 1,
      lastActiveAt: 5,
      error: null,
    } as unknown as Session;

    const projection = getSubagentProjectionState(
      createState(session),
      {
        parentSessionId: 'parent-1',
        parentToolIds: new Set(['task-call-1']),
        directSubagentSessionId: 'subagent-1',
        directSubagentDialogTurnId: 'turn-1',
      },
      { itemsMode: 'last-round' },
    );

    expect(projection.session?.sessionId).toBe('subagent-1');
    expect(projection.turn?.id).toBe('turn-1');
    expect(projection.round?.id).toBe('round-1');
    expect(projection.isRunning).toBe(false);
    expect(projection.items.map(item => item.id)).toEqual(['text-1']);
  });
});
