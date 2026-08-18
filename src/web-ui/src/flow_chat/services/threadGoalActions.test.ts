// @vitest-environment jsdom
import { beforeEach, describe, expect, it } from 'vitest';
import {
  dismissResumePrompt,
  resumePromptKey,
  shouldOpenResumePrompt,
  threadGoalActionsForStatus,
} from './threadGoalActions';
import type { ThreadGoalSnapshot } from './goalService';

describe('threadGoalActionsForStatus', () => {
  it('offers resume for paused goals after user stop', () => {
    expect(threadGoalActionsForStatus('paused')).toEqual(['edit', 'resume', 'clear']);
  });

  it('does not offer resume while goal is still active', () => {
    expect(threadGoalActionsForStatus('active')).toEqual(['edit', 'pause', 'clear']);
  });
});

const pausedGoal: ThreadGoalSnapshot = {
  goalId: 'goal-1',
  objective: 'ship the thing',
  status: 'paused',
  updatedAt: 42,
};

function gate(overrides: Partial<Parameters<typeof shouldOpenResumePrompt>[0]> = {}) {
  return shouldOpenResumePrompt({
    sessionId: 'session-1',
    goal: pausedGoal,
    sceneActive: true,
    disabled: false,
    lastPromptedKey: null,
    ...overrides,
  });
}

describe('resumePromptKey', () => {
  it('keys the prompt by goal, revision and status', () => {
    expect(resumePromptKey(pausedGoal)).toBe('goal-1:42:paused');
  });

  it('has no key for statuses that never prompt', () => {
    expect(resumePromptKey({ ...pausedGoal, status: 'active' })).toBeNull();
    expect(resumePromptKey(null)).toBeNull();
  });
});

describe('shouldOpenResumePrompt', () => {
  beforeEach(() => {
    sessionStorage.clear();
  });

  it('prompts while the user is looking at the session that owns the goal', () => {
    expect(gate()).toBe(true);
  });

  it('stays closed while the session sits in a background scene', () => {
    expect(gate({ sceneActive: false })).toBe(false);
  });

  it('prompts again once the user opens that session, after the gate is re-armed', () => {
    const key = resumePromptKey(pausedGoal);
    expect(gate({ lastPromptedKey: key })).toBe(false);
    expect(gate({ lastPromptedKey: null })).toBe(true);
  });

  it('respects an explicit leave-paused dismissal', () => {
    dismissResumePrompt('session-1', pausedGoal);
    expect(gate()).toBe(false);
  });

  it('does not prompt for goals that are not paused, or when goals are disabled', () => {
    expect(gate({ goal: { ...pausedGoal, status: 'active' } })).toBe(false);
    expect(gate({ goal: null })).toBe(false);
    expect(gate({ disabled: true })).toBe(false);
    expect(gate({ sessionId: undefined })).toBe(false);
  });
});
