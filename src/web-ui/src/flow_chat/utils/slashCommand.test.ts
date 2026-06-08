import { describe, expect, it } from 'vitest';

import {
  isSlashCommand,
  matchesSlashCommand,
  stripSlashCommand,
} from './slashCommand';

describe('matchesSlashCommand', () => {
  it('matches slash command tokens at a whitespace boundary', () => {
    expect(matchesSlashCommand('/goal focus the bug')).toBe('/goal');
    expect(matchesSlashCommand('/btw\tquestion')).toBe('/btw');
    expect(matchesSlashCommand('/usage\n')).toBe('/usage');
    expect(matchesSlashCommand('/mcp:foo-bar arg')).toBe('/mcp:foo-bar');
  });

  it('rejects text without a leading slash command token', () => {
    expect(matchesSlashCommand('')).toBeNull();
    expect(matchesSlashCommand('hello')).toBeNull();
    expect(matchesSlashCommand(' /goal fix bug')).toBeNull();
    expect(matchesSlashCommand('/123')).toBeNull();
    expect(matchesSlashCommand('/-goal')).toBeNull();
  });

  it('does not conflate longer commands with a shorter prefix', () => {
    expect(matchesSlashCommand('/goals')).toBe('/goals');
    expect(matchesSlashCommand('/btwextra')).toBe('/btwextra');
    expect(matchesSlashCommand('/usage2')).toBe('/usage2');
  });
});

describe('isSlashCommand', () => {
  it('matches exact command tokens case-insensitively', () => {
    expect(isSlashCommand('/btw', '/btw')).toBe(true);
    expect(isSlashCommand('/BTW hello', '/btw')).toBe(true);
    expect(isSlashCommand('/goal\nfix bug', '/goal')).toBe(true);
  });

  it('rejects prefix-only and unrelated matches', () => {
    expect(isSlashCommand('/btwextra', '/btw')).toBe(false);
    expect(isSlashCommand('/compactor', '/compact')).toBe(false);
    expect(isSlashCommand('hello', '/btw')).toBe(false);
  });

  it('rejects invalid command definitions and non-string text', () => {
    expect(isSlashCommand('/btw', 'btw' as unknown as `/${string}`)).toBe(false);
    expect(isSlashCommand(null as unknown as string, '/btw')).toBe(false);
  });
});

describe('stripSlashCommand', () => {
  it('removes the command token and immediate whitespace', () => {
    expect(stripSlashCommand('/btw question?', '/btw')).toBe('question?');
    expect(stripSlashCommand('/btw\tquestion?', '/btw')).toBe('question?');
    expect(stripSlashCommand('/btw\nquestion?', '/btw')).toBe('question?');
    expect(stripSlashCommand('/btw', '/btw')).toBe('');
  });

  it('preserves case-insensitive matches', () => {
    expect(stripSlashCommand('/BTW hello', '/btw')).toBe('hello');
  });

  it('returns the original string when the command does not match', () => {
    expect(stripSlashCommand('/btwextra', '/btw')).toBe('/btwextra');
    expect(stripSlashCommand('hello', '/btw')).toBe('hello');
  });
});
