import { beforeEach, describe, expect, it, vi } from 'vitest';

import { localSessionDriver } from './LocalSessionDriver';
import type { DialogTurn } from '../../types/flow-chat';

const { mockStartAcpDialogTurn, mockStartAgenticDialogTurn, mockTransition } = vi.hoisted(() => ({
  mockStartAcpDialogTurn: vi.fn(),
  mockStartAgenticDialogTurn: vi.fn(),
  mockTransition: vi.fn(),
}));

vi.mock('@/infrastructure/api/service-api/ACPClientAPI', () => ({
  ACPClientAPI: { startDialogTurn: mockStartAcpDialogTurn },
}));

vi.mock('@/infrastructure/api/service-api/AgentAPI', () => ({
  agentAPI: { startDialogTurn: mockStartAgenticDialogTurn },
}));

vi.mock('@/infrastructure/api/service-api/SessionAPI', () => ({ sessionAPI: {} }));
vi.mock('@/infrastructure/api/service-api/WorktreeAPI', () => ({ worktreeAPI: {} }));

vi.mock('../../state-machine', () => ({
  stateMachineManager: {
    transition: mockTransition,
    getCurrentState: () => 'idle',
    get: () => undefined,
  },
}));

vi.mock('../shared', () => ({ applyGeneratingTitlePlaceholder: vi.fn() }));

vi.mock('../../utils/modelSync', () => ({ syncSessionModelSelection: vi.fn() }));

const SESSION_ID = 'acp_dsh_session';
const WORKSPACE_PATH = '/Users/user/workspace/project';

function persistedTurn(id: string, storageTurnIndex: number): DialogTurn {
  return {
    id,
    sessionId: SESSION_ID,
    userMessage: { id: `user-${id}`, content: 'earlier', timestamp: 1 },
    modelRounds: [],
    status: 'completed',
    startTime: 1,
    storageTurnIndex,
  };
}

function createHarness(existingTurns: DialogTurn[]) {
  const session: any = {
    sessionId: SESSION_ID,
    dialogTurns: [...existingTurns],
    workspacePath: WORKSPACE_PATH,
    config: {},
    mode: 'acp:dsh',
  };
  const addedTurns: DialogTurn[] = [];

  const context: any = {
    flowChatStore: {
      getState: () => ({ sessions: new Map([[SESSION_ID, session]]) }),
      addDialogTurn: (_sessionId: string, turn: DialogTurn) => {
        addedTurns.push(turn);
        session.dialogTurns = [...session.dialogTurns, turn];
      },
      deleteDialogTurn: vi.fn(),
      updateSessionLastSubmittedMode: vi.fn(),
      setSessionWorktreeIsolationRequested: vi.fn(),
    },
    processingManager: {
      registerStatus: vi.fn(),
      clearSessionStatus: vi.fn(),
    },
    pendingHistoryLoads: new Set<string>(),
    contentBuffers: new Map(),
    activeTextItems: new Map(),
  };

  return { context, session, addedTurns };
}

function startTurnInput(session: any) {
  return {
    sessionId: SESSION_ID,
    message: 'hello',
    displayMessage: 'hello',
    currentAgentType: 'acp:dsh',
    acpClientId: 'dsh',
    isFirstMessage: session.dialogTurns.length === 0,
    readySession: session,
    options: undefined,
  } as any;
}

describe('localSessionDriver.startTurn on an ACP session', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockTransition.mockResolvedValue(true);
    mockStartAcpDialogTurn.mockResolvedValue(undefined);
  });

  it('gives the first turn a storage slot so it can be persisted', async () => {
    const { context, session, addedTurns } = createHarness([]);

    await localSessionDriver.startTurn(context, startTurnInput(session), {
      createdLocalTurnId: null,
      hostAcceptedTurn: false,
    });

    expect(mockStartAcpDialogTurn).toHaveBeenCalledTimes(1);
    expect(addedTurns).toHaveLength(1);
    expect(addedTurns[0].storageTurnIndex).toBe(0);
  });

  it('continues after the turns already on disk when the session is resumed', async () => {
    const { context, session, addedTurns } = createHarness([
      persistedTurn('a', 0),
      persistedTurn('b', 1),
    ]);

    await localSessionDriver.startTurn(context, startTurnInput(session), {
      createdLocalTurnId: null,
      hostAcceptedTurn: false,
    });

    expect(addedTurns[0].storageTurnIndex).toBe(2);
  });

  it('leaves the slot to the runtime for a non-ACP turn', async () => {
    const { context, session, addedTurns } = createHarness([]);
    mockStartAgenticDialogTurn.mockResolvedValue(undefined);

    await localSessionDriver.startTurn(
      context,
      { ...startTurnInput(session), acpClientId: undefined, currentAgentType: 'agentic' },
      { createdLocalTurnId: null, hostAcceptedTurn: false },
    );

    expect(mockStartAgenticDialogTurn).toHaveBeenCalledTimes(1);
    expect(addedTurns[0].storageTurnIndex).toBeUndefined();
  });
});
