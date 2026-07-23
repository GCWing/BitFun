import { describe, expect, it } from 'vitest';
import { deriveChatInputPetMood } from './chatInputPetMood';
import {
  SessionExecutionState,
  ProcessingPhase,
  type SessionStateMachine,
} from '../state-machine/types';

function makeSnapshot(
  state: SessionExecutionState,
  phase: ProcessingPhase | null,
): SessionStateMachine {
  return {
    sessionId: 's1',
    currentState: state,
    context: {
      taskId: null,
      currentDialogTurnId: null,
      currentModelRoundId: null,
      pendingToolConfirmations: new Set(),
      errorMessage: null,
      queuedInput: null,
      processingPhase: phase,
      planner: null,
      stats: {
        startTime: null,
        textCharsGenerated: 0,
        toolsExecuted: 0,
      },
      version: 1,
      lastUpdateTime: 0,
      backendSyncedAt: null,
      errorRecovery: {
        errorCount: 0,
        lastErrorTime: null,
        errorType: null,
        recoverable: false,
      },
    },
    transitionHistory: [],
  };
}

describe('deriveChatInputPetMood', () => {
  it('returns rest when snapshot is null', () => {
    expect(deriveChatInputPetMood(null)).toBe('rest');
  });

  it('returns rest when idle', () => {
    expect(deriveChatInputPetMood(makeSnapshot(SessionExecutionState.IDLE, null))).toBe('rest');
  });

  it('maps only THINKING to analyzing', () => {
    expect(
      deriveChatInputPetMood(
        makeSnapshot(SessionExecutionState.PROCESSING, ProcessingPhase.THINKING),
      ),
    ).toBe('analyzing');
  });

  it('maps starting and compacting to working', () => {
    expect(
      deriveChatInputPetMood(
        makeSnapshot(SessionExecutionState.PROCESSING, ProcessingPhase.STARTING),
      ),
    ).toBe('working');
    expect(
      deriveChatInputPetMood(
        makeSnapshot(SessionExecutionState.PROCESSING, ProcessingPhase.COMPACTING),
      ),
    ).toBe('working');
  });

  it('maps tool phases to waiting', () => {
    expect(
      deriveChatInputPetMood(
        makeSnapshot(SessionExecutionState.PROCESSING, ProcessingPhase.TOOL_CALLING),
      ),
    ).toBe('waiting');
    expect(
      deriveChatInputPetMood(
        makeSnapshot(SessionExecutionState.PROCESSING, ProcessingPhase.TOOL_CONFIRMING),
      ),
    ).toBe('waiting');
  });

  it('maps streaming, finalizing, and null phase to working', () => {
    expect(
      deriveChatInputPetMood(
        makeSnapshot(SessionExecutionState.PROCESSING, ProcessingPhase.STREAMING),
      ),
    ).toBe('working');
    expect(
      deriveChatInputPetMood(
        makeSnapshot(SessionExecutionState.PROCESSING, ProcessingPhase.FINALIZING),
      ),
    ).toBe('working');
    expect(
      deriveChatInputPetMood(makeSnapshot(SessionExecutionState.PROCESSING, null)),
    ).toBe('working');
  });

  it('treats finishing state like processing for mood', () => {
    expect(
      deriveChatInputPetMood(
        makeSnapshot(SessionExecutionState.FINISHING, ProcessingPhase.FINALIZING),
      ),
    ).toBe('working');
  });
});
