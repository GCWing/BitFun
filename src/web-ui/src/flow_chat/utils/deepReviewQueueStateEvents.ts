import type { DeepReviewQueueStateChangedEvent } from '@/infrastructure/api/service-api/AgentAPI';
import type {
  DeepReviewCapacityQueueState,
  DeepReviewCapacityWaitingReviewer,
} from '../store/deepReviewActionBarStore';
import type { Session } from '../types/flow-chat';

function resolveWaitingReviewerDisplayName(
  session: Session | undefined,
  subagentType: string | undefined,
): string | undefined {
  if (!subagentType) {
    return undefined;
  }

  const manifest = session?.deepReviewRunManifest;
  const packet = manifest?.workPackets?.find((item) => item.subagentId === subagentType);
  if (packet?.displayName) {
    return packet.displayName;
  }

  const manifestMember = [
    ...(manifest?.coreReviewers ?? []),
    ...(manifest?.enabledExtraReviewers ?? []),
    ...(manifest?.qualityGateReviewer ? [manifest.qualityGateReviewer] : []),
  ].find((member) => member.subagentId === subagentType);

  return manifestMember?.displayName;
}

function buildWaitingReviewerFromEvent(
  event: DeepReviewQueueStateChangedEvent,
  session: Session | undefined,
): DeepReviewCapacityWaitingReviewer | null {
  const queueState = event.queueState;
  if (queueState.status === 'running' || queueState.status === 'capacity_skipped') {
    return null;
  }

  return {
    toolId: queueState.toolId,
    subagentType: queueState.subagentType,
    displayName: resolveWaitingReviewerDisplayName(session, queueState.subagentType),
    status: queueState.status,
    reason: queueState.reason,
    optional: (queueState.optionalReviewerCount ?? 0) > 0,
    queueElapsedMs: queueState.queueElapsedMs,
    maxQueueWaitSeconds: queueState.maxQueueWaitSeconds,
  };
}

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
  const waitingReviewer = buildWaitingReviewerFromEvent(event, session);

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
    waitingReviewers: waitingReviewer ? [waitingReviewer] : [],
  };
}
