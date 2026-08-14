import type { ThreadGoalSnapshot } from '../../services/goalService';

/**
 * A thread goal drives turns on its own, so a goal that has stopped driving
 * them must not look like the absence of one. Running, parked, and stuck are
 * three different situations and each gets its own tone; only "no goal" and
 * "finished" are quiet, because neither is waiting for the user.
 */
export type ThreadGoalStripIconTone =
  | 'none'
  | 'active'
  | 'paused'
  | 'blocked'
  | 'complete';

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
  switch (normalizeThreadGoalStatus(goal.status)) {
    case 'active':
      return 'active';
    case 'paused':
      return 'paused';
    // A goal out of budget or out of quota is stuck for the same reason as one
    // the runtime blocked: it will not advance until the user acts.
    case 'blocked':
    case 'usageLimited':
    case 'budgetLimited':
      return 'blocked';
    case 'complete':
      return 'complete';
    default:
      return 'none';
  }
}

/**
 * The objective is shown for every state the user may still have to act on. A
 * finished or absent goal has nothing to name.
 */
export function shouldShowThreadGoalObjective(goal: ThreadGoalSnapshot | null): boolean {
  const tone = resolveThreadGoalStripIconTone(goal);
  return tone === 'active' || tone === 'paused' || tone === 'blocked';
}
