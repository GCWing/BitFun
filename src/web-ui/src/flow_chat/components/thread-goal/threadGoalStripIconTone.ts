import type { ThreadGoalSnapshot } from '../../services/goalService';

/** Only a running goal is emphasized; absent, paused, and completed goals stay muted. */
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
  return normalizeThreadGoalStatus(goal.status) === 'active' ? 'active' : 'none';
}

export function isThreadGoalActive(goal: ThreadGoalSnapshot | null): boolean {
  return !!goal && normalizeThreadGoalStatus(goal.status) === 'active';
}
