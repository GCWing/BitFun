import { describe, expect, it } from 'vitest';
import { isGoalSlashCommand, parseGoalCommand } from './goalCommandParser';

describe('goalCommandParser', () => {
  it('parses /goal without args as menu', () => {
    expect(parseGoalCommand('/goal')).toEqual({ kind: 'menu' });
    expect(parseGoalCommand('/goal   ')).toEqual({ kind: 'menu' });
  });

  it('parses /goal with an objective', () => {
    expect(parseGoalCommand('/goal fix login bug')).toEqual({
      kind: 'set',
      objective: 'fix login bug',
    });
  });

  it('parses /goal with a multiline objective', () => {
    const objective = '修复登录问题\n第二行要求\n第三行';
    expect(parseGoalCommand(`/goal ${objective}`)).toEqual({
      kind: 'set',
      objective,
    });
    expect(isGoalSlashCommand(`/goal ${objective}`)).toBe(true);
  });

  it('parses /goal when objective starts on the next line', () => {
    const objective = '修复登录问题\n附带说明';
    expect(parseGoalCommand(`/goal\n${objective}`)).toEqual({
      kind: 'set',
      objective,
    });
    expect(isGoalSlashCommand(`/goal\n${objective}`)).toBe(true);
  });

  it('parses goal control commands', () => {
    expect(parseGoalCommand('/goal clear')).toEqual({ kind: 'clear' });
    expect(parseGoalCommand('/goal pause')).toEqual({ kind: 'pause' });
    expect(parseGoalCommand('/goal resume')).toEqual({ kind: 'resume' });
    expect(parseGoalCommand('/goal edit')).toEqual({ kind: 'edit' });
  });

  it('treats multiline text as objective, not control', () => {
    expect(parseGoalCommand('/goal clear\nextra')).toEqual({
      kind: 'set',
      objective: 'clear\nextra',
    });
  });

  it('detects valid goal commands only', () => {
    expect(isGoalSlashCommand('/goal')).toBe(true);
    expect(isGoalSlashCommand('/goal ship feature')).toBe(true);
    expect(isGoalSlashCommand('/goalie')).toBe(false);
    expect(isGoalSlashCommand('/goals')).toBe(false);
  });
});
