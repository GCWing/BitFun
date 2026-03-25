/**
 * Session status helpers
 * Session.status is derived from the state machine and activeSessionId.
 */

import { stateMachineManager } from '../state-machine';
import { SessionExecutionState } from '../state-machine/types';

/**
 * Compute the derived session status.
 *
 * @param sessionId Session ID
 * @param activeSessionId Active session ID
 * @returns Derived session status
 */
export function deriveSessionStatus(
  sessionId: string,
  activeSessionId: string | null
): 'active' | 'idle' | 'error' {
  // Active session takes priority.
  if (sessionId === activeSessionId) {
    return 'active';
  }
  
  // Fall back to the state machine execution state.
  const machine = stateMachineManager.get(sessionId);
  if (!machine) {
    return 'idle'; // State machine not initialized.
  }
  
  const executionState = machine.getCurrentState();
  
  // Map execution state to session status.
  switch (executionState) {
    case SessionExecutionState.ERROR:
      return 'error';
    
    case SessionExecutionState.IDLE:
    case SessionExecutionState.PROCESSING:
    case SessionExecutionState.FINISHING:
    default:
      return 'idle'; // Non-active sessions show as idle.
  }
}

/**
 * Get the session error message from the state machine.
 */
export function getSessionError(sessionId: string): string | null {
  const machine = stateMachineManager.get(sessionId);
  if (!machine) return null;
  
  const context = machine.getContext();
  return context.errorMessage;
}

