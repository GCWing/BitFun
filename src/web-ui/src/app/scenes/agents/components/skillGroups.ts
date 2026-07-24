import type { UserSkillGroup, UserSkillGroupsConfig } from '@/infrastructure/config/types';

export const USER_SKILL_GROUPS_CONFIG_PATH = 'app.user_skill_groups';
const USER_SKILL_GROUPS_CONFIG_VERSION = 1;

export interface GroupableSkill {
  key: string;
  name: string;
  description: string;
  isBuiltin: boolean;
  groupKey?: string | null;
  isShadowed?: boolean;
  level?: string;
  sourceLabel?: string;
  sourceSlot?: string;
  runtimeStatus?: string;
}

export type ResolvedSkillGroupKind = 'user' | 'builtin' | 'other';

export interface ResolvedSkillGroup {
  id: string;
  kind: ResolvedSkillGroupKind;
  label: string;
  skills: GroupableSkill[];
}

export interface SkillGroupLabels {
  builtin: (groupKey: string) => string;
  other: string;
}

const BUILTIN_SKILL_GROUP_ORDER = [
  'meta',
  'miniapp',
  'computer-use',
  'office',
  'canvas',
  'gstack',
];

const BUILTIN_SKILL_GROUP_LABEL_KEYS: Record<string, string> = {
  office: 'office',
  'computer-use': 'computerUse',
  meta: 'meta',
  miniapp: 'miniapp',
  canvas: 'canvas',
  gstack: 'gstack',
};

function normalizeSkillKeys(value: unknown): string[] {
  if (!Array.isArray(value)) {
    return [];
  }
  return [...new Set(value.filter((key): key is string => (
    typeof key === 'string' && key.trim().length > 0
  )).map((key) => key.trim()))];
}

function normalizeUserSkillGroup(value: unknown): UserSkillGroup | null {
  if (!value || typeof value !== 'object') {
    return null;
  }
  const group = value as Partial<UserSkillGroup>;
  const id = typeof group.id === 'string' ? group.id.trim() : '';
  const name = typeof group.name === 'string' ? group.name.trim() : '';
  if (!id || !name) {
    return null;
  }
  return { id, name, skillKeys: normalizeSkillKeys(group.skillKeys) };
}

function activeSkills(skills: GroupableSkill[]): GroupableSkill[] {
  const seen = new Set<string>();
  return skills.filter((skill) => {
    const key = skill.key.trim();
    if (!key || seen.has(key)) {
      return false;
    }
    seen.add(key);
    return true;
  });
}

function sortSkills(skills: GroupableSkill[]): GroupableSkill[] {
  return [...skills].sort((left, right) => (
    left.name.localeCompare(right.name) || left.key.localeCompare(right.key)
  ));
}

function builtinSkillGroupOrder(groupKey: string): number {
  const index = BUILTIN_SKILL_GROUP_ORDER.indexOf(groupKey);
  return index === -1 ? BUILTIN_SKILL_GROUP_ORDER.length : index;
}

export function builtinSkillGroupLabelKey(groupKey: string): string | null {
  return BUILTIN_SKILL_GROUP_LABEL_KEYS[groupKey] ?? null;
}

export function normalizeUserSkillGroupsConfig(value: unknown): UserSkillGroupsConfig {
  if (!value || typeof value !== 'object') {
    return { version: USER_SKILL_GROUPS_CONFIG_VERSION, groups: [] };
  }
  const record = value as Partial<UserSkillGroupsConfig>;
  const version = typeof record.version === 'number' && Number.isInteger(record.version)
    ? record.version
    : USER_SKILL_GROUPS_CONFIG_VERSION;
  const groups = Array.isArray(record.groups)
    ? record.groups
      .map(normalizeUserSkillGroup)
      .filter((group): group is UserSkillGroup => group !== null)
    : [];
  return { version, groups };
}

export function createUserSkillGroupsConfig(groups: UserSkillGroup[]): UserSkillGroupsConfig {
  return normalizeUserSkillGroupsConfig({
    version: USER_SKILL_GROUPS_CONFIG_VERSION,
    groups,
  });
}

export function resolveSkillGroups(
  skills: GroupableSkill[],
  userGroups: UserSkillGroup[],
  labels: SkillGroupLabels,
): ResolvedSkillGroup[] {
  const availableSkills = activeSkills(skills);
  const availableByKey = new Map(availableSkills.map((skill) => [skill.key, skill]));
  const resolvedUserGroups = userGroups.flatMap((group) => {
    const groupSkills = group.skillKeys
      .map((key) => availableByKey.get(key))
      .filter((skill): skill is GroupableSkill => skill !== undefined);
    return groupSkills.length > 0
      ? [{ id: `user:${group.id}`, kind: 'user' as const, label: group.name, skills: sortSkills(groupSkills) }]
      : [];
  });

  const builtinByGroup = new Map<string, GroupableSkill[]>();
  for (const skill of availableSkills) {
    const groupKey = skill.isBuiltin ? skill.groupKey?.trim() : '';
    if (!groupKey) {
      continue;
    }
    const groupSkills = builtinByGroup.get(groupKey) ?? [];
    groupSkills.push(skill);
    builtinByGroup.set(groupKey, groupSkills);
  }
  const resolvedBuiltinGroups = [...builtinByGroup.entries()]
    .map(([groupKey, groupSkills]) => ({
      id: `builtin:${groupKey}`,
      kind: 'builtin' as const,
      label: labels.builtin(groupKey),
      skills: sortSkills(groupSkills),
      groupKey,
    }))
    .sort((left, right) => (
      builtinSkillGroupOrder(left.groupKey) - builtinSkillGroupOrder(right.groupKey)
      || left.label.localeCompare(right.label)
    ));

  const builtinKeys = new Set(
    resolvedBuiltinGroups.flatMap((group) => group.skills.map((skill) => skill.key)),
  );
  const otherSkills = sortSkills(availableSkills.filter((skill) => !builtinKeys.has(skill.key)));
  const otherGroup = otherSkills.length > 0
    ? [{ id: 'other', kind: 'other' as const, label: labels.other, skills: otherSkills }]
    : [];

  return [...resolvedUserGroups, ...resolvedBuiltinGroups, ...otherGroup];
}

export function resolveSkillGroupSummary(
  skills: GroupableSkill[],
  userGroups: UserSkillGroup[],
  selectedSkillKeys: readonly string[],
  labels: SkillGroupLabels,
): ResolvedSkillGroup[] {
  const selected = new Set(selectedSkillKeys);
  const displayed = new Set<string>();
  return resolveSkillGroups(skills, userGroups, labels).flatMap((group) => {
    const groupSkills = group.skills.filter((skill) => {
      if (!selected.has(skill.key) || displayed.has(skill.key)) {
        return false;
      }
      displayed.add(skill.key);
      return true;
    });
    return groupSkills.length > 0 ? [{ ...group, skills: groupSkills }] : [];
  });
}

export function skillGroupKeys(group: ResolvedSkillGroup): string[] {
  return group.skills.map((skill) => skill.key);
}

export function setSkillGroupSelection(
  selectedSkillKeys: readonly string[],
  groupKeys: readonly string[],
  enabled: boolean,
): string[] {
  const groupKeySet = new Set(groupKeys);
  if (!enabled) {
    return selectedSkillKeys.filter((key) => !groupKeySet.has(key));
  }
  return [...new Set([...selectedSkillKeys, ...groupKeys])];
}

export function toggleSkillSelection(selectedSkillKeys: readonly string[], skillKey: string): string[] {
  return selectedSkillKeys.includes(skillKey)
    ? selectedSkillKeys.filter((key) => key !== skillKey)
    : [...selectedSkillKeys, skillKey];
}

export function unavailableUserSkillKeys(
  group: UserSkillGroup,
  skills: GroupableSkill[],
): string[] {
  const availableKeys = new Set(activeSkills(skills).map((skill) => skill.key));
  return group.skillKeys.filter((key) => !availableKeys.has(key));
}
