import { describe, expect, it } from 'vitest';
import { threadGoalActionsForStatus } from './threadGoalActions';

describe('threadGoalActionsForStatus', () => {
  it('offers resume for paused goals after user stop', () => {
    expect(threadGoalActionsForStatus('paused')).toEqual(['edit', 'resume', 'clear']);
  });

  it('does not offer resume while goal is still active', () => {
    expect(threadGoalActionsForStatus('active')).toEqual(['edit', 'pause', 'clear']);
  });
});
