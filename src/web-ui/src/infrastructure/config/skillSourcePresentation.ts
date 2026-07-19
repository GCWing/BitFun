import type { ModeSkillInfo, SkillInfo } from './types';

const SOURCE_LABEL_BY_ID: Record<string, string> = {
  bitfun: 'BitFun',
  'bitfun-system': 'BitFun',
  'claude-code': 'Claude Code',
  claude: 'Claude Code',
  codex: 'Codex',
  cursor: 'Cursor',
  opencode: 'OpenCode',
  'agent-skills': 'Agent Skills',
  agents: 'Agent Skills',
};

function knownSourceLabel(value: string | undefined): string | undefined {
  const normalized = value?.trim().toLowerCase();
  if (!normalized) {
    return undefined;
  }

  return SOURCE_LABEL_BY_ID[normalized]
    ?? SOURCE_LABEL_BY_ID[normalized.replace(/^home\./, '').replace(/^config\./, '')];
}

export function getSkillSourceLabel(
  skill: SkillInfo,
  fallbackLabel = 'Other source',
): string {
  return skill.sourceLabel?.trim()
    || knownSourceLabel(skill.sourceId)
    || knownSourceLabel(skill.sourceSlot)
    || fallbackLabel;
}

export function canDeleteSkill(skill: SkillInfo): boolean {
  if (skill.isBuiltin) return false;

  const sourceId = skill.sourceId?.trim().toLowerCase();
  if (sourceId) {
    return sourceId === 'bitfun' || sourceId === 'bitfun-system';
  }

  return skill.sourceSlot?.trim().toLowerCase().startsWith('bitfun') ?? false;
}

export interface SkillOriginLabels {
  fallbackSourceLabel: string;
  userLabel: string;
  projectLabel: string;
}

const DEFAULT_ORIGIN_LABELS: SkillOriginLabels = {
  fallbackSourceLabel: 'Other source',
  userLabel: 'User',
  projectLabel: 'Project',
};

export function formatSkillOrigin(
  skill: SkillInfo,
  labels: SkillOriginLabels = DEFAULT_ORIGIN_LABELS,
): string {
  const scopeLabel = skill.level === 'project' ? labels.projectLabel : labels.userLabel;
  return `${getSkillSourceLabel(skill, labels.fallbackSourceLabel)} · ${scopeLabel}`;
}

export function buildSkillCoverageSourceMap(
  allSkills: SkillInfo[],
  fallbackLabel = 'Other source',
): Map<string, string> {
  const skillsByKey = new Map(allSkills.map((skill) => [skill.key, skill]));
  const coverageSources = new Map<string, string>();

  for (const skill of allSkills) {
    const winnerKey = skill.shadowedByKey?.trim();
    if (!skill.isShadowed || !winnerKey) {
      continue;
    }

    const winner = skillsByKey.get(winnerKey);
    if (winner) {
      coverageSources.set(skill.key, getSkillSourceLabel(winner, fallbackLabel));
    }
  }

  return coverageSources;
}

export type ModeSkillRuntimeStatus =
  | { kind: 'selected' }
  | { kind: 'covered'; sourceLabel: string }
  | { kind: 'enabled' }
  | { kind: 'disabled' };

export function getModeSkillRuntimeStatus(
  skill: ModeSkillInfo,
  coverageSourceBySkillKey: ReadonlyMap<string, string>,
  fallbackLabel = 'Other source',
): ModeSkillRuntimeStatus {
  if (!skill.effectiveEnabled) {
    return { kind: 'disabled' };
  }
  if (skill.selectedForRuntime) {
    return { kind: 'selected' };
  }
  if (skill.isShadowed) {
    return {
      kind: 'covered',
      sourceLabel: coverageSourceBySkillKey.get(skill.key) ?? fallbackLabel,
    };
  }
  return { kind: 'enabled' };
}

export function findSkillByKey(skills: SkillInfo[], skillKey: string | null): SkillInfo | null {
  if (!skillKey) {
    return null;
  }
  return skills.find((skill) => skill.key === skillKey) ?? null;
}
