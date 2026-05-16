import type { QuickAction } from './AIExperienceConfigService';

type Translator = (key: string, options?: Record<string, unknown>) => string;

const BUILTIN_QUICK_ACTION_TEXT: Record<string, {
  defaultLabel: string;
  defaultPrompt: string;
  labelKey: string;
  promptKey: string;
}> = {
  commit: {
    defaultLabel: 'Commit',
    defaultPrompt: 'Commit all current code changes',
    labelKey: 'quickActions.defaults.commit.label',
    promptKey: 'quickActions.defaults.commit.prompt',
  },
  create_pr: {
    defaultLabel: 'Create PR',
    defaultPrompt: 'Create a Pull Request for the current branch',
    labelKey: 'quickActions.defaults.createPr.label',
    promptKey: 'quickActions.defaults.createPr.prompt',
  },
};

export function resolveQuickActionText(
  action: Pick<QuickAction, 'id' | 'label' | 'prompt'>,
  t: Translator,
): { label: string; prompt: string } {
  const builtin = BUILTIN_QUICK_ACTION_TEXT[action.id];
  if (!builtin) {
    return {
      label: action.label,
      prompt: action.prompt,
    };
  }

  return {
    label: action.label === builtin.defaultLabel
      ? t(builtin.labelKey, { defaultValue: builtin.defaultLabel })
      : action.label,
    prompt: action.prompt === builtin.defaultPrompt
      ? t(builtin.promptKey, { defaultValue: builtin.defaultPrompt })
      : action.prompt,
  };
}

export function normalizeQuickActionTextForStorage(
  action: Pick<QuickAction, 'id'>,
  label: string,
  prompt: string,
  t: Translator,
): { label: string; prompt: string } {
  const builtin = BUILTIN_QUICK_ACTION_TEXT[action.id];
  if (!builtin) {
    return { label, prompt };
  }

  const localizedLabel = t(builtin.labelKey, { defaultValue: builtin.defaultLabel });
  const localizedPrompt = t(builtin.promptKey, { defaultValue: builtin.defaultPrompt });

  return {
    label: label === localizedLabel ? builtin.defaultLabel : label,
    prompt: prompt === localizedPrompt ? builtin.defaultPrompt : prompt,
  };
}
