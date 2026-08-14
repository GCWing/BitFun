import { describe, expect, it } from 'vitest';
import { resolveThreadGoalStripIconTone } from './threadGoalStripIconTone';

describe('resolveThreadGoalStripIconTone', () => {
  it('returns none when there is no goal', () => {
    expect(resolveThreadGoalStripIconTone(null)).toBe('none');
  });

  it('returns active only while the goal is actively running', () => {
    expect(resolveThreadGoalStripIconTone({
      objective: 'sync',
      status: 'active',
    })).toBe('active');
    expect(resolveThreadGoalStripIconTone({
      objective: 'sync',
      status: 'paused',
    })).toBe('none');
  });

  it('returns complete when the goal is done', () => {
    expect(resolveThreadGoalStripIconTone({
      objective: 'sync',
      status: 'complete',
    })).toBe('complete');
  });
});
