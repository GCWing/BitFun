import { describe, expect, it } from 'vitest';

import {
  appendAdditionalModePromptReferenceToken,
  createAdditionalModePromptReferenceToken,
  expandAdditionalModePromptReferenceTokens,
  getAdditionalModePromptReferenceMatches,
  parseAdditionalModePromptReferenceToken,
} from './additionalModePromptReference';

describe('additionalModePromptReference', () => {
  it('creates and parses the Review capsule token', () => {
    const token = createAdditionalModePromptReferenceToken('review');
    expect(token).toBe('[[openbitfun-additional-mode:review]]');
    expect(parseAdditionalModePromptReferenceToken(token)).toEqual({
      id: 'review',
      displayText: 'Review',
      promptCommand: '/review',
    });
    expect(parseAdditionalModePromptReferenceToken('[[openbitfun-additional-mode:unknown]]')).toBeNull();
  });

  it('finds and appends mode capsules without exposing command text', () => {
    const token = createAdditionalModePromptReferenceToken('review');
    expect(appendAdditionalModePromptReferenceToken('', 'review')).toBe(token);
    expect(appendAdditionalModePromptReferenceToken('Inspect this', 'review')).toBe(
      `Inspect this ${token}`,
    );
    expect(getAdditionalModePromptReferenceMatches(`Inspect ${token}`)).toEqual([
      {
        token,
        start: 8,
        end: 8 + token.length,
        payload: {
          id: 'review',
          displayText: 'Review',
          promptCommand: '/review',
        },
      },
    ]);
  });

  it('moves Review to the native command boundary only when submitting', () => {
    const token = createAdditionalModePromptReferenceToken('review');
    expect(expandAdditionalModePromptReferenceTokens(token)).toBe('/review');
    expect(expandAdditionalModePromptReferenceTokens(`Inspect this ${token}`)).toBe(
      '/review Inspect this',
    );
    expect(expandAdditionalModePromptReferenceTokens('Inspect this')).toBe('Inspect this');
  });
});
