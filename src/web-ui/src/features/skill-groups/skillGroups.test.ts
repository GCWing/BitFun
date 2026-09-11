import { describe, expect, it } from 'vitest';
import type { GroupableSkill } from './skillGroups';
import {
  createUserSkillGroupsConfig,
  normalizeUserSkillGroupsConfig,
  resolveSkillGroupSummary,
  resolveSkillGroups,
  setSkillGroupSelection,
  resolveSkillSelectionGroups,
  saveUserSkillGroup,
  deleteUserSkillGroup,
  moveUserSkillGroup,
} from './skillGroups';

const labels = {
  builtin: (groupKey: string) => `builtin:${groupKey}`,
  other: 'other',
};

const skills: GroupableSkill[] = [
  { key: 'builtin::docs', name: 'Docs', description: '', isBuiltin: true, groupKey: 'office' },
  { key: 'builtin::slides', name: 'Slides', description: '', isBuiltin: true, groupKey: 'office' },
  { key: 'builtin::plan', name: 'Plan', description: '', isBuiltin: true, groupKey: 'planning' },
  { key: 'user::review', name: 'Review', description: '', isBuiltin: false },
];

describe('skillGroups', () => {
  it('keeps user groups before backend-owned builtin groups', () => {
    const groups = resolveSkillGroups(skills, [{
      id: 'daily',
      name: 'Daily work',
      skillKeys: ['user::review', 'builtin::docs'],
    }], labels);

    expect(groups.map((group) => group.id)).toEqual([
      'user:daily',
      'builtin:office',
      'builtin:planning',
    ]);
    expect(groups[0].skills.map((skill) => skill.key)).toEqual(['builtin::docs', 'user::review']);
  });

  it('clears overlapping skills from the final selection', () => {
    expect(setSkillGroupSelection(
      ['builtin::docs', 'builtin::slides', 'user::review'],
      ['builtin::docs', 'user::review'],
      false,
    )).toEqual(['builtin::slides']);
  });

  it('uses personal groups first when summarizing enabled skills', () => {
    const groups = resolveSkillGroupSummary(skills, [{
      id: 'daily',
      name: 'Daily work',
      skillKeys: ['builtin::docs'],
    }], ['builtin::docs', 'builtin::slides'], labels);

    expect(groups.map((group) => group.id)).toEqual(['user:daily', 'builtin:office']);
    expect(groups[1].skills.map((skill) => skill.key)).toEqual(['builtin::slides']);
  });

  it('retains unavailable keys while normalizing persisted groups', () => {
    const config = normalizeUserSkillGroupsConfig({
      version: 1,
      groups: [{
        id: 'daily',
        name: 'Daily work',
        skillKeys: ['builtin::docs', 'missing::skill', 'builtin::docs'],
      }],
    });

    expect(config.groups[0].skillKeys).toEqual(['builtin::docs', 'missing::skill']);
    expect(createUserSkillGroupsConfig(config.groups)).toEqual(config);
  });

  it('keeps empty groups and groups whose members are unavailable', () => {
    const groups = resolveSkillGroups([], [
      { id: 'empty', name: 'Empty', skillKeys: [] },
      { id: 'offline', name: 'Offline', skillKeys: ['remote::docs'] },
    ], labels);
    expect(groups.map(group => group.id)).toEqual(['user:empty', 'user:offline']);
    expect(groups[1].unavailableSkillKeys).toEqual(['remote::docs']);
    expect(groups[1].skillKeys).toEqual(['remote::docs']);
  });

  it('adds ungrouped skills only to the picker, without duplicating personal members there', () => {
    const personal = [{ id: 'daily', name: 'Daily', skillKeys: ['user::review'] }];
    expect(resolveSkillGroups(skills, personal, labels).some(group => group.kind === 'other')).toBe(false);
    expect(resolveSkillSelectionGroups(skills, personal, labels).some(group => group.kind === 'other')).toBe(false);
    expect(resolveSkillSelectionGroups(skills, [], labels).at(-1)?.skillKeys).toEqual(['user::review']);
  });

  it('deduplicates selections from overlapping groups without changing either definition', () => {
    const members = ['builtin::docs', 'user::review'];
    const selected = ['builtin::docs', 'builtin::slides'];
    expect(setSkillGroupSelection(selected, members, true)).toEqual([
      'builtin::docs', 'builtin::slides', 'user::review',
    ]);
    expect(members).toEqual(['builtin::docs', 'user::review']);
    expect(selected).toEqual(['builtin::docs', 'builtin::slides']);
  });

  it('opens older installs with no groups and round trips the existing version-one shape', () => {
    expect(normalizeUserSkillGroupsConfig(undefined)).toEqual({ version: 1, groups: [] });
    const legacy = { version: 1, groups: [{ id: 'empty', name: 'Empty', skillKeys: [] }] };
    expect(createUserSkillGroupsConfig(normalizeUserSkillGroupsConfig(legacy).groups)).toEqual(legacy);
  });

  it.each([
    'invalid', { version: 1 }, { version: 2, groups: [] },
    { version: 1, groups: [{ id: 'a', name: 'A', skillKeys: [42] }] },
    { version: 1, groups: [{ id: 'a', name: 'A', skillKeys: [] }, { id: 'a', name: 'B', skillKeys: [] }] },
  ])('refuses unreadable or future data instead of normalizing it into a destructive empty write: %j', value => {
    expect(() => normalizeUserSkillGroupsConfig(value)).toThrow();
  });

  it('merges one edit with the latest collection and preserves unavailable references', () => {
    const original = { id: 'daily', name: 'Daily', skillKeys: ['missing::skill'] };
    const elsewhere = { id: 'new', name: 'New', skillKeys: [] };
    const result = saveUserSkillGroup([original, elsewhere], { ...original, name: 'Renamed' }, original);
    expect(result).toEqual([{ ...original, name: 'Renamed' }, elsewhere]);
    expect(saveUserSkillGroup(result, { id: 'empty', name: 'Empty', skillKeys: [] }, null)).toHaveLength(3);
  });

  it('detects stale edits, stale deletes and duplicate names', () => {
    const original = { id: 'daily', name: 'Daily', skillKeys: [] };
    const changed = { ...original, skillKeys: ['new::skill'] };
    expect(() => saveUserSkillGroup([changed], original, original)).toThrow();
    expect(() => deleteUserSkillGroup([changed], original)).toThrow();
    expect(() => saveUserSkillGroup([original], { ...original, id: 'other', name: ' daily ' }, null)).toThrow();
  });

  it('reorders and removes only group definitions', () => {
    const first = { id: 'a', name: 'A', skillKeys: ['user::review'] };
    const second = { id: 'b', name: 'B', skillKeys: ['user::review'] };
    expect(moveUserSkillGroup([first, second], 'b', -1)).toEqual([second, first]);
    expect(deleteUserSkillGroup([first, second], first)).toEqual([second]);
    expect(second.skillKeys).toEqual(['user::review']);
  });
});
