import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { DialogTurn, ModelRound } from '../../types/flow-chat';
import {
  clearRuntimeStatus,
  scheduleModelResponseStatus,
} from './RuntimeStatusModule';
import {
  getRuntimeStatus,
  showRuntimeStatus,
  useRuntimeStatusStore,
} from '../../store/runtimeStatusStore';

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
    },
  };
}

describe('RuntimeStatusModule', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    useRuntimeStatusStore.getState().reset();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('delays model response status without adding a model round item', () => {
    const turn = createTurn();
    const context = createContext(turn);

    scheduleModelResponseStatus(context, SESSION_ID, TURN_ID, ROUND_ID, { delayMs: 1000 });

    vi.advanceTimersByTime(999);
    expect(getRuntimeStatus(SESSION_ID)).toBeUndefined();

    vi.advanceTimersByTime(1);
    expect(getRuntimeStatus(SESSION_ID)).toEqual({
      sessionId: SESSION_ID,
      turnId: TURN_ID,
      roundId: ROUND_ID,
    });
    expect(turn.modelRounds[0].items).toHaveLength(0);
    expect(context.activeTextItems.size).toBe(0);
  });

  it('does not show a delayed status after visible round output arrives', () => {
    const turn = createTurn([{ id: 'text-1', type: 'text' }]);
    const context = createContext(turn);

    scheduleModelResponseStatus(context, SESSION_ID, TURN_ID, ROUND_ID, { delayMs: 1000 });
    vi.advanceTimersByTime(1000);

    expect(getRuntimeStatus(SESSION_ID)).toBeUndefined();
  });

  it('clears a pending timer and an already-visible session status', () => {
    const context = createContext();
    scheduleModelResponseStatus(context, SESSION_ID, TURN_ID, ROUND_ID, { delayMs: 1000 });
    clearRuntimeStatus(context, SESSION_ID, TURN_ID, { roundId: ROUND_ID });
    vi.advanceTimersByTime(1000);
    expect(getRuntimeStatus(SESSION_ID)).toBeUndefined();

    showRuntimeStatus({
      sessionId: SESSION_ID,
      turnId: TURN_ID,
      roundId: ROUND_ID,
    });
    clearRuntimeStatus(context, SESSION_ID, TURN_ID, { roundId: ROUND_ID });
    expect(getRuntimeStatus(SESSION_ID)).toBeUndefined();
  });

  it('does not let an older round clear the status owned by a newer round', () => {
    const context = createContext();
    showRuntimeStatus({
      sessionId: SESSION_ID,
      turnId: TURN_ID,
      roundId: 'round-2',
    });

    clearRuntimeStatus(context, SESSION_ID, TURN_ID, { roundId: ROUND_ID });

    expect(getRuntimeStatus(SESSION_ID)?.roundId).toBe('round-2');
  });

  it('cancels an older pending round when a newer round owns the session slot', () => {
    const turn = createTurn();
    turn.modelRounds.push({
      ...turn.modelRounds[0],
      id: 'round-2',
      index: 1,
      items: [],
    });
    const context = createContext(turn);

    scheduleModelResponseStatus(context, SESSION_ID, TURN_ID, ROUND_ID, { delayMs: 500 });
    scheduleModelResponseStatus(context, SESSION_ID, TURN_ID, 'round-2', { delayMs: 1000 });

    vi.advanceTimersByTime(500);
    expect(getRuntimeStatus(SESSION_ID)).toBeUndefined();

    vi.advanceTimersByTime(500);
    expect(getRuntimeStatus(SESSION_ID)?.roundId).toBe('round-2');
  });
});
