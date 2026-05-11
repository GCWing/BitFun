import type { DeepReviewQueueStateChangedEvent } from '@/infrastructure/api/service-api/AgentAPI';
import type { DeepReviewCapacityQueueState } from '../store/deepReviewActionBarStore';
import type { Session } from '../types/flow-chat';

export function buildDeepReviewCapacityQueueStateFromEvent(
  event: DeepReviewQueueStateChangedEvent,
  session: Session | undefined,
): DeepReviewCapacityQueueState | null {
  if (session?.sessionKind !== 'deep_review') {
    return null;
  }

  const queueState = event.queueState;
  if (!queueState) {
    return null;
  }

  return {
    toolId: queueState.toolId,
    subagentType: queueState.subagentType,
    dialogTurnId: event.turnId,
    status: queueState.status,
    reason: queueState.reason,
    queuedReviewerCount: Math.max(0, queueState.queuedReviewerCount ?? 0),
    activeReviewerCount: queueState.activeReviewerCount,
    effectiveParallelInstances: queueState.effectiveParallelInstances,
    optionalReviewerCount: queueState.optionalReviewerCount,
    queueElapsedMs: queueState.queueElapsedMs,
    runElapsedMs: queueState.runElapsedMs,
    maxQueueWaitSeconds: queueState.maxQueueWaitSeconds,
    sessionConcurrencyHigh: queueState.sessionConcurrencyHigh,
    controlMode: 'backend',
  };
}
