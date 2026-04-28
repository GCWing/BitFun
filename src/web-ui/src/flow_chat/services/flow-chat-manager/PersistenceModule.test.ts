import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { DialogTurn, FlowTextItem, ModelRound } from '../../types/flow-chat';
import {
  convertDialogTurnToBackendFormat,
  immediateSaveDialogTurn,
  saveDialogTurnToDisk,
} from './PersistenceModule';

const saveSessionTurn = vi.fn();
const saveSessionMetadata = vi.fn();
const loadSessionMetadata = vi.fn();

vi.mock('@/infrastructure/api', () => ({
  sessionAPI: {
    saveSessionTurn,
    saveSessionMetadata,
    loadSessionMetadata,
  },
}));

const SESSION_ID = 'session-1';
const TURN_ID = 'turn-1';

function createDialogTurn(status: DialogTurn['status'] = 'processing'): DialogTurn {
  const round: ModelRound = {
    id: 'round-1',
    index: 0,
    items: [],
    isStreaming: status !== 'completed',
    isComplete: status === 'completed',
    status: status === 'completed' ? 'completed' : 'streaming',
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
    status,
    startTime: 900,
    endTime: status === 'completed' ? 1200 : undefined,
  };
}

function createContext(dialogTurn: DialogTurn): any {
  const session = {
    sessionId: SESSION_ID,
    dialogTurns: [dialogTurn],
    workspacePath: 'D:/workspace/BitFun',
    createdAt: 1,
    lastActiveAt: 2,
    status: 'active',
    config: {},
    error: null,
    sessionKind: 'normal',
  };

  return {
    saveDebouncers: new Map(),
    lastSaveTimestamps: new Map(),
    lastSaveHashes: new Map(),
    turnSaveInFlight: new Map(),
    turnSavePending: new Set(),
    flowChatStore: {
      getState: () => ({
        sessions: new Map([[SESSION_ID, session]]),
        activeSessionId: SESSION_ID,
      }),
    },
  };
}

async function flushMicrotasks(): Promise<void> {
  await Promise.resolve();
  await Promise.resolve();
}

describe('PersistenceModule', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    saveSessionTurn.mockResolvedValue(undefined);
    saveSessionMetadata.mockResolvedValue(undefined);
    loadSessionMetadata.mockResolvedValue(null);
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.clearAllMocks();
  });

  it('filters transient runtime status items from persisted text items', () => {
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
      content: 'Visible answer',
      timestamp: 1002,
      status: 'completed',
      isStreaming: false,
      isMarkdown: true,
    };
    const turn = createDialogTurn('processing');
    turn.modelRounds[0].items = [runtimeItem, realItem];

    const persisted = convertDialogTurnToBackendFormat(turn, 0);

    expect(persisted.modelRounds[0].textItems.map((item: any) => item.id)).toEqual(['real-text']);
  });

  it('coalesces non-terminal immediate saves into a short latest-state window', async () => {
    const turn = createDialogTurn('processing');
    const context = createContext(turn);

    immediateSaveDialogTurn(context, SESSION_ID, TURN_ID);
    immediateSaveDialogTurn(context, SESSION_ID, TURN_ID);

    await flushMicrotasks();
    expect(saveSessionTurn).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(499);
    expect(saveSessionTurn).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(1);
    await flushMicrotasks();
    expect(saveSessionTurn).toHaveBeenCalledTimes(1);
  });

  it('flushes terminal turn saves immediately', async () => {
    const turn = createDialogTurn('completed');
    const context = createContext(turn);

    immediateSaveDialogTurn(context, SESSION_ID, TURN_ID);
    await vi.advanceTimersByTimeAsync(0);
    await flushMicrotasks();

    expect(saveSessionTurn).toHaveBeenCalledTimes(1);
    expect(context.saveDebouncers.size).toBe(0);
  });

  it('clears pending delayed saves when saving directly', async () => {
    const turn = createDialogTurn('processing');
    const context = createContext(turn);

    immediateSaveDialogTurn(context, SESSION_ID, TURN_ID);
    expect(context.saveDebouncers.size).toBe(1);

    await saveDialogTurnToDisk(context, SESSION_ID, TURN_ID);
    await flushMicrotasks();

    expect(saveSessionTurn).toHaveBeenCalledTimes(1);
    expect(context.saveDebouncers.size).toBe(0);

    await vi.advanceTimersByTimeAsync(500);
    await flushMicrotasks();
    expect(saveSessionTurn).toHaveBeenCalledTimes(1);
  });
});
