import { describe, expect, it } from 'vitest';
import { parseGlobalSearchQuery, scoreTextMatch } from './searchMatching';

describe('global search query grammar', () => {
  it('uses a leading chevron as an explicit command scope', () => {
    expect(parseGlobalSearchQuery('  > terminal  ', 'content')).toEqual({
      query: 'terminal',
      scope: 'actions',
      scopeForcedByPrefix: true,
    });
  });

  it('preserves the selected scope for ordinary text', () => {
    expect(parseGlobalSearchQuery('  readme  ', 'content')).toEqual({
      query: 'readme',
      scope: 'content',
      scopeForcedByPrefix: false,
    });
  });
});

describe('scoreTextMatch', () => {
  it('prioritizes exact, prefix, word-prefix, and contains matches', () => {
    expect(scoreTextMatch('term', ['term'])).toBeGreaterThan(scoreTextMatch('term', ['terminal']));
    expect(scoreTextMatch('term', ['open terminal'])).toBeGreaterThan(scoreTextMatch('term', ['preterminal panel']));
    expect(scoreTextMatch('missing', ['terminal'])).toBe(0);
  });
});
