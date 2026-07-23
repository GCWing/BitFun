export type ThreadGoalWorkflowStepState = 'done' | 'current' | 'pending' | 'skipped';

export interface ThreadGoalWorkflowStep {
  id: string;
  state: ThreadGoalWorkflowStepState;
}

const ACTIVE_WORKFLOW_IDS = ['continue', 'audit', 'complete'] as const;

export function buildThreadGoalWorkflowSteps(status: string): ThreadGoalWorkflowStep[] {
  switch (status) {
    case 'active':
      return [
        { id: 'continue', state: 'done' },
        { id: 'audit', state: 'current' },
        { id: 'complete', state: 'pending' },
      ];
    case 'complete':
      return ACTIVE_WORKFLOW_IDS.map(id => ({ id, state: 'done' as const }));
    case 'paused':
    case 'blocked':
    case 'usageLimited':
    case 'budgetLimited':
      return ACTIVE_WORKFLOW_IDS.map(id => ({ id, state: 'skipped' as const }));
    default:
      return [];
  }
}

export function shouldShowThreadGoalWorkflow(status: string): boolean {
  return buildThreadGoalWorkflowSteps(status).length > 0;
}
