import { isSkillAvailableForUserInvocation } from './skillPromptReference';

const QUICK_SKILL_DEFINITIONS = [
  { id: 'plan', skillName: 'plan', label: 'Plan' },
  { id: 'debug', skillName: 'debug', label: 'Debug' },
  { id: 'multitask', skillName: 'multitask', label: 'Multitask' },
] as const;

export interface ChatInputQuickSkillShortcut<TSkill> {
  id: (typeof QUICK_SKILL_DEFINITIONS)[number]['id'];
  label: string;
  skill: TSkill;
}

export function resolveChatInputQuickSkillShortcuts<
  TSkill extends {
    name: string;
    selectedForRuntime: boolean;
    allowUserInvocation?: boolean;
  },
>(skills: Iterable<TSkill>): ChatInputQuickSkillShortcut<TSkill>[] {
  const availableSkillsByName = new Map<string, TSkill>();

  for (const skill of skills) {
    if (!isSkillAvailableForUserInvocation(skill)) continue;

    const normalizedName = skill.name.trim().toLowerCase();
    if (normalizedName && !availableSkillsByName.has(normalizedName)) {
      availableSkillsByName.set(normalizedName, skill);
    }
  }

  return QUICK_SKILL_DEFINITIONS.flatMap(definition => {
    const skill = availableSkillsByName.get(definition.skillName);
    return skill
      ? [{ id: definition.id, label: definition.label, skill }]
      : [];
  });
}
