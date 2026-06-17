import type { ThreadGoalSnapshot } from './goalService';

export type ThreadGoalUiAction = 'edit' | 'pause' | 'resume' | 'clear' | 'set';

export function threadGoalStatusNeedsResumePrompt(status: string): boolean {
  return status === 'paused' || status === 'blocked' || status === 'usageLimited';
}

export function threadGoalActionsForStatus(status: string): ThreadGoalUiAction[] {
  switch (status) {
    case 'active':
      return ['edit', 'pause', 'clear'];
    case 'paused':
    case 'blocked':
    case 'usageLimited':
      return ['edit', 'resume', 'clear'];
    case 'budgetLimited':
    case 'complete':
      return ['edit', 'clear'];
    default:
      return ['edit', 'clear'];
  }
}

export function resumeDismissStorageKey(sessionId: string, goalId: string): string {
  return `bitfun.threadGoal.resumeDismissed.${sessionId}.${goalId}`;
}

export function isResumePromptDismissed(sessionId: string, goal: ThreadGoalSnapshot): boolean {
  if (!goal.goalId) return false;
  try {
    return sessionStorage.getItem(resumeDismissStorageKey(sessionId, goal.goalId)) === '1';
  } catch {
    return false;
  }
}

export function dismissResumePrompt(sessionId: string, goal: ThreadGoalSnapshot): void {
  if (!goal.goalId) return;
  try {
    sessionStorage.setItem(resumeDismissStorageKey(sessionId, goal.goalId), '1');
  } catch {
    // ignore storage failures
  }
}
