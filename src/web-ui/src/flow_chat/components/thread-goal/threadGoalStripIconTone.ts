import type { ThreadGoalSnapshot } from '../../services/goalService';

/** Strip icon tone: none = gray, active = yellow, complete = green. */
export type ThreadGoalStripIconTone = 'none' | 'active' | 'complete';

function normalizeThreadGoalStatus(status: string | undefined): string {
  const raw = status?.trim() ?? '';
  if (!raw) {
    return '';
  }
  const camel = raw.charAt(0).toLowerCase() + raw.slice(1);
  if (camel === 'usage_limited') {
    return 'usageLimited';
  }
  if (camel === 'budget_limited') {
    return 'budgetLimited';
  }
  return camel;
}

export function resolveThreadGoalStripIconTone(
  goal: ThreadGoalSnapshot | null,
): ThreadGoalStripIconTone {
  if (!goal) {
    return 'none';
  }
  if (normalizeThreadGoalStatus(goal.status) === 'complete') {
    return 'complete';
  }
  return 'active';
}
