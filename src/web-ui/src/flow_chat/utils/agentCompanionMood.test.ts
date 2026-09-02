import { describe, expect, it } from 'vitest';
import { deriveAgentCompanionMood } from './agentCompanionMood';
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

describe('deriveAgentCompanionMood', () => {
  it('returns rest when snapshot is null', () => {
    expect(deriveAgentCompanionMood(null)).toBe('rest');
  });

  it('returns rest when idle', () => {
    expect(deriveAgentCompanionMood(makeSnapshot(SessionExecutionState.IDLE, null))).toBe('rest');
  });

  it('maps only THINKING to analyzing', () => {
    expect(
      deriveAgentCompanionMood(
        makeSnapshot(SessionExecutionState.PROCESSING, ProcessingPhase.THINKING),
      ),
    ).toBe('analyzing');
  });

  it('maps starting and compacting to working', () => {
    expect(
      deriveAgentCompanionMood(
        makeSnapshot(SessionExecutionState.PROCESSING, ProcessingPhase.STARTING),
      ),
    ).toBe('working');
    expect(
      deriveAgentCompanionMood(
        makeSnapshot(SessionExecutionState.PROCESSING, ProcessingPhase.COMPACTING),
      ),
    ).toBe('working');
  });

  it('maps tool phases to waiting', () => {
    expect(
      deriveAgentCompanionMood(
        makeSnapshot(SessionExecutionState.PROCESSING, ProcessingPhase.TOOL_CALLING),
      ),
    ).toBe('waiting');
    expect(
      deriveAgentCompanionMood(
        makeSnapshot(SessionExecutionState.PROCESSING, ProcessingPhase.TOOL_CONFIRMING),
      ),
    ).toBe('waiting');
  });

  it('maps streaming, finalizing, and null phase to working', () => {
    expect(
      deriveAgentCompanionMood(
        makeSnapshot(SessionExecutionState.PROCESSING, ProcessingPhase.STREAMING),
      ),
    ).toBe('working');
    expect(
      deriveAgentCompanionMood(
        makeSnapshot(SessionExecutionState.PROCESSING, ProcessingPhase.FINALIZING),
      ),
    ).toBe('working');
    expect(
      deriveAgentCompanionMood(makeSnapshot(SessionExecutionState.PROCESSING, null)),
    ).toBe('working');
  });

  it('treats finishing state like processing for mood', () => {
    expect(
      deriveAgentCompanionMood(
        makeSnapshot(SessionExecutionState.FINISHING, ProcessingPhase.FINALIZING),
      ),
    ).toBe('working');
  });
});
