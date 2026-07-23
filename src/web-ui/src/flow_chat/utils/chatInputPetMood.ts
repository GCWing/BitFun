/**
 * Maps session state machine snapshot to chat input pixel pet mood.
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

export type ChatInputPetMood = 'rest' | 'analyzing' | 'waiting' | 'working';

export function deriveChatInputPetMood(snapshot: SessionStateMachine | null): ChatInputPetMood {
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
