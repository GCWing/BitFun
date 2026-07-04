import { describe, expect, it } from 'vitest';

import {
  getSlashCommandPickerQuery,
  isSlashCommandPickerQuery,
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

describe('isSlashCommandPickerQuery', () => {
  it('allows single-token command picker queries', () => {
    expect(isSlashCommandPickerQuery('')).toBe(true);
    expect(isSlashCommandPickerQuery('goal')).toBe(true);
    expect(isSlashCommandPickerQuery('mcp:foo-bar')).toBe(true);
  });

  it('rejects path-like queries with another slash', () => {
    expect(isSlashCommandPickerQuery('users/alice')).toBe(false);
    expect(isSlashCommandPickerQuery('foo/bar/baz')).toBe(false);
  });
});

describe('getSlashCommandPickerQuery', () => {
  it('returns a lowercase query for active picker text', () => {
    expect(getSlashCommandPickerQuery('/')).toBe('');
    expect(getSlashCommandPickerQuery('/Goal')).toBe('goal');
    expect(getSlashCommandPickerQuery('/mcp:foo-bar')).toBe('mcp:foo-bar');
  });

  it('closes the picker once the slash token is no longer active', () => {
    expect(getSlashCommandPickerQuery('/goal ')).toBeNull();
    expect(getSlashCommandPickerQuery('/goal focus')).toBeNull();
    expect(getSlashCommandPickerQuery('/users/alice')).toBeNull();
    expect(getSlashCommandPickerQuery('hello /goal')).toBeNull();
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
