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
  return `openbitfun.threadGoal.resumeDismissed.${sessionId}.${goalId}`;
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

/**
 * Identifies one paused-goal state, so the resume prompt opens once per visit to
 * the session instead of on every effect run. Null when the goal cannot prompt.
 */
export function resumePromptKey(goal: ThreadGoalSnapshot | null | undefined): string | null {
  if (!goal?.goalId || !threadGoalStatusNeedsResumePrompt(goal.status)) return null;
  return `${goal.goalId}:${goal.updatedAt ?? 0}:${goal.status}`;
}

export interface ResumePromptGate {
  sessionId: string | undefined;
  goal: ThreadGoalSnapshot | null;
  /** False while the session sits in a scene the user is not looking at. */
  sceneActive: boolean;
  disabled: boolean;
  /** State this controller already prompted for during the current visit. */
  lastPromptedKey: string | null;
}

/**
 * The prompt belongs to the session the user opened. Every scene stays mounted,
 * so without the `sceneActive` gate a paused goal would raise its modal on top
 * of whatever unrelated scene happens to be in front.
 */
export function shouldOpenResumePrompt({
  sessionId,
  goal,
  sceneActive,
  disabled,
  lastPromptedKey,
}: ResumePromptGate): boolean {
  if (disabled || !sceneActive || !sessionId || !goal) return false;
  const key = resumePromptKey(goal);
  if (!key || key === lastPromptedKey) return false;
  return !isResumePromptDismissed(sessionId, goal);
}
