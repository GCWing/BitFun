import { describe, expect, it } from 'vitest';
import {
  resolveThreadGoalHeaderTitle,
  resolveThreadGoalUserMessageDisplay,
} from './threadGoalDisplay';

describe('resolveThreadGoalUserMessageDisplay', () => {
  it('localizes objective-updated metadata', () => {
    const result = resolveThreadGoalUserMessageDisplay('Thread goal updated: foo', {
      threadGoalObjectiveUpdated: true,
      objective: '同步上游',
    });
    expect(result).toContain('同步上游');
    expect(result).not.toContain('Thread goal updated');
  });

  it('localizes auto-continuation completion-check metadata', () => {
    const result = resolveThreadGoalUserMessageDisplay('Continuing thread goal: sync', {
      threadGoalContinuation: true,
      objective: '同步上游',
      autoContinuationAttempt: 2,
      autoContinuationMax: 100,
    });
    expect(result).toContain('同步上游');
    expect(result).not.toContain('Continuing thread goal');
    expect(result).toContain('目标完成检查');
  });
});

describe('resolveThreadGoalHeaderTitle', () => {
  it('returns completion-check header for continuation turns', () => {
    const title = resolveThreadGoalHeaderTitle({
      threadGoalContinuation: true,
      autoContinuationAttempt: 3,
      autoContinuationMax: 100,
    });
    expect(title).toBeTruthy();
    expect(title).toContain('3');
  });
});
