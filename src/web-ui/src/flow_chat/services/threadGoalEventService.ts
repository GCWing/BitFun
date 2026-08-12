import { createLogger } from '@/shared/utils/logger';
import { flowChatStore } from '../store/FlowChatStore';
import type { ThreadGoalSnapshot } from './goalService';

const log = createLogger('threadGoalEventService');

export interface ThreadGoalUpdatedPayload {
  sessionId: string;
  goal?: {
    goalId?: string;
    objective?: string;
    status?: string;
    tokensUsed?: number;
    tokenBudget?: number | null;
    timeUsedSeconds?: number;
    updatedAt?: number;
  } | null;
}

function mapPayloadGoal(
  goal: NonNullable<ThreadGoalUpdatedPayload['goal']>
): ThreadGoalSnapshot | null {
  const objective = goal.objective?.trim();
  const status = goal.status?.trim();
  if (!objective || !status) {
    return null;
  }
  return {
    goalId: goal.goalId,
    objective,
    status,
    tokensUsed: goal.tokensUsed,
    tokenBudget: goal.tokenBudget,
    timeUsedSeconds: goal.timeUsedSeconds,
    updatedAt: goal.updatedAt,
  };
}

/**
 * UI-07: monotonic updatedAt check. Once a session has a thread-goal clock
 * (threadGoalUpdatedAt), only accept an incoming goal that provably carries a
 * newer updatedAt. A missing timestamp cannot prove freshness, so a late
 * thread-goal-updated event after an explicit clear must not resurrect the old
 * goal (the store's own guard falls back to Date.now() for missing updatedAt,
 * which would let a stale event through).
 */
function isGoalStaleForSession(sessionId: string, snapshot: ThreadGoalSnapshot): boolean {
  const lastSeenAt = flowChatStore.getState().sessions.get(sessionId)?.threadGoalUpdatedAt ?? 0;
  if (lastSeenAt <= 0) {
    return false;
  }
  return snapshot.updatedAt == null || snapshot.updatedAt < lastSeenAt;
}

export function handleThreadGoalUpdated(payload: ThreadGoalUpdatedPayload): void {
  if (!payload.sessionId) return;

  if (!payload.goal) {
    flowChatStore.setThreadGoal(payload.sessionId, null);
    return;
  }

  const snapshot = mapPayloadGoal(payload.goal);
  if (!snapshot) {
    log.warn('ThreadGoalUpdated payload missing objective or status; ignoring partial update', {
      sessionId: payload.sessionId,
      goal: payload.goal,
    });
    return;
  }

  if (isGoalStaleForSession(payload.sessionId, snapshot)) {
    log.debug('ThreadGoalUpdated ignored: goal is not newer than the last write/clear', {
      sessionId: payload.sessionId,
      goal: payload.goal,
    });
    return;
  }

  flowChatStore.setThreadGoal(payload.sessionId, {
    goalId: snapshot.goalId ?? `${payload.sessionId}-goal`,
    objective: snapshot.objective,
    status: snapshot.status,
    tokensUsed: snapshot.tokensUsed,
    tokenBudget: snapshot.tokenBudget,
    timeUsedSeconds: snapshot.timeUsedSeconds,
    updatedAt: snapshot.updatedAt,
  });
}
