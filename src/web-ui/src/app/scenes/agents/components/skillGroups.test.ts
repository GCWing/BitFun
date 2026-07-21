import { describe, expect, it } from 'vitest';
import type { GroupableSkill } from './skillGroups';
import {
  createUserSkillGroupsConfig,
  normalizeUserSkillGroupsConfig,
  resolveSkillGroupSummary,
  resolveSkillGroups,
  setSkillGroupSelection,
} from './skillGroups';

const labels = {
  builtin: (groupKey: string) => `builtin:${groupKey}`,
  other: 'other',
};

const skills: GroupableSkill[] = [
  { key: 'builtin::docs', name: 'Docs', description: '', isBuiltin: true, groupKey: 'office' },
  { key: 'builtin::slides', name: 'Slides', description: '', isBuiltin: true, groupKey: 'office' },
  { key: 'user::review', name: 'Review', description: '', isBuiltin: false },
];

describe('skillGroups', () => {
  it('keeps user groups before backend-owned builtin groups', () => {
    const groups = resolveSkillGroups(skills, [{
      id: 'daily',
      name: 'Daily work',
      skillKeys: ['user::review', 'builtin::docs'],
    }], labels);

    expect(groups.map((group) => group.id)).toEqual(['user:daily', 'builtin:office', 'other']);
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
});
