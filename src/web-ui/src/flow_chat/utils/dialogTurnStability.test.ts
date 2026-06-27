import { describe, expect, it } from 'vitest';
import type { DialogTurn } from '../types/flow-chat';
import {
  normalizeRecoveredRoundStatus,
  normalizeRecoveredToolStatus,
  normalizeRecoveredTurnStatus,
  settleInterruptedDialogTurn,
} from './dialogTurnStability';

function createDialogTurn(overrides: Partial<DialogTurn> = {}): DialogTurn {
  return {
    id: 'turn-1',
    sessionId: 'session-1',
    userMessage: {
      id: 'user-1',
      content: 'test',
      timestamp: 1,
    },
    modelRounds: [
      {
        id: 'round-1',
        index: 0,
        items: [
          {
            id: 'tool-1',
            type: 'tool',
            toolName: 'Write',
            toolCall: {
              input: { file_path: 'foo.ts', content: 'hello' },
              id: 'tool-1',
            },
            timestamp: 2,
            status: 'streaming',
            startTime: 2,
            isParamsStreaming: true,
          },
          {
            id: 'text-1',
            type: 'text',
            content: 'partial',
            isStreaming: true,
            timestamp: 3,
            status: 'streaming',
          },
        ],
        isStreaming: true,
        isComplete: false,
        status: 'streaming',
        startTime: 2,
      },
    ],
    status: 'processing',
    startTime: 1,
    error: null,
    ...overrides,
  };
}

describe('dialogTurnStability', () => {
  it('normalizes recovered in-progress turns to cancelled', () => {
    expect(normalizeRecoveredTurnStatus('inprogress', { error: null })).toBe('cancelled');
    expect(normalizeRecoveredTurnStatus('processing', { error: null })).toBe('cancelled');
  });

  it('cancels recovered confirmation states because their runtime approval channel is gone', () => {
    expect(normalizeRecoveredToolStatus('pending_confirmation', 'cancelled', null)).toBe('cancelled');
    expect(normalizeRecoveredToolStatus('confirmed', 'cancelled', null)).toBe('cancelled');
    expect(normalizeRecoveredRoundStatus('pending_confirmation', 'cancelled')).toBe('cancelled');
  });

  it('cancels pending confirmation tools during an explicit cancellation settle', () => {
    const turn = createDialogTurn({
      modelRounds: [
        {
          id: 'round-1',
          index: 0,
          items: [
            {
              id: 'tool-1',
              type: 'tool',
              toolName: 'Terminal',
              toolCall: {
                input: { command: 'echo ok' },
                id: 'tool-1',
              },
              timestamp: 2,
              status: 'pending_confirmation',
              startTime: 2,
            },
          ],
          isStreaming: false,
          isComplete: false,
          status: 'pending_confirmation',
          startTime: 2,
        },
      ],
    });

    const settled = settleInterruptedDialogTurn(turn, 42);
    expect(settled.modelRounds[0].status).toBe('cancelled');
    expect(settled.modelRounds[0].items[0].status).toBe('cancelled');
  });

  it('cancels stale pending confirmation tools even when the persisted turn is already cancelled', () => {
    const turn = createDialogTurn({
      status: 'cancelled',
      endTime: 40,
      modelRounds: [
        {
          id: 'round-1',
          index: 0,
          items: [
            {
              id: 'tool-1',
              type: 'tool',
              toolName: 'Terminal',
              toolCall: {
                input: { command: 'echo ok' },
                id: 'tool-1',
              },
              timestamp: 2,
              status: 'pending_confirmation',
              startTime: 2,
            },
          ],
          isStreaming: false,
          isComplete: false,
          status: 'pending_confirmation',
          startTime: 2,
        },
      ],
    });

    const settled = settleInterruptedDialogTurn(turn, 42);
    const tool = settled.modelRounds[0].items[0];

    expect(settled.status).toBe('cancelled');
    expect(settled.modelRounds[0].status).toBe('cancelled');
    expect(tool.status).toBe('cancelled');
  });

  it('cancels transient nested states when settling an interrupted turn', () => {
    const settledAt = 99;
    const settled = settleInterruptedDialogTurn(createDialogTurn(), settledAt, {
      interruptionReason: 'app_restart',
    });
    const round = settled.modelRounds[0];
    const tool = round.items[0];
    const text = round.items[1];

    expect(settled.status).toBe('cancelled');
    expect(settled.endTime).toBe(settledAt);
    expect(round.status).toBe('cancelled');
    expect(round.isStreaming).toBe(false);
    expect(round.isComplete).toBe(true);
    expect(tool.type).toBe('tool');
    expect(tool.status).toBe('cancelled');
    expect((tool as any).interruptionReason).toBe('app_restart');
    expect((tool as any).isParamsStreaming).toBe(false);
    expect((tool as any).endTime).toBe(settledAt);
    expect(text.type).toBe('text');
    expect(text.status).toBe('cancelled');
    expect((text as any).isStreaming).toBe(false);
  });

  it('cancels transient tool state inside model round attempts', () => {
    const settledAt = 123;
    const toolItem = {
      id: 'call-1',
      type: 'tool' as const,
      toolName: 'ExecCommand',
      toolCall: {
        input: { cmd: 'sleep 60' },
        id: 'call-1',
      },
      timestamp: 2,
      status: 'running' as const,
      startTime: 2,
      isParamsStreaming: false,
      attemptId: 'round-1:attempt:1',
      attemptIndex: 1,
    };
    const turn = createDialogTurn({
      modelRounds: [
        {
          id: 'round-1',
          index: 0,
          items: [toolItem],
          attempts: [
            {
              id: 'round-1:attempt:1',
              index: 1,
              status: 'completed',
              items: [toolItem],
            },
          ],
          isStreaming: false,
          isComplete: true,
          status: 'completed',
          startTime: 2,
        },
      ],
    });

    const settled = settleInterruptedDialogTurn(turn, settledAt);
    const roundItem = settled.modelRounds[0].items[0];
    const attemptItem = settled.modelRounds[0].attempts?.[0]?.items[0];

    expect(settled.status).toBe('cancelled');
    expect(roundItem.status).toBe('cancelled');
    expect((roundItem as any).endTime).toBe(settledAt);
    expect(attemptItem?.status).toBe('cancelled');
    expect((attemptItem as any).endTime).toBe(settledAt);
    expect((attemptItem as any).isParamsStreaming).toBe(false);
  });

  it('preserves completed tools when settling a completed turn', () => {
    const turn = createDialogTurn({
      status: 'completed',
      endTime: 10,
      modelRounds: [
        {
          id: 'round-1',
          index: 0,
          items: [
            {
              id: 'tool-1',
              type: 'tool',
              toolName: 'Terminal',
              toolCall: {
                input: { command: 'echo ok' },
                id: 'tool-1',
              },
              toolResult: {
                result: { stdout: 'ok', exit_code: 0 },
                success: true,
              },
              timestamp: 2,
              status: 'completed',
              startTime: 2,
              endTime: 5,
            },
          ],
          isStreaming: false,
          isComplete: true,
          status: 'completed',
          startTime: 2,
          endTime: 5,
        },
      ],
    });

    const settled = settleInterruptedDialogTurn(turn, 99);
    const tool = settled.modelRounds[0].items[0];

    expect(settled).toEqual(turn);
    expect(tool.status).toBe('completed');
  });
});
