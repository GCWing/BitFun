import { beforeEach, describe, expect, it, vi } from 'vitest';
import { SessionStateMachineImpl } from './SessionStateMachine';
import { SessionExecutionEvent } from './types';

const cancelSessionTask = vi.fn();
const interruptDialogTurn = vi.fn();

vi.mock('@/flow_chat/store/FlowChatStore', () => ({
  flowChatStore: {
    cancelSessionTask,
    getState: () => ({
      sessions: new Map([['session-1', {
        sessionId: 'session-1',
        sessionKind: 'normal',
        mode: 'agentic',
        config: { agentType: 'agentic' },
      }]]),
    }),
  },
}));

vi.mock('@/infrastructure/api/service-api/AgentAPI', () => ({
  agentAPI: {
    interruptDialogTurn,
    cancelDialogTurn: vi.fn(),
  },
}));

vi.mock('@/infrastructure/peer-device/peerModeFlag', () => ({
  isPeerDeviceModeActive: () => false,
}));

describe('SessionStateMachine recoverable interruption', () => {
  beforeEach(() => {
    cancelSessionTask.mockReset();
    interruptDialogTurn.mockReset();
    interruptDialogTurn.mockResolvedValue(undefined);
  });

  it('does not run the legacy optimistic turn persistence before Runtime interruption', async () => {
    const machine = new SessionStateMachineImpl('session-1');
    await machine.transition(SessionExecutionEvent.START, {
      taskId: 'session-1',
      dialogTurnId: 'turn-1',
    });

    await machine.transition(SessionExecutionEvent.USER_CANCEL);

    expect(interruptDialogTurn).toHaveBeenCalledWith('session-1', 'turn-1');
    expect(cancelSessionTask).not.toHaveBeenCalled();
  });

  it('keeps processing when a background command is running after turn completion', async () => {
    const machine = new SessionStateMachineImpl('session-1');
    await machine.transition(SessionExecutionEvent.START, {
      taskId: 'session-1',
      dialogTurnId: 'turn-1',
    });

    // R-WF-25: DialogTurnCompleted with a live background command must keep the
    // session PROCESSING (running icon stays lit) instead of settling to IDLE.
    await machine.transition(SessionExecutionEvent.BACKGROUND_COMMAND_RUNNING);
    expect(machine.getCurrentState()).toBe('processing');
  });
});
