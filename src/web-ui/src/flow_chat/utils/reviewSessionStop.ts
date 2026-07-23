import { stateMachineManager } from '../state-machine';
import { SessionExecutionEvent, SessionExecutionState } from '../state-machine/types';
import { flowChatStore } from '../store/FlowChatStore';

function isStreamingState(state: SessionExecutionState | undefined): boolean {
  return state === SessionExecutionState.PROCESSING || state === SessionExecutionState.FINISHING;
}

export async function settleStoppedReviewSessionState(sessionId: string): Promise<void> {
  // The caller owns backend cancellation; this helper only settles the local review UI promptly.
  flowChatStore.cancelSessionTask(sessionId);

  const currentState = stateMachineManager.getCurrentState(sessionId);
  if (!isStreamingState(currentState)) {
    return;
  }

  await stateMachineManager.transition(sessionId, SessionExecutionEvent.FINISHING_SETTLED);
}
