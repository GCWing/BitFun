import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { DialogTurn, FlowTextItem, ModelRound } from '../../types/flow-chat';
import {
  clearRuntimeStatus,
  scheduleModelResponseStatus,
} from './RuntimeStatusModule';

const SESSION_ID = 'session-1';
const TURN_ID = 'turn-1';
const ROUND_ID = 'round-1';

function createTurn(items: any[] = []): DialogTurn {
  const round: ModelRound = {
    id: ROUND_ID,
    index: 0,
    items,
    isStreaming: true,
    isComplete: false,
    status: 'streaming',
    startTime: 1000,
  };

  return {
    id: TURN_ID,
    sessionId: SESSION_ID,
    userMessage: {
      id: 'user-1',
      content: 'hello',
      timestamp: 900,
    },
    modelRounds: [round],
    status: 'processing',
    startTime: 900,
  };
}

function createContext(turn = createTurn()): any {
  const session = {
    sessionId: SESSION_ID,
    dialogTurns: [turn],
  };

  return {
    runtimeStatusTimers: new Map(),
    activeTextItems: new Map(),
    flowChatStore: {
      getState: () => ({
        sessions: new Map([[SESSION_ID, session]]),
        activeSessionId: SESSION_ID,
      }),
      addModelRoundItem: (_sessionId: string, _turnId: string, item: any, roundId?: string) => {
        const targetRound = turn.modelRounds.find(round => round.id === roundId);
        targetRound?.items.push(item);
      },
      updateDialogTurn: (_sessionId: string, _turnId: string, updater: (next: DialogTurn) => DialogTurn) => {
        const next = updater(turn);
        Object.assign(turn, next);
      },
    },
  };
}

describe('RuntimeStatusModule', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('delays model response status so fast responses do not create UI noise', () => {
    const turn = createTurn();
    const context = createContext(turn);

    scheduleModelResponseStatus(context, SESSION_ID, TURN_ID, ROUND_ID, { delayMs: 1000 });

    vi.advanceTimersByTime(999);
    expect(turn.modelRounds[0].items).toHaveLength(0);

    vi.advanceTimersByTime(1);
    const [statusItem] = turn.modelRounds[0].items as FlowTextItem[];
    expect(statusItem.runtimeStatus?.phase).toBe('waiting_model');
    expect(statusItem.runtimeStatus?.scope).toBe('main');
    expect(statusItem.content).toBe('\u200B');
    expect(context.activeTextItems.get(SESSION_ID)?.get(ROUND_ID)).toBe(statusItem.id);
  });

  it('clears a pending status timer before it can render', () => {
    const turn = createTurn();
    const context = createContext(turn);

    scheduleModelResponseStatus(context, SESSION_ID, TURN_ID, ROUND_ID, { delayMs: 1000 });
    clearRuntimeStatus(context, SESSION_ID, TURN_ID, { roundId: ROUND_ID });

    vi.advanceTimersByTime(1000);
    expect(turn.modelRounds[0].items).toHaveLength(0);
  });

  it('removes an already-rendered runtime status item without touching real output', () => {
    const runtimeItem: FlowTextItem = {
      id: 'runtime-status',
      type: 'text',
      content: '\u200B',
      timestamp: 1001,
      status: 'streaming',
      isStreaming: true,
      isMarkdown: false,
      runtimeStatus: {
        phase: 'waiting_model',
        scope: 'main',
      },
    };
    const realItem: FlowTextItem = {
      id: 'real-text',
      type: 'text',
      content: 'Hello',
      timestamp: 1002,
      status: 'streaming',
      isStreaming: true,
      isMarkdown: true,
    };
    const turn = createTurn([runtimeItem, realItem]);
    const context = createContext(turn);

    clearRuntimeStatus(context, SESSION_ID, TURN_ID, { roundId: ROUND_ID });

    expect(turn.modelRounds[0].items.map(item => item.id)).toEqual(['real-text']);
  });
});
