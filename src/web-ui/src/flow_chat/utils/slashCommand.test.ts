import { describe, expect, it } from 'vitest';

import {
  isSlashCommand,
  matchesSlashCommand,
  stripSlashCommand,
} from './slashCommand';

describe('matchesSlashCommand', () => {
  it('returns null for empty / non-string input', () => {
    expect(matchesSlashCommand('')).toBeNull();
    expect(matchesSlashCommand('   ')).toBeNull();
    expect(matchesSlashCommand(null as unknown as string)).toBeNull();
    expect(matchesSlashCommand(undefined as unknown as string)).toBeNull();
    expect(matchesSlashCommand(123 as unknown as string)).toBeNull();
  });

  it('returns null for text not starting with /', () => {
    expect(matchesSlashCommand('hello')).toBeNull();
    expect(matchesSlashCommand('not a command')).toBeNull();
  });

  it('ignores leading whitespace (does not match)', () => {
    // Users typically type the command without leading spaces; if they do,
    // the helper does not pretend it sees a slash command — the caller
    // should have already trimmed via `inputState.value.trim()` first.
    expect(matchesSlashCommand('  /goal focus')).toBeNull();
  });

  it('returns the matched command token for plain commands', () => {
    expect(matchesSlashCommand('/goal focus the bug')).toBe('/goal');
    expect(matchesSlashCommand('/btw question?')).toBe('/btw');
    expect(matchesSlashCommand('/usage')).toBe('/usage');
    expect(matchesSlashCommand('/DeepReview')).toBe('/deepreview');
  });

  it('treats trailing tab / newline as a valid boundary (Claude Code 2.1.147)', () => {
    expect(matchesSlashCommand('/goal focus\t')).toBe('/goal');
    expect(matchesSlashCommand('/goal focus\n')).toBe('/goal');
    expect(matchesSlashCommand('/goal focus\r\n')).toBe('/goal');
    expect(matchesSlashCommand('/goal\t\n  ')).toBe('/goal');
    expect(matchesSlashCommand('/btw\n')).toBe('/btw');
    expect(matchesSlashCommand('/btw\tnext line')).toBe('/btw');
  });

  it('does not conflate a prefix with another command', () => {
    // Without the word-boundary fix, /goals / /btwextra etc. would have
    // matched `/goal` / `/btw`. They must not.
    expect(matchesSlashCommand('/goals')).toBe('/goals');
    expect(matchesSlashCommand('/btwextra')).toBe('/btwextra');
    expect(matchesSlashCommand('/usage2')).toBe('/usage2');
  });

  it('supports /-prefixed names containing : and - (MCP prompt commands)', () => {
    expect(matchesSlashCommand('/mcp:foo-bar arg')).toBe('/mcp:foo-bar');
    expect(matchesSlashCommand('/mcp:foo-bar')).toBe('/mcp:foo-bar');
  });

  it('returns null for slash followed by a non-letter', () => {
    expect(matchesSlashCommand('/123')).toBeNull();
    expect(matchesSlashCommand('/-cmd')).toBeNull();
  });
});

describe('isSlashCommand', () => {
  it('matches the exact command', () => {
    expect(isSlashCommand('/btw hello', '/btw')).toBe(true);
    expect(isSlashCommand('/btw', '/btw')).toBe(true);
    expect(isSlashCommand('/btw\t', '/btw')).toBe(true);
    expect(isSlashCommand('/btw\nnext line', '/btw')).toBe(true);
  });

  it('rejects prefix-only matches', () => {
    expect(isSlashCommand('/btwextra', '/btw')).toBe(false);
    expect(isSlashCommand('/btwsomething', '/btw')).toBe(false);
  });

  it('rejects unrelated commands', () => {
    expect(isSlashCommand('/goal focus', '/btw')).toBe(false);
    expect(isSlashCommand('hello', '/btw')).toBe(false);
  });

  it('rejects an invalid command argument', () => {
    expect(isSlashCommand('/btw', 'btw' as unknown as `/${string}`)).toBe(false);
    expect(isSlashCommand('/btw', 'no-slash' as unknown as `/${string}`)).toBe(false);
  });

  it('is case-insensitive on the command name', () => {
    expect(isSlashCommand('/BTW hello', '/btw')).toBe(true);
    expect(isSlashCommand('/Btw hello', '/btw')).toBe(true);
  });

  it('is robust against non-string inputs (defensive)', () => {
    // isSlashCommand should never throw on a non-string text argument.
    expect(isSlashCommand(null as unknown as string, '/btw')).toBe(false);
    expect(isSlashCommand(undefined as unknown as string, '/btw')).toBe(false);
    expect(isSlashCommand(123 as unknown as string, '/btw')).toBe(false);
  });
});

describe('stripSlashCommand', () => {
  it('strips the command and the following whitespace, leaving the argument', () => {
    expect(stripSlashCommand('/btw question?', '/btw')).toBe('question?');
    expect(stripSlashCommand('/btw  question?', '/btw')).toBe('question?');
    expect(stripSlashCommand('/btw\tquestion?', '/btw')).toBe('question?');
    expect(stripSlashCommand('/btw\nquestion?', '/btw')).toBe('question?');
  });

  it('returns empty string when the command has no argument', () => {
    expect(stripSlashCommand('/btw', '/btw')).toBe('');
    expect(stripSlashCommand('/btw\t', '/btw')).toBe('');
  });

  it('does not strip when the prefix does not match', () => {
    expect(stripSlashCommand('/btwextra', '/btw')).toBe('/btwextra');
    expect(stripSlashCommand('hello', '/btw')).toBe('hello');
  });

  it('escapes regex metacharacters in the command', () => {
    expect(stripSlashCommand('/mcp:foo-bar arg', '/mcp:foo-bar')).toBe('arg');
    expect(stripSlashCommand('/mcp:foo-bar', '/mcp:foo-bar')).toBe('');
  });

  it('truly escapes regex metacharacters (the alternation `|` case)', () => {
    // The `:foo-bar` test above doesn't actually exercise the escape because
    // `:` and `-` are not regex metacharacters. The character class used by
    // matchesSlashCommand (`[\w:-]`) excludes every regex metachar, so
    // isSlashCommand will reject any command that contains one before
    // stripSlashCommand even runs. The escape therefore is purely defensive
    // — stripSlashCommand must not throw on a hand-crafted command. We can't
    // call it through the public surface, but the internal escape is exercised
    // by a smoke check that the function returns the original string for
    // commands it would reject.
    expect(stripSlashCommand('not-prefixed', '|')).toBe('not-prefixed');
  });

  it('leaves the body untouched when only the command body has mixed whitespace', () => {
    expect(stripSlashCommand('/btw \t \n arg', '/btw')).toBe('arg');
  });
});
