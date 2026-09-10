import { describe, expect, it } from 'vitest';
import type { SkillInfo, SkillMarketItem } from './types';
import { installedSkillMarketIds, isSkillMarketItemInstalled } from './skillMarketInstallation';

const installed = (installationSource?: string): SkillInfo => ({ name: 'eli5', installationSource } as SkillInfo);
const market = (source: string, name = 'eli5'): SkillMarketItem => ({ source, name, installId: `${source}@${name}` } as SkillMarketItem);

describe('skill marketplace installation identity', () => {
  it('marks only the installed repository when several packages share a name', () => {
    const ids = installedSkillMarketIds([installed('first/skills')]);
    expect(['first/skills', 'second/skills', 'third/skills'].map(source => isSkillMarketItemInstalled(market(source), ids)))
      .toEqual([true, false, false]);
    expect(isSkillMarketItemInstalled(market('first/skills', 'another'), ids)).toBe(false);
  });
  it('does not infer provenance for legacy, builtin or manually copied names', () => {
    expect(isSkillMarketItemInstalled(market('first/skills'), installedSkillMarketIds([installed()]))).toBe(false);
  });
  it('matches repository URL aliases but preserves the package identity', () => {
    const ids = installedSkillMarketIds([installed('https://github.com/First/Skills.git/')]);
    expect(isSkillMarketItemInstalled(market('first/skills'), ids)).toBe(true);
    expect(isSkillMarketItemInstalled({ ...market('first/skills'), name: 'Display label' }, ids)).toBe(true);
  });
  it('follows refreshed installs after replacement, removal or workspace changes', () => {
    const first = market('first/skills');
    const second = market('second/skills');
    const ids = installedSkillMarketIds([installed('second/skills')]);
    expect(isSkillMarketItemInstalled(first, ids)).toBe(false);
    expect(isSkillMarketItemInstalled(second, ids)).toBe(true);
    expect(isSkillMarketItemInstalled(second, installedSkillMarketIds([]))).toBe(false);
  });
});
