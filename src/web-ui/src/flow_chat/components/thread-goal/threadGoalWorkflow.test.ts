import { describe, expect, it } from 'vitest';
import { buildThreadGoalWorkflowSteps, shouldShowThreadGoalWorkflow } from './threadGoalWorkflow';

describe('threadGoalWorkflow', () => {
  it('marks audit as current for active goals', () => {
    const steps = buildThreadGoalWorkflowSteps('active');
    expect(steps).toHaveLength(3);
    expect(steps.find(s => s.id === 'audit')?.state).toBe('current');
  });

  it('marks all steps done when complete', () => {
    const steps = buildThreadGoalWorkflowSteps('complete');
    expect(steps.every(s => s.state === 'done')).toBe(true);
  });

  it('shows workflow for active and complete statuses', () => {
    expect(shouldShowThreadGoalWorkflow('active')).toBe(true);
    expect(shouldShowThreadGoalWorkflow('complete')).toBe(true);
    expect(shouldShowThreadGoalWorkflow('paused')).toBe(true);
  });
});
