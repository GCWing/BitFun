import { describe, expect, it } from 'vitest';
import {
  appendSkillPromptReferenceToken,
  createSkillPromptReferenceToken,
  getSkillPromptReferenceMatches,
  isSkillAvailableForUserInvocation,
  isSlashAddressableSkillName,
  parseSkillPromptReferenceToken,
  replaceLeadingSlashCommandWithSkillToken,
} from './skillPromptReference';

describe('skillPromptReference', () => {
  it('continues parsing existing exact nested source references', () => {
    const key = 'project::codex::.system/pdf';
    const token = `[$${key}]`;
    expect(token).toBe('[$project::codex::.system/pdf]');
    expect(parseSkillPromptReferenceToken(token)).toEqual({ skillName: 'pdf', skillKey: key });
    expect(getSkillPromptReferenceMatches(`Use [$pdf] and ${token}`)).toHaveLength(2);
  });

  it('allows enabled shadowed sources while respecting global, mode and author disabling', () => {
    const skill = { selectedForRuntime: false, effectiveEnabled: true, globallyEnabled: true, allowUserInvocation: true };
    expect(isSkillAvailableForUserInvocation(skill)).toBe(true);
    expect(isSkillAvailableForUserInvocation({ ...skill, effectiveEnabled: false })).toBe(false);
    expect(isSkillAvailableForUserInvocation({ ...skill, globallyEnabled: false })).toBe(false);
    expect(isSkillAvailableForUserInvocation({ ...skill, allowUserInvocation: false })).toBe(false);
  });

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

  it('keeps user invocation visibility independent from runtime selection', () => {
    expect(isSkillAvailableForUserInvocation({
      selectedForRuntime: true,
      allowUserInvocation: true,
    })).toBe(true);
    expect(isSkillAvailableForUserInvocation({
      selectedForRuntime: true,
      allowUserInvocation: false,
    })).toBe(false);
    expect(isSkillAvailableForUserInvocation({
      selectedForRuntime: false,
    })).toBe(false);
  });
});
