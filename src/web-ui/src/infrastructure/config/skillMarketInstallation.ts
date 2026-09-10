import type { SkillInfo, SkillMarketItem } from './types';

function repositoryKey(source: string): string {
  return source.trim()
    .replace(/^https?:\/\/github\.com\//i, '')
    .replace(/\/+$/, '')
    .replace(/\.git$/i, '')
    .toLowerCase();
}

function installationKey(source: string, name: string): string {
  return `${repositoryKey(source)}@${name.trim()}`;
}

/** A display name alone cannot establish marketplace provenance. */
export function installedSkillMarketIds(skills: readonly SkillInfo[]): Set<string> {
  return new Set(skills.flatMap(skill => skill.installationSource?.trim()
    ? [installationKey(skill.installationSource, skill.name)]
    : []));
}

export function isSkillMarketItemInstalled(skill: SkillMarketItem, installedIds: ReadonlySet<string>): boolean {
  const separator = skill.installId.lastIndexOf('@');
  if (separator > 0) {
    return installedIds.has(installationKey(skill.installId.slice(0, separator), skill.installId.slice(separator + 1)));
  }
  return !!skill.source.trim() && installedIds.has(installationKey(skill.source, skill.name));
}
