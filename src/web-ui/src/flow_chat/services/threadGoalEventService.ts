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
