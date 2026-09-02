/**
 * Maps a session state-machine snapshot to the Agent companion mood.
 *
 * Design:
 * - rest: task not running (idle / before start / after completion)
 * - analyzing: model thinking only (THINKING)
 * - waiting: tool invocation / confirmation
 * - working: all other in-flight phases (starting, compacting, streaming, finalizing, or phase cleared between steps)
 */

import {
  SessionExecutionState,
  ProcessingPhase,
  type SessionStateMachine,
} from '../state-machine/types';

export type AgentCompanionMood = 'rest' | 'analyzing' | 'waiting' | 'working';

export function deriveAgentCompanionMood(snapshot: SessionStateMachine | null): AgentCompanionMood {
  if (!snapshot) return 'rest';

  const { currentState, context } = snapshot;
  const phase = context.processingPhase;

  const isProcessing =
    currentState === SessionExecutionState.PROCESSING ||
    currentState === SessionExecutionState.FINISHING;

  if (!isProcessing) {
    return 'rest';
  }

  if (phase === ProcessingPhase.THINKING) {
    return 'analyzing';
  }

  if (phase === ProcessingPhase.TOOL_CALLING || phase === ProcessingPhase.TOOL_CONFIRMING) {
    return 'waiting';
  }

  return 'working';
}
