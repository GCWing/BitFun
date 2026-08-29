import { describe, expect, it } from 'vitest';

import { resolveChatInputQuickSkillShortcuts } from './chatInputQuickSkills';

function skill(
  name: string,
  options: {
    selectedForRuntime?: boolean;
    allowUserInvocation?: boolean;
  } = {},
) {
  return {
    name,
    selectedForRuntime: options.selectedForRuntime ?? true,
    allowUserInvocation: options.allowUserInvocation,
  };
}

describe('resolveChatInputQuickSkillShortcuts', () => {
  it('projects Plan, Debug, and Multitask in stable menu order', () => {
    const shortcuts = resolveChatInputQuickSkillShortcuts([
      skill('debug'),
      skill('unrelated'),
      skill(' MULTITASK '),
      skill('Plan'),
    ]);

    expect(shortcuts.map(shortcut => ({
      id: shortcut.id,
      label: shortcut.label,
      skillName: shortcut.skill.name,
    }))).toEqual([
      { id: 'plan', label: 'Plan', skillName: 'Plan' },
      { id: 'debug', label: 'Debug', skillName: 'debug' },
      { id: 'multitask', label: 'Multitask', skillName: ' MULTITASK ' },
    ]);
  });

  it('omits shortcuts whose skill is disabled or unavailable for invocation', () => {
    const shortcuts = resolveChatInputQuickSkillShortcuts([
      skill('plan', { selectedForRuntime: false }),
      skill('multitask', { allowUserInvocation: false }),
      skill('debug'),
    ]);

    expect(shortcuts.map(shortcut => shortcut.id)).toEqual(['debug']);
  });
});
