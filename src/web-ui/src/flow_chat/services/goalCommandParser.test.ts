import { describe, expect, it } from 'vitest';
import { isGoalSlashCommand, parseGoalCommand } from './goalCommandParser';

describe('goalCommandParser', () => {
  it('parses /goal without a hint', () => {
    expect(parseGoalCommand('/goal')).toEqual({ userHint: undefined });
    expect(parseGoalCommand('/goal   ')).toEqual({ userHint: undefined });
  });

  it('parses /goal with a hint', () => {
    expect(parseGoalCommand('/goal fix login bug')).toEqual({
      userHint: 'fix login bug',
    });
  });

  it('detects valid goal commands only', () => {
    expect(isGoalSlashCommand('/goal')).toBe(true);
    expect(isGoalSlashCommand('/goal ship feature')).toBe(true);
    expect(isGoalSlashCommand('/goalie')).toBe(false);
    expect(isGoalSlashCommand('/goals')).toBe(false);
  });
});
