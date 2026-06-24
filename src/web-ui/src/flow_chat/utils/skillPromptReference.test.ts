import { describe, expect, it } from 'vitest';
import {
  appendSkillPromptReferenceToken,
  createSkillPromptReferenceToken,
  getSkillPromptReferenceMatches,
  isSlashAddressableSkillName,
  parseSkillPromptReferenceToken,
  replaceLeadingSlashCommandWithSkillToken,
} from './skillPromptReference';

describe('skillPromptReference', () => {
  it('creates and parses skill prompt tokens', () => {
    const token = createSkillPromptReferenceToken('pdf');

    expect(token).toBe('[$pdf]');
    expect(parseSkillPromptReferenceToken(token)).toEqual({ skillName: 'pdf' });
  });

  it('finds skill tokens inside mixed text', () => {
    expect(getSkillPromptReferenceMatches('Use [$pdf] and [$browser].')).toEqual([
      {
        token: '[$pdf]',
        start: 4,
        end: 10,
        payload: { skillName: 'pdf' },
      },
      {
        token: '[$browser]',
        start: 15,
        end: 25,
        payload: { skillName: 'browser' },
      },
    ]);
  });

  it('appends and replaces slash commands with skill tokens', () => {
    expect(appendSkillPromptReferenceToken('Summarize this', 'pdf')).toBe('Summarize this [$pdf]');
    expect(replaceLeadingSlashCommandWithSkillToken('/pdf summarize this', 'pdf')).toBe(
      '[$pdf] summarize this',
    );
    expect(replaceLeadingSlashCommandWithSkillToken('  /pdf summarize this', 'pdf')).toBe(
      '  [$pdf] summarize this',
    );
  });

  it('matches only slash-addressable skill names', () => {
    expect(isSlashAddressableSkillName('pdf')).toBe(true);
    expect(isSlashAddressableSkillName('browser-control')).toBe(true);
    expect(isSlashAddressableSkillName('browser control')).toBe(false);
  });
});
