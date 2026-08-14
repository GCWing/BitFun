import { describe, expect, it } from 'vitest';
import {
  resolveThreadGoalStripIconTone,
  shouldShowThreadGoalObjective,
} from './threadGoalStripIconTone';

describe('resolveThreadGoalStripIconTone', () => {
  it('returns none when there is no goal', () => {
    expect(resolveThreadGoalStripIconTone(null)).toBe('none');
  });

  it('separates running from parked so a paused goal is not read as no goal', () => {
    expect(resolveThreadGoalStripIconTone({
      objective: 'sync',
      status: 'active',
    })).toBe('active');
    expect(resolveThreadGoalStripIconTone({
      objective: 'sync',
      status: 'paused',
    })).toBe('paused');
  });

  it('treats every stuck reason as one blocked state', () => {
    for (const status of ['blocked', 'usage_limited', 'budget_limited', 'UsageLimited']) {
      expect(resolveThreadGoalStripIconTone({ objective: 'sync', status })).toBe('blocked');
    }
  });

  it('returns complete when the goal is done', () => {
    expect(resolveThreadGoalStripIconTone({
      objective: 'sync',
      status: 'complete',
    })).toBe('complete');
  });

  it('falls back to none for a status it does not recognize', () => {
    expect(resolveThreadGoalStripIconTone({ objective: 'sync', status: 'archived' })).toBe('none');
  });
});

describe('shouldShowThreadGoalObjective', () => {
  it('names the objective for every state the user may still have to act on', () => {
    for (const status of ['active', 'paused', 'blocked', 'usage_limited', 'budget_limited']) {
      expect(shouldShowThreadGoalObjective({ objective: 'sync', status })).toBe(true);
    }
  });

  it('stays quiet when nothing is waiting on the user', () => {
    expect(shouldShowThreadGoalObjective(null)).toBe(false);
    expect(shouldShowThreadGoalObjective({ objective: 'sync', status: 'complete' })).toBe(false);
  });
});
